//! Typed machine status — replaces string parsing across all protocols.
//!
//! Every legacy protocol reports the same information in a different encoding.
//! OMP provides a single, typed schema that machines populate directly.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Complete machine status snapshot.
///
/// Pushed by the machine at 1-2 Hz via `status.update` notifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineStatus {
    /// Current machine state.
    pub state: MachineState,
    /// All heater readings.
    pub heaters: Vec<HeaterStatus>,
    /// All axis positions.
    pub position: Position,
    /// Active job status, if any.
    pub job: Option<JobProgress>,
    /// Spindle status (CNC).
    pub spindle: Option<SpindleStatus>,
    /// Laser status.
    pub laser: Option<LaserStatus>,
    /// Fan speeds (0.0 - 1.0).
    pub fans: Vec<FanStatus>,
    /// Error details if state is Error or Emergency.
    pub error: Option<MachineError>,
    /// Uptime in seconds since last boot.
    pub uptime_s: f64,
    /// Free storage in bytes (SD card / internal).
    pub free_storage_bytes: Option<u64>,
}

/// Machine state — deterministic, no string matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineState {
    /// Machine is starting up.
    Booting,
    /// Ready to accept jobs.
    Idle,
    /// Executing a job.
    Running,
    /// Job paused by user or machine.
    Paused,
    /// Machine is homing.
    Homing,
    /// Machine is probing (bed level, tool length).
    Probing,
    /// Tool change in progress.
    ToolChanging,
    /// Heating up before job start.
    Heating,
    /// Cooling down after job.
    Cooling,
    /// Recoverable error — machine halted, can be reset.
    Error,
    /// Emergency stop — requires physical intervention.
    Emergency,
    /// Firmware update in progress.
    Updating,
}

/// Heater reading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeaterStatus {
    /// Heater name (matches capability declaration).
    pub name: String,
    /// Current temperature in °C.
    pub actual_c: f64,
    /// Target temperature in °C (0 = off).
    pub target_c: f64,
    /// PID power output (0.0 - 1.0).
    pub power: f64,
}

/// Axis positions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Position {
    /// Machine position (absolute, in machine coordinates).
    pub machine: AxisValues,
    /// Work position (after work coordinate offset).
    pub work: AxisValues,
}

/// Axis values.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AxisValues {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Rotary / 4th axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<f64>,
    /// 5th axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<f64>,
    /// 6th axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<f64>,
    /// Extruder position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub e: Option<f64>,
}

/// Job progress within a status update.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobProgress {
    /// Job ID.
    pub job_id: String,
    /// Filename being processed.
    pub filename: String,
    /// Progress 0.0 - 100.0.
    pub progress_pct: f64,
    /// Elapsed time in seconds.
    pub elapsed_s: f64,
    /// Estimated remaining time in seconds.
    pub remaining_s: Option<f64>,
    /// Current layer / total layers (FDM).
    pub layer: Option<LayerInfo>,
    /// Bytes processed / total bytes (for streaming).
    pub bytes: Option<ByteProgress>,
    /// Current feed rate in mm/min.
    pub feed_rate_mm_min: f64,
    /// Current flow rate multiplier (1.0 = 100%).
    pub flow_multiplier: f64,
    /// Current speed multiplier (1.0 = 100%).
    pub speed_multiplier: f64,
}

/// Layer progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerInfo {
    pub current: u32,
    pub total: u32,
    /// Current Z height in mm.
    pub z_mm: f64,
}

/// Byte progress for streaming.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByteProgress {
    pub processed: u64,
    pub total: u64,
}

/// Spindle status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpindleStatus {
    pub name: String,
    /// Current RPM (0 = stopped).
    pub rpm: f64,
    /// Target RPM.
    pub target_rpm: f64,
    /// Spindle load as fraction (0.0 - 1.0).
    pub load: f64,
    /// Clockwise rotation.
    pub clockwise: bool,
}

/// Laser status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaserStatus {
    pub name: String,
    /// Current power (0.0 - 1.0).
    pub power: f64,
    /// Whether the laser is actively firing.
    pub firing: bool,
    /// Pulse frequency in Hz, if pulsed mode.
    pub frequency_hz: Option<f64>,
}

/// Fan reading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanStatus {
    /// Fan name (e.g., "part_cooling", "hotend", "aux").
    pub name: String,
    /// Speed as fraction (0.0 - 1.0).
    pub speed: f64,
}

/// Structured machine error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineError {
    /// Error code (machine-specific).
    pub code: u32,
    /// Error category.
    pub category: ErrorCategory,
    /// Human-readable message.
    pub message: String,
    /// Whether the error is recoverable (can be cleared by software).
    pub recoverable: bool,
}

/// Error category — structured, not string-matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Thermal runaway or heater failure.
    Thermal,
    /// Endstop/limit switch triggered unexpectedly.
    Endstop,
    /// Motor stall or driver error.
    Motor,
    /// Filament runout or jam.
    Filament,
    /// Communication error (serial, CAN bus).
    Communication,
    /// Firmware error.
    Firmware,
    /// Power supply issue.
    Power,
    /// Sensor failure (probe, accelerometer).
    Sensor,
    /// User-triggered emergency stop.
    EmergencyStop,
    /// Other / unknown.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        let status = MachineStatus {
            state: MachineState::Running,
            heaters: vec![
                HeaterStatus { name: "extruder_0".into(), actual_c: 210.0, target_c: 215.0, power: 0.45 },
                HeaterStatus { name: "bed".into(), actual_c: 59.8, target_c: 60.0, power: 0.1 },
            ],
            position: Position {
                machine: AxisValues { x: 120.5, y: 80.3, z: 5.4, e: Some(234.5), ..Default::default() },
                work: AxisValues { x: 120.5, y: 80.3, z: 5.4, e: Some(234.5), ..Default::default() },
            },
            job: Some(JobProgress {
                job_id: "job-001".into(),
                filename: "benchy.gcode".into(),
                progress_pct: 42.5,
                elapsed_s: 1800.0,
                remaining_s: Some(2400.0),
                layer: Some(LayerInfo { current: 85, total: 200, z_mm: 5.4 }),
                bytes: None,
                feed_rate_mm_min: 3600.0,
                flow_multiplier: 1.0,
                speed_multiplier: 1.0,
            }),
            spindle: None,
            laser: None,
            fans: vec![
                FanStatus { name: "part_cooling".into(), speed: 1.0 },
                FanStatus { name: "hotend".into(), speed: 0.5 },
            ],
            error: None,
            uptime_s: 7200.0,
            free_storage_bytes: Some(1024 * 1024 * 512),
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: MachineStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.state, MachineState::Running);
        assert_eq!(parsed.heaters.len(), 2);
        assert!(parsed.job.is_some());
        assert!((parsed.job.unwrap().progress_pct - 42.5).abs() < 1e-8);
    }

    #[test]
    fn error_structured() {
        let err = MachineError {
            code: 1001,
            category: ErrorCategory::Thermal,
            message: "Extruder thermal runaway detected".into(),
            recoverable: false,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("thermal"));
        assert!(json.contains("1001"));
    }
}
