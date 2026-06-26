//! Thread features — cosmetic thread representation and callout generation.
//!
//! Threads are represented as annotations on cylindrical faces, not as
//! modeled helical geometry (matching Onshape/Fusion 360/SolidWorks approach).
//! Thread data comes from the LUT standards tables (ISO 262, ASME B1.1).

use glam::DVec3;
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Thread Standard
// ---------------------------------------------------------------------------

/// Thread standard family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadStandard {
    /// ISO 262 metric coarse.
    MetricCoarse,
    /// ISO 262 metric fine.
    MetricFine,
    /// ASME B1.1 Unified National Coarse.
    UNC,
    /// ASME B1.1 Unified National Fine.
    UNF,
    /// NPT (pipe).
    NPT,
    /// BSP (British Standard Pipe).
    BSP,
}

/// Thread type: internal (tapped hole) or external (bolt/stud).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadType {
    Internal,
    External,
}

/// Thread fit class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadClass {
    /// ISO 6H/6g (medium fit, most common).
    Medium,
    /// ISO 5H/4h (close fit).
    Close,
    /// ISO 7H/8g (loose fit).
    Loose,
}

// ---------------------------------------------------------------------------
// Thread Specification
// ---------------------------------------------------------------------------

/// Complete thread specification for a feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadSpec {
    /// Standard family.
    pub standard: ThreadStandard,
    /// Designation string, e.g., "M8", "M10x1.25", "1/4-20".
    pub designation: String,
    /// Nominal diameter (mm).
    pub nominal_diameter_mm: f64,
    /// Pitch (mm). For imperial: 25.4 / TPI.
    pub pitch_mm: f64,
    /// Internal or external.
    pub thread_type: ThreadType,
    /// Fit class.
    pub class: ThreadClass,
    /// Thread depth (mm). For blind holes, this is the threaded portion.
    pub depth_mm: f64,
    /// Minor diameter (mm) — for internal threads, this is the drill diameter.
    pub minor_diameter_mm: f64,
    /// Pitch diameter (mm).
    pub pitch_diameter_mm: f64,
    /// Tensile stress area (mm^2) — for strength calculations.
    pub tensile_stress_area_mm2: f64,
}

impl ThreadSpec {
    /// Create a metric coarse thread from designation (e.g., "M8").
    pub fn metric_coarse(designation: &str, depth_mm: f64, thread_type: ThreadType) -> Option<Self> {
        let entry = lookup_metric_coarse(designation)?;
        Some(Self {
            standard: ThreadStandard::MetricCoarse,
            designation: designation.to_string(),
            nominal_diameter_mm: entry.nominal_diameter_mm,
            pitch_mm: entry.pitch_mm,
            thread_type,
            class: ThreadClass::Medium,
            depth_mm,
            minor_diameter_mm: entry.minor_diameter_mm,
            pitch_diameter_mm: entry.pitch_diameter_mm,
            tensile_stress_area_mm2: entry.tensile_stress_area_mm2,
        })
    }

    /// Create a metric fine thread from designation (e.g., "M10x1.25").
    pub fn metric_fine(designation: &str, depth_mm: f64, thread_type: ThreadType) -> Option<Self> {
        let entry = lookup_metric_fine(designation)?;
        Some(Self {
            standard: ThreadStandard::MetricFine,
            designation: designation.to_string(),
            nominal_diameter_mm: entry.nominal_diameter_mm,
            pitch_mm: entry.pitch_mm,
            thread_type,
            class: ThreadClass::Medium,
            depth_mm,
            minor_diameter_mm: entry.minor_diameter_mm,
            pitch_diameter_mm: entry.pitch_diameter_mm,
            tensile_stress_area_mm2: entry.tensile_stress_area_mm2,
        })
    }

