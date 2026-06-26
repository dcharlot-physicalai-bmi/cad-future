//! Machine capability declaration.
//!
//! On connect, the machine sends its full capability set. The client adapts
//! its UI and behavior accordingly — no hardcoded assumptions per vendor.
//!
//! This replaces the fragile per-protocol knowledge of "what can this machine do?"
//! with a structured, extensible declaration.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Complete capability declaration sent by the machine on connect.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineCapabilities {
    /// Protocol version the machine speaks.
    pub protocol_version: String,
    /// Unique machine identifier (serial number or UUID).
    pub machine_id: String,
    /// Human-readable name.
    pub name: String,
    /// Manufacturer.
    pub manufacturer: String,
    /// Model name.
    pub model: String,
    /// Firmware version.
    pub firmware_version: String,
    /// Machine type and its type-specific capabilities.
    pub machine_type: MachineType,
    /// Build volume in mm.
    pub build_volume: BuildVolume,
    /// Accepted file formats for job submission.
    pub accepted_formats: Vec<FileFormat>,
    /// Available axes and their properties.
    pub axes: Vec<Axis>,
    /// Heaters (extruder, bed, chamber).
    pub heaters: Vec<HeaterCapability>,
    /// Spindle(s) for CNC.
    pub spindles: Vec<SpindleCapability>,
    /// Laser source(s).
    pub lasers: Vec<LaserCapability>,
    /// Tool changer / multi-material capabilities.
    pub tool_changer: Option<ToolChangerCapability>,
    /// Enclosure / chamber.
    pub enclosure: Option<EnclosureCapability>,
    /// Features this machine supports.
    pub features: Vec<Feature>,
    /// Maximum concurrent jobs in queue (1 for most machines).
    pub max_queue_depth: u32,
    /// G-code streaming buffer size in bytes (for flow control).
    pub stream_buffer_size: u32,
}

/// Machine type with type-specific capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MachineType {
    /// FDM/FFF 3D printer.
    Fdm {
        extruder_count: u32,
        heated_bed: bool,
        heated_chamber: bool,
        filament_sensor: bool,
        auto_bed_leveling: bool,
    },
    /// SLA/DLP/MSLA resin printer.
    Sla {
        technology: String,
        pixel_size_um: Option<f64>,
        uv_power_w: Option<f64>,
    },
    /// CNC mill.
    CncMill {
        axis_count: u32,
        atc_slots: Option<u32>,
        coolant: bool,
        probe: bool,
    },
    /// CNC lathe.
    CncLathe {
        live_tooling: bool,
        turret_positions: Option<u32>,
    },
    /// Laser cutter/engraver.
    Laser {
        source: LaserSource,
        power_w: f64,
        pulse_capable: bool,
        rotary_axis: bool,
    },
}

/// Laser source type.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaserSource {
    Co2,
    Fiber,
    Diode,
    GreenDpss,
    Uv,
}

/// Build volume dimensions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildVolume {
    pub x_mm: f64,
    pub y_mm: f64,
    pub z_mm: f64,
    /// Cylindrical build volume (for delta printers, lathes).
    pub is_cylindrical: bool,
}

/// File format a machine accepts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileFormat {
    /// MIME type (e.g., "application/x-gcode", "application/vnd.ms-package.3dmanufacturing-3dmodel+xml").
    pub mime_type: String,
    /// File extension (e.g., "gcode", "3mf", "bgcode").
    pub extension: String,
    /// Whether this is the preferred format.
    pub preferred: bool,
}

/// Axis capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Axis {
    /// Axis name (X, Y, Z, A, B, C, E).
    pub name: String,
    /// Minimum position in mm.
    pub min_mm: f64,
    /// Maximum position in mm.
    pub max_mm: f64,
    /// Maximum feed rate in mm/min.
    pub max_feed_mm_min: f64,
    /// Maximum acceleration in mm/s².
    pub max_accel_mm_s2: f64,
    /// Home position.
    pub home_mm: f64,
    /// Whether this axis can be homed.
    pub homeable: bool,
}

/// Heater capability (extruder, bed, chamber).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeaterCapability {
    /// Heater name (e.g., "extruder_0", "bed", "chamber").
    pub name: String,
    /// Heater category.
    pub kind: HeaterKind,
    /// Maximum temperature in °C.
    pub max_temp_c: f64,
    /// PID controlled.
    pub pid: bool,
}

/// Heater category.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaterKind {
    Extruder,
    Bed,
    Chamber,
    Enclosure,
}

/// Spindle capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpindleCapability {
    pub name: String,
    pub max_rpm: f64,
    pub min_rpm: f64,
    pub reversible: bool,
}

/// Laser source capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaserCapability {
    pub name: String,
    pub max_power_w: f64,
    pub pwm_capable: bool,
    pub frequency_range_hz: Option<(f64, f64)>,
}

/// Tool changer / multi-material capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolChangerCapability {
    /// Number of tool slots.
    pub slot_count: u32,
    /// Type of tool changing system.
    pub system: ToolChangerSystem,
}

/// Tool changer system type.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChangerSystem {
    /// Direct tool change (e.g., Prusa XL, Jubilee).
    Direct,
    /// AMS/MMU multi-material unit (Bambu AMS, Prusa MMU).
    MultiMaterial,
    /// CNC automatic tool changer (carousel/arm).
    Atc,
}

/// Enclosure/chamber capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnclosureCapability {
    /// Has active heating.
    pub heated: bool,
    /// Has filtration (HEPA, carbon).
    pub filtered: bool,
    /// Has camera.
    pub camera: bool,
    /// Has LED lighting.
    pub lighting: bool,
}

