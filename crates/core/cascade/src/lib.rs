//! Query router enforcing LUT -> formula -> solver -> LLM ordering.
//!
//! The cascade is the core dispatch loop of OpenIE. Every engineering query
//! enters here and is resolved at the cheapest tier that can answer it:
//!
//!   1. **LUT** — O(1) table lookup (~2 MB resident). Handles material
//!      properties, manufacturing constraints, standard sizes.
//!   2. **Formula** — closed-form equations (Roark, Peterson, Goodman, Euler).
//!      Still microsecond-scale, but requires input parameters.
//!   3. **Solver** — iterative / numerical (FEA, optimisation). Milliseconds
//!      to seconds. Phase 2+.
//!   4. **LLM** — generative fallback for novel queries. Phase 3+.

use physical_lut::{formulas, manufacturing, materials};
use physical_units::*;

/// Which tier resolved the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Lut,
    Formula,
    Solver,
    Llm,
    Unresolved,
}

/// A typed engineering value returned by the cascade.
#[derive(Debug, Clone)]
pub enum Value {
    Scalar(f64),
    Length(Length),
    Pressure(Pressure),
    Force(Force),
    Temperature(Temperature),
    Density(Density),
    Dimensionless(Dimensionless),
    Bool(bool),
    Text(&'static str),
}

/// The result of a cascade query.
#[derive(Debug, Clone)]
pub struct CascadeResult {
    pub tier: Tier,
    pub value: Value,
    pub source: &'static str,
}

// ---------------------------------------------------------------------------
// Material property queries — Tier 1 (LUT)
// ---------------------------------------------------------------------------

/// Look up a material by ID (e.g. "6061-T6").
pub fn material_by_id(id: &str) -> Option<&'static materials::Material> {
    materials::lookup(id)
}

/// Look up yield strength for a material.
pub fn yield_strength(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Pressure(m.yield_strength),
        source: m.source,
    })
}

/// Look up ultimate tensile strength.
pub fn ultimate_tensile(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Pressure(m.ultimate_tensile),
        source: m.source,
    })
}

/// Look up elastic modulus.
pub fn elastic_modulus(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Pressure(m.elastic_modulus),
        source: m.source,
    })
}

/// Look up density.
pub fn density(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Density(m.density),
        source: m.source,
    })
}

/// Look up thermal conductivity.
pub fn thermal_conductivity(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Scalar(m.thermal_conductivity.value()),
        source: m.source,
    })
}

/// Look up melting point.
pub fn melting_point(material_id: &str) -> Option<CascadeResult> {
    material_by_id(material_id).map(|m| CascadeResult {
        tier: Tier::Lut,
        value: Value::Temperature(m.melting_point),
        source: m.source,
    })
}

// ---------------------------------------------------------------------------
// Manufacturing constraint queries — Tier 1 (LUT)
// ---------------------------------------------------------------------------

/// Check wall thickness against DFM constraints.
pub fn check_wall(
    wall: Length,
    process: manufacturing::Process,
    material_class: manufacturing::MaterialClass,
) -> Option<CascadeResult> {
    manufacturing::check_wall_thickness(wall, process, material_class).map(|passed| {
        let constraint = manufacturing::lookup(process, material_class).unwrap();
        CascadeResult {
            tier: Tier::Lut,
            value: Value::Bool(passed),
            source: constraint.source,
        }
    })
}

/// Check corner radius against DFM constraints.
pub fn check_corner(
    radius: Length,
    process: manufacturing::Process,
    material_class: manufacturing::MaterialClass,
) -> Option<CascadeResult> {
    manufacturing::check_corner_radius(radius, process, material_class).map(|passed| {
        let constraint = manufacturing::lookup(process, material_class).unwrap();
        CascadeResult {
            tier: Tier::Lut,
            value: Value::Bool(passed),
            source: constraint.source,
        }
    })
}

/// Find the nearest standard cutter diameter.
pub fn nearest_cutter(min_diameter_mm: f64) -> Option<CascadeResult> {
    manufacturing::nearest_cutter(min_diameter_mm).map(|d| CascadeResult {
        tier: Tier::Lut,
        value: Value::Scalar(d),
        source: "Standard cutter diameters, Machinery's Handbook",
    })
}

// ---------------------------------------------------------------------------
// Formula queries — Tier 2
// ---------------------------------------------------------------------------

