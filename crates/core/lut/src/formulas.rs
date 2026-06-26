//! Engineering formula lookup — Roark's beam cases, Peterson's Kt, Goodman fatigue,
//! spring design, gear geometry, bearing life, heat transfer, Hertz contact,
//! pressure vessels, column buckling, weld design, fastener analysis.
//!
//! Pure functions. no_std. Every result is a typed physical quantity.
//! These are the "Formula second" tier of the cascade.
//!
//! References:
//! - Roark's Formulas for Stress and Strain, 9th ed.
//! - Peterson's Stress Concentration Factors, 4th ed.
//! - Shigley's Mechanical Engineering Design, 11th ed.
//! - Machinery's Handbook, 31st ed.
//! - AWS D1.1 Structural Welding Code — Steel
//! - VDI 2230 Systematic Calculation of High Duty Bolted Joints

use physical_units::*;

// ---------------------------------------------------------------------------
// Roark's beam deflection — 20+ cases
// ---------------------------------------------------------------------------

/// Simply supported beam, uniform load.
/// δ_max = 5wL⁴ / 384EI
pub fn beam_simply_supported_uniform(
    load_per_length: f64, // N/m
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(5.0 * load_per_length * l.powi(4) / (384.0 * e * i))
}

/// Simply supported beam, center point load.
/// δ_max = PL³ / 48EI
pub fn beam_simply_supported_center_load(
    load: Force,
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(p * l.powi(3) / (48.0 * e * i))
}

/// Simply supported beam, off-center point load at distance a from left support.
/// δ_max ≈ Pa(L²-a²)^(3/2) / (9√3 EIL) when a ≤ L/2
pub fn beam_simply_supported_offcenter_load(
    load: Force,
    span: Length,
    distance_a: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = span.value();
    let a = distance_a.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    let term = l * l - a * a;
    Length::m(p * a * term.powf(1.5) / (9.0 * 3.0_f64.sqrt() * e * i * l))
}

/// Simply supported beam, two equal point loads at L/3 and 2L/3.
/// δ_max = 23PL³ / 648EI
pub fn beam_simply_supported_two_point_loads(
    load: Force,
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(23.0 * p * l.powi(3) / (648.0 * e * i))
}

/// Simply supported beam, triangular load (zero at left, max at right).
/// δ_max = 0.01304 wL⁴ / EI (at x ≈ 0.5193L)
pub fn beam_simply_supported_triangular(
    max_load_per_length: f64, // N/m at right end
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(0.01304 * max_load_per_length * l.powi(4) / (e * i))
}

/// Cantilever beam, end point load.
/// δ_max = PL³ / 3EI
pub fn beam_cantilever_end_load(
    load: Force,
    length: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = length.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(p * l.powi(3) / (3.0 * e * i))
}

/// Cantilever beam, uniform load.
/// δ_max = wL⁴ / 8EI
pub fn beam_cantilever_uniform(
    load_per_length: f64, // N/m
    length: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let l = length.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(load_per_length * l.powi(4) / (8.0 * e * i))
}

/// Cantilever beam, point load at distance a from support.
/// δ_tip = Pa²(3L - a) / 6EI
pub fn beam_cantilever_intermediate_load(
    load: Force,
    length: Length,
    distance_a: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = length.value();
    let a = distance_a.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(p * a * a * (3.0 * l - a) / (6.0 * e * i))
}

/// Cantilever beam, end moment.
/// δ_max = ML² / 2EI
pub fn beam_cantilever_end_moment(
    moment: Torque,
    length: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let m = moment.value();
    let l = length.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(m * l * l / (2.0 * e * i))
}

/// Fixed-fixed beam, center point load.
/// δ_max = PL³ / 192EI
pub fn beam_fixed_fixed_center_load(
    load: Force,
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(p * l.powi(3) / (192.0 * e * i))
}

/// Fixed-fixed beam, uniform load.
/// δ_max = wL⁴ / 384EI
pub fn beam_fixed_fixed_uniform(
    load_per_length: f64, // N/m
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(load_per_length * l.powi(4) / (384.0 * e * i))
}

/// Fixed-pinned beam, uniform load.
/// δ_max = wL⁴ / (185 EI) at x ≈ 0.4215L
pub fn beam_fixed_pinned_uniform(
    load_per_length: f64, // N/m
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(load_per_length * l.powi(4) / (185.0 * e * i))
}

/// Fixed-pinned beam, center point load.
/// δ_max ≈ PL³ / (107 EI) at x ≈ 0.4472L
pub fn beam_fixed_pinned_center_load(
    load: Force,
    span: Length,
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
) -> Length {
    let p = load.value();
    let l = span.value();
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    Length::m(p * l.powi(3) / (107.0 * e * i))
}

// ---------------------------------------------------------------------------
// Beam stress formulas
// ---------------------------------------------------------------------------

/// Maximum bending stress: σ = Mc/I = M/S
pub fn bending_stress(moment: Torque, section_modulus: SectionModulus) -> Pressure {
    Pressure::pa(moment.value() / section_modulus.value())
}

/// Shear stress in beam: τ = VQ / (Ib) — simplified for rectangular: τ_max = 3V / 2A
pub fn shear_stress_rectangular(shear_force: Force, cross_section_area: Area) -> Pressure {
    Pressure::pa(1.5 * shear_force.value() / cross_section_area.value())
}

/// Combined bending + axial: σ = P/A ± Mc/I
pub fn combined_axial_bending(
    axial_force: Force,
    moment: Torque,
    area: Area,
    section_modulus: SectionModulus,
) -> (Pressure, Pressure) {
    let axial = axial_force.value() / area.value();
    let bending = moment.value() / section_modulus.value();
    (Pressure::pa(axial + bending), Pressure::pa(axial - bending))
}

/// Simply supported beam, center load: max bending moment M = PL/4
pub fn beam_ss_center_moment(load: Force, span: Length) -> Torque {
    Torque::new(load.value() * span.value() / 4.0)
}

/// Cantilever beam, end load: max bending moment at wall M = PL
pub fn beam_cantilever_end_moment_reaction(load: Force, length: Length) -> Torque {
    Torque::new(load.value() * length.value())
}

/// Cantilever beam, uniform load: max bending moment at wall M = wL²/2
pub fn beam_cantilever_uniform_moment(
    load_per_length: f64, // N/m
    length: Length,
) -> Torque {
    let l = length.value();
    Torque::new(load_per_length * l * l / 2.0)
}

// ---------------------------------------------------------------------------
// Section properties
// ---------------------------------------------------------------------------

/// Moment of inertia for rectangular section: I = bh³/12
pub fn rect_moment_of_inertia(width: Length, height: Length) -> MomentOfInertia {
    let b = width.value();
    let h = height.value();
    MomentOfInertia::new(b * h.powi(3) / 12.0)
}

/// Section modulus for rectangular section: S = bh²/6
pub fn rect_section_modulus(width: Length, height: Length) -> SectionModulus {
    let b = width.value();
    let h = height.value();
    SectionModulus::new(b * h * h / 6.0)
}

/// Moment of inertia for circular section: I = πd⁴/64
pub fn circle_moment_of_inertia(diameter: Length) -> MomentOfInertia {
    let d = diameter.value();
    MomentOfInertia::new(core::f64::consts::PI * d.powi(4) / 64.0)
}

/// Section modulus for circular section: S = πd³/32
pub fn circle_section_modulus(diameter: Length) -> SectionModulus {
    let d = diameter.value();
    SectionModulus::new(core::f64::consts::PI * d.powi(3) / 32.0)
}

/// Moment of inertia for hollow circular section: I = π(D⁴-d⁴)/64
pub fn hollow_circle_moment_of_inertia(
    outer_diameter: Length,
    inner_diameter: Length,
) -> MomentOfInertia {
    let d_o = outer_diameter.value();
    let d_i = inner_diameter.value();
    MomentOfInertia::new(core::f64::consts::PI * (d_o.powi(4) - d_i.powi(4)) / 64.0)
}

/// Moment of inertia for hollow rectangular (box) section.
/// I = (BH³ - bh³)/12 where B,H = outer, b,h = inner
pub fn hollow_rect_moment_of_inertia(
    outer_width: Length,
    outer_height: Length,
    inner_width: Length,
    inner_height: Length,
) -> MomentOfInertia {
    let bw = outer_width.value();
    let bh = outer_height.value();
    let iw = inner_width.value();
    let ih = inner_height.value();
    MomentOfInertia::new((bw * bh.powi(3) - iw * ih.powi(3)) / 12.0)
}

/// Area moment of inertia for I-beam (wide flange).
/// Approximate: I = (b*h³)/12 - (b-t_w)*(h-2*t_f)³/12
pub fn i_beam_moment_of_inertia(
    flange_width: Length,
    total_height: Length,
    web_thickness: Length,
    flange_thickness: Length,
) -> MomentOfInertia {
    let b = flange_width.value();
    let h = total_height.value();
    let tw = web_thickness.value();
    let tf = flange_thickness.value();
    let inner_h = h - 2.0 * tf;
    MomentOfInertia::new((b * h.powi(3) - (b - tw) * inner_h.powi(3)) / 12.0)
}

/// Polar moment of inertia for a solid circular shaft: J = πd⁴/32
pub fn polar_moment_solid(diameter: Length) -> MomentOfInertia {
    let d = diameter.value();
    MomentOfInertia::new(core::f64::consts::PI * d.powi(4) / 32.0)
}

/// Polar moment of inertia for a hollow circular shaft: J = π(D⁴-d⁴)/32
pub fn polar_moment_hollow(outer_diameter: Length, inner_diameter: Length) -> MomentOfInertia {
    let d_o = outer_diameter.value();
    let d_i = inner_diameter.value();
    MomentOfInertia::new(core::f64::consts::PI * (d_o.powi(4) - d_i.powi(4)) / 32.0)
}

/// Radius of gyration: r = √(I/A)
pub fn radius_of_gyration(moment_of_inertia: MomentOfInertia, area: Area) -> Length {
    Length::m((moment_of_inertia.value() / area.value()).sqrt())
}

// ---------------------------------------------------------------------------
// Peterson's stress concentration factors (Kt)
// ---------------------------------------------------------------------------

