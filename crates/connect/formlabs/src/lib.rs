//! Formlabs SLA printer connectivity driver via REST API.
//!
//! Communicates with Formlabs printers (Form 3/3+, Form 3L, Form 4) through
//! the Formlabs cloud REST API using Bearer token authentication.

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;
use serde::Deserialize;

/// Default Formlabs API base URL.
const FORMLABS_API_BASE: &str = "https://api.formlabs.com";

/// Formlabs printer connection via cloud REST API.
pub struct FormlabsConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
    token: String,
    /// Formlabs printer serial / identifier within the API.
    printer_id: String,
}

/// Printer status from the Formlabs API.
#[derive(Deserialize)]
struct FormlabsPrinterStatus {
    #[serde(default)]
    status: String,
    #[serde(default)]
    firmware_version: Option<String>,
}

/// Print job response from the Formlabs API.
#[derive(Deserialize)]
struct FormlabsJobResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    elapsed_time_s: f64,
    #[serde(default)]
    estimated_time_s: f64,
    #[serde(default)]
    current_layer: Option<u32>,
    #[serde(default)]
    total_layers: Option<u32>,
}

impl FormlabsConnection {
    /// Create a new Formlabs connection.
    ///
    /// `config.auth` must be `AuthConfig::BearerToken`.
    /// `config.address` is the printer serial/ID for API routing.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let token = match &config.auth {
            AuthConfig::BearerToken { token } => token.clone(),
            _ => {
                return Err(ConnectError::AuthFailed(
                    "Formlabs requires Bearer token authentication".into(),
                ))
            }
        };

        let id = MachineId::new(format!(
            "formlabs-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::Sla,
                protocol: Protocol::Formlabs,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Stl],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url: FORMLABS_API_BASE.to_string(),
            token,
            printer_id: config.address.clone(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// GET request with Bearer auth.
    async fn get(&self, path: &str) -> Result<serde_json::Value, ConnectError> {
        let resp = self
            .client
            .get(self.url(path))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ConnectError::AuthFailed("invalid Bearer token".into()));
        }

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.json()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }

    /// POST request with Bearer auth and JSON body.
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

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ConnectError::AuthFailed("invalid Bearer token".into()));
        }

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
impl MachineConnection for FormlabsConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        let _val = self
            .get(&format!("/printers/{}", self.printer_id))
            .await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let printer_json = self
            .get(&format!("/printers/{}", self.printer_id))
            .await?;

        let printer: FormlabsPrinterStatus = serde_json::from_value(printer_json)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let state = match printer.status.as_str() {
            "idle" | "ready" => MachineState::Idle,
            "printing" | "heating" | "filling" | "peeling" => MachineState::Busy,
            "paused" => MachineState::Paused,
            "error" | "offline" => MachineState::Error,
            _ => MachineState::Offline,
        };

        // Check for active job.
        let active_job = match self
            .get(&format!("/printers/{}/jobs/current", self.printer_id))
            .await
        {
            Ok(job_json) => {
                let job: FormlabsJobResponse = serde_json::from_value(job_json)
                    .map_err(|e| ConnectError::Protocol(e.to_string()))?;
                if !job.id.is_empty() {
                    Some(JobStatus {
                        state: match job.status.as_str() {
                            "printing" | "heating" | "filling" => JobState::Printing,
                            "paused" => JobState::Paused,
                            "complete" => JobState::Complete,
                            "cancelled" => JobState::Cancelled,
                            "failed" => JobState::Failed,
                            _ => JobState::Queued,
                        },
                        progress_pct: job.progress * 100.0,
                        elapsed_s: job.elapsed_time_s,
                        remaining_s: if job.estimated_time_s > job.elapsed_time_s {
                            Some(job.estimated_time_s - job.elapsed_time_s)
                        } else {
                            None
                        },
                        layers: match (job.current_layer, job.total_layers) {
                            (Some(cur), Some(total)) => Some((cur, total)),
                            _ => None,
                        },
                        filename: job.name,
                    })
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        // SLA printers typically do not expose temperature or position over API.
        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position: MachinePosition::default(),
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Stl {
            return Err(ConnectError::FormatNotAccepted(
                "Formlabs accepts STL files".into(),
            ));
        }

        let filename = if job.name.ends_with(".stl") {
            job.name.clone()
        } else {
            format!("{}.stl", job.name)
        };

        // Submit the job via the cloud API.
        let body = serde_json::json!({
            "printer_id": self.printer_id,
            "name": filename,
            "auto_start": job.auto_start,
            "file": base64_encode(&job.payload),
        });

        let resp = self
            .post_json(
                &format!("/printers/{}/jobs", self.printer_id),
                &body,
            )
            .await?;

        let job_id = resp["id"]
            .as_str()
            .unwrap_or(&filename)
            .to_string();

        Ok(JobHandle {
            job_id,
            filename,
        })
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "status": "cancelled" });
        self.post_json(
            &format!(
                "/printers/{}/jobs/{}/cancel",
                self.printer_id, handle.job_id
            ),
            &body,
        )
        .await?;
        Ok(())
    }

    async fn pause_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "status": "paused" });
        self.post_json(
            &format!(
                "/printers/{}/jobs/{}/pause",
                self.printer_id, handle.job_id
            ),
            &body,
        )
        .await?;
        Ok(())
    }

    async fn resume_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        let body = serde_json::json!({ "status": "printing" });
        self.post_json(
            &format!(
                "/printers/{}/jobs/{}/resume",
                self.printer_id, handle.job_id
            ),
            &body,
        )
        .await?;
        Ok(())
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let job_json = self
            .get(&format!(
                "/printers/{}/jobs/{}",
                self.printer_id, handle.job_id
            ))
            .await?;

        let job: FormlabsJobResponse = serde_json::from_value(job_json)
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        Ok(JobStatus {
            state: match job.status.as_str() {
                "printing" | "heating" | "filling" => JobState::Printing,
                "paused" => JobState::Paused,
                "complete" => JobState::Complete,
                "cancelled" => JobState::Cancelled,
                "failed" => JobState::Failed,
                _ => JobState::Queued,
            },
            progress_pct: job.progress * 100.0,
            elapsed_s: job.elapsed_time_s,
            remaining_s: if job.estimated_time_s > job.elapsed_time_s {
                Some(job.estimated_time_s - job.elapsed_time_s)
            } else {
                None
            },
            layers: match (job.current_layer, job.total_layers) {
                (Some(cur), Some(total)) => Some((cur, total)),
                _ => None,
            },
            filename: job.name,
        })
    }

    async fn send_command(&self, _cmd: &str) -> Result<String, ConnectError> {
        Err(ConnectError::Unsupported(
            "Formlabs SLA printers do not accept raw G-code commands".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // Cloud API — nothing to disconnect.
        Ok(())
    }
}

/// Simple base64 encoding for file upload payloads.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MachineConfig {
        MachineConfig {
            name: "Form 3".into(),
            kind: MachineKind::Sla,
            protocol: Protocol::Formlabs,
            address: "printer-serial-123".into(),
            auth: AuthConfig::BearerToken {
                token: "test-token-abc".into(),
            },
        }
    }

    #[test]
    fn create_connection() {
        let conn = FormlabsConnection::new(&test_config()).unwrap();
        assert_eq!(conn.info().protocol, Protocol::Formlabs);
        assert_eq!(conn.info().kind, MachineKind::Sla);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Stl]);
    }

    #[test]
    fn reject_wrong_auth() {
        let mut config = test_config();
        config.auth = AuthConfig::None;
        assert!(FormlabsConnection::new(&config).is_err());
    }

    #[test]
    fn base64_encode_basic() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_encode(b"AB"), "QUI=");
        assert_eq!(base64_encode(b"ABC"), "QUJD");
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }
}