    /// Generate the full hole callout string.
    /// Example: "M8x1.25 6H, depth 15mm" or "1/4-20 UNC 2B, thru"
    pub fn callout(&self) -> String {
        let class_str = match self.class {
            ThreadClass::Medium => match self.thread_type {
                ThreadType::Internal => "6H",
                ThreadType::External => "6g",
            },
            ThreadClass::Close => match self.thread_type {
                ThreadType::Internal => "5H",
                ThreadType::External => "4h",
            },
            ThreadClass::Loose => match self.thread_type {
                ThreadType::Internal => "7H",
                ThreadType::External => "8g",
            },
        };

        let depth_str = if self.depth_mm > self.nominal_diameter_mm * 10.0 {
            "thru".to_string()
        } else {
            format!("depth {:.1}mm", self.depth_mm)
        };

        match self.standard {
            ThreadStandard::MetricCoarse | ThreadStandard::MetricFine => {
                format!("{} {} {}, {}", self.designation, class_str,
                    if self.thread_type == ThreadType::Internal { "int." } else { "ext." },
                    depth_str)
            }
            ThreadStandard::UNC => {
                let unc_class = match self.thread_type {
                    ThreadType::Internal => "2B",
                    ThreadType::External => "2A",
                };
                format!("{} UNC {}, {}", self.designation, unc_class, depth_str)
            }
            ThreadStandard::UNF => {
                let unf_class = match self.thread_type {
                    ThreadType::Internal => "2B",
                    ThreadType::External => "2A",
                };
                format!("{} UNF {}, {}", self.designation, unf_class, depth_str)
            }
            ThreadStandard::NPT | ThreadStandard::BSP => {
                format!("{} {}", self.designation, depth_str)
            }
        }
    }

    /// Tap drill diameter for internal threads (mm).
    /// For metric: tap drill ≈ nominal - pitch.
    pub fn tap_drill_diameter_mm(&self) -> f64 {
        match self.thread_type {
            ThreadType::Internal => self.minor_diameter_mm,
            ThreadType::External => self.nominal_diameter_mm,
        }
    }

    /// Minimum engagement length for full thread strength (mm).
    /// Rule of thumb: 1.5 × diameter for steel-in-steel, 2.0 × for aluminum.
    pub fn min_engagement_steel(&self) -> f64 {
        1.5 * self.nominal_diameter_mm
    }

    pub fn min_engagement_aluminum(&self) -> f64 {
        2.0 * self.nominal_diameter_mm
    }

    /// Thread stripping strength (approximate, kN).
    /// Based on Le (engagement length) and tensile stress area.
    pub fn stripping_strength_kn(&self, engagement_mm: f64, shear_strength_mpa: f64) -> f64 {
        // Shear area ≈ pi × pitch_diameter × Le × 0.5 (thread engagement factor)
        let shear_area = std::f64::consts::PI * self.pitch_diameter_mm * engagement_mm * 0.5;
        shear_area * shear_strength_mpa / 1000.0
    }
}

// ---------------------------------------------------------------------------
// Cosmetic Thread Feature
// ---------------------------------------------------------------------------

/// A cosmetic thread feature applied to a cylindrical face.
/// Does not modify B-Rep geometry — rendered as an annotation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CosmeticThread {
    /// Thread specification.
    pub spec: ThreadSpec,
    /// Axis origin (center of the hole/shaft at the start of threading).
    pub origin: DVec3,
    /// Axis direction (along the thread axis).
    pub axis: DVec3,
}

impl CosmeticThread {
    pub fn new(spec: ThreadSpec, origin: DVec3, axis: DVec3) -> Self {
        Self { spec, origin, axis: axis.normalize() }
    }

    /// Get the thread end point.
    pub fn end_point(&self) -> DVec3 {
        self.origin + self.axis * self.spec.depth_mm
    }

    /// Number of thread turns.
    pub fn turns(&self) -> f64 {
        self.spec.depth_mm / self.spec.pitch_mm
    }
}

