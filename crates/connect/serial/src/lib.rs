//! USB serial connectivity driver — Marlin and GRBL firmware.
//!
//! Provides line-based G-code streaming over USB serial ports with support
//! for both Marlin (3D printers) and GRBL (CNC mills, laser cutters) firmwares.
//!
//! GRBL uses character-counting flow control with a 128-byte planner buffer.
//! Marlin uses simple send-and-wait (`ok`) flow control.

use async_trait::async_trait;
use physical_connect_core::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default GRBL planner buffer size in bytes for character-counting flow control.
pub const GRBL_BUFFER_SIZE: usize = 128;

/// Firmware variant this serial connection speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firmware {
    Marlin,
    Grbl,
}

impl Firmware {
    /// Map firmware variant to a `Protocol`.
    pub fn protocol(&self) -> Protocol {
        match self {
            Firmware::Marlin => Protocol::SerialMarlin,
            Firmware::Grbl => Protocol::SerialGrbl,
        }
    }
}

/// Parsed GRBL-style status report.
///
/// Format: `<State|MPos:x,y,z|WPos:x,y,z|FS:feed,spindle|Bf:blocks,bytes>`
#[derive(Debug, Clone, Default)]
pub struct GrblStatus {
    /// Machine state (Idle, Run, Hold, Alarm, etc.).
    pub state: String,
    /// Machine position [x, y, z].
    pub mpos: [f64; 3],
    /// Work position [x, y, z].
    pub wpos: [f64; 3],
    /// Feed rate in mm/min.
    pub feed_rate: f64,
    /// Spindle speed in RPM.
    pub spindle_speed: f64,
    /// Planner buffer: (available blocks, available bytes).
    pub buffer: Option<(u32, u32)>,
}

impl GrblStatus {
    /// Parse a GRBL-style status string: `<Idle|MPos:0.000,0.000,0.000|...>`
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
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
                if !vals.is_empty() {
                    status.feed_rate = vals[0].parse().unwrap_or(0.0);
                }
                if vals.len() >= 2 {
                    status.spindle_speed = vals[1].parse().unwrap_or(0.0);
                }
            } else if let Some(f) = part.strip_prefix("F:") {
                status.feed_rate = f.parse().unwrap_or(0.0);
            } else if let Some(bf) = part.strip_prefix("Bf:") {
                let vals: Vec<&str> = bf.split(',').collect();
                if vals.len() >= 2 {
                    if let (Ok(blocks), Ok(bytes)) = (vals[0].parse(), vals[1].parse()) {
                        status.buffer = Some((blocks, bytes));
                    }
                }
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

/// Parsed Marlin temperature response.
///
/// Format: `ok T:200.00 /200.00 B:60.00 /60.00`
#[derive(Debug, Clone, Default)]
pub struct MarlinTemps {
    pub hotend_actual: f64,
    pub hotend_target: f64,
    pub bed_actual: f64,
    pub bed_target: f64,
}

impl MarlinTemps {
    /// Parse a Marlin temperature response line.
    ///
    /// Accepts formats like:
    /// - `ok T:200.00 /200.00 B:60.00 /60.00`
    /// - `T:200.00 /200.00 B:60.00 /60.00`
    pub fn parse(line: &str) -> Option<Self> {
        let mut temps = MarlinTemps::default();
        let mut found = false;

        // Parse T: hotend
        if let Some(t_idx) = line.find("T:") {
            let after_t = &line[t_idx + 2..];
            if let Some(actual) = Self::parse_next_float(after_t) {
                temps.hotend_actual = actual;
                found = true;
            }
            // Parse target after '/'
            if let Some(slash_idx) = after_t.find('/') {
                if let Some(target) = Self::parse_next_float(&after_t[slash_idx + 1..]) {
                    temps.hotend_target = target;
                }
            }
        }

        // Parse B: bed
        if let Some(b_idx) = line.find("B:") {
            let after_b = &line[b_idx + 2..];
            if let Some(actual) = Self::parse_next_float(after_b) {
                temps.bed_actual = actual;
                found = true;
            }
            if let Some(slash_idx) = after_b.find('/') {
                if let Some(target) = Self::parse_next_float(&after_b[slash_idx + 1..]) {
                    temps.bed_target = target;
                }
            }
        }

        if found { Some(temps) } else { None }
    }

    fn parse_next_float(s: &str) -> Option<f64> {
        let trimmed = s.trim_start();
        let end = trimmed
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
            .unwrap_or(trimmed.len());
        trimmed[..end].parse().ok()
    }
}

/// Serial port abstraction for testing and runtime use.
///
/// In production, this is backed by `tokio-serial`. In tests, it can be mocked.
#[async_trait]
pub trait SerialPort: Send + Sync {
    /// Write bytes to the serial port.
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), ConnectError>;
    /// Read a line (up to `\n`) from the serial port.
    async fn read_line(&mut self) -> Result<String, ConnectError>;
}