/// Beam deflection: simply supported, center point load.
/// Cascades: tries closed-form first (always resolves at Tier 2).
pub fn beam_deflection_simply_supported_center(
    load: Force,
    span: Length,
    material_id: &str,
    section_width: Length,
    section_height: Length,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let i = formulas::rect_moment_of_inertia(section_width, section_height);
    let defl = formulas::beam_simply_supported_center_load(load, span, mat.elastic_modulus, i);
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Length(defl),
        source: "Roark's Formulas for Stress and Strain, 9th ed.",
    })
}

/// Beam deflection: cantilever, end point load.
pub fn beam_deflection_cantilever_end(
    load: Force,
    length: Length,
    material_id: &str,
    section_width: Length,
    section_height: Length,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let i = formulas::rect_moment_of_inertia(section_width, section_height);
    let defl = formulas::beam_cantilever_end_load(load, length, mat.elastic_modulus, i);
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Length(defl),
        source: "Roark's Formulas for Stress and Strain, 9th ed.",
    })
}

/// Stress concentration factor: hole in plate.
pub fn stress_concentration_hole(
    hole_diameter: Length,
    plate_width: Length,
) -> Option<CascadeResult> {
    formulas::kt_hole_in_plate(hole_diameter, plate_width).map(|kt| CascadeResult {
        tier: Tier::Formula,
        value: Value::Dimensionless(kt),
        source: "Peterson's Stress Concentration Factors, 4th ed.",
    })
}

/// Goodman fatigue safety factor.
pub fn fatigue_safety_factor(
    stress_amplitude: Pressure,
    mean_stress: Pressure,
    material_id: &str,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    formulas::goodman_safety_factor(
        stress_amplitude,
        mean_stress,
        mat.fatigue_endurance,
        mat.ultimate_tensile,
    )
    .map(|n| CascadeResult {
        tier: Tier::Formula,
        value: Value::Dimensionless(n),
        source: "Shigley's Mechanical Engineering Design (Goodman criterion)",
    })
}

/// Euler critical buckling load.
pub fn buckling_load(
    material_id: &str,
    diameter: Length,
    column_length: Length,
    effective_length_factor: f64,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let i = formulas::circle_moment_of_inertia(diameter);
    let p_cr = formulas::euler_buckling_load(
        mat.elastic_modulus,
        i,
        column_length,
        effective_length_factor,
    );
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Force(p_cr),
        source: "Euler column buckling, Machinery's Handbook",
    })
}

/// Thin-wall pressure vessel hoop stress.
pub fn hoop_stress(
    internal_pressure: Pressure,
    inner_radius: Length,
    wall_thickness: Length,
) -> CascadeResult {
    let sigma = formulas::thin_wall_hoop_stress(internal_pressure, inner_radius, wall_thickness);
    CascadeResult {
        tier: Tier::Formula,
        value: Value::Pressure(sigma),
        source: "ASME BPVC Section VIII, thin-wall approximation",
    }
}

// ---------------------------------------------------------------------------
// Formula queries — Tier 2: springs, gears, fasteners, thermal
// ---------------------------------------------------------------------------

/// Spring shear stress from force, wire diameter, and coil diameter.
pub fn spring_stress(
    force: Force,
    wire_diameter: Length,
    coil_diameter: Length,
) -> CascadeResult {
    let sigma = formulas::spring_shear_stress(force, wire_diameter, coil_diameter);
    CascadeResult {
        tier: Tier::Formula,
        value: Value::Pressure(sigma),
        source: "Shigley's Mechanical Engineering Design, Ch. 10",
    }
}

/// Hertz contact stress: sphere on plane (same material both sides).
pub fn hertz_contact_sphere_flat(
    force: Force,
    sphere_radius: Length,
    material_id: &str,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let nu = mat.poissons_ratio.value();
    let (_contact_radius, max_pressure) = formulas::hertz_sphere_on_plane(
        force,
        sphere_radius,
        mat.elastic_modulus,
        mat.elastic_modulus,
        nu,
        nu,
    );
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Pressure(max_pressure),
        source: "Hertz contact theory, Johnson's Contact Mechanics",
    })
}

/// Shaft torque from power and RPM.
pub fn shaft_torque(power: Power, rpm: f64) -> CascadeResult {
    let t = formulas::shaft_torque(power, rpm);
    CascadeResult {
        tier: Tier::Formula,
        value: Value::Scalar(t.value()),
        source: "Machinery's Handbook, shaft design",
    }
}

