//! K40 CO2 laser connectivity driver via USB bulk transfer.
//!
//! Communicates with the ubiquitous K40 CO2 laser cutters using the Lhymicro-GL
//! 34-byte packet protocol over USB. These lasers use a Nano board with USB
//! Vendor ID 0x1A86 and Product ID 0x5512.

use async_trait::async_trait;
use physical_connect_core::*;

/// K40 USB Vendor ID (QinHeng Electronics CH341).
pub const K40_VENDOR_ID: u16 = 0x1A86;

/// K40 USB Product ID.
pub const K40_PRODUCT_ID: u16 = 0x5512;

/// Lhymicro-GL packet size.
const PACKET_SIZE: usize = 34;

/// Connection state for K40 USB laser.
pub struct K40Connection {
    info: MachineInfo,
    /// Whether we have an open USB handle (simulated without libusb dependency).
    connected: bool,
}

impl K40Connection {
    /// Create a new K40 connection.
    ///
    /// `config.address` is informational (e.g., `"usb://1a86:5512"`), since
    /// the K40 is discovered by USB VID/PID.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        if !matches!(config.auth, AuthConfig::None) {
            return Err(ConnectError::AuthFailed(
                "K40 USB does not use authentication".into(),
            ));
        }

        let id = MachineId::new(format!(
            "k40-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::LaserCut,
                protocol: Protocol::K40Usb,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::LhymicroGl],
                build_volume: Some([300.0, 200.0, 0.0]), // typical K40 bed
                firmware: None,
            },
            connected: false,
        })
    }

    /// Build a 34-byte Lhymicro-GL packet from a command byte sequence.
    ///
    /// Pads to `PACKET_SIZE` with 0x00 bytes.
    fn build_packet(cmd: &[u8]) -> Vec<u8> {
        let mut pkt = vec![0u8; PACKET_SIZE];
        let len = cmd.len().min(PACKET_SIZE);
        pkt[..len].copy_from_slice(&cmd[..len]);
        pkt
    }

    /// Send a raw USB bulk-transfer packet (stubbed — requires platform USB library).
    async fn send_packet(&self, _packet: &[u8]) -> Result<(), ConnectError> {
        if !self.connected {
            return Err(ConnectError::ConnectionRefused(
                "K40 USB device not connected".into(),
            ));
        }
        // In a real implementation this would call libusb bulk_transfer.
        Ok(())
    }
}

#[async_trait]
impl MachineConnection for K40Connection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        if !self.connected {
            return Err(ConnectError::ConnectionRefused(
                "K40 USB device not connected".into(),
            ));
        }
        // Send a status request packet.
        let pkt = Self::build_packet(&[0xA6]); // status query
        self.send_packet(&pkt).await
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let state = if self.connected {
            MachineState::Idle
        } else {
            MachineState::Offline
        };

        // K40 has no temperature sensors or position feedback.
        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position: MachinePosition::default(),
            active_job: None,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::LhymicroGl {
            return Err(ConnectError::FormatNotAccepted(
                "K40 only accepts Lhymicro-GL binary format".into(),
            ));
        }

        // Stream Lhymicro-GL data in 34-byte packets.
        for chunk in job.payload.chunks(PACKET_SIZE) {
            let pkt = Self::build_packet(chunk);
            self.send_packet(&pkt).await?;
        }

        let filename = job.name.clone();
        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Send stop/reset packet.
        let pkt = Self::build_packet(&[0xA8]); // emergency stop
        self.send_packet(&pkt).await
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let pkt = Self::build_packet(&[0xA7]); // pause
        self.send_packet(&pkt).await
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let pkt = Self::build_packet(&[0xA9]); // resume
        self.send_packet(&pkt).await
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        // K40 has no job tracking — it streams and fires.
        Ok(JobStatus {
            state: if self.connected {
                JobState::Complete
            } else {
                JobState::Failed
            },
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: String::new(),
        })
    }

    async fn send_command(&self, _cmd: &str) -> Result<String, ConnectError> {
        Err(ConnectError::Unsupported(
            "K40 does not accept text commands — use Lhymicro-GL binary".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        self.connected = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MachineConfig {
        MachineConfig {
            name: "K40 Laser".into(),
            kind: MachineKind::LaserCut,
            protocol: Protocol::K40Usb,
            address: "usb://1a86:5512".into(),
            auth: AuthConfig::None,
        }
    }

    #[test]
    fn create_connection() {
        let conn = K40Connection::new(&test_config()).unwrap();
        assert_eq!(conn.info().protocol, Protocol::K40Usb);
        assert_eq!(conn.info().kind, MachineKind::LaserCut);
        assert_eq!(
            conn.info().accepted_formats,
            vec![AcceptedFormat::LhymicroGl]
        );
    }

    #[test]
    fn reject_auth() {
        let mut config = test_config();
        config.auth = AuthConfig::BearerToken {
            token: "bad".into(),
        };
        assert!(K40Connection::new(&config).is_err());
    }

    #[test]
    fn build_packet_padding() {
        let pkt = K40Connection::build_packet(&[0xA6]);
        assert_eq!(pkt.len(), PACKET_SIZE);
        assert_eq!(pkt[0], 0xA6);
        assert!(pkt[1..].iter().all(|&b| b == 0x00));
    }

    #[test]
    fn build_packet_full() {
        let data = vec![0xAB; PACKET_SIZE];
        let pkt = K40Connection::build_packet(&data);
        assert_eq!(pkt.len(), PACKET_SIZE);
        assert!(pkt.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn default_build_volume() {
        let conn = K40Connection::new(&test_config()).unwrap();
        assert_eq!(conn.info().build_volume, Some([300.0, 200.0, 0.0]));
    }
}
