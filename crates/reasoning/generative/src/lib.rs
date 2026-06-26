//! `physical-generative` — generative CAD design-space exploration.
//!
//! Explores parametric design spaces via Latin Hypercube Sampling, evaluates
//! candidates with rapid analytical formulas (beam theory, thermal resistance,
//! torsional stiffness), and extracts Pareto-optimal fronts.  Each candidate
//! can be converted to a B-Rep [`Solid`] through [`candidate_to_solid`].
//!
//! Cascade: LUT first → analytical formula second → solver third → LLM fourth.

use serde::{Deserialize, Serialize};
use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::solid::Solid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The kind of geometry a design space produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeometryClass {
    Box,
    Cylinder,
    LBracket,
    HeatSink,
    Shaft,
    Housing,
}

/// A single tuneable parameter with name and bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignParameter {
    pub name: String,
    pub min: f64,
    pub max: f64,
}

impl DesignParameter {
    pub fn new(name: &str, min: f64, max: f64) -> Self {
        Self { name: name.to_string(), min, max }
    }

    /// Map a normalised value in [0,1] to the parameter range.
    pub fn denormalise(&self, t: f64) -> f64 {
        self.min + t * (self.max - self.min)
    }
}

/// A constraint on the design: `value ≤ limit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerativeConstraint {
    pub name: String,
    /// Index into the candidate vector whose value must satisfy the limit.
    pub parameter_index: usize,
    pub limit: f64,
}

/// Optimisation objective: minimise or maximise a named metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Objective {
    Minimise(String),
    Maximise(String),
}

/// Full specification of a design space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSpace {
    pub name: String,
    pub geometry: GeometryClass,
    pub parameters: Vec<DesignParameter>,
    pub constraints: Vec<GenerativeConstraint>,
    pub objectives: Vec<Objective>,
}

/// A single candidate: parameter values plus evaluated objective scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub values: Vec<f64>,
    pub scores: Vec<f64>,
}

// ---------------------------------------------------------------------------
// Latin Hypercube Sampling (LCG PRNG + Fisher-Yates shuffle)
// ---------------------------------------------------------------------------

/// Minimal LCG PRNG — deterministic, no external dependency.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    fn next_u64(&mut self) -> u64 {
        // Numerical Recipes LCG constants
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Fisher-Yates shuffle using the LCG.
fn shuffle(rng: &mut Lcg, v: &mut [usize]) {
    let n = v.len();
    for i in (1..n).rev() {
        let j = rng.next_usize(i + 1);
        v.swap(i, j);
    }
}

/// Generate `n` candidates in the design space via Latin Hypercube Sampling.
///
/// Each dimension is divided into `n` equally-probable strata; one sample is
/// drawn from each stratum and the strata are shuffled independently per
/// dimension.  This guarantees full coverage of every marginal.
pub fn latin_hypercube_sample(space: &DesignSpace, n: usize, seed: u64) -> Vec<Vec<f64>> {
    let d = space.parameters.len();
    let mut rng = Lcg::new(seed);

    // For each dimension, create a permutation of 0..n
    let mut perms: Vec<Vec<usize>> = Vec::with_capacity(d);
    for _ in 0..d {
        let mut perm: Vec<usize> = (0..n).collect();
        shuffle(&mut rng, &mut perm);
        perms.push(perm);
    }

    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let mut point = Vec::with_capacity(d);
        for j in 0..d {
            let stratum = perms[j][i];
            let t = (stratum as f64 + rng.next_f64()) / n as f64;
            point.push(space.parameters[j].denormalise(t));
        }
        samples.push(point);
    }
    samples
}

// ---------------------------------------------------------------------------
// Rapid analytical evaluation
// ---------------------------------------------------------------------------

/// Evaluate a candidate in the given design space and return objective scores.
///
/// Uses closed-form engineering formulas:
/// - **Bracket**: beam bending stress and mass (rectangular cross-section).
/// - **HeatSink**: thermal resistance of a fin array plus mass.
/// - **Shaft**: torsional stiffness and mass of a solid cylinder.
/// - **Housing**: enclosure wall bending stiffness and mass.
pub fn evaluate(space: &DesignSpace, values: &[f64]) -> Vec<f64> {
    match space.geometry {
        GeometryClass::LBracket | GeometryClass::Box => evaluate_bracket(values),
        GeometryClass::HeatSink => evaluate_heatsink(values),
        GeometryClass::Shaft | GeometryClass::Cylinder => evaluate_shaft(values),
        GeometryClass::Housing => evaluate_housing(values),
    }
}