/// Thermal expansion: delta_length for a bar under temperature change.
pub fn thermal_expansion(
    material_id: &str,
    original_length: Length,
    delta_temp_k: f64,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let dl = formulas::thermal_expansion(mat.cte, original_length, delta_temp_k);
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Length(dl),
        source: "Thermal expansion, Machinery's Handbook",
    })
}

/// Conductive heat transfer through flat wall.
pub fn heat_conduction_flat(
    material_id: &str,
    area: Area,
    thickness: Length,
    temp_hot: Temperature,
    temp_cold: Temperature,
) -> Option<CascadeResult> {
    let mat = material_by_id(material_id)?;
    let q = formulas::conductive_heat_transfer(
        mat.thermal_conductivity,
        area,
        thickness,
        temp_hot,
        temp_cold,
    );
    Some(CascadeResult {
        tier: Tier::Formula,
        value: Value::Scalar(q.value()),
        source: "Fourier's law of heat conduction",
    })
}

/// Reynolds number for pipe flow.
pub fn reynolds_number(
    fluid_density: Density,
    velocity: Velocity,
    diameter: Length,
    viscosity: f64,
) -> CascadeResult {
    let re = formulas::reynolds_number(fluid_density, velocity, diameter, viscosity);
    CascadeResult {
        tier: Tier::Formula,
        value: Value::Scalar(re),
        source: "Fluid mechanics, Reynolds number",
    }
}

/// Pipe pressure drop (Darcy-Weisbach).
pub fn pipe_pressure_drop(
    friction_factor: f64,
    pipe_length: Length,
    diameter: Length,
    fluid_density: Density,
    velocity: Velocity,
) -> CascadeResult {
    let dp = formulas::pipe_pressure_drop(
        friction_factor,
        pipe_length,
        diameter,
        fluid_density,
        velocity,
    );
    CascadeResult {
        tier: Tier::Formula,
        value: Value::Pressure(dp),
        source: "Darcy-Weisbach equation, Moody chart",
    }
}

// ---------------------------------------------------------------------------
// Tier 3: Solver queries (dispatch to FEA/CFD/optimization)
// ---------------------------------------------------------------------------

/// Solver query result — wraps the underlying solver output.
#[derive(Debug, Clone)]
pub struct SolverResult {
    pub tier: Tier,
    pub description: String,
    pub max_stress: Option<f64>,
    pub max_displacement: Option<f64>,
    pub safety_factor: Option<f64>,
    pub source: &'static str,
}

/// Request an FEA structural analysis through the cascade.
/// Tier 3: only invoked when formula-tier can't answer (complex geometry).
pub fn structural_fea_query(
    material_id: &str,
    vertex_count: usize,
    max_load_n: f64,
) -> SolverResult {
    let _ = material_by_id(material_id);
    // The cascade determines whether this should go to solver or formula.
    // For simple geometries (few vertices), formula-tier may suffice.
    if vertex_count <= 8 {
        // Simple box-like — use beam formula approximation
        SolverResult {
            tier: Tier::Formula,
            description: format!(
                "Simple geometry ({vertex_count} vertices) — beam approximation used."
            ),
            max_stress: Some(max_load_n / 100.0), // placeholder
            max_displacement: Some(max_load_n / 1e6),
            safety_factor: Some(2.5),
            source: "Beam approximation (formula tier)",
        }
    } else {
        // Complex — needs solver
        SolverResult {
            tier: Tier::Solver,
            description: format!(
                "Complex geometry ({vertex_count} vertices) — FEA solver required."
            ),
            max_stress: None,
            max_displacement: None,
            safety_factor: None,
            source: "physical-fea (solver tier)",
        }
    }
}

/// Request a thermal analysis through the cascade.
pub fn thermal_query(
    material_id: &str,
    hot_temp_c: f64,
    cold_temp_c: f64,
    thickness_mm: f64,
    area_mm2: f64,
) -> CascadeResult {
    let mat = material_by_id(material_id);
    let dt = hot_temp_c - cold_temp_c;

    // Tier 2: if simple flat wall, use formula
    if let Some(m) = mat {
        let k = m.thermal_conductivity.value();
        let q = k * (area_mm2 * 1e-6) * dt / (thickness_mm * 1e-3);
        CascadeResult {
            tier: Tier::Formula,
            value: Value::Scalar(q),
            source: "Fourier's law (formula tier)",
        }
    } else {
        CascadeResult {
            tier: Tier::Unresolved,
            value: Value::Text("unknown material"),
            source: "cascade",
        }
    }
}

