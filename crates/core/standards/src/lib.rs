//! `physical-standards` — Automated verification against engineering standards.
//!
//! ISO 2768, ASME BPVC, ASME Y14.5 GD&T, ISO 286 fits, AWS D1.1 welding,
//! bolt torque, pressure vessel minimum wall thickness.

use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Compliance Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub standard: String,
    pub passed: bool,
    pub actual: f64,
    pub limit: f64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// ISO 2768 General Tolerances
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Iso2768Grade { Fine, Medium, Coarse, VeryCoarse }

/// ISO 2768 linear tolerance (mm) for a given nominal size and grade.
pub fn iso2768_tolerance(nominal_mm: f64, grade: Iso2768Grade) -> f64 {
    let tol = match grade {
        Iso2768Grade::Fine => {
            if nominal_mm <= 3.0 { 0.05 }
            else if nominal_mm <= 6.0 { 0.05 }
            else if nominal_mm <= 30.0 { 0.1 }
            else if nominal_mm <= 120.0 { 0.15 }
            else if nominal_mm <= 400.0 { 0.2 }
            else if nominal_mm <= 1000.0 { 0.3 }
            else { 0.5 }
        }
        Iso2768Grade::Medium => {
            if nominal_mm <= 3.0 { 0.1 }
            else if nominal_mm <= 6.0 { 0.1 }
            else if nominal_mm <= 30.0 { 0.2 }
            else if nominal_mm <= 120.0 { 0.3 }
            else if nominal_mm <= 400.0 { 0.5 }
            else if nominal_mm <= 1000.0 { 0.8 }
            else { 1.2 }
        }
        Iso2768Grade::Coarse => {
            if nominal_mm <= 3.0 { 0.2 }
            else if nominal_mm <= 6.0 { 0.3 }
            else if nominal_mm <= 30.0 { 0.5 }
            else if nominal_mm <= 120.0 { 0.8 }
            else if nominal_mm <= 400.0 { 1.2 }
            else if nominal_mm <= 1000.0 { 2.0 }
            else { 3.0 }
        }
        Iso2768Grade::VeryCoarse => {
            if nominal_mm <= 6.0 { 0.5 }
            else if nominal_mm <= 30.0 { 1.0 }
            else if nominal_mm <= 120.0 { 1.5 }
            else if nominal_mm <= 400.0 { 2.5 }
            else if nominal_mm <= 1000.0 { 4.0 }
            else { 6.0 }
        }
    };
    tol
}

pub fn check_general_tolerance(nominal_mm: f64, actual_mm: f64, grade: Iso2768Grade) -> ComplianceResult {
    let tol = iso2768_tolerance(nominal_mm, grade);
    let deviation = (actual_mm - nominal_mm).abs();
    ComplianceResult {
        standard: format!("ISO 2768 {:?}", grade),
        passed: deviation <= tol,
        actual: deviation,
        limit: tol,
        message: if deviation <= tol {
            format!("Deviation {:.3}mm within ±{:.3}mm tolerance", deviation, tol)
        } else {
            format!("FAIL: deviation {:.3}mm exceeds ±{:.3}mm tolerance", deviation, tol)
        },
    }
}

// ---------------------------------------------------------------------------
// Bolt Hole Clearances (ASME B18.2.8)
// ---------------------------------------------------------------------------

/// Standard bolt hole clearances: (bolt_size, close_fit_mm, normal_fit_mm, loose_fit_mm)
static BOLT_HOLES: &[(&str, f64, f64, f64)] = &[
    ("M3",  3.2,  3.4,  3.6),
    ("M4",  4.3,  4.5,  4.8),
    ("M5",  5.3,  5.5,  5.8),
    ("M6",  6.4,  6.6,  7.0),
    ("M8",  8.4,  9.0,  10.0),
    ("M10", 10.5, 11.0, 12.0),
    ("M12", 13.0, 13.5, 14.5),
    ("M16", 17.0, 17.5, 18.5),
    ("M20", 21.0, 22.0, 24.0),
    ("M24", 25.0, 26.0, 28.0),
];

