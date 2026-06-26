//! LinuxCNC TCP connectivity driver.
//!
//! Communicates with LinuxCNC via the text-based TCP protocol on port 5007.
//! Supports MDI commands, position queries, and program status monitoring.
//! Protocol reference: <http://linuxcnc.org/docs/html/man/man1/linuxcncrsh.1.html>

use async_trait::async_trait;
use physical_connect_core::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// LinuxCNC connection via TCP text protocol.
pub struct LinuxCncConnection {
    info: MachineInfo,
    address: String,
    stream: Mutex<Option<BufReader<TcpStream>>>,
}

impl LinuxCncConnection {
    /// Create a new LinuxCNC connection.
    ///
    /// Accepts `AuthConfig::None` or `AuthConfig::UsernamePassword`.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        match &config.auth {
            AuthConfig::None | AuthConfig::UsernamePassword { .. } => {}
            _ => {
                return Err(ConnectError::AuthFailed(
                    "LinuxCNC accepts None or UsernamePassword auth".into(),
                ));
            }
        }

        let address = if config.address.contains(':') {
            config.address.clone()
        } else {
            format!("{}:5007", config.address)
        };

        let id = MachineId::new(format!(
            "linuxcnc-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::CncMill,
                protocol: Protocol::LinuxCnc,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            address,
            stream: Mutex::new(None),
        })
    }

    /// Ensure TCP connection is established and return mutable access.
    async fn ensure_connected(&self) -> Result<(), ConnectError> {
        let mut guard = self.stream.lock().await;
        if guard.is_none() {
            let tcp = TcpStream::connect(&self.address)
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
            let mut reader = BufReader::new(tcp);

            // Read the greeting line from LinuxCNC
            let mut greeting = String::new();
            reader
                .read_line(&mut greeting)
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;

            // Send hello
            reader
                .get_mut()
                .write_all(b"hello EMC localhost 1.0\r\n")
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;

            let mut resp = String::new();
            reader
                .read_line(&mut resp)
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;

            if !resp.starts_with("HELLO ACK") {
                return Err(ConnectError::Protocol(format!(
                    "unexpected hello response: {resp}"
                )));
            }

            // Enable machine
            reader
                .get_mut()
                .write_all(b"set enable EMCTOO\r\n")
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;
            let mut ack = String::new();
            reader
                .read_line(&mut ack)
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;

            *guard = Some(reader);
        }
        Ok(())
    }

    /// Send a command over the TCP connection and read the response line.
    async fn send_recv(&self, cmd: &str) -> Result<String, ConnectError> {
        self.ensure_connected().await?;
        let mut guard = self.stream.lock().await;
        let reader = guard
            .as_mut()
            .ok_or_else(|| ConnectError::ConnectionRefused("not connected".into()))?;

        let line = format!("{cmd}\r\n");
        reader
            .get_mut()
            .write_all(line.as_bytes())
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let mut response = String::new();
        reader
            .read_line(&mut response)
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        Ok(response.trim().to_string())
    }
}