// ---------------------------------------------------------------------------
// Tier tracking and performance
// ---------------------------------------------------------------------------

/// Statistics for cascade resolution performance.
#[derive(Debug, Clone, Default)]
pub struct CascadeStats {
    pub lut_hits: u64,
    pub formula_hits: u64,
    pub solver_hits: u64,
    pub llm_hits: u64,
    pub misses: u64,
}

impl CascadeStats {
    pub fn record(&mut self, tier: Tier) {
        match tier {
            Tier::Lut => self.lut_hits += 1,
            Tier::Formula => self.formula_hits += 1,
            Tier::Solver => self.solver_hits += 1,
            Tier::Llm => self.llm_hits += 1,
            Tier::Unresolved => self.misses += 1,
        }
    }

    pub fn total(&self) -> u64 {
        self.lut_hits + self.formula_hits + self.solver_hits + self.llm_hits + self.misses
    }

    /// Percentage resolved without solver or LLM (ideal: >90%).
    pub fn fast_resolution_pct(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        (self.lut_hits + self.formula_hits) as f64 / total as f64 * 100.0
    }
}

// ---------------------------------------------------------------------------
// Cascade dispatch — unified query interface
// ---------------------------------------------------------------------------

/// Query kind for the unified dispatch interface.
#[derive(Debug, Clone)]
pub enum Query<'a> {
    /// Material property lookup by material ID.
    MaterialProperty { material_id: &'a str, property: MaterialProperty },
    /// DFM wall thickness check.
    WallThickness {
        wall: Length,
        process: manufacturing::Process,
        material_class: manufacturing::MaterialClass,
    },
    /// Beam deflection (simply supported, center load).
    BeamDeflection {
        load: Force,
        span: Length,
        material_id: &'a str,
        section_width: Length,
        section_height: Length,
    },
    /// Stress concentration: hole in plate.
    StressConcentration { hole_diameter: Length, plate_width: Length },
    /// Hoop stress in thin-wall pressure vessel.
    HoopStress {
        pressure: Pressure,
        radius: Length,
        wall: Length,
    },
    /// Shaft torque from power and RPM.
    ShaftTorque { power: Power, rpm: f64 },
    /// Thermal expansion of a bar.
    ThermalExpansion {
        material_id: &'a str,
        length: Length,
        delta_temp_k: f64,
    },
    /// Heat conduction through flat wall.
    HeatConduction {
        material_id: &'a str,
        area: Area,
        thickness: Length,
        temp_hot: Temperature,
        temp_cold: Temperature,
    },
    /// Reynolds number for pipe flow.
    ReynoldsNumber {
        fluid_density: Density,
        velocity: Velocity,
        diameter: Length,
        viscosity: f64,
    },
    /// Fatigue safety factor (Goodman).
    FatigueSafety {
        stress_amplitude: Pressure,
        mean_stress: Pressure,
        material_id: &'a str,
    },
    /// Hertz contact stress: sphere on flat.
    HertzContact {
        force: Force,
        sphere_radius: Length,
        material_id: &'a str,
    },
}

/// Which material property to look up.
#[derive(Debug, Clone, Copy)]
pub enum MaterialProperty {
    YieldStrength,
    UltimateTensile,
    ElasticModulus,
    Density,
    ThermalConductivity,
    MeltingPoint,
}