pub fn check_bolt_hole(diameter_mm: f64, bolt_size: &str) -> ComplianceResult {
    let entry = BOLT_HOLES.iter().find(|(s, _, _, _)| s.eq_ignore_ascii_case(bolt_size));
    match entry {
        Some((_, close, normal, _loose)) => {
            let passed = diameter_mm >= *close && diameter_mm <= *_loose;
            ComplianceResult {
                standard: "ASME B18.2.8 bolt hole clearance".into(),
                passed,
                actual: diameter_mm,
                limit: *normal,
                message: if passed {
                    format!("Hole ⌀{:.1}mm acceptable for {} (close: {}, normal: {}, loose: {})",
                        diameter_mm, bolt_size, close, normal, _loose)
                } else {
                    format!("FAIL: ⌀{:.1}mm outside range [{}, {}] for {}",
                        diameter_mm, close, _loose, bolt_size)
                },
            }
        }
        None => ComplianceResult {
            standard: "ASME B18.2.8".into(),
            passed: false, actual: diameter_mm, limit: 0.0,
            message: format!("Unknown bolt size: {}", bolt_size),
        },
    }
}

// ---------------------------------------------------------------------------
// Bolt Torque (VDI 2230 / general practice)
// ---------------------------------------------------------------------------

/// Standard bolt torque values: (size, grade_8.8_Nm, grade_10.9_Nm, grade_12.9_Nm)
static BOLT_TORQUES: &[(&str, f64, f64, f64)] = &[
    ("M3",  1.2,  1.8,  2.1),
    ("M4",  2.9,  4.1,  4.9),
    ("M5",  5.7,  8.1,  9.7),
    ("M6",  9.9,  14.0, 16.5),
    ("M8",  24.0, 34.0, 40.0),
    ("M10", 47.0, 67.0, 79.0),
    ("M12", 82.0, 116.0, 137.0),
    ("M16", 200.0, 285.0, 335.0),
    ("M20", 390.0, 555.0, 655.0),
    ("M24", 680.0, 960.0, 1130.0),
];

/// Get recommended bolt torque in N·m.
pub fn bolt_torque(bolt_size: &str, grade: &str, lubricated: bool) -> f64 {
    let entry = BOLT_TORQUES.iter().find(|(s, _, _, _)| s.eq_ignore_ascii_case(bolt_size));
    let base = match entry {
        Some((_, g88, g109, g129)) => match grade {
            "8.8" => *g88,
            "10.9" => *g109,
            "12.9" => *g129,
            _ => *g88,
        },
        None => 10.0,
    };
    if lubricated { base * 0.8 } else { base }
}

// ---------------------------------------------------------------------------
// Weld Size (AWS D1.1)
// ---------------------------------------------------------------------------

pub fn check_weld_size(throat_mm: f64, plate_thickness_mm: f64) -> ComplianceResult {
    // AWS D1.1 minimum fillet weld size based on thicker plate
    let min_throat = if plate_thickness_mm <= 6.0 { 3.0 }
        else if plate_thickness_mm <= 13.0 { 5.0 }
        else if plate_thickness_mm <= 19.0 { 6.0 }
        else { 8.0 };
    ComplianceResult {
        standard: "AWS D1.1 minimum fillet weld".into(),
        passed: throat_mm >= min_throat,
        actual: throat_mm,
        limit: min_throat,
        message: if throat_mm >= min_throat {
            format!("Weld throat {:.1}mm meets minimum {:.1}mm for {:.1}mm plate", throat_mm, min_throat, plate_thickness_mm)
        } else {
            format!("FAIL: weld throat {:.1}mm below minimum {:.1}mm", throat_mm, min_throat)
        },
    }
}

// ---------------------------------------------------------------------------
// Pressure Vessel (ASME BPVC Section VIII, Division 1)
// ---------------------------------------------------------------------------

/// Minimum wall thickness for a cylindrical pressure vessel.
/// t = PR / (SE - 0.6P) where S = allowable stress, E = joint efficiency.
pub fn minimum_wall_pressure_vessel(
    pressure_mpa: f64,
    inner_diameter_mm: f64,
    allowable_stress_mpa: f64,
    joint_efficiency: f64,
) -> f64 {
    let r = inner_diameter_mm / 2.0;
    let se = allowable_stress_mpa * joint_efficiency;
    pressure_mpa * r / (se - 0.6 * pressure_mpa)
}

pub fn check_pressure_vessel(
    pressure_mpa: f64,
    inner_diameter_mm: f64,
    wall_mm: f64,
    allowable_stress_mpa: f64,
) -> ComplianceResult {
    let min_wall = minimum_wall_pressure_vessel(pressure_mpa, inner_diameter_mm, allowable_stress_mpa, 0.85);
    ComplianceResult {
        standard: "ASME BPVC Sec. VIII Div. 1".into(),
        passed: wall_mm >= min_wall,
        actual: wall_mm,
        limit: min_wall,
        message: if wall_mm >= min_wall {
            format!("Wall {:.2}mm meets minimum {:.2}mm at {:.1} MPa", wall_mm, min_wall, pressure_mpa)
        } else {
            format!("FAIL: wall {:.2}mm below minimum {:.2}mm at {:.1} MPa", wall_mm, min_wall, pressure_mpa)
        },
    }
}