// ---------------------------------------------------------------------------
// Thread LUT — inline for no_std-friendly brep crate
// ---------------------------------------------------------------------------

/// Thread data entry (matches physical-lut standards format).
#[derive(Clone, Copy, Debug)]
pub struct ThreadData {
    pub designation: &'static str,
    pub nominal_diameter_mm: f64,
    pub pitch_mm: f64,
    pub minor_diameter_mm: f64,
    pub pitch_diameter_mm: f64,
    pub tensile_stress_area_mm2: f64,
}

/// Look up a metric coarse thread by designation.
pub fn lookup_metric_coarse(designation: &str) -> Option<ThreadData> {
    METRIC_COARSE_THREADS.iter()
        .find(|t| t.designation.eq_ignore_ascii_case(designation))
        .copied()
}

/// Look up a metric fine thread by designation.
pub fn lookup_metric_fine(designation: &str) -> Option<ThreadData> {
    METRIC_FINE_THREADS.iter()
        .find(|t| t.designation.eq_ignore_ascii_case(designation))
        .copied()
}

/// Suggest the best metric thread for a given hole diameter.
/// Returns the largest thread that fits (nominal_diameter <= hole_diameter).
pub fn suggest_thread_for_hole(hole_diameter_mm: f64) -> Option<ThreadData> {
    METRIC_COARSE_THREADS.iter()
        .filter(|t| t.nominal_diameter_mm <= hole_diameter_mm)
        .last()
        .copied()
}

/// Common metric coarse threads (ISO 262).
/// Source: ISO 262:1998, ISO 724:1993.
static METRIC_COARSE_THREADS: &[ThreadData] = &[
    ThreadData { designation: "M1",   nominal_diameter_mm:  1.0,  pitch_mm: 0.25,  minor_diameter_mm: 0.729,  pitch_diameter_mm: 0.838,  tensile_stress_area_mm2: 0.460 },
    ThreadData { designation: "M1.6", nominal_diameter_mm:  1.6,  pitch_mm: 0.35,  minor_diameter_mm: 1.221,  pitch_diameter_mm: 1.373,  tensile_stress_area_mm2: 1.27  },
    ThreadData { designation: "M2",   nominal_diameter_mm:  2.0,  pitch_mm: 0.4,   minor_diameter_mm: 1.567,  pitch_diameter_mm: 1.740,  tensile_stress_area_mm2: 2.07  },
    ThreadData { designation: "M2.5", nominal_diameter_mm:  2.5,  pitch_mm: 0.45,  minor_diameter_mm: 2.013,  pitch_diameter_mm: 2.208,  tensile_stress_area_mm2: 3.39  },
    ThreadData { designation: "M3",   nominal_diameter_mm:  3.0,  pitch_mm: 0.5,   minor_diameter_mm: 2.459,  pitch_diameter_mm: 2.675,  tensile_stress_area_mm2: 5.03  },
    ThreadData { designation: "M4",   nominal_diameter_mm:  4.0,  pitch_mm: 0.7,   minor_diameter_mm: 3.242,  pitch_diameter_mm: 3.545,  tensile_stress_area_mm2: 8.78  },
    ThreadData { designation: "M5",   nominal_diameter_mm:  5.0,  pitch_mm: 0.8,   minor_diameter_mm: 4.134,  pitch_diameter_mm: 4.480,  tensile_stress_area_mm2: 14.2  },
    ThreadData { designation: "M6",   nominal_diameter_mm:  6.0,  pitch_mm: 1.0,   minor_diameter_mm: 4.917,  pitch_diameter_mm: 5.350,  tensile_stress_area_mm2: 20.1  },
    ThreadData { designation: "M8",   nominal_diameter_mm:  8.0,  pitch_mm: 1.25,  minor_diameter_mm: 6.647,  pitch_diameter_mm: 7.188,  tensile_stress_area_mm2: 36.6  },
    ThreadData { designation: "M10",  nominal_diameter_mm: 10.0,  pitch_mm: 1.5,   minor_diameter_mm: 8.376,  pitch_diameter_mm: 9.026,  tensile_stress_area_mm2: 58.0  },
    ThreadData { designation: "M12",  nominal_diameter_mm: 12.0,  pitch_mm: 1.75,  minor_diameter_mm: 10.106, pitch_diameter_mm: 10.863, tensile_stress_area_mm2: 84.3  },
    ThreadData { designation: "M14",  nominal_diameter_mm: 14.0,  pitch_mm: 2.0,   minor_diameter_mm: 11.835, pitch_diameter_mm: 12.701, tensile_stress_area_mm2: 115.0 },
    ThreadData { designation: "M16",  nominal_diameter_mm: 16.0,  pitch_mm: 2.0,   minor_diameter_mm: 13.835, pitch_diameter_mm: 14.701, tensile_stress_area_mm2: 157.0 },
    ThreadData { designation: "M20",  nominal_diameter_mm: 20.0,  pitch_mm: 2.5,   minor_diameter_mm: 17.294, pitch_diameter_mm: 18.376, tensile_stress_area_mm2: 245.0 },
    ThreadData { designation: "M24",  nominal_diameter_mm: 24.0,  pitch_mm: 3.0,   minor_diameter_mm: 20.752, pitch_diameter_mm: 22.051, tensile_stress_area_mm2: 353.0 },
    ThreadData { designation: "M30",  nominal_diameter_mm: 30.0,  pitch_mm: 3.5,   minor_diameter_mm: 26.211, pitch_diameter_mm: 27.727, tensile_stress_area_mm2: 561.0 },
    ThreadData { designation: "M36",  nominal_diameter_mm: 36.0,  pitch_mm: 4.0,   minor_diameter_mm: 31.670, pitch_diameter_mm: 33.402, tensile_stress_area_mm2: 817.0 },
    ThreadData { designation: "M42",  nominal_diameter_mm: 42.0,  pitch_mm: 4.5,   minor_diameter_mm: 37.129, pitch_diameter_mm: 39.077, tensile_stress_area_mm2: 1120.0 },
    ThreadData { designation: "M48",  nominal_diameter_mm: 48.0,  pitch_mm: 5.0,   minor_diameter_mm: 42.587, pitch_diameter_mm: 44.752, tensile_stress_area_mm2: 1470.0 },
];