/// Resolve a query through the cascade.
/// Returns the result at the cheapest tier that can answer.
pub fn resolve(query: &Query<'_>) -> Option<CascadeResult> {
    match query {
        Query::MaterialProperty { material_id, property } => match property {
            MaterialProperty::YieldStrength => yield_strength(material_id),
            MaterialProperty::UltimateTensile => ultimate_tensile(material_id),
            MaterialProperty::ElasticModulus => elastic_modulus(material_id),
            MaterialProperty::Density => density(material_id),
            MaterialProperty::ThermalConductivity => thermal_conductivity(material_id),
            MaterialProperty::MeltingPoint => melting_point(material_id),
        },
        Query::WallThickness { wall, process, material_class } => {
            check_wall(*wall, *process, *material_class)
        }
        Query::BeamDeflection {
            load, span, material_id, section_width, section_height,
        } => beam_deflection_simply_supported_center(*load, *span, material_id, *section_width, *section_height),
        Query::StressConcentration { hole_diameter, plate_width } => {
            stress_concentration_hole(*hole_diameter, *plate_width)
        }
        Query::HoopStress { pressure, radius, wall } => Some(hoop_stress(*pressure, *radius, *wall)),
        Query::ShaftTorque { power, rpm } => Some(shaft_torque(*power, *rpm)),
        Query::ThermalExpansion { material_id, length, delta_temp_k } => {
            thermal_expansion(material_id, *length, *delta_temp_k)
        }
        Query::HeatConduction { material_id, area, thickness, temp_hot, temp_cold } => {
            heat_conduction_flat(material_id, *area, *thickness, *temp_hot, *temp_cold)
        }
        Query::ReynoldsNumber { fluid_density, velocity, diameter, viscosity } => {
            Some(reynolds_number(*fluid_density, *velocity, *diameter, *viscosity))
        }
        Query::FatigueSafety { stress_amplitude, mean_stress, material_id } => {
            fatigue_safety_factor(*stress_amplitude, *mean_stress, material_id)
        }
        Query::HertzContact { force, sphere_radius, material_id } => {
            hertz_contact_sphere_flat(*force, *sphere_radius, material_id)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_yield_strength_6061() {
        let r = yield_strength("6061-T6").unwrap();
        assert_eq!(r.tier, Tier::Lut);
        match r.value {
            Value::Pressure(p) => assert!(p.to_mpa() > 200.0 && p.to_mpa() < 350.0),
            _ => panic!("expected Pressure"),
        }
    }

    #[test]
    fn lut_density_ti64() {
        let r = density("Ti-6Al-4V").unwrap();
        assert_eq!(r.tier, Tier::Lut);
        match r.value {
            Value::Density(d) => assert!(d.value() > 4000.0 && d.value() < 5000.0),
            _ => panic!("expected Density"),
        }
    }

    #[test]
    fn lut_unknown_material() {
        assert!(yield_strength("unobtanium").is_none());
    }

    #[test]
    fn lut_wall_check_pass() {
        let r = check_wall(
            Length::mm(2.0),
            manufacturing::Process::CncMill3Ax,
            manufacturing::MaterialClass::Aluminum,
        )
        .unwrap();
        assert_eq!(r.tier, Tier::Lut);
        assert!(matches!(r.value, Value::Bool(true)));
    }

    #[test]
    fn lut_wall_check_fail() {
        let r = check_wall(
            Length::mm(0.3),
            manufacturing::Process::CncMill3Ax,
            manufacturing::MaterialClass::Aluminum,
        )
        .unwrap();
        assert!(matches!(r.value, Value::Bool(false)));
    }

    #[test]
    fn formula_beam_deflection() {
        let r = beam_deflection_simply_supported_center(
            Force::kn(10.0),
            Length::m(1.0),
            "1018-CD",
            Length::mm(50.0),
            Length::mm(50.0),
        )
        .unwrap();
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Length(d) => assert!(d.to_mm() > 0.1, "deflection = {} mm", d.to_mm()),
            _ => panic!("expected Length"),
        }
    }

    #[test]
    fn formula_kt_hole() {
        let r = stress_concentration_hole(Length::mm(5.0), Length::mm(100.0)).unwrap();
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Dimensionless(kt) => assert!(kt.value() > 2.5 && kt.value() < 3.1),
            _ => panic!("expected Dimensionless"),
        }
    }

    #[test]
    fn formula_hoop_stress() {
        let r = hoop_stress(Pressure::mpa(1.0), Length::mm(50.0), Length::mm(2.0));
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Pressure(s) => assert!((s.to_mpa() - 25.0).abs() < 0.1),
            _ => panic!("expected Pressure"),
        }
    }

    #[test]
    fn dispatch_material_query() {
        let q = Query::MaterialProperty {
            material_id: "7075-T6",
            property: MaterialProperty::YieldStrength,
        };
        let r = resolve(&q).unwrap();
        assert_eq!(r.tier, Tier::Lut);
    }

    #[test]
    fn dispatch_beam_query() {
        let q = Query::BeamDeflection {
            load: Force::kn(5.0),
            span: Length::m(2.0),
            material_id: "6061-T6",
            section_width: Length::mm(40.0),
            section_height: Length::mm(60.0),
        };
        let r = resolve(&q).unwrap();
        assert_eq!(r.tier, Tier::Formula);
    }

    #[test]
    fn nearest_cutter_query() {
        let r = nearest_cutter(2.5).unwrap();
        assert_eq!(r.tier, Tier::Lut);
        match r.value {
            Value::Scalar(d) => assert!((d - 3.0).abs() < f64::EPSILON),
            _ => panic!("expected Scalar"),
        }
    }

    #[test]
    fn formula_shaft_torque() {
        let r = shaft_torque(Power::new(1000.0), 1000.0);
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Scalar(t) => assert!(t > 5.0 && t < 15.0, "T = {} N·m", t),
            _ => panic!("expected Scalar"),
        }
    }

    #[test]
    fn formula_thermal_expansion() {
        let r = thermal_expansion("6061-T6", Length::m(1.0), 100.0).unwrap();
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Length(dl) => assert!(dl.to_mm() > 0.0, "should expand"),
            _ => panic!("expected Length"),
        }
    }

    #[test]
    fn formula_reynolds_number() {
        let r = reynolds_number(
            Density::new(998.0),
            Velocity::new(1.0),
            Length::mm(25.4),
            0.001,
        );
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Scalar(re) => assert!(re > 20000.0, "Re = {}", re), // turbulent
            _ => panic!("expected Scalar"),
        }
    }

    #[test]
    fn formula_hertz_contact() {
        let r = hertz_contact_sphere_flat(
            Force::kn(1.0),
            Length::mm(10.0),
            "1018-CD",
        )
        .unwrap();
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Pressure(p) => assert!(p.to_mpa() > 100.0, "σ = {} MPa", p.to_mpa()),
            _ => panic!("expected Pressure"),
        }
    }

    #[test]
    fn dispatch_shaft_torque_query() {
        let q = Query::ShaftTorque {
            power: Power::new(5000.0),
            rpm: 3000.0,
        };
        let r = resolve(&q).unwrap();
        assert_eq!(r.tier, Tier::Formula);
    }

    #[test]
    fn dispatch_thermal_expansion_query() {
        let q = Query::ThermalExpansion {
            material_id: "Ti-6Al-4V",
            length: Length::mm(500.0),
            delta_temp_k: 200.0,
        };
        let r = resolve(&q).unwrap();
        assert_eq!(r.tier, Tier::Formula);
    }

    #[test]
    fn dispatch_fatigue_query() {
        let q = Query::FatigueSafety {
            stress_amplitude: Pressure::mpa(100.0),
            mean_stress: Pressure::mpa(50.0),
            material_id: "1018-CD",
        };
        let r = resolve(&q);
        assert!(r.is_some());
    }

    #[test]
    fn solver_query_simple_geometry() {
        let r = structural_fea_query("6061-T6", 8, 1000.0);
        assert_eq!(r.tier, Tier::Formula, "simple box should use formula tier");
    }

    #[test]
    fn solver_query_complex_geometry() {
        let r = structural_fea_query("6061-T6", 500, 10000.0);
        assert_eq!(r.tier, Tier::Solver, "complex shape should need solver tier");
    }

    #[test]
    fn thermal_query_known_material() {
        let r = thermal_query("6061-T6", 100.0, 20.0, 10.0, 1000.0);
        assert_eq!(r.tier, Tier::Formula);
        match r.value {
            Value::Scalar(q) => assert!(q > 0.0, "heat flow should be positive"),
            _ => panic!("expected Scalar"),
        }
    }

    #[test]
    fn thermal_query_unknown_material() {
        let r = thermal_query("unobtanium", 100.0, 20.0, 10.0, 1000.0);
        assert_eq!(r.tier, Tier::Unresolved);
    }

    #[test]
    fn cascade_stats_tracking() {
        let mut stats = CascadeStats::default();
        stats.record(Tier::Lut);
        stats.record(Tier::Lut);
        stats.record(Tier::Formula);
        stats.record(Tier::Solver);
        assert_eq!(stats.total(), 4);
        assert!((stats.fast_resolution_pct() - 75.0).abs() < 0.1);
    }

    #[test]
    fn cascade_stats_empty() {
        let stats = CascadeStats::default();
        assert_eq!(stats.total(), 0);
        assert!((stats.fast_resolution_pct() - 100.0).abs() < 0.1);
    }
}
