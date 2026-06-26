//! Material cascade tests — LUT → Formula → Cascade dispatch.
//!
//! Verifies the full cascade: material lookup from LUT, formula-based
//! engineering calculations using those values, and the unified dispatch interface.

use physical_analytical::{deflection_simply_supported, bending_stress_simply_supported, safety_factor};
use physical_cascade::{
    self, yield_strength, density, resolve,
    Query, MaterialProperty, Tier, Value,
};
use physical_lut::{materials, manufacturing, standards};
use physical_units::*;

/// Look up 6061-T6 → get yield strength → use in beam deflection formula → verify result.
#[test]
fn al6061_yield_to_beam_deflection() {
    let al = materials::lookup("6061-T6").expect("6061-T6 should exist");

    let yield_mpa = al.yield_strength.to_mpa();
    assert!(yield_mpa > 200.0 && yield_mpa < 350.0, "6061-T6 yield = {yield_mpa} MPa");

    let e_mpa = al.elastic_modulus.to_mpa();
    assert!(e_mpa > 60_000.0 && e_mpa < 75_000.0, "6061-T6 E = {e_mpa} MPa");

    // Beam: 200mm span, 20×10mm cross-section, 500N center load
    let load = 500.0;
    let span = 200.0;
    let width: f64 = 20.0;
    let height: f64 = 10.0;
    let i_moment = width * height.powi(3) / 12.0;

    let deflection = deflection_simply_supported(load, span, e_mpa, i_moment);
    assert!(deflection > 0.0, "deflection should be positive");
    assert!(deflection < 5.0, "deflection {deflection:.4}mm should be small for this beam");

    let stress = bending_stress_simply_supported(load, span, height, i_moment);
    let sf = safety_factor(yield_mpa, stress);
    assert!(sf > 1.0, "safety factor {sf:.2} should be > 1.0");
}

/// Look up manufacturing constraint for CNC + Aluminum → verify min wall thickness.
#[test]
fn cnc_aluminum_min_wall() {
    let constraint = manufacturing::lookup(
        manufacturing::Process::CncMill3Ax,
        manufacturing::MaterialClass::Aluminum,
    )
    .expect("CNC 3-axis + Aluminum should exist in LUT");

    let min_wall = constraint.min_wall_thickness.to_mm();
    assert!(
        min_wall > 0.0 && min_wall < 5.0,
        "CNC aluminum min wall {min_wall:.2}mm should be reasonable"
    );

    // Corner radius should also be present
    let min_corner = constraint.min_corner_radius.to_mm();
    assert!(
        min_corner >= 0.0,
        "CNC aluminum min corner radius should be non-negative"
    );
}

/// Look up M8 thread → verify pitch and minor diameter.
#[test]
fn m8_thread_lookup() {
    let thread = standards::lookup_metric_thread("M8")
        .expect("M8 thread should exist in standards LUT");

    assert_eq!(thread.nominal_diameter_mm, 8.0);
    assert!(
        (thread.coarse_pitch_mm - 1.25).abs() < 0.01,
        "M8 coarse pitch should be 1.25mm, got {}",
        thread.coarse_pitch_mm
    );
    assert!(
        (thread.minor_diameter_mm - 6.647).abs() < 0.01,
        "M8 minor diameter should be ~6.647mm, got {}",
        thread.minor_diameter_mm
    );
    assert!(
        thread.pitch_diameter_mm > thread.minor_diameter_mm,
        "pitch diameter should exceed minor diameter"
    );
    assert!(
        thread.pitch_diameter_mm < thread.nominal_diameter_mm,
        "pitch diameter should be less than nominal"
    );
}

/// Use cascade to query material properties → verify tier is LUT.
#[test]
fn cascade_material_query_tier_lut() {
    let result = yield_strength("6061-T6").expect("cascade yield lookup should work");
    assert_eq!(result.tier, Tier::Lut, "material property should resolve at LUT tier");

    match result.value {
        Value::Pressure(p) => {
            assert!(p.to_mpa() > 200.0 && p.to_mpa() < 350.0, "yield = {} MPa", p.to_mpa());
        }
        _ => panic!("yield_strength should return Pressure"),
    }

    // Density query
    let d_result = density("Ti-6Al-4V").expect("cascade density lookup should work");
    assert_eq!(d_result.tier, Tier::Lut);
    match d_result.value {
        Value::Density(d) => {
            assert!(d.value() > 4000.0 && d.value() < 5000.0, "Ti density = {} kg/m3", d.value());
        }
        _ => panic!("density should return Density"),
    }
}

/// Use the unified resolve() dispatch and verify tier ordering.
#[test]
fn cascade_dispatch_lut_then_formula() {
    // LUT query
    let q_lut = Query::MaterialProperty {
        material_id: "7075-T6",
        property: MaterialProperty::ElasticModulus,
    };
    let r_lut = resolve(&q_lut).expect("should resolve 7075-T6 E");
    assert_eq!(r_lut.tier, Tier::Lut);

    // Formula query
    let q_formula = Query::BeamDeflection {
        load: Force::kn(5.0),
        span: Length::m(1.0),
        material_id: "6061-T6",
        section_width: Length::mm(40.0),
        section_height: Length::mm(20.0),
    };
    let r_formula = resolve(&q_formula).expect("should resolve beam deflection");
    assert_eq!(r_formula.tier, Tier::Formula);

    // Formula results should yield a positive deflection
    match r_formula.value {
        Value::Length(d) => assert!(d.to_mm() > 0.0, "deflection = {} mm", d.to_mm()),
        _ => panic!("beam deflection should return Length"),
    }
}

/// Wall thickness check via cascade dispatch.
#[test]
fn cascade_wall_check() {
    let r_pass = physical_cascade::check_wall(
        Length::mm(2.0),
        manufacturing::Process::CncMill3Ax,
        manufacturing::MaterialClass::Aluminum,
    )
    .expect("wall check should resolve");
    assert_eq!(r_pass.tier, Tier::Lut);
    assert!(matches!(r_pass.value, Value::Bool(true)), "2mm wall should pass CNC check");

    let r_fail = physical_cascade::check_wall(
        Length::mm(0.1),
        manufacturing::Process::CncMill3Ax,
        manufacturing::MaterialClass::Aluminum,
    )
    .expect("wall check should resolve");
    assert!(matches!(r_fail.value, Value::Bool(false)), "0.1mm wall should fail CNC check");
}