/// Stress concentration factor for a circular hole in a wide plate under tension.
/// Kt = 3.0 - 3.13(d/w) + 3.66(d/w)² - 1.53(d/w)³
/// Valid for d/w < 0.6
pub fn kt_hole_in_plate(hole_diameter: Length, plate_width: Length) -> Option<Dimensionless> {
    let ratio = hole_diameter.value() / plate_width.value();
    if ratio >= 0.6 || ratio < 0.0 {
        return None;
    }
    let kt = 3.0 - 3.13 * ratio + 3.66 * ratio.powi(2) - 1.53 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stress concentration factor for a shoulder fillet on a stepped shaft in tension.
/// Simplified for D/d = 1.5: C1=1.836, C2=-2.652, C3=3.202, C4=-1.386
/// Valid for 0.01 ≤ r/d ≤ 0.20
pub fn kt_shoulder_fillet_tension(
    fillet_radius: Length,
    small_diameter: Length,
) -> Option<Dimensionless> {
    let ratio = fillet_radius.value() / small_diameter.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.836 - 2.652 * ratio + 3.202 * ratio.powi(2) - 1.386 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stress concentration factor for a shoulder fillet in bending.
/// Simplified for D/d = 1.5: C1=1.674, C2=-1.862, C3=2.126, C4=-0.938
/// Valid for 0.01 ≤ r/d ≤ 0.20
pub fn kt_shoulder_fillet_bending(
    fillet_radius: Length,
    small_diameter: Length,
) -> Option<Dimensionless> {
    let ratio = fillet_radius.value() / small_diameter.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.674 - 1.862 * ratio + 2.126 * ratio.powi(2) - 0.938 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stress concentration factor for a shoulder fillet in torsion.
/// Simplified for D/d = 1.5: C1=1.553, C2=-2.374, C3=3.507, C4=-1.686
/// Valid for 0.01 ≤ r/d ≤ 0.20
pub fn kt_shoulder_fillet_torsion(
    fillet_radius: Length,
    small_diameter: Length,
) -> Option<Dimensionless> {
    let ratio = fillet_radius.value() / small_diameter.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.553 - 2.374 * ratio + 3.507 * ratio.powi(2) - 1.686 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stress concentration factor for a U-notch in tension.
/// Kt ≈ 1 + 2√(t/r) where t = notch depth, r = notch radius
/// (Neuber's approximation, valid for semi-circular and deep notches)
pub fn kt_u_notch(notch_depth: Length, notch_radius: Length) -> Dimensionless {
    let ratio = notch_depth.value() / notch_radius.value();
    Dimensionless::ratio(1.0 + 2.0 * ratio.sqrt())
}

/// Stress concentration factor for a transverse hole in a shaft under tension.
/// Kt = 3.0 - 3.72(d/D) + 5.88(d/D)² - 2.66(d/D)³
/// Valid for d/D < 0.5
pub fn kt_transverse_hole_shaft(
    hole_diameter: Length,
    shaft_diameter: Length,
) -> Option<Dimensionless> {
    let ratio = hole_diameter.value() / shaft_diameter.value();
    if ratio >= 0.5 || ratio < 0.0 {
        return None;
    }
    let kt = 3.0 - 3.72 * ratio + 5.88 * ratio.powi(2) - 2.66 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stress concentration for a keyway (standard profile).
/// Kt ≈ 2.14 for sled-runner, ≈ 3.0 for profiled end.
pub fn kt_keyway(profiled_end: bool) -> Dimensionless {
    if profiled_end {
        Dimensionless::ratio(3.0)
    } else {
        Dimensionless::ratio(2.14)
    }
}

/// Flat plate with central circular hole under tension.
/// Kt = 3.0 - 3.13(d/w) + 3.66(d/w)² - 1.53(d/w)³  (Peterson 4th ed., Fig. 4.1)
/// Valid for d/w < 0.6.
pub fn kt_plate_central_hole(width: Length, hole_diameter: Length) -> Option<Dimensionless> {
    let ratio = hole_diameter.value() / width.value();
    if !(0.0..0.6).contains(&ratio) {
        return None;
    }
    let kt = 3.0 - 3.13 * ratio + 3.66 * ratio.powi(2) - 1.53 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Edge notch (single-sided) in flat plate under tension.
/// Kt ≈ 1 + 2√(t/ρ) — Neuber notch approximation.
/// t = notch depth, ρ = notch root radius.  Valid for t/w ≤ 0.5.
pub fn kt_plate_edge_notch(
    width: Length,
    notch_depth: Length,
    notch_radius: Length,
) -> Option<Dimensionless> {
    let t = notch_depth.value();
    let w = width.value();
    let rho = notch_radius.value();
    if t / w > 0.5 || rho <= 0.0 {
        return None;
    }
    let kt = 1.0 + 2.0 * (t / rho).sqrt();
    Some(Dimensionless::ratio(kt))
}

/// Stepped shaft in tension — shoulder fillet (explicit big_d/small_d signature).
/// Polynomial fitted for D/d = 1.5.  Valid for 0.01 ≤ r/d ≤ 0.20.
/// (Peterson §3.3)
pub fn kt_shaft_shoulder_fillet(
    big_d: Length,
    small_d: Length,
    fillet_r: Length,
) -> Option<Dimensionless> {
    let _ = big_d; // ratio baked into polynomial for D/d = 1.5
    let ratio = fillet_r.value() / small_d.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.836 - 2.652 * ratio + 3.202 * ratio.powi(2) - 1.386 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stepped shaft in bending — shoulder fillet (explicit big_d/small_d signature).
/// Polynomial fitted for D/d = 1.5.  Valid for 0.01 ≤ r/d ≤ 0.20.
/// (Peterson §3.5)
pub fn kt_shaft_shoulder_fillet_bending(
    big_d: Length,
    small_d: Length,
    fillet_r: Length,
) -> Option<Dimensionless> {
    let _ = big_d;
    let ratio = fillet_r.value() / small_d.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.674 - 1.862 * ratio + 2.126 * ratio.powi(2) - 0.938 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Stepped shaft in torsion — shoulder fillet (explicit big_d/small_d signature).
/// Polynomial fitted for D/d = 1.5.  Valid for 0.01 ≤ r/d ≤ 0.20.
/// (Peterson §3.9)
pub fn kt_shaft_shoulder_fillet_torsion(
    big_d: Length,
    small_d: Length,
    fillet_r: Length,
) -> Option<Dimensionless> {
    let _ = big_d;
    let ratio = fillet_r.value() / small_d.value();
    if !(0.01..=0.20).contains(&ratio) {
        return None;
    }
    let kt = 1.553 - 2.374 * ratio + 3.507 * ratio.powi(2) - 1.686 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Shaft with a circumferential groove under tension.
/// Kt ≈ 1 + 2√(t/ρ) (Neuber) — groove_depth = t, groove_radius = ρ.
/// Valid for t/d ≤ 0.3.
pub fn kt_shaft_groove(
    diameter: Length,
    groove_depth: Length,
    groove_radius: Length,
) -> Option<Dimensionless> {
    let t = groove_depth.value();
    let d = diameter.value();
    let rho = groove_radius.value();
    if t / d > 0.3 || rho <= 0.0 {
        return None;
    }
    let kt = 1.0 + 2.0 * (t / rho).sqrt();
    Some(Dimensionless::ratio(kt))
}

/// Shaft with a transverse (diametral) hole under bending/tension.
/// Kt = 3.0 - 3.72(a/d) + 5.88(a/d)² - 2.66(a/d)³  (Peterson Fig. 4.44)
/// Valid for a/d < 0.5.
pub fn kt_shaft_transverse_hole(
    diameter: Length,
    hole_diameter: Length,
) -> Option<Dimensionless> {
    let ratio = hole_diameter.value() / diameter.value();
    if ratio >= 0.5 || ratio < 0.0 {
        return None;
    }
    let kt = 3.0 - 3.72 * ratio + 5.88 * ratio.powi(2) - 2.66 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

/// Keyway stress concentration in a shaft under torsion.
/// Kt ≈ 1 + 2√(h/ρ) where h = keyway depth, ρ = fillet radius.
/// For zero fillet radius, returns 3.0 (conservative per Peterson §8.3).
pub fn kt_keyway_torsion(
    shaft_diameter: Length,
    keyway_width: Length,
    keyway_depth: Length,
    fillet_r: Length,
) -> Option<Dimensionless> {
    let _ = shaft_diameter;
    let _ = keyway_width;
    let h = keyway_depth.value();
    let rho = fillet_r.value();
    if rho <= 0.0 {
        return Some(Dimensionless::ratio(3.0));
    }
    let kt = 1.0 + 2.0 * (h / rho).sqrt();
    Some(Dimensionless::ratio(kt.min(3.5)))
}

/// T-head (bolt head) fillet stress concentration under axial load.
/// Kt ≈ (1 + 2√(b/ρ)) × (b/t)^0.25 — approximate from Peterson Fig. 5.42.
/// b = half-flange overhang, t = shank thickness, ρ = fillet radius.
pub fn kt_t_head_fillet(
    width: Length,
    thickness: Length,
    fillet_r: Length,
) -> Option<Dimensionless> {
    let b = (width.value() - thickness.value()) / 2.0;
    let t = thickness.value();
    let rho = fillet_r.value();
    if b <= 0.0 || rho <= 0.0 || t <= 0.0 {
        return None;
    }
    let kt = (1.0 + 2.0 * (b / rho).sqrt()) * (b / t).powf(0.25);
    Some(Dimensionless::ratio(kt.max(1.0)))
}

/// Lug / clevis under pin bearing load.
/// Kt = 3.0 - 3.13(d/w) + 3.66(d/w)² - 1.53(d/w)³  (same form as hole-in-plate).
/// d = pin hole diameter, w = lug width.  Valid for d/w < 0.6.
pub fn kt_lug(hole_d: Length, width: Length, _thickness: Length) -> Option<Dimensionless> {
    let ratio = hole_d.value() / width.value();
    if !(0.0..0.6).contains(&ratio) {
        return None;
    }
    let kt = 3.0 - 3.13 * ratio + 3.66 * ratio.powi(2) - 1.53 * ratio.powi(3);
    Some(Dimensionless::ratio(kt))
}

// ---------------------------------------------------------------------------
// Fatigue
// ---------------------------------------------------------------------------

/// Goodman safety factor: n = 1 / (σ_a/S_e + σ_m/S_ut)
pub fn goodman_safety_factor(
    stress_amplitude: Pressure,
    mean_stress: Pressure,
    endurance_limit: Pressure,
    ultimate_tensile: Pressure,
) -> Option<Dimensionless> {
    let ratio = stress_amplitude.value() / endurance_limit.value()
        + mean_stress.value() / ultimate_tensile.value();
    if ratio <= 0.0 {
        return None;
    }
    Some(Dimensionless::ratio(1.0 / ratio))
}

/// Soderberg safety factor: n = 1 / (σ_a/S_e + σ_m/S_y)
/// More conservative than Goodman (uses yield instead of UTS).
pub fn soderberg_safety_factor(
    stress_amplitude: Pressure,
    mean_stress: Pressure,
    endurance_limit: Pressure,
    yield_strength: Pressure,
) -> Option<Dimensionless> {
    let ratio = stress_amplitude.value() / endurance_limit.value()
        + mean_stress.value() / yield_strength.value();
    if ratio <= 0.0 {
        return None;
    }
    Some(Dimensionless::ratio(1.0 / ratio))
}

/// Gerber safety factor: n = (S_ut/2σ_m)² [1 - √(1 - (2σ_m·σ_a)/(S_ut·S_e))]⁻¹
/// Less conservative than Goodman, better fits experimental data for ductile materials.
pub fn gerber_safety_factor(
    stress_amplitude: Pressure,
    mean_stress: Pressure,
    endurance_limit: Pressure,
    ultimate_tensile: Pressure,
) -> Option<Dimensionless> {
    let sa = stress_amplitude.value();
    let sm = mean_stress.value();
    let se = endurance_limit.value();
    let su = ultimate_tensile.value();
    if sm <= 0.0 || se <= 0.0 || su <= 0.0 {
        return None;
    }
    let term = 2.0 * sm * sa / (su * se);
    if term > 1.0 {
        return None;
    }
    let n = 0.5
        * (su / sm).powi(2)
        * (sa / se)
        * (-1.0 + (1.0 + (2.0 * sm * se / (su * sa)).powi(2)).sqrt());
    if n <= 0.0 {
        return None;
    }
    Some(Dimensionless::ratio(1.0 / n))
}

/// Marin factors for endurance limit correction.
/// S_e = k_a × k_b × k_c × k_d × k_e × S_e'
/// k_a = surface factor, k_b = size factor, k_c = load factor,
/// k_d = temperature factor, k_e = reliability factor
pub fn marin_surface_factor(surface_finish_ra_um: f64, ultimate_tensile: Pressure) -> Dimensionless {
    let s_ut_mpa = ultimate_tensile.value() * 1e-6;
    // Machined/cold-drawn: k_a = a × S_ut^b
    // a = 4.51 MPa, b = -0.265 for machined surfaces
    let ka = 4.51 * s_ut_mpa.powf(-0.265);
    // Adjust for rougher surfaces
    let adjustment = if surface_finish_ra_um > 6.3 {
        0.9 // rough machined
    } else if surface_finish_ra_um > 1.6 {
        1.0 // fine machined
    } else {
        1.05 // ground/polished
    };
    Dimensionless::ratio((ka * adjustment).min(1.0))
}

/// Marin size factor for rotating round bar.
/// k_b = 1.0 for d ≤ 8mm, (d/7.62)^-0.107 for 8-250mm
pub fn marin_size_factor(diameter: Length) -> Dimensionless {
    let d_mm = diameter.to_mm();
    let kb = if d_mm <= 8.0 {
        1.0
    } else if d_mm <= 250.0 {
        (d_mm / 7.62).powf(-0.107)
    } else {
        0.6 // very large
    };
    Dimensionless::ratio(kb)
}

/// S-N curve life estimation (Basquin's equation).
/// N = (σ_a / σ_f')^(1/b) where σ_f' ≈ S_ut and b ≈ -0.085 for steels
/// Returns estimated number of cycles to failure.
pub fn sn_life_cycles(stress_amplitude: Pressure, ultimate_tensile: Pressure) -> f64 {
    let sa = stress_amplitude.value();
    let sf = ultimate_tensile.value();
    if sa <= 0.0 || sa >= sf {
        return 0.0;
    }
    let b = -0.085; // typical for steels
    (sa / sf).powf(1.0 / b)
}

// ---------------------------------------------------------------------------
// Column buckling
// ---------------------------------------------------------------------------

/// Euler critical buckling load: P_cr = π²EI / (KL)²
/// K: 0.5 fixed-fixed, 0.7 fixed-pinned, 1.0 pinned-pinned, 2.0 fixed-free
pub fn euler_buckling_load(
    elastic_modulus: Pressure,
    moment_of_inertia: MomentOfInertia,
    length: Length,
    effective_length_factor: f64,
) -> Force {
    let e = elastic_modulus.value();
    let i = moment_of_inertia.value();
    let l = length.value();
    let kl = effective_length_factor * l;
    Force::n(core::f64::consts::PI.powi(2) * e * i / kl.powi(2))
}

/// Slenderness ratio: λ = KL/r where r = √(I/A)
pub fn slenderness_ratio(
    length: Length,
    effective_length_factor: f64,
    moment_of_inertia: MomentOfInertia,
    area: Area,
) -> f64 {
    let r = (moment_of_inertia.value() / area.value()).sqrt();
    effective_length_factor * length.value() / r
}

/// Slenderness ratio given radius of gyration directly: λ = KL / r
pub fn slenderness_ratio_rg(
    length: Length,
    k: f64,
    radius_of_gyration: Length,
) -> f64 {
    k * length.value() / radius_of_gyration.value()
}

/// Johnson's formula for intermediate columns (λ < λ_c).
/// P_cr = A × [S_y - (S_y² / (4π²E)) × (KL/r)²]
pub fn johnson_buckling_load(
    yield_strength: Pressure,
    elastic_modulus: Pressure,
    area: Area,
    slenderness: f64,
) -> Force {
    let sy = yield_strength.value();
    let e = elastic_modulus.value();
    let a = area.value();
    let stress =
        sy - (sy * sy * slenderness * slenderness) / (4.0 * core::f64::consts::PI.powi(2) * e);
    Force::n(a * stress.max(0.0))
}

/// Johnson parabolic buckling stress for short/intermediate columns.
/// σ_J = S_y × [1 - S_y / (4π²E) × λ²]   (Shigley's 11th ed. §4-12)
pub fn johnson_buckling_stress(
    yield_stress: Pressure,
    elastic_modulus: Pressure,
    slenderness: f64,
) -> Pressure {
    let sy = yield_stress.value();
    let e = elastic_modulus.value();
    let sigma =
        sy * (1.0 - sy * slenderness * slenderness / (4.0 * core::f64::consts::PI.powi(2) * e));
    Pressure::pa(sigma.max(0.0))
}

/// Euler–Johnson transition slenderness: λ_c = π √(2E / S_y)
/// Columns with λ > λ_c are Euler-critical; those with λ < λ_c are Johnson-critical.
/// (Shigley's 11th ed. §4-12)
pub fn transition_slenderness(elastic_modulus: Pressure, yield_stress: Pressure) -> f64 {
    core::f64::consts::PI * (2.0 * elastic_modulus.value() / yield_stress.value()).sqrt()
}

// ---------------------------------------------------------------------------
// Pressure vessels
// ---------------------------------------------------------------------------

/// Thin-wall hoop stress: σ_h = pr/t
pub fn thin_wall_hoop_stress(
    internal_pressure: Pressure,
    inner_radius: Length,
    wall_thickness: Length,
) -> Pressure {
    Pressure::pa(internal_pressure.value() * inner_radius.value() / wall_thickness.value())
}

/// Thin-wall axial stress: σ_a = pr/2t
pub fn thin_wall_axial_stress(
    internal_pressure: Pressure,
    inner_radius: Length,
    wall_thickness: Length,
) -> Pressure {
    Pressure::pa(
        internal_pressure.value() * inner_radius.value() / (2.0 * wall_thickness.value()),
    )
}

/// Thick-wall (Lamé) hoop stress at inner surface:
/// σ_h = p(r_o² + r_i²) / (r_o² - r_i²)
pub fn thick_wall_hoop_stress(
    internal_pressure: Pressure,
    inner_radius: Length,
    outer_radius: Length,
) -> Pressure {
    let ri = inner_radius.value();
    let ro = outer_radius.value();
    let p = internal_pressure.value();
    Pressure::pa(p * (ro * ro + ri * ri) / (ro * ro - ri * ri))
}

/// Thick-wall radial stress at inner surface: σ_r = -p
pub fn thick_wall_radial_stress(internal_pressure: Pressure) -> Pressure {
    Pressure::pa(-internal_pressure.value())
}

/// Lamé thick-wall hoop stress at arbitrary radius r.
/// σ_θ = (p_i r_i²)/(r_o²-r_i²) × (1 + r_o²/r²)
/// (Roark 9th ed. Table 13.7)
pub fn thick_wall_hoop_at_r(
    internal_pressure: Pressure,
    inner_r: Length,
    outer_r: Length,
    r: Length,
) -> Pressure {
    let p = internal_pressure.value();
    let ri = inner_r.value();
    let ro = outer_r.value();
    let rv = r.value();
    let sigma = p * ri * ri / (ro * ro - ri * ri) * (1.0 + ro * ro / (rv * rv));
    Pressure::pa(sigma)
}

/// Lamé thick-wall radial stress at arbitrary radius r.
/// σ_r = (p_i r_i²)/(r_o²-r_i²) × (1 - r_o²/r²)
/// (Roark 9th ed. Table 13.7)
pub fn thick_wall_radial_at_r(
    internal_pressure: Pressure,
    inner_r: Length,
    outer_r: Length,
    r: Length,
) -> Pressure {
    let p = internal_pressure.value();
    let ri = inner_r.value();
    let ro = outer_r.value();
    let rv = r.value();
    let sigma = p * ri * ri / (ro * ro - ri * ri) * (1.0 - ro * ro / (rv * rv));
    Pressure::pa(sigma)
}

/// Thin-wall sphere hoop stress: σ = pR/(2t)
/// (Roark 9th ed. Table 13.1)
pub fn sphere_hoop_stress(pressure: Pressure, radius: Length, thickness: Length) -> Pressure {
    Pressure::pa(pressure.value() * radius.value() / (2.0 * thickness.value()))
}

// ---------------------------------------------------------------------------
// Torsion
// ---------------------------------------------------------------------------

/// Torsional shear stress in solid shaft: τ = 16T / (πd³)
pub fn torsion_shear_stress_solid(torque: Torque, diameter: Length) -> Pressure {
    let d = diameter.value();
    Pressure::pa(16.0 * torque.value() / (core::f64::consts::PI * d.powi(3)))
}

/// Torsional shear stress in hollow shaft: τ = 16Td_o / (π(d_o⁴ - d_i⁴))
pub fn torsion_shear_stress_hollow(
    torque: Torque,
    outer_diameter: Length,
    inner_diameter: Length,
) -> Pressure {
    let d_o = outer_diameter.value();
    let d_i = inner_diameter.value();
    Pressure::pa(
        16.0 * torque.value() * d_o / (core::f64::consts::PI * (d_o.powi(4) - d_i.powi(4))),
    )
}

/// Angle of twist: θ = TL / GJ
/// J = πd⁴/32 for solid shaft
pub fn angle_of_twist_solid(
    torque: Torque,
    length: Length,
    shear_modulus: Pressure,
    diameter: Length,
) -> Angle {
    let j = core::f64::consts::PI * diameter.value().powi(4) / 32.0;
    Angle::rad(torque.value() * length.value() / (shear_modulus.value() * j))
}

// ---------------------------------------------------------------------------
// Shaft design helpers
// ---------------------------------------------------------------------------

/// Shaft power–torque–speed relationship: T = P × 60 / (2π × rpm)
pub fn shaft_torque(power: Power, rpm: f64) -> Torque {
    Torque::new(power.value() * 60.0 / (2.0 * core::f64::consts::PI * rpm))
}

/// Shaft diameter from allowable shear stress (solid round shaft in torsion).
/// d = (16T / (π τ_allow))^(1/3)
pub fn shaft_diameter_from_torque(torque: Torque, allowable_shear: Pressure) -> Length {
    let t = torque.value();
    let tau = allowable_shear.value();
    Length::m((16.0 * t / (core::f64::consts::PI * tau)).powf(1.0 / 3.0))
}

/// Critical speed of a shaft (Rayleigh–Ritz, single concentrated mass).
/// ω_c = √(g/δ) where δ = static deflection (m).  Returns rad/s.
pub fn shaft_critical_speed(static_deflection: Length) -> f64 {
    (9.80665 / static_deflection.value()).sqrt()
}

// ---------------------------------------------------------------------------
// Contact stress (Hertz)
// ---------------------------------------------------------------------------

/// Combined elastic modulus for two bodies in contact.
/// 1/E* = (1-ν₁²)/E₁ + (1-ν₂²)/E₂
#[inline]
fn hertz_combined_modulus(e1: f64, e2: f64, nu1: f64, nu2: f64) -> f64 {
    1.0 / ((1.0 - nu1 * nu1) / e1 + (1.0 - nu2 * nu2) / e2)
}

/// Hertz contact stress between two parallel cylinders (line contact).
/// p_max = √(F × E* / (π × L × R*))
/// where E* = combined modulus, R* = combined radius
pub fn hertz_cylinder_contact_stress(
    force_per_length: f64, // N/m
    radius_1: Length,
    radius_2: Length,
    elastic_modulus_1: Pressure,
    elastic_modulus_2: Pressure,
    poissons_1: f64,
    poissons_2: f64,
) -> Pressure {
    let e1 = elastic_modulus_1.value();
    let e2 = elastic_modulus_2.value();
    let r1 = radius_1.value();
    let r2 = radius_2.value();
    let e_star = hertz_combined_modulus(e1, e2, poissons_1, poissons_2);
    let r_star = 1.0 / (1.0 / r1 + 1.0 / r2);
    let p_max = (force_per_length * e_star / (core::f64::consts::PI * r_star)).sqrt();
    Pressure::pa(p_max)
}

/// Hertz contact stress between two spheres (point contact).
/// p_max = (6F × E*² / (π³ × R*²))^(1/3)
pub fn hertz_sphere_contact_stress(
    force: Force,
    radius_1: Length,
    radius_2: Length,
    elastic_modulus_1: Pressure,
    elastic_modulus_2: Pressure,
    poissons_1: f64,
    poissons_2: f64,
) -> Pressure {
    let e1 = elastic_modulus_1.value();
    let e2 = elastic_modulus_2.value();
    let r1 = radius_1.value();
    let r2 = radius_2.value();
    let e_star = hertz_combined_modulus(e1, e2, poissons_1, poissons_2);
    let r_star = 1.0 / (1.0 / r1 + 1.0 / r2);
    let pi = core::f64::consts::PI;
    let p_max = (6.0 * force.value() * e_star * e_star
        / (pi.powi(3) * r_star * r_star))
        .powf(1.0 / 3.0);
    Pressure::pa(p_max)
}

/// Sphere on flat plane (plane = sphere with R₂ → ∞).
/// Contact radius a = (3F R / 4E*)^(1/3)
/// Max pressure  p₀ = 3F / (2πa²)
/// Returns (contact_radius, max_pressure).  (Johnson Contact Mechanics §4.2)
pub fn hertz_sphere_on_plane(
    force: Force,
    sphere_r: Length,
    e1: Pressure,
    e2: Pressure,
    nu1: f64,
    nu2: f64,
) -> (Length, Pressure) {
    let f = force.value();
    let r = sphere_r.value();
    let e_star = hertz_combined_modulus(e1.value(), e2.value(), nu1, nu2);
    let a = (3.0 * f * r / (4.0 * e_star)).powf(1.0 / 3.0);
    let p0 = 3.0 * f / (2.0 * core::f64::consts::PI * a * a);
    (Length::m(a), Pressure::pa(p0))
}

/// Two spheres in point contact.
/// R* = R₁R₂/(R₁+R₂)
/// Contact radius a = (3F R* / 4E*)^(1/3)
/// Max pressure  p₀ = 3F / (2πa²)
/// Returns (contact_radius, max_pressure).
pub fn hertz_sphere_on_sphere(
    force: Force,
    r1: Length,
    r2: Length,
    e1: Pressure,
    e2: Pressure,
    nu1: f64,
    nu2: f64,
) -> (Length, Pressure) {
    let f = force.value();
    let r_star = 1.0 / (1.0 / r1.value() + 1.0 / r2.value());
    let e_star = hertz_combined_modulus(e1.value(), e2.value(), nu1, nu2);
    let a = (3.0 * f * r_star / (4.0 * e_star)).powf(1.0 / 3.0);
    let p0 = 3.0 * f / (2.0 * core::f64::consts::PI * a * a);
    (Length::m(a), Pressure::pa(p0))
}

/// Cylinder on flat plane (line contact).
/// Half-contact-width b = √(4 q R / (π E*))
/// Max pressure p₀ = 2q / (π b)
/// `force_per_length` is F/L in N/m.  Returns (half_width, max_pressure).
pub fn hertz_cylinder_on_plane(
    force_per_length: f64, // N/m
    cyl_r: Length,
    e1: Pressure,
    e2: Pressure,
    nu1: f64,
    nu2: f64,
) -> (Length, Pressure) {
    let r = cyl_r.value();
    let e_star = hertz_combined_modulus(e1.value(), e2.value(), nu1, nu2);
    let b = (4.0 * force_per_length * r / (core::f64::consts::PI * e_star)).sqrt();
    let p0 = 2.0 * force_per_length / (core::f64::consts::PI * b);
    (Length::m(b), Pressure::pa(p0))
}

/// Two parallel cylinders in line contact.
/// R* = R₁R₂/(R₁+R₂)
/// Half-contact-width b = √(4 q R* / (π E*))  where q = F/L (N/m).
/// Max pressure p₀ = 2q / (π b).
/// Returns (half_width, max_pressure).
pub fn hertz_cylinder_on_cylinder(
    force_per_length: f64, // N/m
    r1: Length,
    r2: Length,
    e1: Pressure,
    e2: Pressure,
    nu1: f64,
    nu2: f64,
) -> (Length, Pressure) {
    let r_star = 1.0 / (1.0 / r1.value() + 1.0 / r2.value());
    let e_star = hertz_combined_modulus(e1.value(), e2.value(), nu1, nu2);
    let b = (4.0 * force_per_length * r_star / (core::f64::consts::PI * e_star)).sqrt();
    let p0 = 2.0 * force_per_length / (core::f64::consts::PI * b);
    (Length::m(b), Pressure::pa(p0))
}

// ---------------------------------------------------------------------------
// Spring design (Machinery's Handbook)
// ---------------------------------------------------------------------------

/// Compression spring rate: k = Gd⁴ / (8D³N_a)
/// where d = wire dia, D = mean coil dia, N_a = active coils
pub fn compression_spring_rate(
    wire_diameter: Length,
    mean_coil_diameter: Length,
    active_coils: f64,
    shear_modulus: Pressure,
) -> f64 {
    let d = wire_diameter.value();
    let big_d = mean_coil_diameter.value();
    let g = shear_modulus.value();
    g * d.powi(4) / (8.0 * big_d.powi(3) * active_coils)
}

/// Wahl correction factor for spring stress.
/// K_w = (4C-1)/(4C-4) + 0.615/C where C = D/d
pub fn wahl_factor(spring_index: f64) -> f64 {
    let c = spring_index;
    (4.0 * c - 1.0) / (4.0 * c - 4.0) + 0.615 / c
}

/// Spring shear stress (corrected): τ = K_w × 8FD / (πd³)
pub fn spring_shear_stress(
    force: Force,
    wire_diameter: Length,
    mean_coil_diameter: Length,
) -> Pressure {
    let d = wire_diameter.value();
    let big_d = mean_coil_diameter.value();
    let c = big_d / d;
    let kw = wahl_factor(c);
    Pressure::pa(kw * 8.0 * force.value() * big_d / (core::f64::consts::PI * d.powi(3)))
}

/// Extension spring initial tension range (typical).
/// Returns (min, max) force in N based on spring index.
pub fn extension_spring_initial_tension_range(
    wire_diameter: Length,
    mean_coil_diameter: Length,
    shear_modulus: Pressure,
) -> (Force, Force) {
    let d = wire_diameter.value();
    let big_d = mean_coil_diameter.value();
    let c = big_d / d;
    let g = shear_modulus.value();
    // Empirical: typical initial stress = 10-30% of allowable
    let base_stress = g * d / (8.0 * big_d);
    let min_factor = 0.05 / c;
    let max_factor = 0.15 / c;
    (
        Force::n(base_stress * min_factor * core::f64::consts::PI * d * d),
        Force::n(base_stress * max_factor * core::f64::consts::PI * d * d),
    )
}

/// Torsion spring rate: k_θ = Ed⁴ / (10.186 × D × N_a) [N·m/rad]
pub fn torsion_spring_rate(
    wire_diameter: Length,
    mean_coil_diameter: Length,
    active_coils: f64,
    elastic_modulus: Pressure,
) -> f64 {
    let d = wire_diameter.value();
    let big_d = mean_coil_diameter.value();
    let e = elastic_modulus.value();
    e * d.powi(4) / (10.186 * big_d * active_coils)
}

// ---------------------------------------------------------------------------
// Gear geometry (Machinery's Handbook)
// ---------------------------------------------------------------------------

/// Spur gear pitch diameter from module: d = m × z
pub fn gear_pitch_diameter_metric(module_mm: f64, teeth: u32) -> Length {
    Length::mm(module_mm * teeth as f64)
}

/// Spur gear pitch diameter from diametral pitch: d = z / P (inches)
pub fn gear_pitch_diameter_imperial(diametral_pitch: f64, teeth: u32) -> Length {
    Length::inch(teeth as f64 / diametral_pitch)
}

/// Gear center distance for meshing pair: a = (d1 + d2) / 2
pub fn gear_center_distance(pitch_dia_1: Length, pitch_dia_2: Length) -> Length {
    Length::m((pitch_dia_1.value() + pitch_dia_2.value()) / 2.0)
}

/// Gear contact ratio (approximate for spur gears with standard 20° pressure angle).
/// Simplified: CR ≈ 1.88 - 3.2(1/z1 + 1/z2) for standard gears
pub fn gear_contact_ratio(teeth_1: u32, teeth_2: u32) -> f64 {
    let z1 = teeth_1 as f64;
    let z2 = teeth_2 as f64;
    1.88 - 3.2 * (1.0 / z1 + 1.0 / z2)
}

/// Lewis beam strength for spur gear tooth.
/// σ = Wt / (F × m × Y) where Y = Lewis form factor
pub fn gear_lewis_stress(
    tangential_force: Force,
    face_width: Length,
    module_mm: f64,
    lewis_form_factor: f64,
) -> Pressure {
    let m = module_mm * 1e-3; // to meters
    Pressure::pa(tangential_force.value() / (face_width.value() * m * lewis_form_factor))
}

/// Lewis form factor Y for 20° pressure angle (approximate).
/// Y ≈ 0.484 - 2.87/z + 0.0555/z² (Machinery's Handbook interpolation)
pub fn lewis_form_factor(teeth: u32) -> f64 {
    let z = teeth as f64;
    (0.484 - 2.87 / z + 0.0555 / (z * z)).max(0.05)
}

// ---------------------------------------------------------------------------
// Bearing life (Machinery's Handbook)
// ---------------------------------------------------------------------------

/// Basic bearing life (L10) in millions of revolutions.
/// L10 = (C/P)^p where C = dynamic capacity, P = equivalent load, p = 3 for ball, 10/3 for roller
pub fn bearing_l10_life(dynamic_capacity: Force, equivalent_load: Force, is_ball: bool) -> f64 {
    let ratio = dynamic_capacity.value() / equivalent_load.value();
    if is_ball {
        ratio.powi(3)
    } else {
        ratio.powf(10.0 / 3.0)
    }
}

/// Bearing life in hours at given RPM.
/// L10h = L10 × 1_000_000 / (60 × n)
pub fn bearing_l10_hours(l10_millions_rev: f64, rpm: f64) -> f64 {
    l10_millions_rev * 1_000_000.0 / (60.0 * rpm)
}

/// Equivalent bearing load for combined radial+axial.
/// P = X×Fr + Y×Fa (simplified: X=1.0, Y=0 for light axial; X=0.56, Y=1.63 for heavy axial)
pub fn bearing_equivalent_load(
    radial_force: Force,
    axial_force: Force,
    heavy_axial: bool,
) -> Force {
    if heavy_axial {
        Force::n(0.56 * radial_force.value() + 1.63 * axial_force.value())
    } else {
        radial_force
    }
}

// ---------------------------------------------------------------------------
// Belt/chain power transmission
// ---------------------------------------------------------------------------

/// V-belt power capacity per belt (approximate for B-section belt).
/// P = (T1 - T2) × v where T1/T2 = tight/slack side tensions
/// Simplified: P ≈ 3.3 × v × (1 - e^(-μθ)) for B-section at 1750 rpm
pub fn belt_power_capacity(
    belt_speed: Velocity,
    friction_coefficient: f64,
    wrap_angle: Angle,
) -> Power {
    let v = belt_speed.value();
    let e_ratio = (friction_coefficient * wrap_angle.value()).exp();
    let tension_ratio = (e_ratio - 1.0) / e_ratio;
    // Approximate max tension for B-section belt: ~1000N
    let t_max = 1000.0;
    Power::new(t_max * tension_ratio * v)
}

// ---------------------------------------------------------------------------
// Heat transfer
// ---------------------------------------------------------------------------

/// Convective heat transfer: Q = hA(T_s - T_∞)
pub fn convective_heat_transfer(
    h: f64, // W/(m²·K) convection coefficient
    area: Area,
    surface_temp: Temperature,
    ambient_temp: Temperature,
) -> Power {
    Power::new(h * area.value() * (surface_temp.value() - ambient_temp.value()))
}

/// Conductive heat transfer through a flat wall: Q = kA(T1-T2)/L
pub fn conductive_heat_transfer(
    conductivity: ThermalConductivity,
    area: Area,
    thickness: Length,
    temp_hot: Temperature,
    temp_cold: Temperature,
) -> Power {
    let dt = temp_hot.value() - temp_cold.value();
    Power::new(conductivity.value() * area.value() * dt / thickness.value())
}

/// Radiative heat transfer: Q = εσA(T_s⁴ - T_surr⁴)
/// σ = 5.670374419e-8 W/(m²·K⁴)
pub fn radiative_heat_transfer(
    emissivity: f64,
    area: Area,
    surface_temp: Temperature,
    surroundings_temp: Temperature,
) -> Power {
    const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
    let ts = surface_temp.value();
    let tsurr = surroundings_temp.value();
    Power::new(emissivity * STEFAN_BOLTZMANN * area.value() * (ts.powi(4) - tsurr.powi(4)))
}

/// Thermal resistance of a flat wall: R = L / (kA)
pub fn thermal_resistance_flat(
    thickness: Length,
    conductivity: ThermalConductivity,
    area: Area,
) -> f64 {
    thickness.value() / (conductivity.value() * area.value())
}

/// Thermal resistance of a cylindrical wall: R = ln(r_o/r_i) / (2πkL)
pub fn thermal_resistance_cylinder(
    inner_radius: Length,
    outer_radius: Length,
    conductivity: ThermalConductivity,
    length: Length,
) -> f64 {
    let ri = inner_radius.value();
    let ro = outer_radius.value();
    (ro / ri).ln() / (2.0 * core::f64::consts::PI * conductivity.value() * length.value())
}

/// Fin efficiency for a rectangular fin.
/// η = tanh(mL) / (mL) where m = √(2h / kt)
pub fn fin_efficiency_rectangular(
    convection_h: f64, // W/(m²·K)
    conductivity: ThermalConductivity,
    fin_thickness: Length,
    fin_length: Length,
) -> f64 {
    let m = (2.0 * convection_h / (conductivity.value() * fin_thickness.value())).sqrt();
    let ml = m * fin_length.value();
    if ml < 1e-6 {
        1.0
    } else {
        ml.tanh() / ml
    }
}

/// Natural convection coefficient (vertical plate, simplified).
/// h ≈ 1.42 × (ΔT/L)^0.25 for laminar (Ra < 10⁹), air at ~25°C
pub fn natural_convection_vertical_plate(delta_t: f64, height: Length) -> f64 {
    1.42 * (delta_t / height.value()).powf(0.25)
}

/// Forced convection over flat plate (laminar, Re_L < 5×10⁵).
/// Nu = 0.664 × Re^0.5 × Pr^(1/3)
/// h = Nu × k / L
pub fn forced_convection_flat_plate_laminar(
    velocity: Velocity,
    plate_length: Length,
    fluid_density: Density,
    fluid_viscosity: f64,    // Pa·s (dynamic viscosity)
    fluid_conductivity: f64, // W/(m·K)
    fluid_prandtl: f64,
) -> f64 {
    let re = fluid_density.value() * velocity.value() * plate_length.value() / fluid_viscosity;
    let nu = 0.664 * re.sqrt() * fluid_prandtl.powf(1.0 / 3.0);
    nu * fluid_conductivity / plate_length.value()
}

// ---------------------------------------------------------------------------
// Fluid mechanics
// ---------------------------------------------------------------------------

/// Reynolds number: Re = ρvD/μ
pub fn reynolds_number(
    density: Density,
    velocity: Velocity,
    hydraulic_diameter: Length,
    dynamic_viscosity: f64, // Pa·s
) -> f64 {
    density.value() * velocity.value() * hydraulic_diameter.value() / dynamic_viscosity
}

/// Pressure drop in pipe (Darcy-Weisbach): ΔP = f × (L/D) × (ρv²/2)
pub fn pipe_pressure_drop(
    friction_factor: f64,
    pipe_length: Length,
    hydraulic_diameter: Length,
    density: Density,
    velocity: Velocity,
) -> Pressure {
    let dp = friction_factor * (pipe_length.value() / hydraulic_diameter.value())
        * 0.5
        * density.value()
        * velocity.value().powi(2);
    Pressure::pa(dp)
}

/// Moody friction factor (Swamee-Jain explicit approximation).
/// f = 0.25 / [log₁₀(ε/(3.7D) + 5.74/Re^0.9)]²
pub fn moody_friction_factor(
    roughness: Length,
    hydraulic_diameter: Length,
    reynolds: f64,
) -> f64 {
    if reynolds < 2300.0 {
        // Laminar flow
        return 64.0 / reynolds;
    }
    let eps_d = roughness.value() / hydraulic_diameter.value();
    let term = eps_d / 3.7 + 5.74 / reynolds.powf(0.9);
    0.25 / term.log10().powi(2)
}

/// Bernoulli's equation: P1 + 0.5ρv1² + ρgh1 = P2 + 0.5ρv2² + ρgh2
/// Returns P2 given other quantities.
pub fn bernoulli_pressure(
    p1: Pressure,
    v1: Velocity,
    h1: Length,
    v2: Velocity,
    h2: Length,
    density: Density,
) -> Pressure {
    let rho = density.value();
    let g = 9.80665;
    let p2 = p1.value()
        + 0.5 * rho * (v1.value().powi(2) - v2.value().powi(2))
        + rho * g * (h1.value() - h2.value());
    Pressure::pa(p2)
}

// ---------------------------------------------------------------------------
// Von Mises yield criterion
// ---------------------------------------------------------------------------

/// Von Mises equivalent stress (plane stress):
/// σ_vm = √(σ_x² - σ_x·σ_y + σ_y² + 3τ_xy²)
pub fn von_mises_plane_stress(
    sigma_x: Pressure,
    sigma_y: Pressure,
    tau_xy: Pressure,
) -> Pressure {
    let sx = sigma_x.value();
    let sy = sigma_y.value();
    let txy = tau_xy.value();
    let vm = (sx * sx - sx * sy + sy * sy + 3.0 * txy * txy).sqrt();
    Pressure::pa(vm)
}

/// Von Mises safety factor: n = S_y / σ_vm
pub fn von_mises_safety_factor(
    yield_strength: Pressure,
    von_mises_stress: Pressure,
) -> Dimensionless {
    Dimensionless::ratio(yield_strength.value() / von_mises_stress.value())
}

// ---------------------------------------------------------------------------
// Thermal expansion
// ---------------------------------------------------------------------------

/// Linear thermal expansion: ΔL = α × L × ΔT
pub fn thermal_expansion(
    cte: CTE,
    original_length: Length,
    delta_temperature: f64, // Kelvin or °C (same magnitude)
) -> Length {
    Length::m(cte.value() * original_length.value() * delta_temperature)
}

/// Thermal stress when expansion is constrained: σ = E × α × ΔT
pub fn thermal_stress(
    elastic_modulus: Pressure,
    cte: CTE,
    delta_temperature: f64,
) -> Pressure {
    Pressure::pa(elastic_modulus.value() * cte.value() * delta_temperature)
}

// ---------------------------------------------------------------------------
// Heat exchanger analysis
// ---------------------------------------------------------------------------

/// Number of Transfer Units: NTU = UA / C_min
pub fn heat_exchanger_ntu(ua: f64, c_min: f64) -> f64 {
    ua / c_min
}

/// Effectiveness of a counterflow heat exchanger.
/// ε = (1 - exp(-NTU(1 - C_r))) / (1 - C_r·exp(-NTU(1 - C_r)))
/// For C_r = 1: ε = NTU / (1 + NTU)
pub fn heat_exchanger_effectiveness_counterflow(ntu: f64, c_ratio: f64) -> f64 {
    if (c_ratio - 1.0).abs() < 1e-10 {
        ntu / (1.0 + ntu)
    } else {
        let e = (-ntu * (1.0 - c_ratio)).exp();
        (1.0 - e) / (1.0 - c_ratio * e)
    }
}

/// Effectiveness of a parallel-flow heat exchanger.
/// ε = (1 - exp(-NTU(1 + C_r))) / (1 + C_r)
pub fn heat_exchanger_effectiveness_parallel(ntu: f64, c_ratio: f64) -> f64 {
    (1.0 - (-ntu * (1.0 + c_ratio)).exp()) / (1.0 + c_ratio)
}

/// Effectiveness of a 1-shell, 2-tube-pass heat exchanger.
/// ε = 2 / (1 + C_r + √(1+C_r²) · coth(NTU·√(1+C_r²)/2))
/// coth(x) = (e^x + e^-x)/(e^x - e^-x)
pub fn heat_exchanger_effectiveness_shell_tube(ntu: f64, c_ratio: f64) -> f64 {
    let cr2 = c_ratio * c_ratio;
    let factor = (1.0 + cr2).sqrt();
    let arg = ntu * factor / 2.0;
    let coth = if arg > 20.0 {
        1.0
    } else {
        let e_pos = arg.exp();
        let e_neg = (-arg).exp();
        (e_pos + e_neg) / (e_pos - e_neg)
    };
    2.0 / (1.0 + c_ratio + factor * coth)
}

/// Log Mean Temperature Difference: LMTD = (ΔT₁ - ΔT₂) / ln(ΔT₁/ΔT₂)
/// When ΔT₁ ≈ ΔT₂, returns ΔT₁ (L'Hôpital).
pub fn heat_exchanger_lmtd(dt1: f64, dt2: f64) -> f64 {
    if (dt1 - dt2).abs() < 1e-10 {
        dt1
    } else {
        (dt1 - dt2) / (dt1 / dt2).ln()
    }
}

/// Heat transfer rate from the NTU-effectiveness method:
/// Q = ε · C_min · (T_hot,in - T_cold,in)
pub fn heat_exchanger_q_ntu(
    c_min: f64,
    effectiveness: f64,
    t_hot_in: Temperature,
    t_cold_in: Temperature,
) -> Power {
    Power::new(effectiveness * c_min * (t_hot_in.value() - t_cold_in.value()))
}

// ---------------------------------------------------------------------------
// Nusselt number correlations
// ---------------------------------------------------------------------------

/// Dittus-Boelter correlation for turbulent flow in a pipe (heating).
/// Nu = 0.023 · Re^0.8 · Pr^0.4
pub fn nusselt_pipe_turbulent(re: f64, pr: f64) -> f64 {
    0.023 * re.powf(0.8) * pr.powf(0.4)
}

/// Fully developed laminar pipe flow, constant wall temperature.
/// Nu = 3.66
pub fn nusselt_pipe_laminar_constant_wall_temp() -> f64 {
    3.66
}

/// Fully developed laminar pipe flow, constant heat flux.
/// Nu = 4.36
pub fn nusselt_pipe_laminar_constant_heat_flux() -> f64 {
    4.36
}

/// Churchill-Bernstein correlation for crossflow over a cylinder.
/// Nu = 0.3 + (0.62·Re^0.5·Pr^(1/3)) / (1 + (0.4/Pr)^(2/3))^0.25
///     × (1 + (Re/282000)^(5/8))^(4/5)
pub fn nusselt_cylinder_crossflow(re: f64, pr: f64) -> f64 {
    let num = 0.62 * re.sqrt() * pr.powf(1.0 / 3.0);
    let denom = (1.0 + (0.4 / pr).powf(2.0 / 3.0)).powf(0.25);
    let correction = (1.0 + (re / 282_000.0).powf(5.0 / 8.0)).powf(4.0 / 5.0);
    0.3 + (num / denom) * correction
}

/// Whitaker correlation for flow over a sphere.
/// Nu = 2 + (0.4·Re^0.5 + 0.06·Re^(2/3)) · Pr^0.4
pub fn nusselt_sphere(re: f64, pr: f64) -> f64 {
    2.0 + (0.4 * re.sqrt() + 0.06 * re.powf(2.0 / 3.0)) * pr.powf(0.4)
}

/// Convection coefficient from Nusselt number for pipe flow.
/// h = Nu · k / D
pub fn forced_convection_pipe(nusselt: f64, fluid_k: f64, diameter: Length) -> f64 {
    nusselt * fluid_k / diameter.value()
}

// ---------------------------------------------------------------------------
// Fin enhancements
// ---------------------------------------------------------------------------

/// Annular fin efficiency.
/// η = (2r_i / (m(r_o² - r_i²))) · (I₁(mr_o)K₁(mr_i) - K₁(mr_o)I₁(mr_i))
///   / (I₀(mr_i)K₁(mr_o) + K₀(mr_i)I₁(mr_o))
///
/// Uses the approximate method: η ≈ tanh(m·L_c·φ) / (m·L_c·φ)
/// where L_c = r_o - r_i + t/2, φ = (r_o/r_i - 1)(1 + 0.35·ln(r_o/r_i))
pub fn fin_efficiency_annular(
    convection_h: f64,
    conductivity: ThermalConductivity,
    thickness: Length,
    r_inner: Length,
    r_outer: Length,
) -> f64 {
    let ri = r_inner.value();
    let ro = r_outer.value();
    let t = thickness.value();
    let m = (2.0 * convection_h / (conductivity.value() * t)).sqrt();
    let lc = ro - ri + t / 2.0;
    let ratio = ro / ri;
    let phi = (ratio - 1.0) * (1.0 + 0.35 * ratio.ln());
    let ml = m * lc * phi;
    if ml < 1e-6 {
        1.0
    } else {
        ml.tanh() / ml
    }
}

/// Overall surface efficiency with fins.
/// η_o = 1 - (n·A_f / A_t) · (1 - η_f)
pub fn overall_surface_efficiency(
    n_fins: usize,
    fin_efficiency: f64,
    a_fin: f64,
    a_total: f64,
) -> f64 {
    1.0 - (n_fins as f64 * a_fin / a_total) * (1.0 - fin_efficiency)
}

/// Heat transfer rate from a fin.
/// Q = η_f · h · A_fin · (T_base - T_ambient)
pub fn fin_heat_rate(
    h: f64,
    eta_fin: f64,
    fin_area: Area,
    t_base: Temperature,
    t_ambient: Temperature,
) -> Power {
    Power::new(eta_fin * h * fin_area.value() * (t_base.value() - t_ambient.value()))
}

// ---------------------------------------------------------------------------
// Thermal network
// ---------------------------------------------------------------------------

/// Convective thermal resistance: R = 1 / (h · A)
pub fn thermal_resistance_convection(h: f64, area: Area) -> f64 {
    1.0 / (h * area.value())
}

/// Linearized radiation thermal resistance.
/// R_rad = 1 / (ε · σ · A · (T_s² + T_surr²)(T_s + T_surr))
pub fn thermal_resistance_radiation(
    emissivity: f64,
    area: Area,
    surface_temp: Temperature,
    surr_temp: Temperature,
) -> f64 {
    const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
    let ts = surface_temp.value();
    let tsurr = surr_temp.value();
    let h_rad = emissivity * STEFAN_BOLTZMANN * (ts * ts + tsurr * tsurr) * (ts + tsurr);
    1.0 / (h_rad * area.value())
}

/// Series thermal resistance: R_total = ΣR
pub fn thermal_resistance_series(resistances: &[f64]) -> f64 {
    resistances.iter().sum()
}

/// Parallel thermal resistance: 1/R_total = Σ(1/R_i)
pub fn thermal_resistance_parallel(resistances: &[f64]) -> f64 {
    let sum_inv: f64 = resistances.iter().map(|r| 1.0 / r).sum();
    1.0 / sum_inv
}

// ---------------------------------------------------------------------------
// Weld design
// ---------------------------------------------------------------------------

/// Effective throat of a fillet weld.
/// t_e = 0.707 × leg_size  (equal-leg 45° fillet)
/// (AWS D1.1 §2.4)
pub fn fillet_weld_throat(leg_size: Length) -> Length {
    Length::m(0.707 * leg_size.value())
}

/// Shear capacity of a fillet weld.
/// V = 0.707 × w × L × F_nw  (AWS D1.1)
pub fn fillet_weld_strength(
    leg_size: Length,
    length: Length,
    allowable_shear: Pressure,
) -> Force {
    let throat = 0.707 * leg_size.value();
    Force::n(throat * length.value() * allowable_shear.value())
}

/// Tensile/compressive capacity of a full-penetration groove (butt) weld.
/// P = t × L × F_allow  (AWS D1.1 Table 2.3)
pub fn butt_weld_strength(
    thickness: Length,
    length: Length,
    allowable_stress: Pressure,
) -> Force {
    Force::n(thickness.value() * length.value() * allowable_stress.value())
}

/// AWS D1.1 fatigue category index for common weld joint types.
/// Returns integer 1–7 (A through X): 1 = best (Category A), 7 = worst.
/// Joint type codes: 0=base metal, 1=CJP butt ground flush, 2=CJP butt as-welded,
/// 3=fillet toe on base metal, 4=fillet throat, 5=plug/slot, 6=attachment.
/// (AWS D1.1-2020 Table 2.5)
pub fn weld_fatigue_category(joint_type: u8) -> u8 {
    match joint_type {
        0 => 1, // Category A
        1 => 2, // Category B
        2 => 3, // Category C
        3 => 3, // Category C (toe)
        4 => 5, // Category E (throat)
        5 => 5, // Category E
        6 => 6, // Category F
        _ => 7, // Category X (worst case / unknown)
    }
}

// ---------------------------------------------------------------------------
// Fastener analysis
// ---------------------------------------------------------------------------

/// ISO 898 tensile stress area for a metric bolt.
/// A_s = π/4 × [(d₂ + d₃)/2]²
/// d₂ = d - 0.6495P,  d₃ = d - 1.2269P
/// (ISO 898-1:2013 §9.1.6.2)
pub fn bolt_tensile_stress_area(nominal_d: Length, pitch: Length) -> Area {
    let d = nominal_d.value();
    let p = pitch.value();
    let d2 = d - 0.6495 * p;
    let d3 = d - 1.2269 * p;
    let d_mean = (d2 + d3) / 2.0;
    Area::m2(core::f64::consts::PI / 4.0 * d_mean * d_mean)
}

/// Torque–tension relationship for a bolt.
/// T = K × d × F_i  (nut factor K method)
/// `friction_coeff` is used directly as K (typical: 0.2 dry, 0.15 lubricated).
/// (Shigley's 11th ed. §8-4; VDI 2230 §5.5)
pub fn bolt_torque_to_preload(
    preload: Force,
    nominal_d: Length,
    friction_coeff: f64, // nut factor K
) -> Torque {
    Torque::new(friction_coeff * nominal_d.value() * preload.value())
}

/// Recommended assembly preload per VDI 2230 for a metric bolt.
/// F_M = 0.9 × R_p0.2 × A_s
/// Grade codes: 88 = 8.8, 109 = 10.9, 129 = 12.9.  Returns None for unknown grades.
/// (VDI 2230 Part 1:2015 §5.4)
pub fn bolt_vdi_2230_preload(grade: u16, nominal_d: Length) -> Option<Force> {
    let rp: f64 = match grade {
        88 => 640e6,
        109 => 940e6,
        129 => 1100e6,
        _ => return None,
    };
    // Approximate pitch: p ≈ 0.15 × d (covers M6–M64 coarse thread range)
    let d = nominal_d.value();
    let p = 0.15 * d;
    let d2 = d - 0.6495 * p;
    let d3 = d - 1.2269 * p;
    let d_mean = (d2 + d3) / 2.0;
    let a_s = core::f64::consts::PI / 4.0 * d_mean * d_mean;
    Some(Force::n(0.9 * rp * a_s))
}

// ---------------------------------------------------------------------------
// Thread geometry
// ---------------------------------------------------------------------------

/// ISO metric thread minor diameter (root): d₁ = d - 1.2269P
pub fn thread_minor_diameter(nominal_d: Length, pitch: Length) -> Length {
    Length::m(nominal_d.value() - 1.2269 * pitch.value())
}

/// ISO metric thread pitch diameter: d₂ = d - 0.6495P
pub fn thread_pitch_diameter(nominal_d: Length, pitch: Length) -> Length {
    Length::m(nominal_d.value() - 0.6495 * pitch.value())
}

/// Lead angle of a power screw thread.
/// λ = arctan(l / (π d₂)) where l = lead, d₂ = pitch diameter
pub fn thread_lead_angle(lead: Length, pitch_diameter: Length) -> Angle {
    let ratio = lead.value() / (core::f64::consts::PI * pitch_diameter.value());
    Angle::rad(ratio.atan())
}

/// Power screw efficiency (raising load).
/// e = tan(λ) / tan(λ + φ)  where φ = arctan(μ) = friction angle.
/// Returns efficiency as a dimensionless ratio 0–1.
/// (Shigley's 11th ed. §8-2)
pub fn power_screw_efficiency_raising(lead_angle: Angle, friction_coeff: f64) -> Dimensionless {
    let lam = lead_angle.value();
    let phi = friction_coeff.atan();
    let e = lam.tan() / (lam + phi).tan();
    Dimensionless::ratio(e.max(0.0).min(1.0))
}

/// Torque required to raise a load on a power screw.
/// T = F × d₂/2 × (l + π μ d₂) / (π d₂ - μ l)
/// (Shigley's 11th ed. §8-2)
pub fn power_screw_torque_raising(
    axial_load: Force,
    pitch_diameter: Length,
    lead: Length,
    friction_coeff: f64,
) -> Torque {
    let f = axial_load.value();
    let d2 = pitch_diameter.value();
    let l = lead.value();
    let mu = friction_coeff;
    let pi = core::f64::consts::PI;
    let numerator = l + pi * mu * d2;
    let denominator = pi * d2 - mu * l;
    Torque::new(f * d2 / 2.0 * numerator / denominator)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simply_supported_center_load_known_solution() {
        let e = Pressure::gpa(205.0);
        let i = rect_moment_of_inertia(Length::mm(50.0), Length::mm(50.0));
        let defl = beam_simply_supported_center_load(Force::kn(10.0), Length::m(1.0), e, i);
        let defl_mm = defl.to_mm();
        assert!(defl_mm > 1.5 && defl_mm < 2.5, "deflection = {defl_mm} mm");
    }

    #[test]
    fn cantilever_end_load_known_solution() {
        let e = Pressure::gpa(68.9);
        let i = rect_moment_of_inertia(Length::mm(25.0), Length::mm(10.0));
        let defl = beam_cantilever_end_load(Force::n(100.0), Length::mm(500.0), e, i);
        let defl_mm = defl.to_mm();
        assert!(defl_mm > 25.0 && defl_mm < 35.0, "deflection = {defl_mm} mm");
    }

    #[test]
    fn kt_hole_classic_result() {
        let kt = kt_hole_in_plate(Length::mm(1.0), Length::mm(1000.0)).unwrap();
        assert!((kt.value() - 3.0).abs() < 0.01);
    }

    #[test]
    fn kt_hole_out_of_range() {
        assert!(kt_hole_in_plate(Length::mm(70.0), Length::mm(100.0)).is_none());
    }

    #[test]
    fn goodman_safety_factor_basic() {
        let n = goodman_safety_factor(
            Pressure::mpa(100.0),
            Pressure::mpa(50.0),
            Pressure::mpa(200.0),
            Pressure::mpa(500.0),
        )
        .unwrap();
        assert!((n.value() - 1.667).abs() < 0.01);
    }

    #[test]
    fn euler_buckling_pinned_pinned() {
        let e = Pressure::gpa(205.0);
        let i = circle_moment_of_inertia(Length::mm(20.0));
        let p_cr = euler_buckling_load(e, i, Length::m(1.0), 1.0);
        let p_kn = p_cr.value() / 1000.0;
        assert!(p_kn > 14.0 && p_kn < 17.0, "P_cr = {p_kn} kN");
    }

    #[test]
    fn thin_wall_hoop_stress_basic() {
        let sigma = thin_wall_hoop_stress(
            Pressure::mpa(1.0),
            Length::mm(50.0),
            Length::mm(2.0),
        );
        assert!((sigma.to_mpa() - 25.0).abs() < 0.1);
    }

    #[test]
    fn torsion_solid_shaft() {
        let tau = torsion_shear_stress_solid(Torque::new(100.0), Length::mm(20.0));
        // τ = 16×100 / (π×0.02³) = 63.66 MPa
        assert!((tau.to_mpa() - 63.66).abs() < 1.0);
    }

    #[test]
    fn spring_rate_calculation() {
        // Steel spring: d=2mm, D=16mm, Na=10, G=79.3 GPa
        let k = compression_spring_rate(
            Length::mm(2.0),
            Length::mm(16.0),
            10.0,
            Pressure::gpa(79.3),
        );
        assert!(k > 3500.0 && k < 4500.0, "spring rate = {k} N/m");
    }

    #[test]
    fn wahl_factor_typical() {
        // Spring index C = 8 → K_w ≈ 1.18
        let kw = wahl_factor(8.0);
        assert!((kw - 1.184).abs() < 0.01, "Kw = {kw}");
    }

    #[test]
    fn gear_pitch_diameter() {
        let d = gear_pitch_diameter_metric(2.0, 30);
        assert!((d.to_mm() - 60.0).abs() < 0.01);
    }

    #[test]
    fn bearing_life_basic() {
        // C = 25kN, P = 5kN, ball bearing → L10 = (25/5)³ = 125 million revs
        let l10 = bearing_l10_life(Force::kn(25.0), Force::kn(5.0), true);
        assert!((l10 - 125.0).abs() < 0.1);
    }

    #[test]
    fn bearing_life_hours() {
        let hours = bearing_l10_hours(125.0, 1500.0);
        // 125e6 / (60 × 1500) = 1388.9 hours
        assert!((hours - 1388.9).abs() < 1.0);
    }

    #[test]
    fn moody_laminar() {
        let f = moody_friction_factor(Length::um(0.0), Length::mm(25.0), 1000.0);
        assert!((f - 0.064).abs() < 0.001);
    }

    #[test]
    fn von_mises_uniaxial() {
        let vm = von_mises_plane_stress(
            Pressure::mpa(100.0),
            Pressure::mpa(0.0),
            Pressure::mpa(0.0),
        );
        assert!((vm.to_mpa() - 100.0).abs() < 0.1);
    }

    #[test]
    fn thermal_expansion_steel() {
        let dl = thermal_expansion(CTE::um_mk(12.0), Length::m(1.0), 100.0);
        // ΔL = 12e-6 × 1.0 × 100 = 1.2e-3 m = 1.2 mm
        assert!((dl.to_mm() - 1.2).abs() < 0.01);
    }

    #[test]
    fn hollow_circle_inertia() {
        let i = hollow_circle_moment_of_inertia(Length::mm(30.0), Length::mm(20.0));
        let i_outer = circle_moment_of_inertia(Length::mm(30.0));
        let i_inner = circle_moment_of_inertia(Length::mm(20.0));
        assert!((i.value() - (i_outer.value() - i_inner.value())).abs() < 1e-20);
    }

    #[test]
    fn contact_ratio_standard_gears() {
        let cr = gear_contact_ratio(20, 30);
        assert!(cr > 1.5, "contact ratio = {cr}");
    }

    #[test]
    fn natural_convection_typical() {
        let h = natural_convection_vertical_plate(30.0, Length::m(0.3));
        assert!(h > 3.0 && h < 15.0, "h = {h} W/(m²·K)");
    }

    #[test]
    fn kt_shoulder_fillet_bending_typical() {
        let kt = kt_shoulder_fillet_bending(Length::mm(2.0), Length::mm(20.0)).unwrap();
        assert!(kt.value() > 1.3 && kt.value() < 1.8, "Kt = {}", kt.value());
    }

    #[test]
    fn thick_wall_pressure_vessel() {
        let sigma = thick_wall_hoop_stress(
            Pressure::mpa(10.0),
            Length::mm(50.0),
            Length::mm(75.0),
        );
        // σ = 10 × (75² + 50²) / (75² - 50²) = 10 × 8125/3125 = 26.0 MPa
        assert!((sigma.to_mpa() - 26.0).abs() < 0.1);
    }

    #[test]
    fn soderberg_more_conservative() {
        let n_goodman = goodman_safety_factor(
            Pressure::mpa(100.0),
            Pressure::mpa(100.0),
            Pressure::mpa(200.0),
            Pressure::mpa(500.0),
        )
        .unwrap();
        let n_soderberg = soderberg_safety_factor(
            Pressure::mpa(100.0),
            Pressure::mpa(100.0),
            Pressure::mpa(200.0),
            Pressure::mpa(300.0), // yield < UTS
        )
        .unwrap();
        assert!(n_soderberg.value() < n_goodman.value());
    }

    // ---- Peterson extended set ----

    #[test]
    fn kt_plate_central_hole_small_ratio_is_3() {
        let kt = kt_plate_central_hole(Length::m(1.0), Length::mm(1.0)).unwrap();
        assert!((kt.value() - 3.0).abs() < 0.05);
    }

    #[test]
    fn kt_plate_central_hole_out_of_range() {
        assert!(kt_plate_central_hole(Length::mm(100.0), Length::mm(80.0)).is_none());
    }

    #[test]
    fn kt_plate_edge_notch_neuber_identity() {
        // t = ρ → Kt = 1 + 2 = 3
        let kt =
            kt_plate_edge_notch(Length::mm(100.0), Length::mm(5.0), Length::mm(5.0)).unwrap();
        assert!((kt.value() - 3.0).abs() < 0.01);
    }

    #[test]
    fn kt_shaft_shoulder_tension_range() {
        let kt =
            kt_shaft_shoulder_fillet(Length::mm(30.0), Length::mm(20.0), Length::mm(2.0))
                .unwrap();
        assert!(kt.value() > 1.5 && kt.value() < 3.0);
    }

    #[test]
    fn kt_shaft_shoulder_bending_range() {
        let kt = kt_shaft_shoulder_fillet_bending(
            Length::mm(30.0),
            Length::mm(20.0),
            Length::mm(2.0),
        )
        .unwrap();
        assert!(kt.value() > 1.3 && kt.value() < 2.5);
    }

    #[test]
    fn kt_shaft_shoulder_torsion_range() {
        let kt = kt_shaft_shoulder_fillet_torsion(
            Length::mm(30.0),
            Length::mm(20.0),
            Length::mm(2.0),
        )
        .unwrap();
        assert!(kt.value() > 1.2 && kt.value() < 2.5);
    }

    #[test]
    fn kt_shaft_groove_semi_circle() {
        // t = ρ → Kt = 1 + 2 = 3
        let kt =
            kt_shaft_groove(Length::mm(40.0), Length::mm(2.0), Length::mm(2.0)).unwrap();
        assert!((kt.value() - 3.0).abs() < 0.01);
    }

    #[test]
    fn kt_shaft_transverse_hole_zero_ratio() {
        let kt = kt_shaft_transverse_hole(Length::mm(50.0), Length::mm(1.0)).unwrap();
        assert!((kt.value() - 3.0).abs() < 0.1);
    }

    #[test]
    fn kt_keyway_no_fillet_is_3() {
        let kt = kt_keyway_torsion(
            Length::mm(25.0),
            Length::mm(8.0),
            Length::mm(4.0),
            Length::m(0.0),
        )
        .unwrap();
        assert_eq!(kt.value(), 3.0);
    }

    #[test]
    fn kt_t_head_fillet_basic() {
        let kt =
            kt_t_head_fillet(Length::mm(40.0), Length::mm(20.0), Length::mm(2.0)).unwrap();
        assert!(kt.value() > 1.0);
    }

    #[test]
    fn kt_lug_small_hole() {
        let kt = kt_lug(Length::mm(5.0), Length::mm(50.0), Length::mm(10.0)).unwrap();
        // ratio=0.1 → Kt≈2.72 per Peterson's polynomial
        assert!((kt.value() - 2.72).abs() < 0.05);
    }

    // ---- Hertz contact ----

    #[test]
    fn hertz_sphere_on_plane_contact_radius_positive() {
        let (a, p0) = hertz_sphere_on_plane(
            Force::n(1000.0),
            Length::mm(25.0),
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        assert!(a.to_mm() > 0.0 && p0.to_mpa() > 0.0);
    }

    #[test]
    fn hertz_sphere_on_sphere_equal_spheres() {
        // Two equal spheres → R* = R/2, same as sphere on plane with R/2
        let (a_ss, _) = hertz_sphere_on_sphere(
            Force::n(1000.0),
            Length::mm(25.0),
            Length::mm(25.0),
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        let (a_sp, _) = hertz_sphere_on_plane(
            Force::n(1000.0),
            Length::mm(12.5), // R* = R/2
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        assert!((a_ss.to_mm() - a_sp.to_mm()).abs() < 0.001);
    }

    #[test]
    fn hertz_cylinder_on_plane_positive() {
        let (b, p0) = hertz_cylinder_on_plane(
            5000.0,
            Length::mm(25.0),
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        assert!(b.to_mm() > 0.0 && p0.to_mpa() > 0.0);
    }

    #[test]
    fn hertz_cylinder_on_cylinder_equals_on_plane_same_r_star() {
        // R₁=R₂=25mm → R*=12.5mm, same as single cylinder R=12.5mm on plane
        let (b_cc, _) = hertz_cylinder_on_cylinder(
            5000.0,
            Length::mm(25.0),
            Length::mm(25.0),
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        let (b_cp, _) = hertz_cylinder_on_plane(
            5000.0,
            Length::mm(12.5),
            Pressure::gpa(200.0),
            Pressure::gpa(200.0),
            0.3,
            0.3,
        );
        assert!((b_cc.to_mm() - b_cp.to_mm()).abs() < 0.001);
    }

    // ---- Pressure vessel extended ----

    #[test]
    fn sphere_hoop_stress_equals_axial() {
        // Sphere hoop = thin-wall cylinder axial (both σ = pR/2t)
        let p = Pressure::mpa(2.0);
        let r = Length::mm(100.0);
        let t = Length::mm(5.0);
        let sigma_sphere = sphere_hoop_stress(p, r, t);
        let sigma_axial = thin_wall_axial_stress(p, r, t);
        assert!((sigma_sphere.to_mpa() - sigma_axial.to_mpa()).abs() < 0.01);
    }

    #[test]
    fn thick_wall_hoop_at_r_inner_matches_existing() {
        let p = Pressure::mpa(10.0);
        let ri = Length::mm(50.0);
        let ro = Length::mm(75.0);
        let sigma_old = thick_wall_hoop_stress(p, ri, ro);
        let sigma_new = thick_wall_hoop_at_r(p, ri, ro, ri);
        assert!((sigma_old.to_mpa() - sigma_new.to_mpa()).abs() < 0.001);
    }

    #[test]
    fn thick_wall_radial_at_r_inner_is_minus_pressure() {
        let p = Pressure::mpa(10.0);
        let ri = Length::mm(50.0);
        let ro = Length::mm(75.0);
        let sigma_r = thick_wall_radial_at_r(p, ri, ro, ri);
        assert!((sigma_r.to_mpa() - (-10.0)).abs() < 0.001);
    }

    // ---- Column buckling additional ----

    #[test]
    fn johnson_stress_below_yield() {
        let sy = Pressure::mpa(250.0);
        let e = Pressure::gpa(205.0);
        let sigma = johnson_buckling_stress(sy, e, 80.0);
        assert!(sigma.to_mpa() < 250.0 && sigma.to_mpa() > 0.0);
    }

    #[test]
    fn transition_slenderness_steel() {
        // For steel E=205 GPa, Sy=250 MPa → λ_c ≈ π√(2×205e3/250) ≈ 127
        let lam_c = transition_slenderness(Pressure::gpa(205.0), Pressure::mpa(250.0));
        assert!(lam_c > 100.0 && lam_c < 160.0, "λ_c = {lam_c}");
    }

    #[test]
    fn slenderness_ratio_rg_basic() {
        let lam = slenderness_ratio_rg(Length::m(1.0), 1.0, Length::mm(10.0));
        assert!((lam - 100.0).abs() < 0.001);
    }

    // ---- Weld design ----

    #[test]
    fn fillet_weld_throat_6mm_leg() {
        let t = fillet_weld_throat(Length::mm(6.0));
        assert!((t.to_mm() - 4.243).abs() < 0.01);
    }

    #[test]
    fn fillet_weld_strength_basic() {
        // 6mm leg, 100mm length, 75 MPa allowable shear → ~31.8 kN
        let f = fillet_weld_strength(
            Length::mm(6.0),
            Length::mm(100.0),
            Pressure::mpa(75.0),
        );
        assert!(f.value() > 30_000.0 && f.value() < 35_000.0, "F = {} N", f.value());
    }

    #[test]
    fn butt_weld_strength_basic() {
        // 10mm plate, 200mm long, 150 MPa → 300 kN
        let f = butt_weld_strength(
            Length::mm(10.0),
            Length::mm(200.0),
            Pressure::mpa(150.0),
        );
        assert!((f.value() - 300_000.0).abs() < 1.0);
    }

    #[test]
    fn weld_fatigue_category_base_metal() {
        assert_eq!(weld_fatigue_category(0), 1);
        assert_eq!(weld_fatigue_category(6), 6);
        assert_eq!(weld_fatigue_category(99), 7);
    }

    // ---- Fastener analysis ----

    #[test]
    fn bolt_tensile_stress_area_m10() {
        // M10×1.5: A_s ≈ 58 mm²
        let a = bolt_tensile_stress_area(Length::mm(10.0), Length::mm(1.5));
        let a_mm2 = a.value() * 1e6;
        assert!(a_mm2 > 50.0 && a_mm2 < 65.0, "A_s = {a_mm2:.1} mm²");
    }

    #[test]
    fn bolt_torque_to_preload_m10_88() {
        // M10, F_i=30 kN, K=0.2 → T = 0.2 × 0.01 × 30000 = 60 N·m
        let t = bolt_torque_to_preload(Force::kn(30.0), Length::mm(10.0), 0.2);
        assert!((t.value() - 60.0).abs() < 0.1, "T = {} N·m", t.value());
    }

    #[test]
    fn bolt_vdi_preload_m10_grade109() {
        let f = bolt_vdi_2230_preload(109, Length::mm(10.0)).unwrap();
        let f_kn = f.value() / 1000.0;
        assert!(f_kn > 30.0 && f_kn < 80.0, "F_M = {f_kn:.1} kN");
    }

    #[test]
    fn bolt_vdi_preload_unknown_grade_returns_none() {
        assert!(bolt_vdi_2230_preload(66, Length::mm(10.0)).is_none());
    }

    // ---- Thread geometry ----

    #[test]
    fn thread_minor_diameter_m10() {
        // M10×1.5: d₁ = 10 - 1.2269×1.5 = 8.160 mm
        let d1 = thread_minor_diameter(Length::mm(10.0), Length::mm(1.5));
        assert!((d1.to_mm() - 8.160).abs() < 0.01, "d₁ = {} mm", d1.to_mm());
    }

    #[test]
    fn thread_pitch_diameter_m10() {
        // M10×1.5: d₂ = 10 - 0.6495×1.5 = 9.026 mm
        let d2 = thread_pitch_diameter(Length::mm(10.0), Length::mm(1.5));
        assert!((d2.to_mm() - 9.026).abs() < 0.01, "d₂ = {} mm", d2.to_mm());
    }

    #[test]
    fn thread_lead_angle_positive() {
        let d2 = thread_pitch_diameter(Length::mm(10.0), Length::mm(1.5));
        let lam = thread_lead_angle(Length::mm(1.5), d2);
        assert!(
            lam.to_deg() > 2.0 && lam.to_deg() < 5.0,
            "λ = {} deg",
            lam.to_deg()
        );
    }

    #[test]
    fn power_screw_efficiency_positive() {
        // Lead angle 5°, μ=0.1 → should give positive efficiency < 1
        let lam = Angle::deg(5.0);
        let e = power_screw_efficiency_raising(lam, 0.1);
        assert!(e.value() > 0.0 && e.value() < 1.0, "eff = {}", e.value());
    }

    #[test]
    fn power_screw_torque_raising_positive() {
        // 10 kN load, d₂=9mm, lead=1.5mm, μ=0.15
        let t = power_screw_torque_raising(
            Force::kn(10.0),
            Length::mm(9.0),
            Length::mm(1.5),
            0.15,
        );
        assert!(t.value() > 0.0, "T = {} N·m", t.value());
    }

    // ---- Section helpers ----

    #[test]
    fn radius_of_gyration_solid_circle() {
        // r = d/4 for solid circle
        let d = Length::mm(40.0);
        let i = circle_moment_of_inertia(d);
        let a = Area::m2(core::f64::consts::PI * (0.04_f64).powi(2) / 4.0);
        let r = radius_of_gyration(i, a);
        assert!((r.to_mm() - 10.0).abs() < 0.01);
    }

    #[test]
    fn polar_moment_solid_20mm() {
        let j = polar_moment_solid(Length::mm(20.0));
        // J = π × 0.02⁴/32 ≈ 15.708e-9 m⁴
        assert!((j.value() - 15.708e-9).abs() < 1e-11);
    }

    #[test]
    fn polar_moment_hollow_equals_difference() {
        let j_outer = polar_moment_solid(Length::mm(30.0));
        let j_inner = polar_moment_solid(Length::mm(20.0));
        let j_hollow = polar_moment_hollow(Length::mm(30.0), Length::mm(20.0));
        assert!((j_hollow.value() - (j_outer.value() - j_inner.value())).abs() < 1e-20);
    }

    // ---- Beam moments ----

    #[test]
    fn ss_center_moment_basic() {
        // P=10 kN, L=2 m → M = 10000 × 2 / 4 = 5000 N·m
        let m = beam_ss_center_moment(Force::kn(10.0), Length::m(2.0));
        assert!((m.value() - 5000.0).abs() < 0.1);
    }

    #[test]
    fn cantilever_uniform_moment_basic() {
        // w=500 N/m, L=2 m → M = 500 × 4 / 2 = 1000 N·m
        let m = beam_cantilever_uniform_moment(500.0, Length::m(2.0));
        assert!((m.value() - 1000.0).abs() < 0.1);
    }

    // ---- Shaft formulas ----

    #[test]
    fn shaft_torque_1kw_1000rpm() {
        // T = 1000 × 60 / (2π × 1000) ≈ 9.549 N·m
        let t = shaft_torque(Power::new(1000.0), 1000.0);
        assert!((t.value() - 9.549).abs() < 0.01, "T = {} N·m", t.value());
    }

    #[test]
    fn shaft_diameter_from_torque_sensible() {
        // T=100 N·m, τ_allow=40 MPa → d ≈ 23–25 mm
        let d = shaft_diameter_from_torque(Torque::new(100.0), Pressure::mpa(40.0));
        assert!(
            d.to_mm() > 20.0 && d.to_mm() < 30.0,
            "d = {} mm",
            d.to_mm()
        );
    }

    #[test]
    fn shaft_critical_speed_positive() {
        // δ = 1 mm → ω_c = √(9.80665/0.001) ≈ 99 rad/s
        let w = shaft_critical_speed(Length::mm(1.0));
        assert!((w - 99.0).abs() < 1.0, "ω_c = {w} rad/s");
    }

    // ---- Heat exchanger analysis ----

    #[test]
    fn heat_exchanger_ntu_basic() {
        // UA = 5000 W/K, C_min = 1000 W/K → NTU = 5.0
        let ntu = heat_exchanger_ntu(5000.0, 1000.0);
        assert!((ntu - 5.0).abs() < 1e-10);
    }

    #[test]
    fn heat_exchanger_effectiveness_counterflow_balanced() {
        // C_r = 1.0, NTU = 2.0 → ε = 2/(1+2) = 0.6667
        let eps = heat_exchanger_effectiveness_counterflow(2.0, 1.0);
        assert!((eps - 0.6667).abs() < 0.001, "ε = {eps}");
    }

    #[test]
    fn heat_exchanger_effectiveness_counterflow_unbalanced() {
        // C_r = 0.5, NTU = 2.0
        let eps = heat_exchanger_effectiveness_counterflow(2.0, 0.5);
        // ε = (1 - exp(-2*0.5))/(1 - 0.5*exp(-2*0.5)) = (1 - e^-1)/(1 - 0.5*e^-1)
        let e1 = (-1.0_f64).exp();
        let expected = (1.0 - e1) / (1.0 - 0.5 * e1);
        assert!((eps - expected).abs() < 1e-10, "ε = {eps}, expected = {expected}");
    }

    #[test]
    fn heat_exchanger_effectiveness_parallel_basic() {
        // C_r = 1.0, NTU = 1.0 → ε = (1 - exp(-2))/2
        let eps = heat_exchanger_effectiveness_parallel(1.0, 1.0);
        let expected = (1.0 - (-2.0_f64).exp()) / 2.0;
        assert!((eps - expected).abs() < 1e-10, "ε = {eps}");
    }

    #[test]
    fn heat_exchanger_effectiveness_shell_tube_basic() {
        let eps = heat_exchanger_effectiveness_shell_tube(1.0, 0.5);
        // Must be between 0 and 1
        assert!(eps > 0.0 && eps < 1.0, "ε = {eps}");
        // Shell-tube effectiveness should be between parallel and counterflow
        let eps_par = heat_exchanger_effectiveness_parallel(1.0, 0.5);
        let eps_cf = heat_exchanger_effectiveness_counterflow(1.0, 0.5);
        assert!(eps >= eps_par && eps <= eps_cf,
            "shell-tube ε={eps} should be between parallel={eps_par} and counterflow={eps_cf}");
    }

    #[test]
    fn heat_exchanger_lmtd_basic() {
        // ΔT1 = 100, ΔT2 = 50 → LMTD = 50/ln(2) ≈ 72.13
        let lmtd = heat_exchanger_lmtd(100.0, 50.0);
        let expected = 50.0 / (2.0_f64).ln();
        assert!((lmtd - expected).abs() < 0.01, "LMTD = {lmtd}");
    }

    #[test]
    fn heat_exchanger_lmtd_equal_dt() {
        // When ΔT1 = ΔT2, LMTD should return ΔT1
        let lmtd = heat_exchanger_lmtd(50.0, 50.0);
        assert!((lmtd - 50.0).abs() < 1e-6);
    }

    #[test]
    fn heat_exchanger_q_ntu_basic() {
        // C_min=500, ε=0.8, T_hot=400K, T_cold=300K → Q = 0.8*500*100 = 40000 W
        let q = heat_exchanger_q_ntu(
            500.0, 0.8,
            Temperature::new(400.0),
            Temperature::new(300.0),
        );
        assert!((q.value() - 40_000.0).abs() < 0.1, "Q = {} W", q.value());
    }

    // ---- Nusselt correlations ----

    #[test]
    fn nusselt_pipe_turbulent_basic() {
        // Re=50000, Pr=0.71 (air) → Nu = 0.023 * 50000^0.8 * 0.71^0.4
        let nu = nusselt_pipe_turbulent(50_000.0, 0.71);
        let expected = 0.023 * 50_000.0_f64.powf(0.8) * 0.71_f64.powf(0.4);
        assert!((nu - expected).abs() < 0.01, "Nu = {nu}");
    }

    #[test]
    fn nusselt_laminar_constants() {
        assert!((nusselt_pipe_laminar_constant_wall_temp() - 3.66).abs() < 1e-10);
        assert!((nusselt_pipe_laminar_constant_heat_flux() - 4.36).abs() < 1e-10);
    }

    #[test]
    fn nusselt_cylinder_crossflow_basic() {
        // Re=10000, Pr=0.71
        let nu = nusselt_cylinder_crossflow(10_000.0, 0.71);
        // Should be in the range of ~40-80 for this Re
        assert!(nu > 30.0 && nu < 100.0, "Nu = {nu}");
    }

    #[test]
    fn nusselt_sphere_basic() {
        // Re=1000, Pr=0.71 → Nu = 2 + (0.4*√1000 + 0.06*1000^(2/3))*0.71^0.4
        let nu = nusselt_sphere(1000.0, 0.71);
        let expected = 2.0
            + (0.4 * 1000.0_f64.sqrt() + 0.06 * 1000.0_f64.powf(2.0 / 3.0))
                * 0.71_f64.powf(0.4);
        assert!((nu - expected).abs() < 0.01, "Nu = {nu}");
    }

    #[test]
    fn forced_convection_pipe_basic() {
        // Nu=100, k=0.6 W/(m·K), D=25mm → h = 100*0.6/0.025 = 2400 W/(m²·K)
        let h = forced_convection_pipe(100.0, 0.6, Length::mm(25.0));
        assert!((h - 2400.0).abs() < 0.1, "h = {h}");
    }

    // ---- Fin enhancements ----

    #[test]
    fn fin_efficiency_annular_basic() {
        let eta = fin_efficiency_annular(
            50.0,
            ThermalConductivity::new(200.0),
            Length::mm(1.0),
            Length::mm(25.0),
            Length::mm(50.0),
        );
        assert!(eta > 0.0 && eta <= 1.0, "η = {eta}");
    }

    #[test]
    fn overall_surface_efficiency_basic() {
        // 10 fins, η_f=0.9, A_fin=0.001 m², A_total=0.02 m²
        let eta_o = overall_surface_efficiency(10, 0.9, 0.001, 0.02);
        // η_o = 1 - (10*0.001/0.02)*(1-0.9) = 1 - 0.5*0.1 = 0.95
        assert!((eta_o - 0.95).abs() < 1e-10, "η_o = {eta_o}");
    }

    #[test]
    fn fin_heat_rate_basic() {
        // η=0.9, h=50, A=0.01 m², T_base=400K, T_amb=300K
        // Q = 0.9*50*0.01*100 = 45 W
        let q = fin_heat_rate(
            50.0, 0.9,
            Area::m2(0.01),
            Temperature::new(400.0),
            Temperature::new(300.0),
        );
        assert!((q.value() - 45.0).abs() < 0.01, "Q = {} W", q.value());
    }

    // ---- Thermal network ----

    #[test]
    fn thermal_resistance_convection_basic() {
        // h=100, A=0.5 → R = 1/(100*0.5) = 0.02 K/W
        let r = thermal_resistance_convection(100.0, Area::m2(0.5));
        assert!((r - 0.02).abs() < 1e-10, "R = {r}");
    }

    #[test]
    fn thermal_resistance_radiation_basic() {
        // Linearized radiation resistance should be positive and finite
        let r = thermal_resistance_radiation(
            0.9,
            Area::m2(1.0),
            Temperature::new(400.0),
            Temperature::new(300.0),
        );
        assert!(r > 0.0 && r.is_finite(), "R_rad = {r}");
    }

    #[test]
    fn thermal_resistance_series_basic() {
        let r = thermal_resistance_series(&[0.1, 0.2, 0.3]);
        assert!((r - 0.6).abs() < 1e-10, "R_total = {r}");
    }

    #[test]
    fn thermal_resistance_parallel_basic() {
        // Two equal resistances in parallel: R_total = R/2
        let r = thermal_resistance_parallel(&[0.4, 0.4]);
        assert!((r - 0.2).abs() < 1e-10, "R_total = {r}");
    }

    #[test]
    fn thermal_resistance_parallel_unequal() {
        // R1=1.0, R2=2.0 → 1/R = 1+0.5 = 1.5 → R = 0.6667
        let r = thermal_resistance_parallel(&[1.0, 2.0]);
        assert!((r - 2.0 / 3.0).abs() < 1e-10, "R_total = {r}");
    }
}
