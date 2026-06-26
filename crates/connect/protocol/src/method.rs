//! JSON-RPC method names — the complete OMP method catalog.
//!
//! Every interaction between client and machine is one of these methods.
//! No undocumented endpoints, no protocol-specific quirks.

/// Method names as constants for type safety.
pub mod names {
    // === Connection lifecycle ===

    /// Client → Machine. First message after WebSocket connect.
    /// Params: `{ "protocol_version": "0.1.0", "client_name": "OpenIE CAD", "auth": {...} }`
    /// Result: `MachineCapabilities`
    pub const HELLO: &str = "hello";

    /// Client → Machine. Graceful disconnect.
    pub const GOODBYE: &str = "goodbye";

    // === Machine info ===

    /// Client → Machine. Get full capability declaration.
    /// Result: `MachineCapabilities`
    pub const CAPABILITIES: &str = "machine.capabilities";

    /// Client → Machine. Get current status snapshot.
    /// Result: `MachineStatus`
    pub const STATUS: &str = "machine.status";

    /// Client → Machine. Subscribe to status push notifications.
    /// Params: `{ "interval_ms": 500 }` (min 100, max 5000)
    /// Result: `{ "subscribed": true }`
    pub const STATUS_SUBSCRIBE: &str = "machine.status.subscribe";

    /// Client → Machine. Unsubscribe from status pushes.
    pub const STATUS_UNSUBSCRIBE: &str = "machine.status.unsubscribe";

    // === Job management ===

    /// Client → Machine. Submit a new job.
    /// Params: `JobSubmitRequest`
    /// Result: `JobSubmitResponse` (includes upload_id for binary frames)
    pub const JOB_SUBMIT: &str = "job.submit";

    /// Client → Machine. Start a queued job.
    /// Params: `{ "job_id": "..." }`
    pub const JOB_START: &str = "job.start";

    /// Client → Machine. Pause a running job.
    /// Params: `{ "job_id": "..." }`
    pub const JOB_PAUSE: &str = "job.pause";

    /// Client → Machine. Resume a paused job.
    /// Params: `{ "job_id": "..." }`
    pub const JOB_RESUME: &str = "job.resume";

    /// Client → Machine. Cancel a job (any non-terminal state).
    /// Params: `{ "job_id": "..." }`
    pub const JOB_CANCEL: &str = "job.cancel";

    /// Client → Machine. Get status of a specific job.
    /// Params: `{ "job_id": "..." }`
    /// Result: `JobStatus`
    pub const JOB_STATUS: &str = "job.status";

    /// Client → Machine. List all jobs (queue + history).
    /// Params: `{ "include_completed": false }`
    /// Result: `{ "jobs": [JobQueueEntry] }`
    pub const JOB_LIST: &str = "job.list";

    /// Machine → Client (notification). Upload fully received.
    /// Params: `{ "job_id": "...", "upload_id": N, "bytes_received": N }`
    pub const JOB_UPLOAD_COMPLETE: &str = "job.upload.complete";

    // === G-code streaming ===

    /// Client → Machine. Send G-code line(s) for immediate execution.
    /// Params: `{ "lines": ["G28", "G1 X10 F1000"], "sequence": N }`
    /// Result: `{ "buffered": N }` (number of lines accepted into buffer)
    pub const GCODE_SEND: &str = "gcode.send";

    /// Machine → Client (notification). G-code line acknowledged.
    /// Params: `{ "sequence": N, "response": "ok", "line": "G28" }`
    pub const GCODE_ACK: &str = "gcode.ack";

    /// Machine → Client (notification). G-code line error.
    /// Params: `{ "sequence": N, "error": "...", "line": "..." }`
    pub const GCODE_ERROR: &str = "gcode.error";

    /// Client → Machine. Query buffer state.
    /// Result: `{ "capacity": N, "used": N, "free": N }`
    pub const GCODE_BUFFER: &str = "gcode.buffer";

    // === Manual control ===

    /// Client → Machine. Home one or more axes.
    /// Params: `{ "axes": ["X", "Y", "Z"] }` (empty = home all)
    pub const HOME: &str = "control.home";

    /// Client → Machine. Relative jog move.
    /// Params: `{ "x": 10.0, "y": 0, "z": 0, "feed_mm_min": 3000.0 }`
    pub const JOG: &str = "control.jog";

    /// Client → Machine. Set heater temperature.
    /// Params: `{ "heater": "extruder_0", "target_c": 215.0 }`
    pub const SET_TEMPERATURE: &str = "control.temperature";

