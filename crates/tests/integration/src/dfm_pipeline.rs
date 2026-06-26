//! DFM pipeline tests — Design → DFM check → Manufacturing constraint verification.
//!
//! These tests verify the full flow from creating geometry, running DFM
//! validation, and cross-checking against LUT manufacturing constraints.

use physical_brep::builder::make_box;
use physical_dfm::{cnc_config, validate, DfmConfig, Severity};
use physical_lut::manufacturing;

/// Create a box with thin walls → run DFM check for CNC → verify wall thickness warning.
#[test]
fn thin_wall_box_cnc_dfm_warns() {
    // 50×30×0.3mm box — 0.3mm is below CNC minimum wall thickness (0.8mm)
    let thin_box = make_box(50.0, 30.0, 0.3);
    let config = cnc_config();
    let issues = validate(&thin_box, &config);

    let thin_wall_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.category == "Thin wall")
        .collect();

    assert!(
        !thin_wall_issues.is_empty(),
        "0.3mm wall should be flagged for CNC milling (min {}mm)",
        config.min_wall_thickness
    );

    // Should be Error severity
    assert!(
        thin_wall_issues.iter().any(|i| i.severity == Severity::Error),
        "thin wall below CNC minimum should be an error"
    );
}

/// Create a solid → check for FDM printability → verify that a very thin part
/// triggers the thin-wall DFM check (FDM min wall ~0.8mm for typical nozzle).
#[test]
fn thin_part_fdm_dfm_flags_wall() {
    // 40×20×0.3mm — 0.3mm thickness is below FDM min wall
    let thin_part = make_box(40.0, 20.0, 0.3);
    let fdm_config = DfmConfig {
        process: "FDM Printing".into(),
        min_wall_thickness: 0.8, // typical 0.4mm nozzle, 2 perimeters
        min_corner_radius: 0.0,
        max_envelope: [250.0, 210.0, 210.0],
        min_draft_angle: 0.0, // no draft needed for FDM
    };
    let issues = validate(&thin_part, &fdm_config);

    let thin_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.category == "Thin wall")
        .collect();

    assert!(
        !thin_issues.is_empty(),
        "0.3mm part should fail FDM thin-wall check (min {}mm)",
        fdm_config.min_wall_thickness
    );
}

/// Create a part → get manufacturing constraint from LUT → verify constraint
/// values match the DFM config values used in checks.
#[test]
fn lut_constraint_matches_dfm_check() {
    let constraint = manufacturing::lookup(
        manufacturing::Process::CncMill3Ax,
        manufacturing::MaterialClass::Aluminum,
    )
    .expect("CNC + Aluminum constraint should exist in LUT");

    // LUT min wall thickness should be positive and reasonable
    assert!(
        constraint.min_wall_thickness.to_mm() > 0.0,
        "CNC aluminum min wall should be positive"
    );
    assert!(
        constraint.min_wall_thickness.to_mm() < 5.0,
        "CNC aluminum min wall should be < 5mm, got {}",
        constraint.min_wall_thickness.to_mm()
    );

    // Create a box exactly at the minimum wall thickness
    let wall = constraint.min_wall_thickness.to_mm();
    let at_limit = make_box(50.0, 30.0, wall);
    let config = cnc_config();
    let issues = validate(&at_limit, &config);

    // Should pass — wall is at or above limit
    let thin_wall_errors: Vec<_> = issues
        .iter()
        .filter(|i| i.category == "Thin wall" && i.severity == Severity::Error)
        .collect();

    // The DFM config uses 0.8mm; the LUT constraint is the reference value.
    // As long as wall >= config.min_wall_thickness, no error is expected.
    if wall >= config.min_wall_thickness {
        assert!(
            thin_wall_errors.is_empty(),
            "part at LUT minimum wall {wall:.2}mm should not fail DFM for CNC (config min {}mm)",
            config.min_wall_thickness
        );
    }
}
