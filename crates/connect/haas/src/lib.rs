//! Haas MDC (Machine Data Collection) connectivity driver via HTTP.
//!
//! Communicates with Haas CNC machines using the MDC protocol over HTTP port 80.
//! This is a read-heavy protocol: Haas machines receive jobs via network shares,
//! not through the MDC interface.
//!
//! MDC macro reference:
//! - Q100: Machine status
//! - Q104: Running program
//! - Q500: Max spindle speed
//! - Q600: Total machine time

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;

/// Haas MDC connection via HTTP GET requests.
pub struct HaasMdcConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
}

impl HaasMdcConnection {
    /// Create a new Haas MDC connection.
    ///
    /// Accepts `AuthConfig::None` only (MDC has no authentication).
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        if !matches!(config.auth, AuthConfig::None) {
            return Err(ConnectError::AuthFailed(
                "Haas MDC does not use authentication".into(),
            ));
        }

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "haas-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::CncMill,
                protocol: Protocol::HaasMdc,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
        })
    }

    /// Query an MDC macro by number.
    async fn query_mdc(&self, macro_num: u16) -> Result<String, ConnectError> {
        let url = format!("{}/MDC?Q{}", self.base_url, macro_num);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.text()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }
}

/// Parse Q100 machine status response into a MachineState.
fn parse_q100_status(resp: &str) -> MachineState {
    let trimmed = resp.trim();
    // Q100 returns comma-separated fields; first field after macro name is status
    if let Some(status_part) = trimmed.split(',').nth(1) {
        let s = status_part.trim().to_uppercase();
        match s.as_str() {
            "IDLE" => MachineState::Idle,
            "FEED HOLD" => MachineState::Paused,
            "ALARM" => MachineState::Error,
            _ if s.contains("RUN") => MachineState::Busy,
            _ => MachineState::Offline,
        }
    } else {
        MachineState::Offline
    }
}

/// Parse Q104 running program name from response.
fn parse_q104_program(resp: &str) -> Option<String> {
    let trimmed = resp.trim();
    trimmed
        .split(',')
        .nth(1)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "MDI" && s != "NONE")
}

#[async_trait]
impl MachineConnection for HaasMdcConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.query_mdc(100).await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let q100 = self.query_mdc(100).await?;
        let state = parse_q100_status(&q100);

        let q104 = self.query_mdc(104).await.ok();
        let active_job = q104.as_deref().and_then(parse_q104_program).map(|prog| {
            JobStatus {
                state: if state == MachineState::Busy {
                    JobState::Printing
                } else if state == MachineState::Paused {
                    JobState::Paused
                } else {
                    JobState::Queued
                },
                progress_pct: 0.0,
                elapsed_s: 0.0,
                remaining_s: None,
                layers: None,
                filename: prog,
            }
        });

        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position: MachinePosition::default(),
            active_job,
        })
    }

    async fn submit_job(&self, _job: JobSubmission) -> Result<JobHandle, ConnectError> {
        Err(ConnectError::Unsupported(
            "Haas machines receive jobs via network share, not MDC".into(),
        ))
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "Job control not available via Haas MDC".into(),
        ))
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "Job control not available via Haas MDC".into(),
        ))
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "Job control not available via Haas MDC".into(),
        ))
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let q100 = self.query_mdc(100).await?;
        let state = parse_q100_status(&q100);

        let q104 = self.query_mdc(104).await?;
        let program = parse_q104_program(&q104);

        let job_state = if state == MachineState::Busy {
            JobState::Printing
        } else if state == MachineState::Paused {
            JobState::Paused
        } else {
            JobState::Complete
        };

        Ok(JobStatus {
            state: job_state,
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: program.unwrap_or_else(|| handle.filename.clone()),
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        // MDC supports limited command queries; attempt as a raw macro query
        let macro_num: u16 = cmd
            .trim_start_matches('Q')
            .parse()
            .map_err(|_| ConnectError::Protocol(format!("invalid MDC macro: {cmd}")))?;
        self.query_mdc(macro_num).await
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // HTTP is stateless; nothing to close.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection() {
        let config = MachineConfig {
            name: "Haas VF-2".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::HaasMdc,
            address: "192.168.1.200".into(),
            auth: AuthConfig::None,
        };
        let conn = HaasMdcConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::HaasMdc);
        assert_eq!(conn.info().kind, MachineKind::CncMill);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
        assert_eq!(conn.info().name, "Haas VF-2");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::HaasMdc,
            address: "localhost".into(),
            auth: AuthConfig::ApiKey {
                key: "nope".into(),
            },
        };
        assert!(HaasMdcConnection::new(&config).is_err());
    }

    #[test]
    fn parse_status_idle() {
        assert_eq!(
            parse_q100_status("Q100, IDLE, 0, 0"),
            MachineState::Idle
        );
    }

    #[test]
    fn parse_status_running() {
        assert_eq!(
            parse_q100_status("Q100, RUN, 12345, 100"),
            MachineState::Busy
        );
    }

    #[test]
    fn parse_status_alarm() {
        assert_eq!(
            parse_q100_status("Q100, ALARM, 200, 0"),
            MachineState::Error
        );
    }

    #[test]
    fn parse_status_feed_hold() {
        assert_eq!(
            parse_q100_status("Q100, FEED HOLD, 0, 0"),
            MachineState::Paused
        );
    }

    #[test]
    fn parse_program_name() {
        assert_eq!(
            parse_q104_program("Q104, O01234"),
            Some("O01234".to_string())
        );
    }

    #[test]
    fn parse_program_none() {
        assert_eq!(parse_q104_program("Q104, NONE"), None);
    }

    #[test]
    fn url_generation() {
        let config = MachineConfig {
            name: "Haas".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::HaasMdc,
            address: "10.0.0.50".into(),
            auth: AuthConfig::None,
        };
        let conn = HaasMdcConnection::new(&config).unwrap();
        assert_eq!(conn.base_url, "http://10.0.0.50");
    }
}