/// Optional machine features.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Can pause and resume jobs.
    PauseResume,
    /// Can report layer progress.
    LayerTracking,
    /// Can report filament usage.
    FilamentTracking,
    /// Can provide camera stream URL.
    CameraStream,
    /// Supports power loss recovery.
    PowerLossRecovery,
    /// Supports remote firmware update.
    FirmwareUpdate,
    /// Has filament runout detection.
    FilamentRunout,
    /// Has nozzle clog detection.
    ClogDetection,
    /// Can stream G-code in real-time (vs. upload-first).
    GcodeStreaming,
    /// Can accept raw G-code commands.
    RawGcode,
    /// Supports emergency stop.
    EmergencyStop,
    /// Can do relative jog moves.
    Jog,
    /// Can home axes.
    Home,
    /// Can probe (bed leveling, tool length).
    Probe,
}

impl MachineCapabilities {
    /// Check if the machine supports a specific feature.
    pub fn has_feature(&self, feature: &Feature) -> bool {
        self.features.contains(feature)
    }

    /// Check if the machine accepts a specific file format by extension.
    pub fn accepts_format(&self, extension: &str) -> bool {
        self.accepted_formats.iter().any(|f| f.extension == extension)
    }

    /// Get the preferred file format, or the first accepted.
    pub fn preferred_format(&self) -> Option<&FileFormat> {
        self.accepted_formats.iter().find(|f| f.preferred)
            .or(self.accepted_formats.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_fdm_printer() -> MachineCapabilities {
        MachineCapabilities {
            protocol_version: "0.1.0".into(),
            machine_id: "TEST-001".into(),
            name: "Test Printer".into(),
            manufacturer: "OpenIE".into(),
            model: "Dev Kit".into(),
            firmware_version: "1.0.0".into(),
            machine_type: MachineType::Fdm {
                extruder_count: 1,
                heated_bed: true,
                heated_chamber: false,
                filament_sensor: true,
                auto_bed_leveling: true,
            },
            build_volume: BuildVolume {
                x_mm: 256.0, y_mm: 256.0, z_mm: 256.0,
                is_cylindrical: false,
            },
            accepted_formats: vec![
                FileFormat { mime_type: "application/x-gcode".into(), extension: "gcode".into(), preferred: false },
                FileFormat { mime_type: "application/vnd.ms-package.3dmanufacturing-3dmodel+xml".into(), extension: "3mf".into(), preferred: true },
            ],
            axes: vec![
                Axis { name: "X".into(), min_mm: 0.0, max_mm: 256.0, max_feed_mm_min: 12000.0, max_accel_mm_s2: 5000.0, home_mm: 0.0, homeable: true },
                Axis { name: "Y".into(), min_mm: 0.0, max_mm: 256.0, max_feed_mm_min: 12000.0, max_accel_mm_s2: 5000.0, home_mm: 0.0, homeable: true },
                Axis { name: "Z".into(), min_mm: 0.0, max_mm: 256.0, max_feed_mm_min: 600.0, max_accel_mm_s2: 500.0, home_mm: 0.0, homeable: true },
            ],
            heaters: vec![
                HeaterCapability { name: "extruder_0".into(), kind: HeaterKind::Extruder, max_temp_c: 300.0, pid: true },
                HeaterCapability { name: "bed".into(), kind: HeaterKind::Bed, max_temp_c: 110.0, pid: true },
            ],
            spindles: Vec::new(),
            lasers: Vec::new(),
            tool_changer: None,
            enclosure: Some(EnclosureCapability {
                heated: false, filtered: true, camera: true, lighting: true,
            }),
            features: vec![
                Feature::PauseResume, Feature::LayerTracking, Feature::FilamentTracking,
                Feature::CameraStream, Feature::GcodeStreaming, Feature::RawGcode,
                Feature::EmergencyStop, Feature::Jog, Feature::Home,
                Feature::FilamentRunout, Feature::PowerLossRecovery,
            ],
            max_queue_depth: 1,
            stream_buffer_size: 4096,
        }
    }

    #[test]
    fn capabilities_roundtrip() {
        let caps = test_fdm_printer();
        let json = serde_json::to_string_pretty(&caps).unwrap();
        let parsed: MachineCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.machine_id, "TEST-001");
        assert_eq!(parsed.axes.len(), 3);
    }

    #[test]
    fn feature_check() {
        let caps = test_fdm_printer();
        assert!(caps.has_feature(&Feature::PauseResume));
        assert!(caps.has_feature(&Feature::EmergencyStop));
        assert!(!caps.has_feature(&Feature::Probe));
    }

    #[test]
    fn format_check() {
        let caps = test_fdm_printer();
        assert!(caps.accepts_format("gcode"));
        assert!(caps.accepts_format("3mf"));
        assert!(!caps.accepts_format("stl"));
    }

    #[test]
    fn preferred_format() {
        let caps = test_fdm_printer();
        let pref = caps.preferred_format().unwrap();
        assert_eq!(pref.extension, "3mf");
        assert!(pref.preferred);
    }

    #[test]
    fn machine_type_fdm() {
        let caps = test_fdm_printer();
        match &caps.machine_type {
            MachineType::Fdm { heated_bed, auto_bed_leveling, .. } => {
                assert!(heated_bed);
                assert!(auto_bed_leveling);
            }
            _ => panic!("expected FDM"),
        }
    }
}
