//! LightBurn Bridge TCP connectivity driver.
//!
//! Connects to a LightBurn Bridge instance on port 5555, which acts as a TCP-to-serial
//! bridge for GRBL and Ruida controllers. Commands are plain-text G-code sent over TCP.

use async_trait::async_trait;
use physical_connect_core::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// Default LightBurn Bridge port.
const LIGHTBURN_PORT: u16 = 5555;

/// LightBurn Bridge TCP connection.
pub struct LightBurnConnection {
    info: MachineInfo,
    address: String,
    stream: Mutex<Option<TcpStream>>,
}

impl LightBurnConnection {
    /// Create a new LightBurn Bridge connection.
    ///
    /// `config.address` should be `"host"` or `"host:port"` (default port 5555).
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        if !matches!(config.auth, AuthConfig::None) {
            return Err(ConnectError::AuthFailed(
                "LightBurn Bridge does not use authentication".into(),
            ));
        }

        let address = if config.address.contains(':') {
            config.address.clone()
        } else {
            format!("{}:{}", config.address, LIGHTBURN_PORT)
        };

        let id = MachineId::new(format!(
            "lightburn-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::LaserCut,
                protocol: Protocol::LightBurnBridge,
                address: address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            address,
            stream: Mutex::new(None),
        })
    }

    /// Ensure the TCP connection is established.
    async fn ensure_connected(&self) -> Result<(), ConnectError> {
        let mut stream = self.stream.lock().await;
        if stream.is_none() {
            let tcp = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                TcpStream::connect(&self.address),
            )
            .await
            .map_err(|_| ConnectError::Timeout("LightBurn Bridge connection timeout".into()))?
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
            *stream = Some(tcp);
        }
        Ok(())
    }

    /// Send a line and read the response.
    async fn send_line(&self, line: &str) -> Result<String, ConnectError> {
        self.ensure_connected().await?;
        let mut stream = self.stream.lock().await;
        let tcp = stream
            .as_mut()
            .ok_or_else(|| ConnectError::ConnectionRefused("not connected".into()))?;

        let cmd = format!("{}\n", line);
        tcp.write_all(cmd.as_bytes())
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let mut buf = [0u8; 4096];
        let n = tokio::time::timeout(std::time::Duration::from_secs(5), tcp.read(&mut buf))
            .await
            .map_err(|_| ConnectError::Timeout("response timeout".into()))?
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        Ok(String::from_utf8_lossy(&buf[..n]).to_string())
    }
}

#[async_trait]
impl MachineConnection for LightBurnConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.ensure_connected().await?;
        // Send a GRBL status query
        let _resp = self.send_line("?").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let resp = self.send_line("?").await?;

        // Parse GRBL-style status: <Idle|MPos:0.000,0.000,0.000|...>
        let state = if resp.contains("Idle") {
            MachineState::Idle
        } else if resp.contains("Run") {
            MachineState::Busy
        } else if resp.contains("Hold") {
            MachineState::Paused
        } else if resp.contains("Alarm") {
            MachineState::Error
        } else {
            MachineState::Offline
        };

        let mut position = MachinePosition::default();
        if let Some(mpos_start) = resp.find("MPos:") {
            let mpos_str = &resp[mpos_start + 5..];
            if let Some(end) = mpos_str.find('|').or_else(|| mpos_str.find('>')) {
                let coords: Vec<&str> = mpos_str[..end].split(',').collect();
                if coords.len() >= 3 {
                    position.x = coords[0].parse().unwrap_or(0.0);
                    position.y = coords[1].parse().unwrap_or(0.0);
                    position.z = coords[2].parse().unwrap_or(0.0);
                }
            }
        }

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
                "LightBurn Bridge only accepts G-code".into(),
            ));
        }

        let gcode = String::from_utf8(job.payload)
            .map_err(|e| ConnectError::Protocol(format!("invalid UTF-8 G-code: {e}")))?;

        // Stream G-code lines one at a time through the bridge.
        for line in gcode.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }
            let resp = self.send_line(trimmed).await?;
            if resp.contains("error") {
                return Err(ConnectError::Protocol(format!(
                    "GRBL error on line: {trimmed} — {resp}"
                )));
            }
        }

        let filename = if job.name.ends_with(".gcode") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // GRBL soft reset
        self.send_line("\x18").await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // GRBL feed hold
        self.send_line("!").await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // GRBL cycle resume
        self.send_line("~").await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let resp = self.send_line("?").await?;

        let state = if resp.contains("Run") {
            JobState::Printing
        } else if resp.contains("Hold") {
            JobState::Paused
        } else if resp.contains("Idle") {
            JobState::Complete
        } else {
            JobState::Failed
        };

        Ok(JobStatus {
            state,
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: String::new(),
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        self.send_line(cmd).await
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        let mut stream = self.stream.lock().await;
        if let Some(mut tcp) = stream.take() {
            let _ = tcp.shutdown().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MachineConfig {
        MachineConfig {
            name: "LightBurn Laser".into(),
            kind: MachineKind::LaserCut,
            protocol: Protocol::LightBurnBridge,
            address: "192.168.1.100".into(),
            auth: AuthConfig::None,
        }
    }

    #[test]
    fn create_connection() {
        let conn = LightBurnConnection::new(&test_config()).unwrap();
        assert_eq!(conn.info().protocol, Protocol::LightBurnBridge);
        assert_eq!(conn.info().kind, MachineKind::LaserCut);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
    }

    #[test]
    fn default_port_appended() {
        let conn = LightBurnConnection::new(&test_config()).unwrap();
        assert_eq!(conn.address, "192.168.1.100:5555");
    }

    #[test]
    fn custom_port_preserved() {
        let mut config = test_config();
        config.address = "10.0.0.5:9999".into();
        let conn = LightBurnConnection::new(&config).unwrap();
        assert_eq!(conn.address, "10.0.0.5:9999");
    }

    #[test]
    fn reject_auth() {
        let mut config = test_config();
        config.auth = AuthConfig::ApiKey {
            key: "bad".into(),
        };
        assert!(LightBurnConnection::new(&config).is_err());
    }
}