#[async_trait]
impl MachineConnection for LinuxCncConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.ensure_connected().await
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        // Query position
        let pos_resp = self.send_recv("get position").await?;
        let position = parse_position(&pos_resp);

        // Query program status
        let status_resp = self.send_recv("get program_status").await?;
        let state = parse_machine_state(&status_resp);

        Ok(MachineStatus {
            state,
            temperatures: Vec::new(), // CNC mills don't have heaters
            position,
            active_job: None,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "LinuxCNC only accepts G-code".into(),
            ));
        }

        // Switch to MDI mode and stream G-code lines
        self.send_recv("set mode mdi").await?;

        let gcode = String::from_utf8_lossy(&job.payload);
        for line in gcode.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('(') {
                continue;
            }
            let cmd = format!("set mdi {trimmed}");
            let resp = self.send_recv(&cmd).await?;
            if resp.contains("NAK") || resp.contains("error") {
                return Err(ConnectError::Protocol(format!(
                    "G-code error on '{trimmed}': {resp}"
                )));
            }
        }

        Ok(JobHandle {
            job_id: job.name.clone(),
            filename: job.name,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_recv("set abort").await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_recv("set pause").await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        self.send_recv("set resume").await?;
        Ok(())
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let resp = self.send_recv("get program_status").await?;
        let state = if resp.contains("running") {
            JobState::Printing
        } else if resp.contains("paused") {
            JobState::Paused
        } else if resp.contains("idle") {
            JobState::Complete
        } else {
            JobState::Queued
        };

        Ok(JobStatus {
            state,
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: handle.filename.clone(),
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        self.send_recv("set mode mdi").await?;
        self.send_recv(&format!("set mdi {cmd}")).await
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        let mut guard = self.stream.lock().await;
        if let Some(ref mut reader) = *guard {
            let _ = reader.get_mut().write_all(b"quit\r\n").await;
            let _ = reader.get_mut().shutdown().await;
        }
        *guard = None;
        Ok(())
    }
}

/// Parse a position response like "POSITION X1.000 Y2.000 Z3.000".
fn parse_position(resp: &str) -> MachinePosition {
    let mut pos = MachinePosition::default();
    for token in resp.split_whitespace() {
        if let Some(val) = token.strip_prefix('X') {
            pos.x = val.parse().unwrap_or(0.0);
        } else if let Some(val) = token.strip_prefix('Y') {
            pos.y = val.parse().unwrap_or(0.0);
        } else if let Some(val) = token.strip_prefix('Z') {
            pos.z = val.parse().unwrap_or(0.0);
        }
    }
    pos
}

/// Parse program_status response into a MachineState.
fn parse_machine_state(resp: &str) -> MachineState {
    let lower = resp.to_lowercase();
    if lower.contains("running") {
        MachineState::Busy
    } else if lower.contains("paused") {
        MachineState::Paused
    } else if lower.contains("idle") {
        MachineState::Idle
    } else {
        MachineState::Offline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection_no_auth() {
        let config = MachineConfig {
            name: "Bridgeport CNC".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::LinuxCnc,
            address: "192.168.1.50".into(),
            auth: AuthConfig::None,
        };
        let conn = LinuxCncConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::LinuxCnc);
        assert_eq!(conn.info().kind, MachineKind::CncMill);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
    }

    #[test]
    fn create_connection_with_password() {
        let config = MachineConfig {
            name: "Shop Mill".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::LinuxCnc,
            address: "10.0.0.100:5007".into(),
            auth: AuthConfig::UsernamePassword {
                username: "operator".into(),
                password: "secret".into(),
            },
        };
        let conn = LinuxCncConnection::new(&config).unwrap();
        assert_eq!(conn.info().name, "Shop Mill");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::LinuxCnc,
            address: "localhost".into(),
            auth: AuthConfig::ApiKey {
                key: "nope".into(),
            },
        };
        assert!(LinuxCncConnection::new(&config).is_err());
    }

    #[test]
    fn parse_position_response() {
        let pos = parse_position("POSITION X10.500 Y-3.200 Z0.000");
        assert!((pos.x - 10.5).abs() < f64::EPSILON);
        assert!((pos.y - -3.2).abs() < f64::EPSILON);
        assert!((pos.z - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_state_idle() {
        assert_eq!(parse_machine_state("PROGRAM_STATUS idle"), MachineState::Idle);
    }

    #[test]
    fn parse_state_running() {
        assert_eq!(
            parse_machine_state("PROGRAM_STATUS running"),
            MachineState::Busy
        );
    }

    #[test]
    fn default_port_appended() {
        let config = MachineConfig {
            name: "Mill".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::LinuxCnc,
            address: "192.168.1.50".into(),
            auth: AuthConfig::None,
        };
        let conn = LinuxCncConnection::new(&config).unwrap();
        assert_eq!(conn.address, "192.168.1.50:5007");
    }
}