    /// Client → Machine. Set fan speed.
    /// Params: `{ "fan": "part_cooling", "speed": 1.0 }`
    pub const SET_FAN: &str = "control.fan";

    /// Client → Machine. Set spindle RPM (CNC).
    /// Params: `{ "spindle": "main", "rpm": 12000, "clockwise": true }`
    pub const SET_SPINDLE: &str = "control.spindle";

    /// Client → Machine. Set laser power (laser).
    /// Params: `{ "laser": "main", "power": 0.8 }`
    pub const SET_LASER: &str = "control.laser";

    /// Client → Machine. Set speed override multiplier.
    /// Params: `{ "multiplier": 1.2 }` (1.0 = 100%)
    pub const SET_SPEED: &str = "control.speed";

    /// Client → Machine. Set flow override multiplier.
    /// Params: `{ "multiplier": 0.95 }`
    pub const SET_FLOW: &str = "control.flow";

    /// Client → Machine. Emergency stop. MUST be processed immediately.
    /// No params. Machine must halt all motion and heaters.
    pub const EMERGENCY_STOP: &str = "control.emergency_stop";

    /// Client → Machine. Reset after emergency stop or error.
    pub const RESET: &str = "control.reset";

    // === Status notifications (machine → client) ===

    /// Machine → Client. Periodic status update.
    /// Params: `MachineStatus`
    pub const STATUS_UPDATE: &str = "status.update";

    /// Machine → Client. Job state changed.
    /// Params: `{ "job_id": "...", "from": "running", "to": "paused", "reason": "..." }`
    pub const JOB_STATE_CHANGED: &str = "job.state_changed";

    /// Machine → Client. Error occurred.
    /// Params: `MachineError`
    pub const ERROR_OCCURRED: &str = "error.occurred";

    /// Machine → Client. Error cleared.
    /// Params: `{ "code": N }`
    pub const ERROR_CLEARED: &str = "error.cleared";
}

/// Authentication request sent in the `hello` handshake.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication (open/local network).
    None,
    /// Pre-shared key.
    ApiKey { key: alloc::string::String },
    /// OAuth2 bearer token.
    Bearer { token: alloc::string::String },
    /// mTLS — auth handled at transport layer, this is just a marker.
    Mtls,
}

/// Hello request — first message from client.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HelloRequest {
    pub protocol_version: alloc::string::String,
    pub client_name: alloc::string::String,
    pub client_version: alloc::string::String,
    pub auth: AuthMethod,
}

/// Hello response — machine responds with capabilities.
pub type HelloResponse = super::capability::MachineCapabilities;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrip() {
        let hello = HelloRequest {
            protocol_version: "0.1.0".into(),
            client_name: "OpenIE CAD".into(),
            client_version: "0.1.0".into(),
            auth: AuthMethod::ApiKey { key: "test-key".into() },
        };
        let json = serde_json::to_string(&hello).unwrap();
        let parsed: HelloRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.protocol_version, "0.1.0");
        match parsed.auth {
            AuthMethod::ApiKey { key } => assert_eq!(key, "test-key"),
            _ => panic!("expected ApiKey"),
        }
    }

    #[test]
    fn method_names_unique() {
        let methods = [
            names::HELLO, names::GOODBYE, names::CAPABILITIES, names::STATUS,
            names::STATUS_SUBSCRIBE, names::STATUS_UNSUBSCRIBE,
            names::JOB_SUBMIT, names::JOB_START, names::JOB_PAUSE,
            names::JOB_RESUME, names::JOB_CANCEL, names::JOB_STATUS, names::JOB_LIST,
            names::GCODE_SEND, names::GCODE_ACK, names::GCODE_ERROR, names::GCODE_BUFFER,
            names::HOME, names::JOG, names::SET_TEMPERATURE, names::SET_FAN,
            names::SET_SPINDLE, names::SET_LASER, names::SET_SPEED, names::SET_FLOW,
            names::EMERGENCY_STOP, names::RESET,
            names::STATUS_UPDATE, names::JOB_STATE_CHANGED,
            names::ERROR_OCCURRED, names::ERROR_CLEARED,
            names::JOB_UPLOAD_COMPLETE,
        ];
        let mut seen = alloc::collections::BTreeSet::new();
        for m in methods {
            assert!(seen.insert(m), "duplicate method name: {m}");
        }
    }
}