// ---------------------------------------------------------------------------
// Fit Classification (ISO 286)
// ---------------------------------------------------------------------------

pub fn check_fit(shaft_mm: f64, hole_mm: f64, fit_class: &str) -> ComplianceResult {
    let clearance = hole_mm - shaft_mm;
    let (min_c, max_c, fit_type) = match fit_class {
        "H7/h6" => (0.0, 0.030, "Transition/clearance"),
        "H7/g6" => (0.005, 0.040, "Clearance (sliding)"),
        "H7/f7" => (0.020, 0.060, "Clearance (running)"),
        "H7/p6" => (-0.025, 0.0, "Interference (press)"),
        "H7/s6" => (-0.040, -0.010, "Interference (shrink)"),
        _ => (0.0, 0.050, "Unknown"),
    };
    let passed = clearance >= min_c && clearance <= max_c;
    ComplianceResult {
        standard: format!("ISO 286 {}", fit_class),
        passed,
        actual: clearance,
        limit: max_c,
        message: format!("{} fit: clearance {:.3}mm (range [{:.3}, {:.3}]mm) — {}",
            fit_type, clearance, min_c, max_c, if passed { "OK" } else { "FAIL" }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso2768_medium_50mm() {
        let tol = iso2768_tolerance(50.0, Iso2768Grade::Medium);
        assert!((tol - 0.3).abs() < 0.01, "50mm medium should be ±0.3mm, got {}", tol);
    }

    #[test]
    fn general_tolerance_pass() {
        let r = check_general_tolerance(50.0, 50.2, Iso2768Grade::Medium);
        assert!(r.passed, "0.2mm deviation should pass medium grade");
    }

    #[test]
    fn general_tolerance_fail() {
        let r = check_general_tolerance(50.0, 50.5, Iso2768Grade::Medium);
        assert!(!r.passed, "0.5mm deviation should fail medium grade (limit 0.3)");
    }

    #[test]
    fn bolt_hole_m8_normal() {
        let r = check_bolt_hole(9.0, "M8");
        assert!(r.passed, "9.0mm hole should be OK for M8");
    }

    #[test]
    fn bolt_hole_too_small() {
        let r = check_bolt_hole(7.0, "M8");
        assert!(!r.passed, "7.0mm hole too small for M8");
    }

    #[test]
    fn bolt_torque_m10_88() {
        let t = bolt_torque("M10", "8.8", false);
        assert!((t - 47.0).abs() < 0.1, "M10 grade 8.8 should be 47 Nm, got {}", t);
    }

    #[test]
    fn bolt_torque_lubricated_lower() {
        let dry = bolt_torque("M10", "8.8", false);
        let lub = bolt_torque("M10", "8.8", true);
        assert!(lub < dry, "lubricated torque should be lower");
    }

    #[test]
    fn weld_size_pass() {
        let r = check_weld_size(6.0, 10.0);
        assert!(r.passed, "6mm throat should pass for 10mm plate (min 5mm)");
    }

    #[test]
    fn weld_size_fail() {
        let r = check_weld_size(3.0, 15.0);
        assert!(!r.passed, "3mm throat too small for 15mm plate (min 6mm)");
    }

    #[test]
    fn pressure_vessel_wall() {
        // 1 MPa, 200mm ID, 138 MPa allowable → t ≈ 0.85mm
        let t = minimum_wall_pressure_vessel(1.0, 200.0, 138.0, 0.85);
        assert!(t > 0.5 && t < 2.0, "min wall = {:.2}mm", t);
    }

    #[test]
    fn pressure_vessel_check() {
        let r = check_pressure_vessel(1.0, 200.0, 5.0, 138.0);
        assert!(r.passed, "5mm wall should be plenty for 1 MPa");
    }

    #[test]
    fn fit_clearance() {
        let r = check_fit(20.000, 20.021, "H7/h6");
        assert!(r.passed, "0.021mm clearance should pass H7/h6");
    }

    #[test]
    fn fit_interference() {
        let r = check_fit(20.020, 20.000, "H7/p6");
        assert!(r.passed, "-0.020mm (interference) should pass H7/p6");
    }
}
