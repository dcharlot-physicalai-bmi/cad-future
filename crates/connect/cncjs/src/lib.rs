//! CNCjs connectivity driver — Socket.IO + REST API.
//!
//! Communicates with CNCjs browser-based CNC controller via its REST API.
//! Supports file upload, G-code streaming, and status monitoring.
//! API reference: <https://github.com/cncjs/cncjs/wiki/API-Reference>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// CNCjs connection via REST API with bearer token auth.
pub struct CncJsConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    token: String,
}

impl CncJsConnection {
    /// Create a new CNCjs connection.
    ///
    /// Requires `AuthConfig::BearerToken`.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let token = match &config.auth {
            AuthConfig::BearerToken { token } => token.clone(),
            _ => {
                return Err(ConnectError::AuthFailed(
                    "CNCjs requires a bearer token".into(),
                ));
            }
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "cncjs-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::CncMill,
                protocol: Protocol::CncJs,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            token,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let resp = self
            .client
            .get(self.url(path))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }

    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let resp = self
            .client
            .post(self.url(path))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        let text = resp
            .text()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        if text.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::from_str(&text).map_err(|e| ConnectError::Protocol(e.to_string()))
        }
    }
}

/// CNCjs version response.
#[derive(Deserialize)]
struct VersionResponse {
    #[allow(dead_code)]
    version: String,
}

#[async_trait]
impl MachineConnection for CncJsConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        let _: VersionResponse = serde_json::from_value(self.get("/version").await?)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let data = self.get("/state").await?;

        let state_str = data["status"]["activeState"]
            .as_str()
            .unwrap_or("Unknown");
        let state = match state_str {
            "Idle" => MachineState::Idle,
            "Run" => MachineState::Busy,
            "Hold" => MachineState::Paused,
            "Alarm" => MachineState::Error,
            _ => MachineState::Offline,
        };

        let wpos = &data["status"]["wpos"];
        let position = MachinePosition {
            x: wpos["x"].as_f64().unwrap_or(0.0),
            y: wpos["y"].as_f64().unwrap_or(0.0),
            z: wpos["z"].as_f64().unwrap_or(0.0),
        };

        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position,
            active_job: None,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "CNCjs only accepts G-code".into(),
            ));
        }

        let filename = if job.name.ends_with(".gcode") || job.name.ends_with(".nc") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        // Upload file via multipart POST
        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("gcode", part)
            .text("port", "");

        let resp = self
            .client
            .post(self.url("/gcode"))
            .bearer_auth(&self.token)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectError::Protocol(format!(
                "upload failed: HTTP {status} -- {body}"
            )));
        }

        // Start the job if requested
        if job.auto_start {
            let start_body = serde_json::json!({ "command": "gcode:start" });
            self.post_json("/commands", &start_body).await?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "gcode:stop" });
        self.post_json("/commands", &body).await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "gcode:pause" });
        self.post_json("/commands", &body).await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "command": "gcode:resume" });
        self.post_json("/commands", &body).await?;
        Ok(())
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let data = self.get("/gcode").await?;

        let state_str = data["state"].as_str().unwrap_or("idle");
        let state = match state_str {
            "running" => JobState::Printing,
            "paused" | "hold" => JobState::Paused,
            "idle" => JobState::Complete,
            _ => JobState::Queued,
        };

        let sent = data["sent"].as_f64().unwrap_or(0.0);
        let total = data["total"].as_f64().unwrap_or(1.0);
        let progress = if total > 0.0 {
            (sent / total) * 100.0
        } else {
            0.0
        };

        Ok(JobStatus {
            state,
            progress_pct: progress,
            elapsed_s: data["elapsedTime"].as_f64().unwrap_or(0.0),
            remaining_s: data["remainingTime"].as_f64(),
            layers: None,
            filename: handle.filename.clone(),
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let body = serde_json::json!({ "command": "gcode", "args": cmd });
        self.post_json("/commands", &body).await?;
        Ok("ok".into())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // CNCjs REST API is stateless; nothing to close.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection() {
        let config = MachineConfig {
            name: "Shop Router".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::CncJs,
            address: "192.168.1.75:8000".into(),
            auth: AuthConfig::BearerToken {
                token: "test-token-123".into(),
            },
        };
        let conn = CncJsConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::CncJs);
        assert_eq!(conn.info().kind, MachineKind::CncMill);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
        assert_eq!(conn.info().name, "Shop Router");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::CncJs,
            address: "localhost:8000".into(),
            auth: AuthConfig::None,
        };
        assert!(CncJsConnection::new(&config).is_err());
    }

    #[test]
    fn url_generation() {
        let config = MachineConfig {
            name: "Router".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::CncJs,
            address: "10.0.0.50:8000".into(),
            auth: AuthConfig::BearerToken {
                token: "tok".into(),
            },
        };
        let conn = CncJsConnection::new(&config).unwrap();
        assert_eq!(conn.url("/version"), "http://10.0.0.50:8000/api/version");
    }

    #[test]
    fn url_with_http_prefix() {
        let config = MachineConfig {
            name: "Router".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::CncJs,
            address: "https://cnc.local".into(),
            auth: AuthConfig::BearerToken {
                token: "tok".into(),
            },
        };
        let conn = CncJsConnection::new(&config).unwrap();
        assert_eq!(conn.url("/state"), "https://cnc.local/api/state");
    }
}
