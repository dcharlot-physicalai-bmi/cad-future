//! Extended cascade integration tests — formula tier, solver dispatch, stats.

use physical_cascade::{
    self, resolve, thermal_query, structural_fea_query,
    Query, Tier, Value, CascadeStats,
};
use physical_units::*;

/// Shaft torque calculation via cascade dispatch.
#[test]
fn cascade_shaft_torque() {
    let q = Query::ShaftTorque {
        power: Power::new(5000.0), // 5 kW
        rpm: 1500.0,
    };
    let r = resolve(&q).unwrap();
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        Value::Scalar(t) => {
            // T = P × 60 / (2π × N) = 5000 × 60 / (2π × 1500) ≈ 31.83 N·m
            assert!(t > 25.0 && t < 40.0, "torque = {} N·m", t);
        }
        _ => panic!("expected Scalar"),
    }
}

/// Thermal expansion via cascade dispatch.
#[test]
fn cascade_thermal_expansion() {
    let q = Query::ThermalExpansion {
        material_id: "6061-T6",
        length: Length::m(1.0),
        delta_temp_k: 100.0,
    };
    let r = resolve(&q).unwrap();
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        // 6061-T6 CTE ≈ 23.6e-6 /K → ΔL ≈ 23.6e-6 × 1.0 × 100 ≈ 2.36 mm
        Value::Length(dl) => {
            assert!(dl.to_mm() > 1.0 && dl.to_mm() < 5.0,
                "thermal expansion = {} mm (expected ~2.36)", dl.to_mm());
        }
        _ => panic!("expected Length"),
    }
}

/// Reynolds number for water in a pipe.
#[test]
fn cascade_reynolds_number() {
    let q = Query::ReynoldsNumber {
        fluid_density: Density::new(998.0),
        velocity: Velocity::new(2.0),
        diameter: Length::mm(50.0),
        viscosity: 0.001,
    };
    let r = resolve(&q).unwrap();
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        // Re = ρvD/μ = 998 × 2 × 0.05 / 0.001 = 99800
        Value::Scalar(re) => {
            assert!(re > 50_000.0 && re < 150_000.0, "Re = {}", re);
        }
        _ => panic!("expected Scalar"),
    }
}

/// Heat conduction through aluminum wall.
#[test]
fn cascade_heat_conduction() {
    let q = Query::HeatConduction {
        material_id: "6061-T6",
        area: Area::m2(0.01),             // 100 cm²
        thickness: Length::mm(10.0),
        temp_hot: Temperature::new(373.15),   // 100°C
        temp_cold: Temperature::new(293.15),  // 20°C
    };
    let r = resolve(&q).unwrap();
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        // Q = k × A × ΔT / L ≈ 167 × 0.01 × 80 / 0.01 ≈ 13360 W
        Value::Scalar(q_watts) => {
            assert!(q_watts > 5000.0, "heat flow = {} W should be substantial", q_watts);
        }
        _ => panic!("expected Scalar"),
    }
}

/// Hertz contact via cascade: ball bearing on flat.
#[test]
fn cascade_hertz_contact() {
    let q = Query::HertzContact {
        force: Force::kn(1.0),
        sphere_radius: Length::mm(5.0),
        material_id: "1018-CD",
    };
    let r = resolve(&q).unwrap();
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        Value::Pressure(p) => {
            assert!(p.to_mpa() > 500.0, "contact stress = {} MPa", p.to_mpa());
        }
        _ => panic!("expected Pressure"),
    }
}

/// Thermal query — simple flat wall uses formula tier.
#[test]
fn thermal_query_uses_formula() {
    let r = thermal_query("6061-T6", 100.0, 20.0, 10.0, 5000.0);
    assert_eq!(r.tier, Tier::Formula);
    match r.value {
        Value::Scalar(q) => assert!(q > 0.0, "heat transfer should be positive"),
        _ => panic!("expected Scalar"),
    }
}

/// Structural FEA query — simple geometry stays at formula tier.
#[test]
fn structural_query_formula_tier() {
    let r = structural_fea_query("6061-T6", 8, 5000.0);
    assert_eq!(r.tier, Tier::Formula);
    assert!(r.description.contains("Simple geometry"));
}

/// Structural FEA query — complex geometry escalates to solver tier.
#[test]
fn structural_query_solver_tier() {
    let r = structural_fea_query("Ti-6Al-4V", 1000, 50_000.0);
    assert_eq!(r.tier, Tier::Solver);
    assert!(r.description.contains("FEA solver required"));
}

/// Cascade stats tracking over a realistic workflow.
#[test]
fn cascade_stats_workflow() {
    let mut stats = CascadeStats::default();

    // Simulate a typical session: many LUT lookups, some formulas, rare solver
    for _ in 0..20 { stats.record(Tier::Lut); }
    for _ in 0..8 { stats.record(Tier::Formula); }
    stats.record(Tier::Solver);
    stats.record(Tier::Solver);

    assert_eq!(stats.total(), 30);
    assert_eq!(stats.lut_hits, 20);
    assert_eq!(stats.formula_hits, 8);
    assert_eq!(stats.solver_hits, 2);

    // Fast resolution: (20 + 8) / 30 ≈ 93.3%
    let pct = stats.fast_resolution_pct();
    assert!(pct > 90.0, "fast resolution = {pct:.1}% should be >90%");
}

/// End-to-end: material lookup → formula → check cascade ordering.
#[test]
fn full_cascade_ordering() {
    // Tier 1: Material LUT
    let ys = physical_cascade::yield_strength("7075-T6").unwrap();
    assert_eq!(ys.tier, Tier::Lut);

    // Tier 2: Beam deflection formula
    let beam = resolve(&Query::BeamDeflection {
        load: Force::kn(2.0),
        span: Length::m(0.5),
        material_id: "7075-T6",
        section_width: Length::mm(25.0),
        section_height: Length::mm(25.0),
    }).unwrap();
    assert_eq!(beam.tier, Tier::Formula);

    // Verify: LUT tier < Formula tier (cheaper resolves first)
    assert!(matches!(ys.tier, Tier::Lut));
    assert!(matches!(beam.tier, Tier::Formula));
}
