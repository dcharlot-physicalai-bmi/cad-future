//! OctoPrint connectivity driver — HTTP REST API.
//!
//! Supports OctoPrint instances (including OctoPi) for FDM 3D printers.
//! API reference: <https://docs.octoprint.org/en/master/api/>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// OctoPrint connection via HTTP REST.
pub struct OctoPrintConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    api_key: String,
}

impl OctoPrintConnection {
    /// Create a new OctoPrint connection.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let api_key = match &config.auth {
            AuthConfig::ApiKey { key } => key.clone(),
            _ => return Err(ConnectError::AuthFailed("OctoPrint requires an API key".into())),
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!("octoprint-{}", config.address.replace([':', '/', '.'], "-")));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::OctoPrint,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            api_key,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let resp = self.client
            .get(self.url(path))
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.json().await.map_err(|e| ConnectError::Protocol(e.to_string()))
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ConnectError> {
        let resp = self.client
            .post(self.url(path))
            .header("X-Api-Key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        let text = resp.text().await.map_err(|e| ConnectError::Protocol(e.to_string()))?;
        if text.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::from_str(&text).map_err(|e| ConnectError::Protocol(e.to_string()))
        }
    }
}

/// OctoPrint version response.
#[derive(Deserialize)]
struct VersionResponse {
    #[allow(dead_code)]
    server: String,
    #[allow(dead_code)]
    api: String,
}

#[async_trait]
impl MachineConnection for OctoPrintConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        let _: VersionResponse = serde_json::from_value(self.get("/version").await?)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        // GET /api/printer for temperatures and state
        let printer = self.get("/printer").await;
        let job_data = self.get("/job").await;

        let (state, temps) = match printer {
            Ok(p) => {
                let flags = &p["state"]["flags"];
                let state = if flags["error"].as_bool().unwrap_or(false) {
                    MachineState::Error
                } else if flags["paused"].as_bool().unwrap_or(false)
                    || flags["pausing"].as_bool().unwrap_or(false)
                {
                    MachineState::Paused
                } else if flags["printing"].as_bool().unwrap_or(false) {
                    MachineState::Busy
                } else if flags["ready"].as_bool().unwrap_or(false)
                    || flags["operational"].as_bool().unwrap_or(false)
                {
                    MachineState::Idle
                } else {
                    MachineState::Offline
                };

                let mut temps = Vec::new();
                if let Some(tool0) = p["temperature"]["tool0"].as_object() {
                    temps.push(Temperature {
                        name: "extruder".into(),
                        actual: tool0.get("actual").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        target: tool0.get("target").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    });
                }
                if let Some(bed) = p["temperature"]["bed"].as_object() {
                    temps.push(Temperature {
                        name: "bed".into(),
                        actual: bed.get("actual").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        target: bed.get("target").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    });
                }

                (state, temps)
            }
            Err(_) => (MachineState::Offline, Vec::new()),
        };

        let active_job = match job_data {
            Ok(j) if j["state"].as_str() != Some("Operational") && j["job"]["file"]["name"].as_str().is_some() => {
                let progress = &j["progress"];
                Some(JobStatus {
                    state: match j["state"].as_str().unwrap_or("") {
                        s if s.contains("Printing") => JobState::Printing,
                        s if s.contains("Paused") => JobState::Paused,
                        _ => JobState::Queued,
                    },
                    progress_pct: progress["completion"].as_f64().unwrap_or(0.0),
                    elapsed_s: progress["printTime"].as_f64().unwrap_or(0.0),
                    remaining_s: progress["printTimeLeft"].as_f64(),
                    layers: None,
                    filename: j["job"]["file"]["name"].as_str().unwrap_or("").to_string(),
                })
            }
            _ => None,
        };

        Ok(MachineStatus {
            state,
            temperatures: temps,
            position: MachinePosition::default(),
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "OctoPrint only accepts G-code".into(),
            ));
        }

        let filename = if job.name.ends_with(".gcode") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        // Upload via multipart POST to /api/files/local
        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("select", if job.auto_start { "true" } else { "false" })
            .text("print", if job.auto_start { "true" } else { "false" });

        let resp = self.client
            .post(self.url("/files/local"))
            .header("X-Api-Key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectError::Protocol(format!("upload failed: HTTP {status} — {body}")));
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "cancel" });
        self.post_json("/job", &body).await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "pause", "action": "pause" });
        self.post_json("/job", &body).await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "pause", "action": "resume" });
        self.post_json("/job", &body).await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let j = self.get("/job").await?;
        let progress = &j["progress"];

        Ok(JobStatus {
            state: match j["state"].as_str().unwrap_or("") {
                s if s.contains("Printing") => JobState::Printing,
                s if s.contains("Paused") => JobState::Paused,
                "Operational" => JobState::Complete,
                _ => JobState::Queued,
            },
            progress_pct: progress["completion"].as_f64().unwrap_or(0.0),
            elapsed_s: progress["printTime"].as_f64().unwrap_or(0.0),
            remaining_s: progress["printTimeLeft"].as_f64(),
            layers: None,
            filename: j["job"]["file"]["name"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let body = serde_json::json!({ "commands": [cmd] });
        self.post_json("/printer/command", &body).await?;
        Ok("ok".into())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "disconnect" });
        let _ = self.post_json("/connection", &body).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection() {
        let config = MachineConfig {
            name: "Test Printer".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::OctoPrint,
            address: "192.168.1.100:5000".into(),
            auth: AuthConfig::ApiKey { key: "test-key".into() },
        };
        let conn = OctoPrintConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::OctoPrint);
        assert_eq!(conn.info().name, "Test Printer");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::OctoPrint,
            address: "localhost".into(),
            auth: AuthConfig::None,
        };
        assert!(OctoPrintConnection::new(&config).is_err());
    }
}
