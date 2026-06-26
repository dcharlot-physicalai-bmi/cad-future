//! FluidNC connectivity driver — HTTP + WebSocket.
//!
//! Supports FluidNC firmware running on ESP32-based CNC mills and laser cutters.
//! FluidNC uses a GRBL-compatible protocol over WebSocket for real-time control,
//! and HTTP endpoints for file upload.
//! Reference: <https://github.com/bdring/FluidNC>

use async_trait::async_trait;
use physical_connect_core::*;
use reqwest::Client;

/// FluidNC connection via HTTP REST + WebSocket.
pub struct FluidNcConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
}

/// Parsed GRBL-style status report from FluidNC.
///
/// Format: `<State|MPos:x,y,z|WPos:x,y,z|FS:feed,spindle|...>`
#[derive(Debug, Clone, Default)]
pub struct GrblStatus {
    /// Machine state string (Idle, Run, Hold, Alarm, etc.).
    pub state: String,
    /// Machine position [x, y, z].
    pub mpos: [f64; 3],
    /// Work position [x, y, z].
    pub wpos: [f64; 3],
    /// Feed rate in mm/min.
    pub feed_rate: f64,
    /// Spindle speed in RPM.
    pub spindle_speed: f64,
}

impl GrblStatus {
    /// Parse a GRBL-style status string.
    ///
    /// Input format: `<Idle|MPos:0.000,0.000,0.000|FS:0,0>`
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        // Strip angle brackets
        let inner = trimmed.strip_prefix('<')?.strip_suffix('>')?;
        let mut parts = inner.split('|');

        let state = parts.next()?.to_string();
        let mut status = GrblStatus {
            state,
            ..Default::default()
        };

        for part in parts {
            if let Some(coords) = part.strip_prefix("MPos:") {
                status.mpos = Self::parse_coords(coords);
            } else if let Some(coords) = part.strip_prefix("WPos:") {
                status.wpos = Self::parse_coords(coords);
            } else if let Some(fs) = part.strip_prefix("FS:") {
                let vals: Vec<&str> = fs.split(',').collect();
                if vals.len() >= 1 {
                    status.feed_rate = vals[0].parse().unwrap_or(0.0);
                }
                if vals.len() >= 2 {
                    status.spindle_speed = vals[1].parse().unwrap_or(0.0);
                }
            } else if let Some(f) = part.strip_prefix("F:") {
                status.feed_rate = f.parse().unwrap_or(0.0);
            }
        }

        Some(status)
    }

    fn parse_coords(s: &str) -> [f64; 3] {
        let vals: Vec<f64> = s.split(',').filter_map(|v| v.parse().ok()).collect();
        [
            vals.first().copied().unwrap_or(0.0),
            vals.get(1).copied().unwrap_or(0.0),
            vals.get(2).copied().unwrap_or(0.0),
        ]
    }
}

impl FluidNcConnection {
    /// Create a new FluidNC connection.
    ///
    /// FluidNC does not require authentication; `config.auth` should be `AuthConfig::None`.
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        match &config.auth {
            AuthConfig::None => {}
            _ => {
                return Err(ConnectError::AuthFailed(
                    "FluidNC does not use authentication".into(),
                ))
            }
        }

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "fluidnc-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::FluidNc,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
        })
    }

    /// Send a G-code command via HTTP and return the response text.
    async fn send_gcode_http(&self, cmd: &str) -> Result<String, ConnectError> {
        let url = format!("{}/command?commandText={}", self.base_url, cmd);
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

    /// Query the machine status via the `?` realtime command.
    async fn query_status(&self) -> Result<GrblStatus, ConnectError> {
        let text = self.send_gcode_http("?").await?;

        // FluidNC may return multiple lines; find the status line
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('<') && trimmed.ends_with('>') {
                if let Some(status) = GrblStatus::parse(trimmed) {
                    return Ok(status);
                }
            }
        }

        Err(ConnectError::Protocol(
            "no GRBL status response received".into(),
        ))
    }

    fn grbl_state_to_machine_state(grbl_state: &str) -> MachineState {
        match grbl_state {
            "Idle" => MachineState::Idle,
            "Run" => MachineState::Busy,
            "Hold" | "Hold:0" | "Hold:1" => MachineState::Paused,
            "Alarm" | "Door" | "Door:0" | "Door:1" | "Door:2" | "Door:3" => MachineState::Error,
            "Check" | "Home" | "Jog" => MachineState::Busy,
            "Sleep" => MachineState::Idle,
            _ => MachineState::Idle,
        }
    }
}

