//! Ruida UDP laser controller connectivity driver.
//!
//! Communicates with Ruida-based CO2 laser controllers (RDC6442G, RDC6445G, etc.)
//! using the proprietary binary UDP protocol on ports 50200 (command) and 40200 (data).
//! Bytes are scrambled with XOR 0x88 before transmission.

use async_trait::async_trait;
use physical_connect_core::*;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// Default Ruida command port.
const RUIDA_CMD_PORT: u16 = 50200;

/// Default Ruida data/file-upload port.
const RUIDA_DATA_PORT: u16 = 40200;

/// XOR scramble byte used in the Ruida protocol.
const SCRAMBLE_BYTE: u8 = 0x88;

/// Ruida laser controller connection via UDP.
pub struct RuidaConnection {
    info: MachineInfo,
    /// Controller address (host only — ports are fixed).
    target_addr: String,
    /// Bound local socket for command channel (port 50200).
    cmd_socket: Option<UdpSocket>,
}

impl RuidaConnection {
    /// Create a new Ruida connection.
    ///
    /// `config.address` should be the controller IP (e.g., `"192.168.1.50"`).
    /// Authentication is not used — Ruida controllers have no auth.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        if !matches!(config.auth, AuthConfig::None) {
            return Err(ConnectError::AuthFailed(
                "Ruida controllers do not use authentication".into(),
            ));
        }

        let id = MachineId::new(format!(
            "ruida-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::LaserCut,
                protocol: Protocol::RuidaUdp,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::RuidaRd],
                build_volume: None,
                firmware: None,
            },
            target_addr: config.address.clone(),
            cmd_socket: None,
        })
    }

    /// Scramble or unscramble a buffer using XOR 0x88.
    fn scramble(data: &[u8]) -> Vec<u8> {
        data.iter().map(|b| b ^ SCRAMBLE_BYTE).collect()
    }

    /// Ensure the command UDP socket is bound.
    async fn ensure_socket(&mut self) -> Result<&UdpSocket, ConnectError> {
        if self.cmd_socket.is_none() {
            let sock = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
            self.cmd_socket = Some(sock);
        }
        Ok(self.cmd_socket.as_ref().unwrap())
    }

    /// Send a scrambled command packet and receive the response.
    async fn send_cmd(&self, packet: &[u8]) -> Result<Vec<u8>, ConnectError> {
        let socket = self
            .cmd_socket
            .as_ref()
            .ok_or_else(|| ConnectError::ConnectionRefused("socket not initialised".into()))?;

        let scrambled = Self::scramble(packet);
        let dest: SocketAddr = format!("{}:{}", self.target_addr, RUIDA_CMD_PORT)
            .parse()
            .map_err(|e: std::net::AddrParseError| ConnectError::ConnectionRefused(e.to_string()))?;

        socket
            .send_to(&scrambled, dest)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        let mut buf = [0u8; 1024];
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            socket.recv_from(&mut buf),
        )
        .await
        .map_err(|_| ConnectError::Timeout("Ruida command timeout".into()))?
        .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        let (len, _addr) = timeout;
        Ok(Self::scramble(&buf[..len]))
    }

    /// Upload a .rd file to the controller on the data port.
    async fn upload_file(&self, filename: &str, data: &[u8]) -> Result<(), ConnectError> {
        let data_sock = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        let dest: SocketAddr = format!("{}:{}", self.target_addr, RUIDA_DATA_PORT)
            .parse()
            .map_err(|e: std::net::AddrParseError| ConnectError::ConnectionRefused(e.to_string()))?;

        // Send file-start header (simplified: real protocol has more framing)
        let mut header = Vec::new();
        header.push(0xD8); // file transfer command
        header.extend_from_slice(filename.as_bytes());
        header.push(0x00); // null terminator

        let scrambled_header = Self::scramble(&header);
        data_sock
            .send_to(&scrambled_header, dest)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        // Send file data in chunks (max 1000 bytes per UDP datagram)
        for chunk in data.chunks(1000) {
            let mut pkt = Vec::with_capacity(chunk.len() + 1);
            pkt.push(0xD9); // data chunk command
            pkt.extend_from_slice(chunk);
            let scrambled_chunk = Self::scramble(&pkt);
            data_sock
                .send_to(&scrambled_chunk, dest)
                .await
                .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        }

        // Send end-of-file marker
        let eof = Self::scramble(&[0xDA]);
        data_sock
            .send_to(&eof, dest)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl MachineConnection for RuidaConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        // Send a status query (0xDA 0x00 0x04) and check for any response.
        let _resp = self.send_cmd(&[0xDA, 0x00, 0x04]).await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let resp = self.send_cmd(&[0xDA, 0x00, 0x04]).await?;

        // Parse the basic status byte from response.
        let state = if resp.len() >= 2 {
            match resp[1] {
                0x00 => MachineState::Idle,
                0x01 => MachineState::Busy,
                0x02 => MachineState::Paused,
                0xFF => MachineState::Error,
                _ => MachineState::Idle,
            }
        } else {
            MachineState::Offline
        };

        // Ruida controllers do not report temperatures.
        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position: MachinePosition::default(),
            active_job: None,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::RuidaRd {
            return Err(ConnectError::FormatNotAccepted(
                "Ruida controllers only accept .rd binary files".into(),
            ));
        }

        let filename = if job.name.ends_with(".rd") {
            job.name.clone()
        } else {
            format!("{}.rd", job.name)
        };

        self.upload_file(&filename, &job.payload).await?;

        if job.auto_start {
            // Send start command (0xD7 0x00)
            self.send_cmd(&[0xD7, 0x00]).await?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Stop/abort command
        self.send_cmd(&[0xD8, 0x02]).await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Pause command
        self.send_cmd(&[0xD8, 0x01]).await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Resume command
        self.send_cmd(&[0xD8, 0x00]).await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let resp = self.send_cmd(&[0xDA, 0x00, 0x04]).await?;

        let state = if resp.len() >= 2 {
            match resp[1] {
                0x01 => JobState::Printing,
                0x02 => JobState::Paused,
                _ => JobState::Complete,
            }
        } else {
            JobState::Complete
        };

        // Ruida does not report fine-grained progress over UDP.
        Ok(JobStatus {
            state,
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: String::new(),
        })
    }

    async fn send_command(&self, _cmd: &str) -> Result<String, ConnectError> {
        Err(ConnectError::Unsupported(
            "Ruida controllers do not accept raw G-code commands".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        self.cmd_socket = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MachineConfig {
        MachineConfig {
            name: "Ruida Laser".into(),
            kind: MachineKind::LaserCut,
            protocol: Protocol::RuidaUdp,
            address: "192.168.1.50".into(),
            auth: AuthConfig::None,
        }
    }

    #[test]
    fn create_connection() {
        let conn = RuidaConnection::new(&test_config()).unwrap();
        assert_eq!(conn.info().protocol, Protocol::RuidaUdp);
        assert_eq!(conn.info().kind, MachineKind::LaserCut);
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::RuidaRd]);
    }

    #[test]
    fn reject_auth() {
        let mut config = test_config();
        config.auth = AuthConfig::ApiKey {
            key: "bad".into(),
        };
        assert!(RuidaConnection::new(&config).is_err());
    }

    #[test]
    fn scramble_roundtrip() {
        let data = b"hello ruida";
        let scrambled = RuidaConnection::scramble(data);
        let unscrambled = RuidaConnection::scramble(&scrambled);
        assert_eq!(&unscrambled, data);
    }

    #[test]
    fn scramble_xor_value() {
        let data = [0x00, 0xFF, 0x88];
        let scrambled = RuidaConnection::scramble(&data);
        assert_eq!(scrambled, vec![0x88, 0x77, 0x00]);
    }
}
