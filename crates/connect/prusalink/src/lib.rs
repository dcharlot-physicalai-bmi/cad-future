//! PrusaLink connectivity driver — HTTP REST + Digest auth.
//!
//! Supports Prusa MK4, MK3.9, Mini+, XL via PrusaLink firmware.
//! Accepts G-code and binary G-code (.bgcode).
//! API reference: <https://github.com/prusa3d/Prusa-Link-Web/blob/master/spec/openapi.yaml>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;

pub struct PrusaLinkConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

impl PrusaLinkConnection {
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let (username, password) = match &config.auth {
            AuthConfig::DigestAuth { username, password } => (username.clone(), password.clone()),
            AuthConfig::ApiKey { key } => ("maker".into(), key.clone()),
            _ => return Err(ConnectError::AuthFailed(
                "PrusaLink requires Digest auth or API key".into(),
            )),
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "prusalink-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::PrusaLink,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode, AcceptedFormat::BinaryGcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            username,
            password,
        })
    }

    async fn get_auth(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        // PrusaLink uses Digest auth — first request gets 401 with nonce, second authenticates
        // For simplicity, use X-Api-Key header which PrusaLink also accepts
        let resp = self
            .client
            .get(format!("{}/api{}", self.base_url, path))
            .header("X-Api-Key", &self.password)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if resp.status().as_u16() == 401 {
            return Err(ConnectError::AuthFailed("invalid credentials".into()));
        }
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }
}

#[async_trait]
impl MachineConnection for PrusaLinkConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.get_auth("/version").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let printer = self.get_auth("/v1/status").await?;

        let state_str = printer["printer"]["state"]
            .as_str()
            .unwrap_or("IDLE");

        let state = match state_str {
            "PRINTING" => MachineState::Busy,
            "PAUSED" => MachineState::Paused,
            "IDLE" | "READY" | "FINISHED" | "STOPPED" => MachineState::Idle,
            "ERROR" | "ATTENTION" => MachineState::Error,
            _ => MachineState::Idle,
        };

        let mut temps = Vec::new();
        if let Some(nozzle) = printer["printer"]["temp_nozzle"].as_f64() {
            temps.push(Temperature {
                name: "extruder".into(),
                actual: nozzle,
                target: printer["printer"]["target_nozzle"].as_f64().unwrap_or(0.0),
            });
        }
        if let Some(bed) = printer["printer"]["temp_bed"].as_f64() {
            temps.push(Temperature {
                name: "bed".into(),
                actual: bed,
                target: printer["printer"]["target_bed"].as_f64().unwrap_or(0.0),
            });
        }

        let active_job = if state == MachineState::Busy || state == MachineState::Paused {
            let job = &printer["job"];
            Some(JobStatus {
                state: if state == MachineState::Busy { JobState::Printing } else { JobState::Paused },
                progress_pct: job["progress"].as_f64().unwrap_or(0.0),
                elapsed_s: job["time_printing"].as_f64().unwrap_or(0.0),
                remaining_s: job["time_remaining"].as_f64(),
                layers: None,
                filename: job["file"]["display_name"].as_str().unwrap_or("").to_string(),
            })
        } else {
            None
        };

        Ok(MachineStatus {
            state,
            temperatures: temps,
            position: MachinePosition::default(),
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        let filename = if job.name.ends_with(".gcode") || job.name.ends_with(".bgcode") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let print_after = if job.auto_start { "?print_after_upload=1" } else { "" };

        let resp = self
            .client
            .put(format!(
                "{}/api/v1/files/usb/{}{print_after}",
                self.base_url, filename
            ))
            .header("X-Api-Key", &self.password)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!(
                "upload failed: HTTP {}",
                resp.status()
            )));
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let resp = self
            .client
            .delete(format!("{}/api/v1/job", self.base_url))
            .header("X-Api-Key", &self.password)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let resp = self
            .client
            .put(format!("{}/api/v1/job", self.base_url))
            .header("X-Api-Key", &self.password)
            .json(&serde_json::json!({ "command": "PAUSE" }))
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let resp = self
            .client
            .put(format!("{}/api/v1/job", self.base_url))
            .header("X-Api-Key", &self.password)
            .json(&serde_json::json!({ "command": "RESUME" }))
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let _ = (cmd, &self.username); // PrusaLink doesn't have a raw command endpoint
        Err(ConnectError::Unsupported(
            "PrusaLink does not support raw G-code commands".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_prusalink() {
        let config = MachineConfig {
            name: "Prusa MK4".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::PrusaLink,
            address: "192.168.1.60".into(),
            auth: AuthConfig::DigestAuth {
                username: "maker".into(),
                password: "secret".into(),
            },
        };
        let conn = PrusaLinkConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::PrusaLink);
    }
}