/// Bracket: params = [width, height, thickness, length]
/// Objectives: [stress_mpa (minimise), mass_kg (minimise)]
fn evaluate_bracket(v: &[f64]) -> Vec<f64> {
    let width = v[0];     // mm
    let height = v[1];    // mm
    let thickness = v[2]; // mm
    let length = v[3];    // mm

    // Rectangular cross-section moment of inertia: I = b*h^3 / 12
    let i_mm4 = width * height.powi(3) / 12.0;
    // Bending stress σ = M*c / I  with M = F*L, c = h/2, F = 1000 N reference load
    let load_n = 1000.0;
    let moment = load_n * length;
    let stress = if i_mm4 > 0.0 { moment * (height / 2.0) / i_mm4 } else { f64::MAX };

    // Mass: density of steel 7850 kg/m³
    let vol_mm3 = width * height * length + 2.0 * thickness * height * length;
    let mass = vol_mm3 * 7.85e-6; // kg

    vec![stress, mass]
}

/// HeatSink: params = [base_width, base_height, fin_count, fin_height, fin_thickness]
/// Objectives: [thermal_resistance_K_per_W (minimise), mass_kg (minimise)]
fn evaluate_heatsink(v: &[f64]) -> Vec<f64> {
    let base_w = v[0];
    let base_h = v[1];
    let fin_count = v[2].round().max(1.0);
    let fin_h = v[3];
    let fin_t = v[4];

    // Thermal resistance of fin array (natural convection approximation)
    // R_th ≈ 1 / (h_conv * A_total)
    // h_conv ~ 10 W/(m²·K) for natural convection in air
    let h_conv = 10.0; // W/(m²·K)
    let base_area = base_w * base_h * 1e-6; // m²
    let fin_area = fin_count * 2.0 * fin_h * base_h * 1e-6; // m² (both sides)
    let total_area = base_area + fin_area;
    let r_th = if total_area > 0.0 { 1.0 / (h_conv * total_area) } else { f64::MAX };

    // Mass (aluminium 2700 kg/m³)
    let base_vol = base_w * base_h * 3.0; // assume 3 mm base thickness
    let fin_vol = fin_count * fin_t * fin_h * base_h;
    let mass = (base_vol + fin_vol) * 2.7e-6; // kg

    vec![r_th, mass]
}

/// Shaft: params = [diameter, length]
/// Objectives: [inv_torsional_stiffness (minimise → stiffer is better), mass_kg (minimise)]
fn evaluate_shaft(v: &[f64]) -> Vec<f64> {
    let d = v[0]; // mm
    let l = v[1]; // mm

    // Polar moment of inertia: J = π d⁴ / 32
    let j = std::f64::consts::PI * d.powi(4) / 32.0; // mm⁴
    // Torsional stiffness: k = G*J/L, G_steel ~ 80 GPa = 80000 MPa
    let g = 80_000.0; // MPa
    let stiffness = if l > 0.0 { g * j / l } else { f64::MAX }; // N·mm/rad
    let inv_stiff = if stiffness > 0.0 { 1.0 / stiffness } else { f64::MAX };

    // Mass (steel)
    let vol = std::f64::consts::PI * (d / 2.0).powi(2) * l; // mm³
    let mass = vol * 7.85e-6;

    vec![inv_stiff, mass]
}

/// Housing: params = [width, depth, height, wall_thickness]
/// Objectives: [inv_wall_stiffness (minimise), mass_kg (minimise)]
fn evaluate_housing(v: &[f64]) -> Vec<f64> {
    let w = v[0];
    let depth = v[1];
    let h = v[2];
    let t = v[3];

    // Simplified plate bending stiffness: D = E*t³ / (12*(1-ν²))
    // E_aluminium = 70 GPa = 70000 MPa, ν = 0.33
    let e = 70_000.0;
    let nu = 0.33;
    let d_stiff = e * t.powi(3) / (12.0 * (1.0 - nu * nu));
    let inv_stiff = if d_stiff > 0.0 { 1.0 / d_stiff } else { f64::MAX };

    // Mass (aluminium shell)
    let surface = 2.0 * (w * depth + w * h + depth * h);
    let mass = surface * t * 2.7e-6;

    vec![inv_stiff, mass]
}

// ---------------------------------------------------------------------------
// Constraint penalty
// ---------------------------------------------------------------------------