/// Common metric fine threads (ISO 262).
static METRIC_FINE_THREADS: &[ThreadData] = &[
    ThreadData { designation: "M8x1",     nominal_diameter_mm:  8.0,  pitch_mm: 1.0,  minor_diameter_mm: 6.917,  pitch_diameter_mm: 7.350,  tensile_stress_area_mm2: 39.2  },
    ThreadData { designation: "M10x1",    nominal_diameter_mm: 10.0,  pitch_mm: 1.0,  minor_diameter_mm: 8.917,  pitch_diameter_mm: 9.350,  tensile_stress_area_mm2: 64.5  },
    ThreadData { designation: "M10x1.25", nominal_diameter_mm: 10.0,  pitch_mm: 1.25, minor_diameter_mm: 8.647,  pitch_diameter_mm: 9.188,  tensile_stress_area_mm2: 61.2  },
    ThreadData { designation: "M12x1.25", nominal_diameter_mm: 12.0,  pitch_mm: 1.25, minor_diameter_mm: 10.647, pitch_diameter_mm: 11.188, tensile_stress_area_mm2: 92.1  },
    ThreadData { designation: "M12x1.5",  nominal_diameter_mm: 12.0,  pitch_mm: 1.5,  minor_diameter_mm: 10.376, pitch_diameter_mm: 11.026, tensile_stress_area_mm2: 88.1  },
    ThreadData { designation: "M16x1.5",  nominal_diameter_mm: 16.0,  pitch_mm: 1.5,  minor_diameter_mm: 14.376, pitch_diameter_mm: 15.026, tensile_stress_area_mm2: 167.0 },
    ThreadData { designation: "M20x1.5",  nominal_diameter_mm: 20.0,  pitch_mm: 1.5,  minor_diameter_mm: 18.376, pitch_diameter_mm: 19.026, tensile_stress_area_mm2: 272.0 },
    ThreadData { designation: "M20x2",    nominal_diameter_mm: 20.0,  pitch_mm: 2.0,  minor_diameter_mm: 17.835, pitch_diameter_mm: 18.701, tensile_stress_area_mm2: 258.0 },
    ThreadData { designation: "M24x2",    nominal_diameter_mm: 24.0,  pitch_mm: 2.0,  minor_diameter_mm: 21.835, pitch_diameter_mm: 22.701, tensile_stress_area_mm2: 384.0 },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_m8_coarse() {
        let t = lookup_metric_coarse("M8").unwrap();
        assert!((t.nominal_diameter_mm - 8.0).abs() < 0.01);
        assert!((t.pitch_mm - 1.25).abs() < 0.01);
        assert!((t.minor_diameter_mm - 6.647).abs() < 0.01);
    }

    #[test]
    fn lookup_m10_fine() {
        let t = lookup_metric_fine("M10x1.25").unwrap();
        assert!((t.nominal_diameter_mm - 10.0).abs() < 0.01);
        assert!((t.pitch_mm - 1.25).abs() < 0.01);
    }

    #[test]
    fn lookup_nonexistent() {
        assert!(lookup_metric_coarse("M999").is_none());
    }

    #[test]
    fn thread_spec_callout() {
        let spec = ThreadSpec::metric_coarse("M8", 15.0, ThreadType::Internal).unwrap();
        let callout = spec.callout();
        assert!(callout.contains("M8"));
        assert!(callout.contains("6H"));
        assert!(callout.contains("15.0mm"));
    }

    #[test]
    fn tap_drill_diameter() {
        let spec = ThreadSpec::metric_coarse("M8", 15.0, ThreadType::Internal).unwrap();
        assert!((spec.tap_drill_diameter_mm() - 6.647).abs() < 0.01);
    }

    #[test]
    fn thread_engagement() {
        let spec = ThreadSpec::metric_coarse("M10", 20.0, ThreadType::External).unwrap();
        assert!((spec.min_engagement_steel() - 15.0).abs() < 0.01);
        assert!((spec.min_engagement_aluminum() - 20.0).abs() < 0.01);
    }

    #[test]
    fn suggest_thread() {
        // 8.5mm hole → should suggest M8
        let t = suggest_thread_for_hole(8.5).unwrap();
        assert_eq!(t.designation, "M8");
    }

    #[test]
    fn cosmetic_thread_turns() {
        let spec = ThreadSpec::metric_coarse("M8", 12.0, ThreadType::Internal).unwrap();
        let ct = CosmeticThread::new(spec, DVec3::ZERO, DVec3::Z);
        let turns = ct.turns();
        // 12mm / 1.25mm pitch ≈ 9.6 turns
        assert!((turns - 9.6).abs() < 0.1, "turns={}", turns);
    }

    #[test]
    fn stripping_strength() {
        let spec = ThreadSpec::metric_coarse("M8", 15.0, ThreadType::Internal).unwrap();
        // Mild steel shear strength ≈ 300 MPa
        let kn = spec.stripping_strength_kn(12.0, 300.0);
        assert!(kn > 20.0, "stripping={} kN", kn); // should be substantial
    }

    #[test]
    fn thread_spec_fine() {
        let spec = ThreadSpec::metric_fine("M10x1.25", 20.0, ThreadType::External).unwrap();
        assert!(spec.callout().contains("M10x1.25"));
        assert!(spec.callout().contains("6g"));
    }
}