/// USB serial connection supporting Marlin and GRBL firmwares.
pub struct SerialConnection {
    info: MachineInfo,
    firmware: Firmware,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    /// GRBL character-counting buffer: tracks how many bytes are in the planner.
    grbl_buffer_used: Arc<Mutex<usize>>,
    /// Pending command lengths for GRBL character counting.
    grbl_pending_lengths: Arc<Mutex<Vec<usize>>>,
}

impl SerialConnection {
    /// Create a new serial connection with the given serial port implementation.
    pub fn new(
        config: &MachineConfig,
        firmware: Firmware,
        port: Box<dyn SerialPort>,
    ) -> Result<Self, ConnectError> {
        match &config.auth {
            AuthConfig::None => {}
            _ => {
                return Err(ConnectError::AuthFailed(
                    "Serial connections do not use authentication".into(),
                ))
            }
        }

        let protocol = firmware.protocol();
        let id = MachineId::new(format!(
            "serial-{}-{}",
            match firmware {
                Firmware::Marlin => "marlin",
                Firmware::Grbl => "grbl",
            },
            config.address.replace([':', '/', '.', ' '], "-")
        ));

        let kind = match firmware {
            Firmware::Marlin => config.kind,
            Firmware::Grbl => config.kind,
        };

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind,
                protocol,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            firmware,
            port: Arc::new(Mutex::new(port)),
            grbl_buffer_used: Arc::new(Mutex::new(0)),
            grbl_pending_lengths: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Send a line and wait for `ok` or `error:N`.
    async fn send_and_wait(&self, line: &str) -> Result<String, ConnectError> {
        let mut port = self.port.lock().await;

        let cmd = format!("{}\n", line.trim());

        if self.firmware == Firmware::Grbl {
            // Character-counting flow control for GRBL
            let cmd_len = cmd.len();
            loop {
                let used = *self.grbl_buffer_used.lock().await;
                if used + cmd_len <= GRBL_BUFFER_SIZE {
                    break;
                }
                // Wait for an `ok` to free buffer space
                let resp = port.read_line().await?;
                if resp.trim() == "ok" || resp.trim().starts_with("error:") {
                    let mut pending = self.grbl_pending_lengths.lock().await;
                    if let Some(freed) = pending.first().copied() {
                        pending.remove(0);
                        *self.grbl_buffer_used.lock().await -= freed;
                    }
                }
            }
            *self.grbl_buffer_used.lock().await += cmd_len;
            self.grbl_pending_lengths.lock().await.push(cmd_len);
        }

        port.write_all(cmd.as_bytes()).await?;

        // Read response lines until we get `ok` or `error:N`
        let mut response = String::new();
        loop {
            let line = port.read_line().await?;
            let trimmed = line.trim();

            if trimmed == "ok" {
                if self.firmware == Firmware::Grbl {
                    let mut pending = self.grbl_pending_lengths.lock().await;
                    if let Some(freed) = pending.first().copied() {
                        pending.remove(0);
                        *self.grbl_buffer_used.lock().await -= freed;
                    }
                }
                if response.is_empty() {
                    return Ok("ok".into());
                }
                return Ok(response);
            }

            if trimmed.starts_with("error:") || trimmed.starts_with("error ") {
                if self.firmware == Firmware::Grbl {
                    let mut pending = self.grbl_pending_lengths.lock().await;
                    if let Some(freed) = pending.first().copied() {
                        pending.remove(0);
                        *self.grbl_buffer_used.lock().await -= freed;
                    }
                }
                return Err(ConnectError::Protocol(trimmed.to_string()));
            }

            // Accumulate non-ok/error lines as response content
            if !response.is_empty() {
                response.push('\n');
            }
            response.push_str(trimmed);
        }
    }

    /// Send a GRBL real-time command (single byte, no newline, no response expected).
    async fn send_realtime(&self, byte: u8) -> Result<(), ConnectError> {
        let mut port = self.port.lock().await;
        port.write_all(&[byte]).await
    }
}

#[async_trait]
impl MachineConnection for SerialConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        match self.firmware {
            Firmware::Marlin => {
                // Send M115 (firmware info) — should always return `ok`
                self.send_and_wait("M115").await?;
            }
            Firmware::Grbl => {
                // Send $I (build info)
                self.send_and_wait("$I").await?;
            }
        }
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        match self.firmware {
            Firmware::Marlin => {
                // M105: report temperatures
                let resp = self.send_and_wait("M105").await?;
                let temps = MarlinTemps::parse(&resp);
                let temperatures = if let Some(t) = temps {
                    vec![
                        Temperature {
                            name: "extruder".into(),
                            actual: t.hotend_actual,
                            target: t.hotend_target,
                        },
                        Temperature {
                            name: "bed".into(),
                            actual: t.bed_actual,
                            target: t.bed_target,
                        },
                    ]
                } else {
                    Vec::new()
                };

                // M114: report current position
                let pos_resp = self.send_and_wait("M114").await.unwrap_or_default();
                let position = Self::parse_marlin_position(&pos_resp);

                let state = if temperatures.iter().any(|t| t.target > 0.0) {
                    MachineState::Busy
                } else {
                    MachineState::Idle
                };

                Ok(MachineStatus {
                    state,
                    temperatures,
                    position,
                    active_job: None,
                })
            }
            Firmware::Grbl => {
                // Send '?' realtime status query
                self.send_realtime(b'?').await?;

                // Read the status response
                let mut port = self.port.lock().await;
                let line = port.read_line().await?;
                drop(port);

                if let Some(grbl) = GrblStatus::parse(line.trim()) {
                    let state = match grbl.state.as_str() {
                        "Idle" => MachineState::Idle,
                        "Run" | "Home" | "Jog" | "Check" => MachineState::Busy,
                        "Hold" | "Hold:0" | "Hold:1" => MachineState::Paused,
                        "Alarm" | "Door" => MachineState::Error,
                        "Sleep" => MachineState::Idle,
                        _ => MachineState::Idle,
                    };

                    Ok(MachineStatus {
                        state,
                        temperatures: Vec::new(),
                        position: MachinePosition {
                            x: grbl.mpos[0],
                            y: grbl.mpos[1],
                            z: grbl.mpos[2],
                        },
                        active_job: if state == MachineState::Busy {
                            Some(JobStatus {
                                state: JobState::Printing,
                                progress_pct: 0.0,
                                elapsed_s: 0.0,
                                remaining_s: None,
                                layers: None,
                                filename: String::new(),
                            })
                        } else {
                            None
                        },
                    })
                } else {
                    Ok(MachineStatus {
                        state: MachineState::Idle,
                        temperatures: Vec::new(),
                        position: MachinePosition::default(),
                        active_job: None,
                    })
                }
            }
        }
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        if job.format != AcceptedFormat::Gcode {
            return Err(ConnectError::FormatNotAccepted(
                "Serial connections only accept G-code".into(),
            ));
        }