/// Apply constraint penalties to scores.  Violated constraints add a large
/// penalty proportional to the violation magnitude.
pub fn penalise_constraints(space: &DesignSpace, values: &[f64], scores: &mut [f64]) {
    let penalty_weight = 1e6;
    for c in &space.constraints {
        if c.parameter_index < values.len() {
            let val = values[c.parameter_index];
            if val > c.limit {
                let violation = val - c.limit;
                for s in scores.iter_mut() {
                    *s += penalty_weight * violation;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pareto front — non-dominated sorting
// ---------------------------------------------------------------------------

/// Returns `true` if candidate `a` dominates candidate `b`.
///
/// A candidate dominates another when it is at least as good in every
/// objective and strictly better in at least one.  All objectives are
/// treated as minimisation (use `1/value` for maximisation before calling).
fn dominates(a: &[f64], b: &[f64]) -> bool {
    let mut dominated_one = false;
    for (ai, bi) in a.iter().zip(b.iter()) {
        if ai > bi {
            return false;
        }
        if ai < bi {
            dominated_one = true;
        }
    }
    dominated_one
}

/// Extract the Pareto front (rank 0) from a set of candidates.
///
/// Returns the indices of non-dominated candidates.
pub fn pareto_front(candidates: &[Candidate]) -> Vec<usize> {
    let n = candidates.len();
    let mut front = Vec::new();
    for i in 0..n {
        let mut dominated = false;
        for j in 0..n {
            if i != j && dominates(&candidates[j].scores, &candidates[i].scores) {
                dominated = true;
                break;
            }
        }
        if !dominated {
            front.push(i);
        }
    }
    front
}

// ---------------------------------------------------------------------------
// Candidate → Solid
// ---------------------------------------------------------------------------

/// Convert a candidate's parameters into a B-Rep [`Solid`] according to the
/// geometry class of the design space.
pub fn candidate_to_solid(space: &DesignSpace, values: &[f64]) -> Solid {
    match space.geometry {
        GeometryClass::Box | GeometryClass::LBracket | GeometryClass::Housing => {
            // width, height, thickness (or depth)
            let w = values.first().copied().unwrap_or(10.0);
            let h = values.get(1).copied().unwrap_or(10.0);
            let d = values.get(2).copied().unwrap_or(2.0);
            make_box(w, h, d)
        }
        GeometryClass::Cylinder | GeometryClass::Shaft => {
            let diameter = values.first().copied().unwrap_or(10.0);
            let length = values.get(1).copied().unwrap_or(50.0);
            make_cylinder(diameter / 2.0, length, 24)
        }
        GeometryClass::HeatSink => {
            // Approximate as a box encompassing the fin envelope
            let base_w = values.first().copied().unwrap_or(40.0);
            let base_h = values.get(1).copied().unwrap_or(40.0);
            let fin_h = values.get(3).copied().unwrap_or(15.0);
            make_box(base_w, fin_h + 3.0, base_h) // base + fin height
        }
    }
}

// ---------------------------------------------------------------------------
// Full pipeline helper
// ---------------------------------------------------------------------------

/// Sample, evaluate, penalise, and return candidates with their Pareto front.
pub fn generate_and_rank(
    space: &DesignSpace,
    count: usize,
    seed: u64,
) -> (Vec<Candidate>, Vec<usize>) {
    let samples = latin_hypercube_sample(space, count, seed);
    let mut candidates = Vec::with_capacity(count);
    for values in samples {
        let mut scores = evaluate(space, &values);
        penalise_constraints(space, &values, &mut scores);
        candidates.push(Candidate { values, scores });
    }
    let front = pareto_front(&candidates);
    (candidates, front)
}

// ---------------------------------------------------------------------------
// Pre-built design spaces
// ---------------------------------------------------------------------------

/// L-bracket design space: width, height, thickness, length.
pub fn bracket_design_space() -> DesignSpace {
    DesignSpace {
        name: "L-Bracket".into(),
        geometry: GeometryClass::LBracket,
        parameters: vec![
            DesignParameter::new("width", 10.0, 100.0),
            DesignParameter::new("height", 10.0, 80.0),
            DesignParameter::new("thickness", 1.0, 10.0),
            DesignParameter::new("length", 50.0, 300.0),
        ],
        constraints: vec![
            GenerativeConstraint { name: "max_length".into(), parameter_index: 3, limit: 250.0 },
        ],
        objectives: vec![
            Objective::Minimise("stress".into()),
            Objective::Minimise("mass".into()),
        ],
    }
}

/// Heat-sink design space: base_width, base_height, fin_count, fin_height, fin_thickness.
pub fn heatsink_design_space() -> DesignSpace {
    DesignSpace {
        name: "HeatSink".into(),
        geometry: GeometryClass::HeatSink,
        parameters: vec![
            DesignParameter::new("base_width", 20.0, 100.0),
            DesignParameter::new("base_height", 20.0, 100.0),
            DesignParameter::new("fin_count", 4.0, 30.0),
            DesignParameter::new("fin_height", 5.0, 40.0),
            DesignParameter::new("fin_thickness", 0.5, 3.0),
        ],
        constraints: vec![],
        objectives: vec![
            Objective::Minimise("thermal_resistance".into()),
            Objective::Minimise("mass".into()),
        ],
    }
}

/// Shaft design space: diameter, length.
pub fn shaft_design_space() -> DesignSpace {
    DesignSpace {
        name: "Shaft".into(),
        geometry: GeometryClass::Shaft,
        parameters: vec![
            DesignParameter::new("diameter", 5.0, 50.0),
            DesignParameter::new("length", 50.0, 500.0),
        ],
        constraints: vec![],
        objectives: vec![
            Objective::Minimise("inv_torsional_stiffness".into()),
            Objective::Minimise("mass".into()),
        ],
    }
}

/// Housing / enclosure design space: width, depth, height, wall_thickness.
pub fn housing_design_space() -> DesignSpace {
    DesignSpace {
        name: "Housing".into(),
        geometry: GeometryClass::Housing,
        parameters: vec![
            DesignParameter::new("width", 40.0, 200.0),
            DesignParameter::new("depth", 40.0, 200.0),
            DesignParameter::new("height", 20.0, 100.0),
            DesignParameter::new("wall_thickness", 1.0, 5.0),
        ],
        constraints: vec![],
        objectives: vec![
            Objective::Minimise("inv_wall_stiffness".into()),
            Objective::Minimise("mass".into()),
        ],
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_hypercube_sampling_covers_range() {
        let space = bracket_design_space();
        let samples = latin_hypercube_sample(&space, 50, 42);
        assert_eq!(samples.len(), 50);
        for s in &samples {
            assert_eq!(s.len(), space.parameters.len());
            for (val, p) in s.iter().zip(&space.parameters) {
                assert!(*val >= p.min, "{} = {} < min {}", p.name, val, p.min);
                assert!(*val <= p.max, "{} = {} > max {}", p.name, val, p.max);
            }
        }
    }

    #[test]
    fn evaluate_candidate_returns_score() {
        let space = bracket_design_space();
        let values = vec![50.0, 30.0, 3.0, 150.0];
        let scores = evaluate(&space, &values);
        assert_eq!(scores.len(), 2);
        assert!(scores[0] > 0.0, "stress must be positive");
        assert!(scores[1] > 0.0, "mass must be positive");
    }

    #[test]
    fn pareto_front_finds_nondominated() {
        let space = bracket_design_space();
        let (candidates, front) = generate_and_rank(&space, 100, 7);
        assert!(!front.is_empty(), "Pareto front must not be empty");
        assert!(front.len() <= candidates.len());
        // Every front member must be non-dominated by any other candidate
        for &i in &front {
            for (j, c) in candidates.iter().enumerate() {
                if j != i {
                    assert!(
                        !dominates(&c.scores, &candidates[i].scores),
                        "front member {} is dominated by {}",
                        i, j,
                    );
                }
            }
        }
    }

    #[test]
    fn bracket_design_space_valid() {
        let ds = bracket_design_space();
        assert_eq!(ds.parameters.len(), 4);
        assert_eq!(ds.objectives.len(), 2);
        assert!(!ds.name.is_empty());
        for p in &ds.parameters {
            assert!(p.min < p.max);
        }
    }

    #[test]
    fn heatsink_design_space_valid() {
        let ds = heatsink_design_space();
        assert_eq!(ds.parameters.len(), 5);
        assert_eq!(ds.objectives.len(), 2);
        for p in &ds.parameters {
            assert!(p.min < p.max);
        }
    }

    #[test]
    fn shaft_design_space_valid() {
        let ds = shaft_design_space();
        assert_eq!(ds.parameters.len(), 2);
        assert_eq!(ds.objectives.len(), 2);
        for p in &ds.parameters {
            assert!(p.min < p.max);
        }
    }

    #[test]
    fn generate_candidates_count() {
        let space = shaft_design_space();
        let samples = latin_hypercube_sample(&space, 200, 0);
        assert_eq!(samples.len(), 200);
    }

    #[test]
    fn candidate_to_solid_produces_geometry() {
        let space = bracket_design_space();
        let values = vec![40.0, 25.0, 3.0, 120.0];
        let solid = candidate_to_solid(&space, &values);
        assert!(solid.vertex_count() > 0);
        assert!(solid.face_count() > 0);

        let space2 = shaft_design_space();
        let values2 = vec![20.0, 100.0];
        let solid2 = candidate_to_solid(&space2, &values2);
        assert!(solid2.vertex_count() > 0);
        assert!(solid2.face_count() > 0);
    }

    #[test]
    fn heavier_bracket_stronger() {
        let space = bracket_design_space();
        // Thin bracket
        let thin = vec![30.0, 15.0, 1.5, 200.0];
        let thin_scores = evaluate(&space, &thin);
        // Thick bracket (same width/length, bigger cross-section)
        let thick = vec![30.0, 40.0, 5.0, 200.0];
        let thick_scores = evaluate(&space, &thick);
        // Thicker bracket should have lower stress
        assert!(thick_scores[0] < thin_scores[0], "thicker bracket should have lower stress");
        // ...but higher mass
        assert!(thick_scores[1] > thin_scores[1], "thicker bracket should have more mass");
    }

    #[test]
    fn pareto_front_single_objective() {
        // With a single objective, the Pareto front is just the best candidate.
        let candidates = vec![
            Candidate { values: vec![1.0], scores: vec![5.0] },
            Candidate { values: vec![2.0], scores: vec![3.0] },
            Candidate { values: vec![3.0], scores: vec![7.0] },
        ];
        let front = pareto_front(&candidates);
        assert_eq!(front.len(), 1);
        assert_eq!(front[0], 1); // index of score 3.0
    }

    #[test]
    fn pareto_front_two_objectives() {
        let candidates = vec![
            Candidate { values: vec![], scores: vec![1.0, 5.0] }, // good on obj0
            Candidate { values: vec![], scores: vec![5.0, 1.0] }, // good on obj1
            Candidate { values: vec![], scores: vec![3.0, 3.0] }, // middle
            Candidate { values: vec![], scores: vec![4.0, 4.0] }, // dominated by middle
        ];
        let front = pareto_front(&candidates);
        // Candidates 0, 1, 2 are non-dominated; candidate 3 is dominated by 2.
        assert_eq!(front.len(), 3);
        assert!(front.contains(&0));
        assert!(front.contains(&1));
        assert!(front.contains(&2));
        assert!(!front.contains(&3));
    }

    #[test]
    fn design_space_parameter_bounds() {
        let space = housing_design_space();
        for p in &space.parameters {
            assert!(p.min >= 0.0, "parameter {} min should be non-negative", p.name);
            assert!(p.max > p.min, "parameter {} max must exceed min", p.name);
            // Denormalise endpoints
            assert!((p.denormalise(0.0) - p.min).abs() < 1e-12);
            assert!((p.denormalise(1.0) - p.max).abs() < 1e-12);
        }
    }

    #[test]
    fn evaluation_penalizes_constraint_violation() {
        let space = bracket_design_space();
        // Within constraint (length 200 ≤ limit 250)
        let ok_vals = vec![50.0, 30.0, 3.0, 200.0];
        let mut ok_scores = evaluate(&space, &ok_vals);
        let ok_copy = ok_scores.clone();
        penalise_constraints(&space, &ok_vals, &mut ok_scores);
        assert_eq!(ok_scores, ok_copy, "no penalty when within constraints");

        // Violating constraint (length 300 > limit 250)
        let bad_vals = vec![50.0, 30.0, 3.0, 300.0];
        let mut bad_scores = evaluate(&space, &bad_vals);
        penalise_constraints(&space, &bad_vals, &mut bad_scores);
        assert!(bad_scores[0] > ok_copy[0] * 10.0, "penalty must dominate score");
    }

    #[test]
    fn latin_hypercube_no_duplicates() {
        let space = shaft_design_space();
        let samples = latin_hypercube_sample(&space, 100, 99);
        // No two samples should be identical (astronomically unlikely with LHS)
        for i in 0..samples.len() {
            for j in (i + 1)..samples.len() {
                assert_ne!(samples[i], samples[j], "duplicate samples at {} and {}", i, j);
            }
        }
    }

    #[test]
    fn full_pipeline_bracket() {
        let space = bracket_design_space();
        let (candidates, front) = generate_and_rank(&space, 200, 12345);
        assert_eq!(candidates.len(), 200);
        assert!(!front.is_empty());

        // Convert a front member to solid
        let best = &candidates[front[0]];
        let solid = candidate_to_solid(&space, &best.values);
        assert!(solid.face_count() >= 6, "bracket solid should be a box with ≥ 6 faces");
        assert!(solid.is_valid_shell(), "solid must satisfy Euler formula");
    }
}
