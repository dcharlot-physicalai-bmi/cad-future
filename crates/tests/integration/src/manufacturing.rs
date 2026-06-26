//! Manufacturing pipeline tests — design a part, run DFM validation,
//! verify material lookups integrate correctly, and check that
//! the cascade (LUT → formula → solver) produces consistent results.

use glam::DVec3;
use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::Profile;
use physical_analytical::{
    mass_properties, beam_approximation,
    deflection_simply_supported, bending_stress_simply_supported,
    safety_factor, von_mises,
};
use physical_dfm::{validate, injection_mold_config, cnc_config, Severity};
use physical_lut::materials;

/// Full design-to-DFM pipeline: design a thin-walled housing,
/// run DFM for injection molding, verify it catches the thin wall.
#[test]
fn thin_wall_housing_dfm_catches_issue() {
    // 0.5mm thin box — below injection mold minimum (1.0mm)
    let thin_part = make_box(40.0, 20.0, 0.5);
    let config = injection_mold_config();
    let issues = validate(&thin_part, &config);

    let thin_wall_issues: Vec<_> = issues.iter()
        .filter(|i| i.category == "Thin wall")
        .collect();

    assert!(
        !thin_wall_issues.is_empty(),
        "0.5mm wall should be flagged for injection molding (min {}mm)",
        config.min_wall_thickness
    );

    // Verify severity is Error (not just warning)
    assert!(
        thin_wall_issues.iter().any(|i| i.severity == Severity::Error),
        "thin wall should be an error, not just a warning"
    );
}