#[async_trait]
impl MachineConnection for FluidNcConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        // Send empty command to check connectivity
        let url = format!("{}/command?commandText=%24I", self.base_url);
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

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let grbl = self.query_status().await?;

        let state = Self::grbl_state_to_machine_state(&grbl.state);
        let position = MachinePosition {
            x: grbl.mpos[0],
            y: grbl.mpos[1],
            z: grbl.mpos[2],
        };

        // FluidNC CNC/laser machines typically don't have temperature sensors,
        // but we report spindle/feed as part of the status via active_job.
        let active_job = if state == MachineState::Busy {
            Some(JobStatus {
                state: JobState::Printing,
                progress_pct: 0.0, // FluidNC does not report job progress via status
                elapsed_s: 0.0,
                remaining_s: None,
                layers: None,
                filename: String::new(),
            })
        } else {
            None
        };

        Ok(MachineStatus {
            state,
            temperatures: Vec::new(),
            position,
            active_job,
        })
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "FluidNC only accepts G-code".into(),
            ));
        }

        let filename = if job.name.ends_with(".gcode") || job.name.ends_with(".nc") || job.name.ends_with(".gc") {
            job.name.clone()
        } else {
            format!("{}.gcode", job.name)
        };

        // Upload via multipart POST to /upload
        let part = reqwest::multipart::Part::bytes(job.payload)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("path", "/");

        let resp = self
            .client
            .post(format!("{}/upload", self.base_url))
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

        // Start running the file if auto_start
        if job.auto_start {
            let run_cmd = format!("$SD/Run=/{}", filename);
            self.send_gcode_http(&run_cmd).await?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Send reset command (Ctrl-X / 0x18) via HTTP
        self.send_gcode_http("%18").await?;
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Feed hold: '!' character
        self.send_gcode_http("!").await?;
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        // Cycle start/resume: '~' character
        self.send_gcode_http("~").await?;
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        self.send_gcode_http(cmd).await
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // FluidNC HTTP is stateless; no explicit disconnect needed.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_fluidnc_cnc() {
        let config = MachineConfig {
            name: "Shapeoko ESP32".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::FluidNc,
            address: "192.168.1.200".into(),
            auth: AuthConfig::None,
        };
        let conn = FluidNcConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::FluidNc);
        assert_eq!(conn.info().kind, MachineKind::CncMill);
        assert_eq!(conn.info().name, "Shapeoko ESP32");
    }

    #[test]
    fn create_fluidnc_laser() {
        let config = MachineConfig {
            name: "Laser Diode".into(),
            kind: MachineKind::LaserCut,
            protocol: Protocol::FluidNc,
            address: "laser.local".into(),
            auth: AuthConfig::None,
        };
        let conn = FluidNcConnection::new(&config).unwrap();
        assert_eq!(conn.info().kind, MachineKind::LaserCut);
    }

    #[test]
    fn reject_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::FluidNc,
            address: "localhost".into(),
            auth: AuthConfig::ApiKey {
                key: "nope".into(),
            },
        };
        assert!(FluidNcConnection::new(&config).is_err());
    }

    #[test]
    fn parse_grbl_status_idle() {
        let s = "<Idle|MPos:0.000,0.000,0.000|FS:0,0>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Idle");
        assert_eq!(status.mpos, [0.0, 0.0, 0.0]);
        assert_eq!(status.feed_rate, 0.0);
        assert_eq!(status.spindle_speed, 0.0);
    }

    #[test]
    fn parse_grbl_status_running() {
        let s = "<Run|MPos:12.500,-3.200,0.000|WPos:12.500,-3.200,0.000|FS:1500,10000>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Run");
        assert_eq!(status.mpos, [12.5, -3.2, 0.0]);
        assert_eq!(status.wpos, [12.5, -3.2, 0.0]);
        assert_eq!(status.feed_rate, 1500.0);
        assert_eq!(status.spindle_speed, 10000.0);
    }

    #[test]
    fn parse_grbl_status_hold() {
        let s = "<Hold:0|MPos:5.000,10.000,2.000|F:800>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Hold:0");
        assert_eq!(status.mpos, [5.0, 10.0, 2.0]);
        assert_eq!(status.feed_rate, 800.0);
    }

    #[test]
    fn parse_grbl_status_alarm() {
        let s = "<Alarm|MPos:0.000,0.000,0.000>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Alarm");
    }

    #[test]
    fn grbl_state_mapping() {
        assert_eq!(
            FluidNcConnection::grbl_state_to_machine_state("Idle"),
            MachineState::Idle
        );
        assert_eq!(
            FluidNcConnection::grbl_state_to_machine_state("Run"),
            MachineState::Busy
        );
        assert_eq!(
            FluidNcConnection::grbl_state_to_machine_state("Hold:0"),
            MachineState::Paused
        );
        assert_eq!(
            FluidNcConnection::grbl_state_to_machine_state("Alarm"),
            MachineState::Error
        );
    }

    #[test]
    fn accepted_formats() {
        let config = MachineConfig {
            name: "CNC".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::FluidNc,
            address: "localhost".into(),
            auth: AuthConfig::None,
        };
        let conn = FluidNcConnection::new(&config).unwrap();
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
    }
}
