//! OpenIE Manufacturing Protocol firmware adapter.
//!
//! Embeddable in ESP32, RP2040, or any microcontroller with a WebSocket stack.
//! This crate handles the machine side of the OMP protocol:
//!
//! 1. Receives JSON-RPC requests from clients
//! 2. Dispatches to machine-specific handlers
//! 3. Sends responses and status notifications
//!
//! The firmware implementor provides a `MachineHandler` that translates
//! protocol commands into actual machine operations (stepping, heating, etc.).
//!
//! ## Usage
//!
//! ```ignore
//! let mut server = OmpServer::new(my_handler, my_capabilities);
//! // When a WebSocket text frame arrives:
//! let response = server.handle_message(incoming_text);
//! // Send response back over WebSocket
//! ws_send(response);
//! // Periodically push status:
//! let status_json = server.status_notification();
//! ws_send(status_json);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use physical_connect_protocol::auth::Permissions;
use physical_connect_protocol::capability::MachineCapabilities;
use physical_connect_protocol::job::{self as omp_job};
use physical_connect_protocol::message::*;
use physical_connect_protocol::method;
use physical_connect_protocol::status::MachineStatus;
use physical_connect_protocol::stream;

/// Trait that firmware implementors provide.
///
/// Maps OMP protocol commands to actual machine operations.
pub trait MachineHandler {
    /// Get current machine status.
    fn status(&self) -> MachineStatus;

    /// Execute G-code line(s). Returns number of lines buffered.
    fn execute_gcode(&mut self, lines: &[String]) -> u32;

    /// Start a job that was previously uploaded.
    fn start_job(&mut self, job_id: &str) -> Result<(), String>;

    /// Pause the current job.
    fn pause_job(&mut self) -> Result<(), String>;

    /// Resume the current job.
    fn resume_job(&mut self) -> Result<(), String>;

    /// Cancel the current job.
    fn cancel_job(&mut self) -> Result<(), String>;

    /// Get job status by ID.
    fn job_status(&self, job_id: &str) -> Option<omp_job::JobStatus>;

    /// Home specified axes (empty = all).
    fn home(&mut self, axes: &[String]) -> Result<(), String>;

    /// Relative jog move.
    fn jog(&mut self, x: f64, y: f64, z: f64, feed: f64) -> Result<(), String>;

    /// Set heater temperature.
    fn set_temperature(&mut self, heater: &str, target_c: f64) -> Result<(), String>;

    /// Set fan speed (0.0 - 1.0).
    fn set_fan(&mut self, fan: &str, speed: f64) -> Result<(), String>;

    /// Emergency stop.
    fn emergency_stop(&mut self) -> Result<(), String>;

    /// Reset after emergency stop.
    fn reset(&mut self) -> Result<(), String>;

    /// Accept file upload data. Returns true if complete.
    fn receive_upload_chunk(
        &mut self,
        upload_id: u64,
        offset: u32,
        total_size: u32,
        data: &[u8],
    ) -> bool;

    /// Begin a job from an uploaded file.
    fn begin_uploaded_job(&mut self, job_id: &str, upload_id: u64) -> Result<(), String>;

    /// Get buffer status for G-code streaming.
    fn buffer_status(&self) -> stream::BufferStatus;
}

/// OMP server — handles incoming messages and produces responses.
///
/// Sits between the WebSocket transport and the `MachineHandler`.
pub struct OmpServer<H: MachineHandler> {
    handler: H,
    capabilities: MachineCapabilities,
    authenticated: bool,
    pub permissions: Permissions,
    status_interval_ms: u32,
    pending_jobs: Vec<PendingJob>,
}

struct PendingJob {
    job_id: String,
    upload_id: u64,
    auto_start: bool,
}

impl<H: MachineHandler> OmpServer<H> {
    pub fn new(handler: H, capabilities: MachineCapabilities) -> Self {
        Self {
            handler,
            capabilities,
            authenticated: false,
            permissions: Permissions::full(), // Default to full for LAN
            status_interval_ms: 500,
            pending_jobs: Vec::new(),
        }
    }

