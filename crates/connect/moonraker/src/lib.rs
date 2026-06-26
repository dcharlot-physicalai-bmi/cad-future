//! Moonraker connectivity driver — HTTP + JSON-RPC WebSocket.
//!
//! Supports Klipper-based printers via Moonraker API (including Creality K1/V3,
//! Voron, RatRig, etc.).
//! API reference: <https://moonraker.readthedocs.io/en/latest/web_api/>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// Moonraker connection via HTTP REST + JSON-RPC.
pub struct MoonrakerConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl MoonrakerConnection {
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let api_key = match &config.auth {
            AuthConfig::ApiKey { key } => Some(key.clone()),
            AuthConfig::None => None,
            _ => return Err(ConnectError::AuthFailed(
                "Moonraker accepts API key or no auth".into(),
            )),
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "moonraker-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::Moonraker,
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

    async fn get(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let mut req = self.client.get(format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        Ok(val.get("result").cloned().unwrap_or(val))
    }

    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let mut req = self.client.post(format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
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


#[async_trait]
impl MachineConnection for MoonrakerConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.get("/server/info").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        // Query printer objects for state, temps, position
        let query = "/printer/objects/query?extruder&heater_bed&toolhead&print_stats";
        let data = self.get(query).await?;

        let status = &data["status"];

        let mut temps = Vec::new();
        if let Some(ext) = status.get("extruder") {
            temps.push(Temperature {
                name: "extruder".into(),
                actual: ext["temperature"].as_f64().unwrap_or(0.0),
                target: ext["target"].as_f64().unwrap_or(0.0),
            });
        }
        if let Some(bed) = status.get("heater_bed") {
            temps.push(Temperature {
                name: "bed".into(),
                actual: bed["temperature"].as_f64().unwrap_or(0.0),
                target: bed["target"].as_f64().unwrap_or(0.0),
            });
        }

        let position = if let Some(th) = status.get("toolhead") {
            let pos = &th["position"];
            MachinePosition {
                x: pos[0].as_f64().unwrap_or(0.0),
                y: pos[1].as_f64().unwrap_or(0.0),
                z: pos[2].as_f64().unwrap_or(0.0),
            }
        } else {
            MachinePosition::default()
        };

        let print_stats = status.get("print_stats");
        let klipper_state = print_stats
            .and_then(|ps| ps["state"].as_str())
            .unwrap_or("standby");

        let state = match klipper_state {
            "printing" => MachineState::Busy,
            "paused" => MachineState::Paused,
            "error" => MachineState::Error,
            "standby" | "complete" | "cancelled" => MachineState::Idle,
            _ => MachineState::Idle,
        };

        let active_job = if klipper_state == "printing" || klipper_state == "paused" {
            let ps = print_stats.unwrap();
            let filename = ps["filename"].as_str().unwrap_or("").to_string();
            let elapsed = ps["total_duration"].as_f64().unwrap_or(0.0);

            // Get progress from virtual_sdcard
            let sd_data = self
                .get("/printer/objects/query?virtual_sdcard")
                .await
                .ok();
            let progress = sd_data
                .as_ref()
                .and_then(|d| d["status"]["virtual_sdcard"]["progress"].as_f64())
                .unwrap_or(0.0)
                * 100.0;

            Some(JobStatus {
                state: if klipper_state == "printing" {
                    JobState::Printing
                } else {
                    JobState::Paused
                },
                progress_pct: progress,
                elapsed_s: elapsed,
                remaining_s: if progress > 0.0 {
                    Some(elapsed / progress * (100.0 - progress))
                } else {
                    None
                },
                layers: None,
                filename,
            })
        } else {
            None
        };

        Ok(MachineStatus {
            state,
            temperatures: temps,
            position,
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "Moonraker only accepts G-code".into(),
            ));
        }

        let filename = if job.name.ends_with(".gcode") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        // Upload via POST /server/files/upload
        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("root", "gcodes");

        let mut req = self
            .client
            .post(format!("{}/server/files/upload", self.base_url));
        if let Some(key) = &self.api_key {
            req = req.header("X-Api-Key", key);
        }

        let resp = req
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

        // Start print if auto_start
        if job.auto_start {
            self.post_json(
                "/printer/print/start",
                &serde_json::json!({ "filename": &filename }),
            )
            .await?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.post_json("/printer/print/cancel", &serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.post_json("/printer/print/pause", &serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.post_json("/printer/print/resume", &serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        self.post_json(
            "/printer/gcode/script",
            &serde_json::json!({ "script": cmd }),
        )
        .await?;
        Ok("ok".into())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_moonraker() {
        let config = MachineConfig {
            name: "Voron 2.4".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Moonraker,
            address: "voron.local:7125".into(),
            auth: AuthConfig::None,
        };
        let conn = MoonrakerConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::Moonraker);
    }
}
