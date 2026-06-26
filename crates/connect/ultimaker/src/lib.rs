//! UltiMaker connectivity driver — HTTP REST with Digest authentication.
//!
//! Supports UltiMaker S-series (S3, S5, S7) and Method printers via their
//! local REST API at `/api/v1/`. Authentication uses HTTP Digest.

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// UltiMaker printer connection via HTTP REST + Digest auth.
pub struct UltiMakerConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

/// Parsed printer status from the UltiMaker API.
#[derive(Deserialize)]
struct PrinterStatus {
    status: String,
    #[serde(default)]
    heads: Vec<HeadStatus>,
    #[serde(default)]
    bed: Option<BedStatus>,
}

#[derive(Deserialize)]
struct HeadStatus {
    #[serde(default)]
    extruders: Vec<ExtruderStatus>,
    #[serde(default)]
    position: PositionStatus,
}

#[derive(Deserialize, Default)]
struct PositionStatus {
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    z: f64,
}

#[derive(Deserialize)]
struct ExtruderStatus {
    #[serde(default)]
    hotend: HotendStatus,
}

#[derive(Deserialize, Default)]
struct HotendStatus {
    #[serde(default)]
    temperature: TempReading,
}

#[derive(Deserialize, Default)]
struct TempReading {
    #[serde(default)]
    current: f64,
    #[serde(default)]
    target: f64,
}

#[derive(Deserialize)]
struct BedStatus {
    temperature: TempReading,
}

/// Print job response from UltiMaker API.
#[derive(Deserialize)]
struct PrintJobResponse {
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    time_elapsed: f64,
    #[serde(default)]
    time_total: f64,
}

impl UltiMakerConnection {
    /// Create a new UltiMaker connection.
    ///
    /// `config.auth` must be `AuthConfig::DigestAuth` with username and password.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let (username, password) = match &config.auth {
            AuthConfig::DigestAuth { username, password } => {
                (username.clone(), password.clone())
            }
            _ => {
                return Err(ConnectError::AuthFailed(
                    "UltiMaker requires Digest authentication".into(),
                ))
            }
        };

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "ultimaker-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::Fdm,
                protocol: Protocol::UltiMaker,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Ufp, AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
            username,
            password,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base_url, path)
    }

    /// Perform a GET request with Digest authentication.
    ///
    /// UltiMaker printers use HTTP Digest auth. On a 401 response we parse
    /// the WWW-Authenticate header and retry with the computed digest.
    async fn get_json(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let url = self.url(path);

        // First request — expect 401 with WWW-Authenticate header.
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let www_auth = resp
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    ConnectError::AuthFailed("missing WWW-Authenticate header".into())
                })?;

            let context = digest_auth::AuthContext::new(
                &self.username,
                &self.password,
                &url,
            );
            let mut prompt =
                digest_auth::parse(www_auth).map_err(|e| ConnectError::AuthFailed(e.to_string()))?;
            let auth_header = prompt
                .respond(&context)
                .map_err(|e| ConnectError::AuthFailed(e.to_string()))?
                .to_header_string();

            let resp2 = self
                .client
                .get(&url)
                .header("Authorization", auth_header)
                .send()
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

            if !resp2.status().is_success() {
                return Err(ConnectError::Protocol(format!("HTTP {}", resp2.status())));
            }

            return resp2
                .json()
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()));
        }

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }

    /// Perform a PUT request with Digest authentication.
    async fn put_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = self.url(path);

        let resp = self
            .client
            .put(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let www_auth = resp
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    ConnectError::AuthFailed("missing WWW-Authenticate header".into())
                })?;

            let context = digest_auth::AuthContext::new(
                &self.username,
                &self.password,
                &url,
            );
            let mut prompt =
                digest_auth::parse(www_auth).map_err(|e| ConnectError::AuthFailed(e.to_string()))?;
            let auth_header = prompt
                .respond(&context)
                .map_err(|e| ConnectError::AuthFailed(e.to_string()))?
                .to_header_string();

            let resp2 = self
                .client
                .put(&url)
                .header("Authorization", auth_header)
                .json(body)
                .send()
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

            if !resp2.status().is_success() {
                return Err(ConnectError::Protocol(format!("HTTP {}", resp2.status())));
            }

            let text = resp2
                .text()
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;
            if text.is_empty() {
                return Ok(serde_json::Value::Null);
            }
            return serde_json::from_str(&text)
                .map_err(|e| ConnectError::Protocol(e.to_string()));
        }

        Err(ConnectError::Protocol(format!(
            "expected 401 challenge, got HTTP {}",
            resp.status()
        )))
    }
}

