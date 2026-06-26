//! Repetier-Server connectivity driver — HTTP REST + WebSocket.
//!
//! Supports Repetier-Server instances managing multiple printers.
//! Each server can host several printer slugs, and this driver targets
//! one specific printer slug per connection.
//! API reference: <https://www.repetier-server.com/manuals/programming/API/>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// Repetier-Server connection via HTTP REST + WebSocket.
pub struct RepetierConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    api_key: String,
    /// The printer slug on this Repetier-Server instance.
    slug: String,
}

/// Response from Repetier-Server stateList API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RepetierState {
    #[serde(rename = "activeExtruder")]
    active_extruder: Option<i32>,
    #[serde(rename = "debugLevel")]
    debug_level: Option<i32>,
    firmware: Option<String>,
    #[serde(rename = "numExtruder")]
    num_extruder: Option<i32>,
    #[serde(rename = "sdcardMounted")]
    sdcard_mounted: Option<bool>,
}

/// Temperature data from Repetier-Server.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RepetierTemp {
    #[serde(rename = "tempRead")]
    temp_read: f64,
    #[serde(rename = "tempSet")]
    temp_set: f64,
}

impl RepetierConnection {
    /// Create a new Repetier-Server connection.
    ///
    /// The `config.address` should be `host:port/slug` or just `host:port`.
    /// If no slug is present, it defaults to `"default"`.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let api_key = match &config.auth {
            AuthConfig::ApiKey { key } => key.clone(),
            _ => {
                return Err(ConnectError::AuthFailed(
                    "Repetier-Server requires an API key".into(),
                ))
            }
        };

        // Parse address: "host:port/slug" or "host:port"
        let (base_addr, slug) = if let Some(idx) = config.address.find('/') {
            (
                config.address[..idx].to_string(),
                config.address[idx + 1..].to_string(),
            )
        } else {
            (config.address.clone(), "default".to_string())
        };

        let base_url = if base_addr.starts_with("http") {
            base_addr
        } else {
            format!("http://{}", base_addr)
        };

        let id = MachineId::new(format!(
            "repetier-{}-{}",
            config.address.replace([':', '/', '.'], "-"),
            slug
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::Repetier,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            api_key,
            slug,
        })
    }

    /// Build a Repetier API URL for the printer slug.
    fn api_url(&self, action: &str) -> String {
        format!(
            "{}/printer/api/{}?a={}&apikey={}",
            self.base_url, self.slug, action, self.api_key
        )
    }

    /// Perform a GET request to the Repetier API.
    async fn api_get(&self, action: &str) -> Result<serde_json::Value, ConnectError> {
        let resp = self
            .client
            .get(self.api_url(action))
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

    /// Perform a POST request with JSON body to the Repetier API.
    async fn api_post(
        &self,
        action: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let resp = self
            .client
            .post(self.api_url(action))
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

    /// Send a G-code command via the Repetier API.
    async fn send_gcode(&self, gcode: &str) -> Result<(), ConnectError> {
        let url = format!(
            "{}/printer/api/{}?a=send&data={}&apikey={}",
            self.base_url, self.slug, gcode, self.api_key
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }
        Ok(())
    }
}

#[async_trait]
impl MachineConnection for RepetierConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        // Use stateList to verify connectivity
        self.api_get("stateList").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let data = self.api_get("stateList").await?;

        // Parse printer state from the response
        let printer_state = data
            .get(&self.slug)
            .or_else(|| data.as_array().and_then(|a| a.first()));

        let (state, temps) = if let Some(ps) = printer_state {
            let active = ps["active"].as_bool().unwrap_or(false);
            let paused = ps["paused"].as_bool().unwrap_or(false);
            let has_error = ps["hasError"].as_bool().unwrap_or(false);

            let state = if has_error {
                MachineState::Error
            } else if paused {
                MachineState::Paused
            } else if active {
                MachineState::Busy
            } else {
                MachineState::Idle
            };

            let mut temps = Vec::new();

            // Parse extruder temperatures
            if let Some(ext_temps) = ps.get("extruder") {
                if let Some(arr) = ext_temps.as_array() {
                    for (i, ext) in arr.iter().enumerate() {
                        temps.push(Temperature {
                            name: format!("extruder{}", i),
                            actual: ext["tempRead"].as_f64().unwrap_or(0.0),
                            target: ext["tempSet"].as_f64().unwrap_or(0.0),
                        });
                    }
                }
            }

            // Parse heated bed temperature
            if let Some(bed) = ps.get("heatedBed") {
                temps.push(Temperature {
                    name: "bed".into(),
                    actual: bed["tempRead"].as_f64().unwrap_or(0.0),
                    target: bed["tempSet"].as_f64().unwrap_or(0.0),
                });
            }

            (state, temps)
        } else {
            (MachineState::Offline, Vec::new())
        };

        // Get position from state data
        let position = if let Some(ps) = printer_state {
            MachinePosition {
                x: ps["x"].as_f64().unwrap_or(0.0),
                y: ps["y"].as_f64().unwrap_or(0.0),
                z: ps["z"].as_f64().unwrap_or(0.0),
            }
        } else {
            MachinePosition::default()
        };

        // Get active job info
        let active_job = if let Some(ps) = printer_state {
            let job_name = ps["job"].as_str().unwrap_or("");
            if !job_name.is_empty() {
                let done = ps["done"].as_f64().unwrap_or(0.0);
                let total_lines = ps["totalLines"].as_f64().unwrap_or(1.0);
                let progress = if total_lines > 0.0 {
                    (done / total_lines) * 100.0
                } else {
                    0.0
                };
                let elapsed = ps["printTime"].as_f64().unwrap_or(0.0);

                Some(JobStatus {
                    state: if state == MachineState::Paused {
                        JobState::Paused
                    } else {
                        JobState::Printing
                    },
                    progress_pct: progress,
                    elapsed_s: elapsed,
                    remaining_s: if progress > 0.0 {
                        Some(elapsed / progress * (100.0 - progress))
                    } else {
                        None
                    },
                    layers: None,
                    filename: job_name.to_string(),
                })
            } else {
                None
            }
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
                "Repetier-Server only accepts G-code".into(),
            ));
        }

        let filename = if job.name.ends_with(".gcode") || job.name.ends_with(".gco") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        // Upload via multipart POST to /printer/model/<slug>
        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let upload_url = format!(
            "{}/printer/model/{}?a=upload&apikey={}",
            self.base_url, self.slug, self.api_key
        );

        let resp = self
            .client
            .post(&upload_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectError::Protocol(format!(
                "upload failed: HTTP {status} — {body}"
            )));
        }

        // Start the print if auto_start is set
        if job.auto_start {
            let start_url = format!(
                "{}/printer/api/{}?a=startJob&data={{\"id\":\"{}\"}}&apikey={}",
                self.base_url, self.slug, filename, self.api_key
            );
            let _ = self
                .client
                .get(&start_url)
                .send()
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.api_get("stopJob").await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.api_get("pauseJob").await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.api_get("continueJob").await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job on this printer".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        self.send_gcode(cmd).await?;
        Ok("ok".into())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // Repetier-Server doesn't require explicit disconnect; the HTTP session
        // is stateless. No-op.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_repetier_connection() {
        let config = MachineConfig {
            name: "Prusa i3 MK3".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Repetier,
            address: "192.168.1.50:3344/prusa_mk3".into(),
            auth: AuthConfig::ApiKey {
                key: "abc123".into(),
            },
        };
        let conn = RepetierConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::Repetier);
        assert_eq!(conn.info().name, "Prusa i3 MK3");
        assert_eq!(conn.slug, "prusa_mk3");
    }

    #[test]
    fn default_slug_when_not_specified() {
        let config = MachineConfig {
            name: "Test Printer".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Repetier,
            address: "localhost:3344".into(),
            auth: AuthConfig::ApiKey {
                key: "test-key".into(),
            },
        };
        let conn = RepetierConnection::new(&config).unwrap();
        assert_eq!(conn.slug, "default");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Repetier,
            address: "localhost".into(),
            auth: AuthConfig::None,
        };
        assert!(RepetierConnection::new(&config).is_err());
    }

    #[test]
    fn api_url_construction() {
        let config = MachineConfig {
            name: "Printer".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Repetier,
            address: "192.168.1.50:3344/myprinter".into(),
            auth: AuthConfig::ApiKey {
                key: "mykey".into(),
            },
        };
        let conn = RepetierConnection::new(&config).unwrap();
        let url = conn.api_url("stateList");
        assert!(url.contains("/printer/api/myprinter"));
        assert!(url.contains("a=stateList"));
        assert!(url.contains("apikey=mykey"));
    }

    #[test]
    fn accepted_formats() {
        let config = MachineConfig {
            name: "Printer".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::Repetier,
            address: "localhost:3344".into(),
            auth: AuthConfig::ApiKey {
                key: "key".into(),
            },
        };
        let conn = RepetierConnection::new(&config).unwrap();
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
    }
}