    /// Handle an incoming WebSocket text message. Returns response text to send back.
    pub fn handle_text(&mut self, text: &str) -> Option<String> {
        let req: Request = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(_) => {
                let resp = Response::err(MessageId::Int(0), PARSE_ERROR, "invalid JSON-RPC");
                return Some(serde_json::to_string(&resp).unwrap_or_default());
            }
        };

        let resp = self.dispatch(&req);
        Some(serde_json::to_string(&resp).unwrap_or_default())
    }

    /// Handle an incoming binary frame (file upload chunk).
    pub fn handle_binary(&mut self, data: &[u8]) -> Option<String> {
        if data.len() < BinaryHeader::SIZE {
            return None;
        }

        let header_bytes: [u8; 16] = data[..16].try_into().unwrap();
        let header = BinaryHeader::decode(&header_bytes);
        let chunk = &data[16..];

        let complete = self.handler.receive_upload_chunk(
            header.upload_id,
            header.offset,
            header.total_size,
            chunk,
        );

        if complete {
            // Check if any pending job needs to start
            if let Some(idx) = self
                .pending_jobs
                .iter()
                .position(|j| j.upload_id == header.upload_id)
            {
                let job = self.pending_jobs.remove(idx);
                let notif = Notification::new(
                    method::names::JOB_UPLOAD_COMPLETE,
                    Some(serde_json::json!({
                        "job_id": &job.job_id,
                        "upload_id": header.upload_id,
                        "bytes_received": header.total_size,
                    })),
                );

                if job.auto_start {
                    let _ = self.handler.begin_uploaded_job(&job.job_id, job.upload_id);
                }

                return Some(serde_json::to_string(&notif).unwrap_or_default());
            }
        }

        None
    }

    /// Generate a status notification for periodic push.
    pub fn status_notification(&self) -> String {
        let status = self.handler.status();
        let notif = Notification::new(
            method::names::STATUS_UPDATE,
            Some(serde_json::to_value(&status).unwrap()),
        );
        serde_json::to_string(&notif).unwrap_or_default()
    }

    /// Configured status push interval.
    pub fn status_interval_ms(&self) -> u32 {
        self.status_interval_ms
    }

    fn dispatch(&mut self, req: &Request) -> Response {
        match req.method.as_str() {
            method::names::HELLO => self.handle_hello(req),
            method::names::GOODBYE => Response::ok(req.id.clone(), serde_json::json!({})),
            method::names::CAPABILITIES => {
                Response::ok(req.id.clone(), serde_json::to_value(&self.capabilities).unwrap())
            }
            method::names::STATUS => {
                let status = self.handler.status();
                Response::ok(req.id.clone(), serde_json::to_value(&status).unwrap())
            }
            method::names::STATUS_SUBSCRIBE => self.handle_subscribe(req),
            method::names::STATUS_UNSUBSCRIBE => {
                Response::ok(req.id.clone(), serde_json::json!({"subscribed": false}))
            }
            method::names::JOB_SUBMIT => self.handle_job_submit(req),
            method::names::JOB_START => self.handle_simple_job_cmd(req, "start"),
            method::names::JOB_PAUSE => self.handle_simple_job_cmd(req, "pause"),
            method::names::JOB_RESUME => self.handle_simple_job_cmd(req, "resume"),
            method::names::JOB_CANCEL => self.handle_simple_job_cmd(req, "cancel"),
            method::names::JOB_STATUS => self.handle_job_status(req),
            method::names::GCODE_SEND => self.handle_gcode_send(req),
            method::names::GCODE_BUFFER => {
                let buf = self.handler.buffer_status();
                Response::ok(req.id.clone(), serde_json::to_value(&buf).unwrap())
            }
            method::names::HOME => self.handle_home(req),
            method::names::JOG => self.handle_jog(req),
            method::names::SET_TEMPERATURE => self.handle_set_temp(req),
            method::names::SET_FAN => self.handle_set_fan(req),
            method::names::EMERGENCY_STOP => {
                match self.handler.emergency_stop() {
                    Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
                    Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
                }
            }
            method::names::RESET => {
                match self.handler.reset() {
                    Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
                    Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
                }
            }
            _ => Response::err(req.id.clone(), METHOD_NOT_FOUND, format!("unknown method: {}", req.method)),
        }
    }

    fn handle_hello(&mut self, req: &Request) -> Response {
        if let Some(params) = &req.params {
            if let Ok(_hello) = serde_json::from_value::<method::HelloRequest>(params.clone()) {
                self.authenticated = true;
                // Return capabilities
                return Response::ok(
                    req.id.clone(),
                    serde_json::to_value(&self.capabilities).unwrap(),
                );
            }
        }
        Response::err(req.id.clone(), INVALID_PARAMS, "invalid hello request")
    }

    fn handle_subscribe(&mut self, req: &Request) -> Response {
        if let Some(params) = &req.params {
            if let Some(interval) = params["interval_ms"].as_u64() {
                self.status_interval_ms = (interval as u32).clamp(100, 5000);
            }
        }
        Response::ok(req.id.clone(), serde_json::json!({"subscribed": true}))
    }

    fn handle_job_submit(&mut self, req: &Request) -> Response {
        let params = match &req.params {
            Some(p) => p,
            None => return Response::err(req.id.clone(), INVALID_PARAMS, "missing params"),
        };

        let submit: omp_job::JobSubmitRequest = match serde_json::from_value(params.clone()) {
            Ok(s) => s,
            Err(e) => return Response::err(req.id.clone(), INVALID_PARAMS, e.to_string()),
        };

        // Generate job ID and upload ID
        let job_id = format!("job-{}", self.pending_jobs.len() + 1);
        let upload_id = job_id.len() as u64 + 1000; // Simple ID generation

        self.pending_jobs.push(PendingJob {
            job_id: job_id.clone(),
            upload_id,
            auto_start: submit.auto_start,
        });

        let resp = omp_job::JobSubmitResponse {
            job_id,
            upload_id,
            max_chunk_bytes: 65536,
        };

        Response::ok(req.id.clone(), serde_json::to_value(&resp).unwrap())
    }

    fn handle_simple_job_cmd(&mut self, req: &Request, cmd: &str) -> Response {
        let job_id = req
            .params
            .as_ref()
            .and_then(|p| p["job_id"].as_str())
            .unwrap_or("");

        let result = match cmd {
            "start" => self.handler.start_job(job_id),
            "pause" => self.handler.pause_job(),
            "resume" => self.handler.resume_job(),
            "cancel" => self.handler.cancel_job(),
            _ => Err("unknown command".into()),
        };

        match result {
            Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
            Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
        }
    }

    fn handle_job_status(&self, req: &Request) -> Response {
        let job_id = req
            .params
            .as_ref()
            .and_then(|p| p["job_id"].as_str())
            .unwrap_or("");

        match self.handler.job_status(job_id) {
            Some(status) => Response::ok(req.id.clone(), serde_json::to_value(&status).unwrap()),
            None => Response::err(req.id.clone(), JOB_NOT_FOUND, format!("job {job_id} not found")),
        }
    }

    fn handle_gcode_send(&mut self, req: &Request) -> Response {
        let params = match &req.params {
            Some(p) => p,
            None => return Response::err(req.id.clone(), INVALID_PARAMS, "missing params"),
        };

        let send_req: stream::GcodeSendRequest = match serde_json::from_value(params.clone()) {
            Ok(s) => s,
            Err(e) => return Response::err(req.id.clone(), INVALID_PARAMS, e.to_string()),
        };

        let buffered = self.handler.execute_gcode(&send_req.lines);
        let rejected = send_req.lines.len() as u32 - buffered;

        let resp = stream::GcodeSendResponse { buffered, rejected };
        Response::ok(req.id.clone(), serde_json::to_value(&resp).unwrap())
    }

    fn handle_home(&mut self, req: &Request) -> Response {
        let axes: Vec<String> = req
            .params
            .as_ref()
            .and_then(|p| p["axes"].as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        match self.handler.home(&axes) {
            Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
            Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
        }
    }

    fn handle_jog(&mut self, req: &Request) -> Response {
        let p = match &req.params {
            Some(p) => p,
            None => return Response::err(req.id.clone(), INVALID_PARAMS, "missing params"),
        };

        let x = p["x"].as_f64().unwrap_or(0.0);
        let y = p["y"].as_f64().unwrap_or(0.0);
        let z = p["z"].as_f64().unwrap_or(0.0);
        let feed = p["feed_mm_min"].as_f64().unwrap_or(1000.0);

        match self.handler.jog(x, y, z, feed) {
            Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
            Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
        }
    }

    fn handle_set_temp(&mut self, req: &Request) -> Response {
        let p = match &req.params {
            Some(p) => p,
            None => return Response::err(req.id.clone(), INVALID_PARAMS, "missing params"),
        };

        let heater = p["heater"].as_str().unwrap_or("");
        let target = p["target_c"].as_f64().unwrap_or(0.0);

        match self.handler.set_temperature(heater, target) {
            Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
            Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
        }
    }

    fn handle_set_fan(&mut self, req: &Request) -> Response {
        let p = match &req.params {
            Some(p) => p,
            None => return Response::err(req.id.clone(), INVALID_PARAMS, "missing params"),
        };

        let fan = p["fan"].as_str().unwrap_or("");
        let speed = p["speed"].as_f64().unwrap_or(0.0);

        match self.handler.set_fan(fan, speed) {
            Ok(()) => Response::ok(req.id.clone(), serde_json::json!({})),
            Err(e) => Response::err(req.id.clone(), MACHINE_ERROR, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_connect_protocol::capability::*;
    use physical_connect_protocol::status::*;

    /// Mock machine handler for testing.
    struct MockHandler {
        state: MachineState,
    }

    impl MockHandler {
        fn new() -> Self {
            Self {
                state: MachineState::Idle,
            }
        }
    }

    impl MachineHandler for MockHandler {
        fn status(&self) -> MachineStatus {
            MachineStatus {
                state: self.state,
                heaters: alloc::vec![HeaterStatus {
                    name: "extruder_0".into(),
                    actual_c: 25.0,
                    target_c: 0.0,
                    power: 0.0,
                }],
                position: Position::default(),
                job: None,
                spindle: None,
                laser: None,
                fans: Vec::new(),
                error: None,
                uptime_s: 0.0,
                free_storage_bytes: None,
            }
        }

        fn execute_gcode(&mut self, lines: &[String]) -> u32 {
            lines.len() as u32
        }

        fn start_job(&mut self, _job_id: &str) -> Result<(), String> {
            self.state = MachineState::Running;
            Ok(())
        }

        fn pause_job(&mut self) -> Result<(), String> {
            self.state = MachineState::Paused;
            Ok(())
        }

        fn resume_job(&mut self) -> Result<(), String> {
            self.state = MachineState::Running;
            Ok(())
        }

        fn cancel_job(&mut self) -> Result<(), String> {
            self.state = MachineState::Idle;
            Ok(())
        }

        fn job_status(&self, _job_id: &str) -> Option<omp_job::JobStatus> {
            None
        }

        fn home(&mut self, _axes: &[String]) -> Result<(), String> {
            Ok(())
        }

        fn jog(&mut self, _x: f64, _y: f64, _z: f64, _feed: f64) -> Result<(), String> {
            Ok(())
        }

        fn set_temperature(&mut self, _heater: &str, _target_c: f64) -> Result<(), String> {
            Ok(())
        }

        fn set_fan(&mut self, _fan: &str, _speed: f64) -> Result<(), String> {
            Ok(())
        }

        fn emergency_stop(&mut self) -> Result<(), String> {
            self.state = MachineState::Emergency;
            Ok(())
        }

        fn reset(&mut self) -> Result<(), String> {
            self.state = MachineState::Idle;
            Ok(())
        }

        fn receive_upload_chunk(&mut self, _upload_id: u64, _offset: u32, _total_size: u32, _data: &[u8]) -> bool {
            false
        }

        fn begin_uploaded_job(&mut self, _job_id: &str, _upload_id: u64) -> Result<(), String> {
            self.state = MachineState::Running;
            Ok(())
        }

        fn buffer_status(&self) -> stream::BufferStatus {
            stream::BufferStatus {
                capacity: 16,
                used: 0,
                free: 16,
                last_ack_sequence: 0,
            }
        }
    }

    fn test_caps() -> MachineCapabilities {
        MachineCapabilities {
            protocol_version: "0.1.0".into(),
            machine_id: "TEST-001".into(),
            name: "Mock Printer".into(),
            manufacturer: "OpenIE".into(),
            model: "Test".into(),
            firmware_version: "1.0.0".into(),
            machine_type: MachineType::Fdm {
                extruder_count: 1, heated_bed: true, heated_chamber: false,
                filament_sensor: false, auto_bed_leveling: false,
            },
            build_volume: BuildVolume { x_mm: 220.0, y_mm: 220.0, z_mm: 250.0, is_cylindrical: false },
            accepted_formats: alloc::vec![FileFormat {
                mime_type: "application/x-gcode".into(), extension: "gcode".into(), preferred: true,
            }],
            axes: Vec::new(),
            heaters: Vec::new(),
            spindles: Vec::new(),
            lasers: Vec::new(),
            tool_changer: None,
            enclosure: None,
            features: alloc::vec![Feature::PauseResume, Feature::EmergencyStop, Feature::RawGcode],
            max_queue_depth: 1,
            stream_buffer_size: 16,
        }
    }

    #[test]
    fn hello_handshake() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let hello = r#"{"jsonrpc":"2.0","id":1,"method":"hello","params":{"protocol_version":"0.1.0","client_name":"Test","client_version":"0.1.0","auth":{"method":"none"}}}"#;
        let resp = server.handle_text(hello).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_none());
        assert!(parsed.result.is_some());
        assert!(server.authenticated);
    }

    #[test]
    fn status_query() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let req = r#"{"jsonrpc":"2.0","id":2,"method":"machine.status"}"#;
        let resp = server.handle_text(req).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_none());
        let status: MachineStatus = serde_json::from_value(parsed.result.unwrap()).unwrap();
        assert_eq!(status.state, MachineState::Idle);
    }

    #[test]
    fn gcode_send() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let req = r#"{"jsonrpc":"2.0","id":3,"method":"gcode.send","params":{"lines":["G28","G1 X10 F1000"],"sequence":0}}"#;
        let resp = server.handle_text(req).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_none());
        let send_resp: stream::GcodeSendResponse = serde_json::from_value(parsed.result.unwrap()).unwrap();
        assert_eq!(send_resp.buffered, 2);
        assert_eq!(send_resp.rejected, 0);
    }

    #[test]
    fn emergency_stop() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let req = r#"{"jsonrpc":"2.0","id":4,"method":"control.emergency_stop"}"#;
        let resp = server.handle_text(req).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_none());
    }

    #[test]
    fn unknown_method() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let req = r#"{"jsonrpc":"2.0","id":5,"method":"nonexistent.method"}"#;
        let resp = server.handle_text(req).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_some());
        assert_eq!(parsed.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn status_notification_format() {
        let server = OmpServer::new(MockHandler::new(), test_caps());
        let notif_str = server.status_notification();
        let notif: Notification = serde_json::from_str(&notif_str).unwrap();
        assert_eq!(notif.method, "status.update");
        assert!(notif.params.is_some());
    }

    #[test]
    fn job_submit_and_start() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let req = r#"{"jsonrpc":"2.0","id":6,"method":"job.submit","params":{"name":"test.gcode","mime_type":"application/x-gcode","extension":"gcode","size_bytes":1024,"auto_start":true}}"#;
        let resp = server.handle_text(req).unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_none());
        let submit_resp: omp_job::JobSubmitResponse = serde_json::from_value(parsed.result.unwrap()).unwrap();
        assert!(!submit_resp.job_id.is_empty());
        assert!(submit_resp.upload_id > 0);
    }

    #[test]
    fn invalid_json() {
        let mut server = OmpServer::new(MockHandler::new(), test_caps());
        let resp = server.handle_text("not json at all").unwrap();
        let parsed: Response = serde_json::from_str(&resp).unwrap();
        assert!(parsed.error.is_some());
        assert_eq!(parsed.error.unwrap().code, PARSE_ERROR);
    }
}