#[async_trait]
impl MachineConnection for UltiMakerConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        let _val = self.get_json("/system").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let printer_json = self.get_json("/printer").await?;
        let printer: PrinterStatus = serde_json::from_value(printer_json)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let state = match printer.status.as_str() {
            "idle" | "booting" => MachineState::Idle,
            "printing" | "pre_print" | "post_print" => MachineState::Busy,
            "pausing" | "paused" => MachineState::Paused,
            "error" => MachineState::Error,
            _ => MachineState::Offline,
        };

        let mut temperatures = Vec::new();
        for (i, head) in printer.heads.iter().enumerate() {
            for (j, ext) in head.extruders.iter().enumerate() {
                temperatures.push(Temperature {
                    name: format!("extruder_{i}_{j}"),
                    actual: ext.hotend.temperature.current,
                    target: ext.hotend.temperature.target,
                });
            }
        }
        if let Some(bed) = &printer.bed {
            temperatures.push(Temperature {
                name: "bed".into(),
                actual: bed.temperature.current,
                target: bed.temperature.target,
            });
        }

        let position = printer
            .heads
            .first()
            .map(|h| MachinePosition {
                x: h.position.x,
                y: h.position.y,
                z: h.position.z,
            })
            .unwrap_or_default();

        // Check for active print job.
        let active_job = match self.get_json("/print_job").await {
            Ok(job_json) => {
                let job: PrintJobResponse = serde_json::from_value(job_json)
                    .map_err(|e| ConnectError::Protocol(e.to_string()))?;
                if !job.uuid.is_empty() {
                    Some(JobStatus {
                        state: match job.state.as_str() {
                            "printing" | "pre_print" => JobState::Printing,
                            "pausing" | "paused" => JobState::Paused,
                            "wait_cleanup" | "post_print" => JobState::Complete,
                            _ => JobState::Queued,
                        },
                        progress_pct: job.progress * 100.0,
                        elapsed_s: job.time_elapsed,
                        remaining_s: if job.time_total > job.time_elapsed {
                            Some(job.time_total - job.time_elapsed)
                        } else {
                            None
                        },
                        layers: None,
                        filename: job.name,
                    })
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        Ok(MachineStatus {
            state,
            temperatures,
            position,
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if !matches!(job.format, AcceptedFormat::Ufp | AcceptedFormat::Gcode) {
            return Err(ConnectError::FormatNotAccepted(
                "UltiMaker accepts UFP or G-code".into(),
            ));
        }

        let filename = job.name.clone();
        let mime = match job.format {
            AcceptedFormat::Ufp => "application/x-ufp",
            AcceptedFormat::Gcode => "text/x-gcode",
            _ => "application/octet-stream",
        };

        let url = self.url("/print_job");

        // Perform digest-auth handshake for the upload.
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ConnectError::AuthFailed("missing WWW-Authenticate header".into()))?
            .to_string();

        let context = digest_auth::AuthContext::new(&self.username, &self.password, &url);
        let mut prompt =
            digest_auth::parse(&www_auth).map_err(|e| ConnectError::AuthFailed(e.to_string()))?;
        let auth_header = prompt
            .respond(&context)
            .map_err(|e| ConnectError::AuthFailed(e.to_string()))?
            .to_header_string();

        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str(mime)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("jobname", filename.clone());

        let resp2 = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp2.status().is_success() {
            let status = resp2.status();
            let body = resp2.text().await.unwrap_or_default();
            return Err(ConnectError::Protocol(format!(
                "upload failed: HTTP {status} — {body}"
            )));
        }

        let result: serde_json::Value = resp2
            .json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let job_id = result["uuid"]
            .as_str()
            .unwrap_or(&filename)
            .to_string();

        Ok(JobHandle {
            job_id,
            filename,
        })
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "state": "abort" });
        self.put_json(&format!("/print_job/{}", handle.job_id), &body)
            .await?;
        Ok(())
    }

    async fn pause_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "state": "pause" });
        self.put_json(&format!("/print_job/{}", handle.job_id), &body)
            .await?;
        Ok(())
    }

    async fn resume_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "state": "print" });
        self.put_json(&format!("/print_job/{}", handle.job_id), &body)
            .await?;
        Ok(())
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let job_json = self
            .get_json(&format!("/print_job/{}", handle.job_id))
            .await?;

        let job: PrintJobResponse = serde_json::from_value(job_json)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        Ok(JobStatus {
            state: match job.state.as_str() {
                "printing" | "pre_print" => JobState::Printing,
                "pausing" | "paused" => JobState::Paused,
                "wait_cleanup" | "post_print" => JobState::Complete,
                "abort" => JobState::Cancelled,
                _ => JobState::Queued,
            },
            progress_pct: job.progress * 100.0,
            elapsed_s: job.time_elapsed,
            remaining_s: if job.time_total > job.time_elapsed {
                Some(job.time_total - job.time_elapsed)
            } else {
                None
            },
            layers: None,
            filename: job.name,
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let body = serde_json::json!({ "command": cmd });
        let resp = self.put_json("/printer/command", &body).await?;
        Ok(resp.to_string())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // HTTP is stateless — nothing to disconnect.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MachineConfig {
        MachineConfig {
            name: "UltiMaker S5".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::UltiMaker,
            address: "192.168.1.200".into(),
            auth: AuthConfig::DigestAuth {
                username: "admin".into(),
                password: "secret".into(),
            },
        }
    }

    #[test]
    fn create_connection() {
        let conn = UltiMakerConnection::new(&test_config()).unwrap();
        assert_eq!(conn.info().protocol, Protocol::UltiMaker);
        assert_eq!(conn.info().kind, MachineKind::Fdm);
        assert_eq!(
            conn.info().accepted_formats,
            vec![AcceptedFormat::Ufp, AcceptedFormat::Gcode]
        );
    }

    #[test]
    fn reject_wrong_auth() {
        let mut config = test_config();
        config.auth = AuthConfig::None;
        assert!(UltiMakerConnection::new(&config).is_err());
    }

    #[test]
    fn url_formatting() {
        let conn = UltiMakerConnection::new(&test_config()).unwrap();
        assert_eq!(
            conn.url("/printer"),
            "http://192.168.1.200/api/v1/printer"
        );
    }

    #[test]
    fn url_with_http_prefix() {
        let mut config = test_config();
        config.address = "http://my-printer.local".into();
        let conn = UltiMakerConnection::new(&config).unwrap();
        assert_eq!(
            conn.url("/system"),
            "http://my-printer.local/api/v1/system"
        );
    }
}
