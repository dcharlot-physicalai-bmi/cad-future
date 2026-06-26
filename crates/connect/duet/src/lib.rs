//! Duet3D connectivity driver — HTTP REST with session management.
//!
//! Supports Duet 2 (RepRapFirmware 2.x) and Duet 3 (RepRapFirmware 3.x).
//! Uses /rr_connect for session, /rr_status for polling, /rr_upload for files.
//! Duet 3 also supports the newer /machine/... Object Model endpoints.

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;

pub struct DuetConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    password: String,
}

impl DuetConnection {
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let password = match &config.auth {
            AuthConfig::UsernamePassword { password, .. } => password.clone(),
            AuthConfig::None => String::new(),
            _ => return Err(ConnectError::AuthFailed(
                "Duet accepts password or no auth".into(),
            )),
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "duet-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::Duet,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            password,
        })
    }

    async fn connect_session(&self) -> Result<(), ConnectError> {
        let url = format!(
            "{}/rr_connect?password={}",
            self.base_url,
            urlencoding::encode(&self.password)
        );
        let resp = self.client.get(&url).send().await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::AuthFailed("Duet session connect failed".into()));
        }
        let json: serde_json::Value = resp.json().await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        if json["err"].as_i64().unwrap_or(1) != 0 {
            return Err(ConnectError::AuthFailed("Duet rejected password".into()));
        }
        Ok(())
    }

    async fn rr_get(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let resp = self.client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        resp.json().await.map_err(|e| ConnectError::Protocol(e.to_string()))
    }
}

#[async_trait]
impl MachineConnection for DuetConnection {
    fn info(&self) -> &MachineInfo { &self.info }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.connect_session().await
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let s = self.rr_get("/rr_status?type=2").await?;

        let status_char = s["status"].as_str().unwrap_or("I");
        let state = match status_char {
            "P" | "R" | "M" => MachineState::Busy,
            "D" | "S" => MachineState::Paused,
            "I" | "F" | "O" | "T" | "C" => MachineState::Idle,
            _ => MachineState::Offline,
        };

        let mut temps = Vec::new();
        if let Some(heaters) = s["temps"]["current"].as_array() {
            let targets = s["temps"]["tools"]["active"].as_array();
            for (i, h) in heaters.iter().enumerate() {
                let name = if i == 0 { "bed" } else { "extruder" };
                temps.push(Temperature {
                    name: name.into(),
                    actual: h.as_f64().unwrap_or(0.0),
                    target: targets
                        .and_then(|t| t.first())
                        .and_then(|t| t.as_array())
                        .and_then(|t| t.get(i))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                });
            }
        }

        let pos = &s["coords"]["xyz"];
        let position = MachinePosition {
            x: pos[0].as_f64().unwrap_or(0.0),
            y: pos[1].as_f64().unwrap_or(0.0),
            z: pos[2].as_f64().unwrap_or(0.0),
        };

        let active_job = if state == MachineState::Busy || state == MachineState::Paused {
            Some(JobStatus {
                state: if state == MachineState::Busy { JobState::Printing } else { JobState::Paused },
                progress_pct: s["fractionPrinted"].as_f64().unwrap_or(0.0) * 100.0,
                elapsed_s: s["printDuration"].as_f64().unwrap_or(0.0),
                remaining_s: None,
                layers: None,
                filename: s["job"]["file"]["fileName"].as_str().unwrap_or("").to_string(),
            })
        } else {
            None
        };

        Ok(MachineStatus { state, temperatures: temps, position, active_job })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        let filename = if job.name.ends_with(".gcode") { job.name.clone() } else { format!("{}.gcode", job.name) };

        let resp = self.client
            .post(format!("{}/rr_upload?name=0:/gcodes/{}", self.base_url, filename))
            .body(job.payload)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("upload failed: HTTP {}", resp.status())));
        }

        if job.auto_start {
            self.send_command(&format!("M32 \"0:/gcodes/{filename}\"")).await?;
        }

        Ok(JobHandle { job_id: filename.clone(), filename })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_command("M0").await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_command("M25").await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_command("M24").await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status.active_job.ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let resp = self.client
            .get(format!("{}/rr_gcode?gcode={}", self.base_url, urlencoding::encode(cmd)))
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        let text = resp.text().await.map_err(|e| ConnectError::Protocol(e.to_string()))?;
        Ok(text)
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        let _ = self.client.get(format!("{}/rr_disconnect", self.base_url)).send().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_duet() {
        let config = MachineConfig {
            name: "Duet 3".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Duet,
            address: "duet3.local".into(),
            auth: AuthConfig::None,
        };
        let conn = DuetConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::Duet);
    }
}