        let gcode = String::from_utf8(job.payload)
            .map_err(|e| ConnectError::Protocol(format!("invalid UTF-8 in G-code: {e}")))?;

        // Stream G-code lines to the machine
        for line in gcode.lines() {
            let trimmed = line.trim();
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('(') {
                continue;
            }
            // Strip inline comments
            let cmd = if let Some(idx) = trimmed.find(';') {
                trimmed[..idx].trim()
            } else {
                trimmed
            };
            if !cmd.is_empty() {
                self.send_and_wait(cmd).await?;
            }
        }

        let filename = job.name.clone();
        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        match self.firmware {
            Firmware::Marlin => {
                // M524: abort SD print (also stops serial streaming)
                let _ = self.send_and_wait("M524").await;
            }
            Firmware::Grbl => {
                // Ctrl-X (0x18): soft reset
                self.send_realtime(0x18).await?;
            }
        }
        Ok(())
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        match self.firmware {
            Firmware::Marlin => {
                // M25: pause SD print / pause serial streaming
                self.send_and_wait("M25").await?;
            }
            Firmware::Grbl => {
                // '!' (0x21): feed hold
                self.send_realtime(b'!').await?;
            }
        }
        Ok(())
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        match self.firmware {
            Firmware::Marlin => {
                // M24: resume SD print / resume serial streaming
                self.send_and_wait("M24").await?;
            }
            Firmware::Grbl => {
                // '~' (0x7E): cycle start/resume
                self.send_realtime(b'~').await?;
            }
        }
        Ok(())
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        // Handle GRBL real-time commands
        if self.firmware == Firmware::Grbl {
            match cmd {
                "?" => {
                    self.send_realtime(b'?').await?;
                    let mut port = self.port.lock().await;
                    return port.read_line().await;
                }
                "!" => {
                    self.send_realtime(b'!').await?;
                    return Ok("ok".into());
                }
                "~" => {
                    self.send_realtime(b'~').await?;
                    return Ok("ok".into());
                }
                "\x18" => {
                    self.send_realtime(0x18).await?;
                    return Ok("ok".into());
                }
                _ => {}
            }
        }

