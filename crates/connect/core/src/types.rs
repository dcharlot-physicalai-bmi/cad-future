//! Shared types for machine connectivity.

use serde::{Deserialize, Serialize};

/// Persistent machine identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MachineId(pub String);

impl MachineId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for MachineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Category of manufacturing machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineKind {
    /// Fused Deposition Modeling (FDM/FFF) 3D printer.
    Fdm,
    /// Stereolithography (SLA/DLP/MSLA) resin printer.
    Sla,
    /// CNC milling machine.
    CncMill,
    /// CNC lathe/turning center.
    CncLathe,
    /// Laser cutter (CO2, fiber, diode).
    LaserCut,
    /// Laser marker/engraver.
    LaserMark,
}

/// File format a machine accepts for jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptedFormat {
    /// Standard ASCII G-code.
    Gcode,
    /// PrusaSlicer binary G-code.
    BinaryGcode,
    /// 3MF (3D Manufacturing Format) — ZIP+XML.
    ThreeMf,
    /// STL mesh for printers that slice on-device.
    Stl,
    /// UltiMaker UFP (ZIP + G-code + thumbnail).
    Ufp,
    /// Ruida .rd binary for CO2 laser controllers.
    RuidaRd,
    /// DXF 2D drawing for laser/waterjet.
    Dxf,
    /// SVG 2D vector.
    Svg,
    /// Lhymicro-GL binary for K40 lasers.
    LhymicroGl,
}

/// Communication protocol used to connect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    OctoPrint,
    Moonraker,
    BambuLan,
    BambuCloud,
    PrusaLink,
    Repetier,
    Duet,
    FluidNc,
    SerialMarlin,
    SerialGrbl,
    LinuxCnc,
    CncJs,
    HaasMdc,
    MtConnect,
    RuidaUdp,
    LightBurnBridge,
    K40Usb,
    UltiMaker,
    Formlabs,
    /// OpenIE Manufacturing Protocol (OMP).
    OpenIE,
}

/// Static information about a machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineInfo {
    pub id: MachineId,
    pub name: String,
    pub kind: MachineKind,
    pub protocol: Protocol,
    /// Network address (host:port) or serial port path.
    pub address: String,
    /// File formats the machine accepts.
    pub accepted_formats: Vec<AcceptedFormat>,
    /// Build volume in mm [x, y, z]. None if unknown.
    pub build_volume: Option<[f64; 3]>,
    /// Firmware version string, if known.
    pub firmware: Option<String>,
}

/// Machine discovered on the network (not yet registered).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredMachine {
    pub name: String,
    pub kind: MachineKind,
    pub protocol: Protocol,
    pub address: String,
    pub accepted_formats: Vec<AcceptedFormat>,
    pub build_volume: Option<[f64; 3]>,
    pub firmware: Option<String>,
}

/// Current state of a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineState {
    Offline,
    Idle,
    Busy,
    Paused,
    Error,
}

/// Temperature reading for a heater.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Temperature {
    /// Heater name (e.g., "extruder", "bed", "chamber").
    pub name: String,
    /// Current temperature in °C.
    pub actual: f64,
    /// Target temperature in °C.
    pub target: f64,
}

/// Machine position in mm.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MachinePosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Real-time machine status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineStatus {
    pub state: MachineState,
    pub temperatures: Vec<Temperature>,
    pub position: MachinePosition,
    /// Current job info, if any.
    pub active_job: Option<JobStatus>,
}

/// Handle to a submitted job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobHandle {
    /// Machine-side job identifier.
    pub job_id: String,
    /// Original filename.
    pub filename: String,
}

/// Job to submit to a machine.
pub struct JobSubmission {
    /// Display name for the job.
    pub name: String,
    /// File format of the payload.
    pub format: AcceptedFormat,
    /// Raw file bytes (G-code, 3MF, etc.).
    pub payload: Vec<u8>,
    /// Start printing immediately after upload.
    pub auto_start: bool,
}

/// State of a submitted job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Printing,
    Paused,
    Complete,
    Cancelled,
    Failed,
}

/// Status of a specific job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobStatus {
    pub state: JobState,
    /// Progress as a percentage (0.0 to 100.0).
    pub progress_pct: f64,
    /// Elapsed time in seconds.
    pub elapsed_s: f64,
    /// Estimated remaining time in seconds.
    pub remaining_s: Option<f64>,
    /// Current layer / total layers (for FDM).
    pub layers: Option<(u32, u32)>,
    /// Current filename.
    pub filename: String,
}

/// Authentication configuration for a machine connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication needed.
    None,
    /// API key (OctoPrint, Repetier).
    ApiKey { key: String },
    /// Username + password (HTTP Basic, LinuxCNC).
    UsernamePassword { username: String, password: String },
    /// Bearer token (Formlabs Cloud).
    BearerToken { token: String },
    /// Bambu LAN mode: user "bblp" + access code.
    BambuLan { access_code: String, serial: String },
    /// HTTP Digest authentication (PrusaLink, UltiMaker).
    DigestAuth { username: String, password: String },
}

/// Configuration needed to create a machine connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineConfig {
    pub name: String,
    pub kind: MachineKind,
    pub protocol: Protocol,
    pub address: String,
    pub auth: AuthConfig,
}