/// A properly designed CNC part should pass DFM with no errors.
#[test]
fn thick_box_passes_cnc_dfm() {
    let part = make_box(50.0, 30.0, 20.0);
    let config = cnc_config();
    let issues = validate(&part, &config);

    let errors: Vec<_> = issues.iter()
        .filter(|i| i.severity == Severity::Error)
        .collect();

    assert!(
        errors.is_empty(),
        "50×30×20mm box should pass CNC DFM, but got errors: {:?}",
        errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

/// Draft angle check: a box with vertical walls should fail
/// injection mold draft check (0° draft vs required 1°).
#[test]
fn vertical_walls_fail_injection_mold_draft() {
    let box_part = make_box(40.0, 40.0, 40.0);
    let config = injection_mold_config();
    let issues = validate(&box_part, &config);

    let draft_issues: Vec<_> = issues.iter()
        .filter(|i| i.category == "Insufficient draft")
        .collect();

    assert!(
        !draft_issues.is_empty(),
        "box with 0° draft should fail injection mold draft check (min {:.1}°)",
        config.min_draft_angle
    );

    // Should flag the vertical side faces but NOT the top/bottom
    // A box has 4 vertical faces
    assert!(
        draft_issues.len() >= 2,
        "should flag multiple vertical faces, got {}",
        draft_issues.len()
    );
}

/// Material lookup → analytical stress → safety factor pipeline.
/// Use 6061-T6 aluminum for a beam under load.
#[test]
fn material_to_safety_factor_pipeline() {
    let al = materials::lookup("6061-T6")
        .expect("6061-T6 should exist in materials database");

    assert!(al.elastic_modulus.to_mpa() > 60_000.0, "E should be ~69000 MPa");
    assert!(al.yield_strength.to_mpa() > 200.0, "yield should be ~276 MPa");

    // Beam: 200mm span, 20mm wide, 10mm tall, 500N center load
    let load: f64 = 500.0; // N
    let span: f64 = 200.0; // mm
    let width: f64 = 20.0;
    let height: f64 = 10.0;
    let i_moment = width * height.powi(3) / 12.0; // mm⁴

    let deflection = deflection_simply_supported(load, span, al.elastic_modulus.to_mpa(), i_moment);
    assert!(deflection > 0.0, "beam should deflect");
    assert!(deflection < 10.0, "deflection {deflection:.3}mm should be small");

    let stress = bending_stress_simply_supported(load, span, height, i_moment);
    assert!(stress > 0.0, "stress should be positive");

    let sf = safety_factor(al.yield_strength.to_mpa(), stress);
    assert!(sf > 1.0, "safety factor {sf:.2} should be > 1 for this load");

    // Verify the cascade makes sense: higher load → lower safety factor
    let high_stress = bending_stress_simply_supported(load * 10.0, span, height, i_moment);
    let low_sf = safety_factor(al.yield_strength.to_mpa(), high_stress);
    assert!(
        low_sf < sf,
        "10x load should lower safety factor: {low_sf:.2} vs {sf:.2}"
    );
}

/// Von Mises stress should reduce to uniaxial stress for simple loading.
#[test]
fn von_mises_reduces_to_uniaxial() {
    // Pure tension: σ_vm = σ_x
    let vm = von_mises(100.0, 0.0, 0.0);
    assert!(
        (vm - 100.0).abs() < 0.1,
        "uniaxial von Mises should equal applied stress: {vm}"
    );

    // Equal biaxial: σ_vm = σ (for σ1 = σ2, σ3 = 0)
    let vm_bi = von_mises(100.0, 100.0, 0.0);
    assert!(
        (vm_bi - 100.0).abs() < 0.1,
        "equal biaxial von Mises should equal applied stress: {vm_bi}"
    );

    // Hydrostatic: σ_vm = 0 (no distortion energy)
    let vm_hydro = von_mises(100.0, 100.0, 100.0);
    assert!(
        vm_hydro < 0.1,
        "hydrostatic von Mises should be ~0: {vm_hydro}"
    );
}

/// Cross-crate check: mass from LUT density × computed volume
/// should give a reasonable part mass.
#[test]
fn mass_from_lut_density_and_volume() {
    let part = make_box(50.0, 30.0, 10.0); // mm
    let props = mass_properties(&part);

    let steel = materials::lookup("4140-QT")
        .expect("AISI-4140 should exist in materials database");

    // Volume in mm³, density in kg/m³ → mass in kg
    let volume_m3 = props.volume * 1e-9; // mm³ → m³
    let mass_kg = steel.density.value() * volume_m3;

    // 50×30×10mm = 15000mm³ = 15cm³ of 4140 steel (density ~7850 kg/m³)
    // mass ≈ 0.118 kg
    assert!(
        mass_kg > 0.05 && mass_kg < 0.5,
        "4140 steel part mass {mass_kg:.4} kg should be reasonable"
    );
    assert!(
        (props.volume - 15_000.0).abs() / 15_000.0 < 0.01,
        "volume {:.1} should be ~15000 mm³",
        props.volume
    );
}

/// DFM issues should have valid location data (near the part geometry).
#[test]
fn dfm_issue_locations_are_valid() {
    let thin = make_box(40.0, 20.0, 0.5);
    let (min, max) = thin.bounding_box();
    let config = injection_mold_config();
    let issues = validate(&thin, &config);

    for issue in &issues {
        if issue.category == "Thin wall" || issue.category == "Insufficient draft" {
            // Issue location should be within or near the bounding box
            let margin = 5.0;
            assert!(
                issue.location.x >= min.x - margin && issue.location.x <= max.x + margin
                && issue.location.y >= min.y - margin && issue.location.y <= max.y + margin
                && issue.location.z >= min.z - margin && issue.location.z <= max.z + margin,
                "issue location {} should be near part bounds [{}, {}]",
                issue.location, min, max
            );
        }
    }
}

/// Material database should have all expected alloys with valid properties.
#[test]
fn material_database_completeness() {
    let expected_ids = [
        "6061-T6", "7075-T6", "2024-T3",
        "1018-CD", "4140-QT", "1045-HR",
        "304", "316",
        "Ti-6Al-4V",
    ];

    for id in &expected_ids {
        let mat = materials::lookup(id)
            .unwrap_or_else(|| panic!("material '{id}' should exist in database"));

        // All mechanical properties should be positive
        assert!(mat.density.value() > 0.0, "{id}: density should be positive");
        assert!(mat.elastic_modulus.to_mpa() > 0.0, "{id}: E should be positive");
        assert!(mat.yield_strength.to_mpa() > 0.0, "{id}: yield should be positive");
        assert!(mat.ultimate_tensile.to_mpa() > 0.0, "{id}: UTS should be positive");
        assert!(mat.poissons_ratio.value() > 0.0 && mat.poissons_ratio.value() < 0.5,
            "{id}: Poisson's ratio {} should be in (0, 0.5)", mat.poissons_ratio.value());

        // Yield should not exceed UTS
        assert!(
            mat.yield_strength.to_mpa() <= mat.ultimate_tensile.to_mpa(),
            "{id}: yield {:.0} MPa should not exceed UTS {:.0} MPa",
            mat.yield_strength.to_mpa(), mat.ultimate_tensile.to_mpa()
        );

        // Thermal properties should be positive
        assert!(mat.thermal_conductivity.value() > 0.0, "{id}: k should be positive");
        assert!(mat.melting_point.value() > 300.0, "{id}: melting point should be above room temp");
    }
}