        self.send_and_wait(cmd).await
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // No explicit disconnect needed for serial; dropping the port handle closes it.
        Ok(())
    }
}

impl SerialConnection {
    /// Parse Marlin M114 position response: `X:10.00 Y:20.00 Z:5.00 E:0.00 Count ...`
    fn parse_marlin_position(resp: &str) -> MachinePosition {
        let mut pos = MachinePosition::default();

        // Stop at "Count" — after that, Marlin reports raw stepper counts (e.g. "X:1600")
        // which would overwrite the real mm positions.
        let useful = if let Some(idx) = resp.find("Count") {
            &resp[..idx]
        } else {
            resp
        };

        for part in useful.split_whitespace() {
            if let Some(val) = part.strip_prefix("X:") {
                pos.x = val.parse().unwrap_or(0.0);
            } else if let Some(val) = part.strip_prefix("Y:") {
                pos.y = val.parse().unwrap_or(0.0);
            } else if let Some(val) = part.strip_prefix("Z:") {
                pos.z = val.parse().unwrap_or(0.0);
            }
        }

        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Mock serial port for testing.
    struct MockSerial {
        /// Lines that will be returned by `read_line`, in order.
        responses: VecDeque<String>,
        /// Commands that were written.
        written: Vec<Vec<u8>>,
    }

    impl MockSerial {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: responses.into_iter().map(|s| s.to_string()).collect(),
                written: Vec::new(),
            }
        }
    }

    #[async_trait]
    impl SerialPort for MockSerial {
        async fn write_all(&mut self, buf: &[u8]) -> Result<(), ConnectError> {
            self.written.push(buf.to_vec());
            Ok(())
        }

        async fn read_line(&mut self) -> Result<String, ConnectError> {
            self.responses
                .pop_front()
                .ok_or_else(|| ConnectError::Protocol("no more mock responses".into()))
        }
    }

    fn make_marlin_config() -> MachineConfig {
        MachineConfig {
            name: "Ender 3".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::SerialMarlin,
            address: "/dev/ttyUSB0".into(),
            auth: AuthConfig::None,
        }
    }

    fn make_grbl_config() -> MachineConfig {
        MachineConfig {
            name: "Shapeoko".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::SerialGrbl,
            address: "/dev/ttyACM0".into(),
            auth: AuthConfig::None,
        }
    }

    #[test]
    fn create_marlin_connection() {
        let config = make_marlin_config();
        let port = Box::new(MockSerial::new(vec![]));
        let conn = SerialConnection::new(&config, Firmware::Marlin, port).unwrap();
        assert_eq!(conn.info().protocol, Protocol::SerialMarlin);
        assert_eq!(conn.info().name, "Ender 3");
        assert_eq!(conn.firmware, Firmware::Marlin);
    }

    #[test]
    fn create_grbl_connection() {
        let config = make_grbl_config();
        let port = Box::new(MockSerial::new(vec![]));
        let conn = SerialConnection::new(&config, Firmware::Grbl, port).unwrap();
        assert_eq!(conn.info().protocol, Protocol::SerialGrbl);
        assert_eq!(conn.info().name, "Shapeoko");
        assert_eq!(conn.firmware, Firmware::Grbl);
    }

    #[test]
    fn reject_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::SerialMarlin,
            address: "/dev/ttyUSB0".into(),
            auth: AuthConfig::ApiKey {
                key: "nope".into(),
            },
        };
        let port = Box::new(MockSerial::new(vec![]));
        assert!(SerialConnection::new(&config, Firmware::Marlin, port).is_err());
    }

    #[test]
    fn parse_grbl_status_idle() {
        let s = "<Idle|MPos:0.000,0.000,0.000|FS:0,0>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Idle");
        assert_eq!(status.mpos, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn parse_grbl_status_with_buffer() {
        let s = "<Run|MPos:10.0,20.0,5.0|Bf:15,120|FS:1000,0>";
        let status = GrblStatus::parse(s).unwrap();
        assert_eq!(status.state, "Run");
        assert_eq!(status.buffer, Some((15, 120)));
        assert_eq!(status.feed_rate, 1000.0);
    }

    #[test]
    fn parse_marlin_temps() {
        let line = "ok T:200.00 /200.00 B:60.00 /60.00";
        let temps = MarlinTemps::parse(line).unwrap();
        assert!((temps.hotend_actual - 200.0).abs() < 0.01);
        assert!((temps.hotend_target - 200.0).abs() < 0.01);
        assert!((temps.bed_actual - 60.0).abs() < 0.01);
        assert!((temps.bed_target - 60.0).abs() < 0.01);
    }

    #[test]
    fn parse_marlin_temps_no_prefix() {
        let line = "T:210.50 /215.00 B:55.30 /60.00";
        let temps = MarlinTemps::parse(line).unwrap();
        assert!((temps.hotend_actual - 210.5).abs() < 0.01);
        assert!((temps.hotend_target - 215.0).abs() < 0.01);
    }

    #[test]
    fn parse_marlin_position() {
        let resp = "X:10.00 Y:20.00 Z:5.00 E:0.00 Count X:1600 Y:3200 Z:8000";
        let pos = SerialConnection::parse_marlin_position(resp);
        assert!((pos.x - 10.0).abs() < 0.01);
        assert!((pos.y - 20.0).abs() < 0.01);
        assert!((pos.z - 5.0).abs() < 0.01);
    }

    #[test]
    fn accepted_formats() {
        let config = make_marlin_config();
        let port = Box::new(MockSerial::new(vec![]));
        let conn = SerialConnection::new(&config, Firmware::Marlin, port).unwrap();
        assert_eq!(conn.info().accepted_formats, vec![AcceptedFormat::Gcode]);
    }

    #[test]
    fn firmware_protocol_mapping() {
        assert_eq!(Firmware::Marlin.protocol(), Protocol::SerialMarlin);
        assert_eq!(Firmware::Grbl.protocol(), Protocol::SerialGrbl);
    }

    #[tokio::test]
    async fn marlin_ping() {
        let config = make_marlin_config();
        let port = Box::new(MockSerial::new(vec![
            "FIRMWARE_NAME:Marlin 2.1.2",
            "ok",
        ]));
        let conn = SerialConnection::new(&config, Firmware::Marlin, port).unwrap();
        assert!(conn.ping().await.is_ok());
    }

    #[tokio::test]
    async fn grbl_ping() {
        let config = make_grbl_config();
        let port = Box::new(MockSerial::new(vec![
            "[VER:1.1h.20190825:]",
            "ok",
        ]));
        let conn = SerialConnection::new(&config, Firmware::Grbl, port).unwrap();
        assert!(conn.ping().await.is_ok());
    }

    #[tokio::test]
    async fn marlin_send_command() {
        let config = make_marlin_config();
        let port = Box::new(MockSerial::new(vec!["ok"]));
        let conn = SerialConnection::new(&config, Firmware::Marlin, port).unwrap();
        let resp = conn.send_command("G28").await.unwrap();
        assert_eq!(resp, "ok");
    }
}
