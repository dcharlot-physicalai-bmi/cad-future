//! `physical-flow-cad` — Flow matching with projected constraints in a CAD-native latent space.
//!
//! Combines flow matching (straight ODE trajectories from noise to data),
//! a CAD-native token-based latent representation, hard manufacturing constraint
//! projection, and an iterative self-refinement loop.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Flow matching backbone
// ---------------------------------------------------------------------------

/// Noise schedule variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoiseSchedule {
    /// Linear decay from 1.0 to 0.0.
    Linear,
    /// Cosine schedule (smoother at endpoints).
    Cosine,
}

impl Default for NoiseSchedule {
    fn default() -> Self {
        Self::Linear
    }
}

/// Configuration for the flow matching process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Number of denoising steps (default 5).
    pub num_steps: u32,
    /// Noise schedule.
    pub noise_schedule: NoiseSchedule,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            num_steps: 5,
            noise_schedule: NoiseSchedule::Linear,
        }
    }
}

/// A single step along the flow trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStep {
    /// Step index (0-based).
    pub index: u32,
    /// Interpolation parameter t in [0, 1].
    pub t: f64,
    /// Parameter vector at this step.
    pub state: Vec<f64>,
}

/// Linear interpolation along the straight ODE trajectory from `current` to `target`.
///
/// `t` = 0.0 returns `current`, `t` = 1.0 returns `target`.
pub fn flow_step(current: &[f64], target: &[f64], t: f64) -> Vec<f64> {
    assert_eq!(current.len(), target.len(), "dimension mismatch");
    current
        .iter()
        .zip(target.iter())
        .map(|(&c, &tgt)| c + t * (tgt - c))
        .collect()
}

/// Add pseudo-random noise to CAD parameters.
///
/// Uses a simple deterministic LCG seeded from the data so results are reproducible.
pub fn add_noise(data: &[f64], noise_level: f64) -> Vec<f64> {
    let mut seed: u64 = 42;
    data.iter()
        .map(|&v| {
            // LCG step.
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            // Map to [-1, 1].
            let u = ((seed >> 33) as f64) / (u32::MAX as f64) * 2.0 - 1.0;
            v + u * noise_level
        })
        .collect()
}

/// One step of flow matching denoising: move from `noisy` toward the data manifold.
pub fn denoise_step(noisy: &[f64], step: u32, total_steps: u32) -> Vec<f64> {
    assert!(total_steps > 0, "total_steps must be > 0");
    // Each step removes 1/total_steps of the remaining noise,
    // shrinking toward the mean (0.0) as a baseline predictor.
    let alpha = 1.0 - ((step + 1) as f64 / total_steps as f64);
    noisy.iter().map(|&v| v * alpha).collect()
}

// ---------------------------------------------------------------------------
// CAD-native latent space
// ---------------------------------------------------------------------------

/// A single token in the CAD latent vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CadToken {
    SketchLine,
    SketchArc,
    SketchCircle,
    Extrude,
    Revolve,
    Fillet,
    Chamfer,
    Hole,
    Pattern,
    Material,
    /// Numeric dimension value attached to the preceding token.
    Dimension(f64),
}

impl CadToken {
    /// Human-readable keyword for this token kind.
    pub fn keyword(&self) -> &str {
        match self {
            Self::SketchLine => "line",
            Self::SketchArc => "arc",
            Self::SketchCircle => "circle",
            Self::Extrude => "extrude",
            Self::Revolve => "revolve",
            Self::Fillet => "fillet",
            Self::Chamfer => "chamfer",
            Self::Hole => "hole",
            Self::Pattern => "pattern",
            Self::Material => "material",
            Self::Dimension(_) => "dim",
        }
    }
}

/// A parametric CAD representation as a token sequence (the latent space).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadLatent {
    pub tokens: Vec<CadToken>,
}

/// Encode a feature-tree (list of feature description strings) into a token sequence.
pub fn encode_to_latent(features: &[String]) -> CadLatent {
    let mut tokens = Vec::new();
    for feat in features {
        let lower = feat.to_lowercase();
        if lower.contains("line") {
            tokens.push(CadToken::SketchLine);
        }
        if lower.contains("arc") {
            tokens.push(CadToken::SketchArc);
        }
        if lower.contains("circle") {
            tokens.push(CadToken::SketchCircle);
        }
        if lower.contains("extrude") || lower.contains("boss") {
            tokens.push(CadToken::Extrude);
        }
        if lower.contains("revolve") {
            tokens.push(CadToken::Revolve);
        }
        if lower.contains("fillet") {
            tokens.push(CadToken::Fillet);
        }
        if lower.contains("chamfer") {
            tokens.push(CadToken::Chamfer);
        }
        if lower.contains("hole") {
            tokens.push(CadToken::Hole);
        }
        if lower.contains("pattern") || lower.contains("array") {
            tokens.push(CadToken::Pattern);
        }
        if lower.contains("material") {
            tokens.push(CadToken::Material);
        }
        // Extract numeric dimensions.
        for word in lower.split_whitespace() {
            if let Ok(v) = word.trim_end_matches("mm").parse::<f64>() {
                tokens.push(CadToken::Dimension(v));
            }
        }
    }
    CadLatent { tokens }
}

/// Decode a token sequence back into human-readable feature descriptions.
pub fn decode_from_latent(latent: &CadLatent) -> Vec<String> {
    let mut features = Vec::new();
    let mut i = 0;
    let toks = &latent.tokens;
    while i < toks.len() {
        let tok = &toks[i];
        match tok {
            CadToken::Dimension(_) => {
                // Stray dimension — skip.
                i += 1;
                continue;
            }
            _ => {
                let keyword = tok.keyword();
                // Collect trailing dimensions.
                let mut dims = Vec::new();
                let mut j = i + 1;
                while j < toks.len() {
                    if let CadToken::Dimension(v) = toks[j] {
                        dims.push(v);
                        j += 1;
                    } else {
                        break;
                    }
                }
                let desc = if dims.is_empty() {
                    keyword.to_string()
                } else {
                    let dim_str: Vec<String> = dims.iter().map(|d| format!("{d}")).collect();
                    format!("{keyword} ({})", dim_str.join(", "))
                };
                features.push(desc);
                i = j;
            }
        }
    }
    features
}

// ---------------------------------------------------------------------------
// Projected constraints
// ---------------------------------------------------------------------------

/// A hard manufacturing / design constraint that can be projected onto parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectConstraint {
    /// Minimum wall thickness in mm.
    MinWall(f64),
    /// Maximum part weight in kg.
    MaxWeight(f64),
    /// Required material name.
    MaterialMatch(String),
    /// Required manufacturing process (e.g. "CNC", "FDM").
    ManufactureProcess(String),
    /// A named dimension must be within [min, max].
    DimensionRange(String, f64, f64),
}

/// Clamp parameters to satisfy all hard constraints at every flow step.
///
/// Numeric constraints are enforced by clamping relevant parameter indices.
/// For this simplified version, `MinWall` clamps all params to be >= the minimum,
/// `MaxWeight` clamps all params to be <= the maximum, and `DimensionRange` clamps
/// by index position (treating each param as a dimension).
pub fn project_onto_constraints(params: &mut Vec<f64>, constraints: &[ProjectConstraint]) {
    for c in constraints {
        match c {
            ProjectConstraint::MinWall(min) => {
                for p in params.iter_mut() {
                    if *p < *min {
                        *p = *min;
                    }
                }
            }
            ProjectConstraint::MaxWeight(max) => {
                for p in params.iter_mut() {
                    if *p > *max {
                        *p = *max;
                    }
                }
            }
            ProjectConstraint::DimensionRange(_name, lo, hi) => {
                for p in params.iter_mut() {
                    *p = p.clamp(*lo, *hi);
                }
            }
            // Material and process constraints are non-numeric — validated in `is_feasible`.
            ProjectConstraint::MaterialMatch(_) | ProjectConstraint::ManufactureProcess(_) => {}
        }
    }
}

/// Check whether all numeric constraints are satisfied.
pub fn is_feasible(params: &[f64], constraints: &[ProjectConstraint]) -> bool {
    for c in constraints {
        match c {
            ProjectConstraint::MinWall(min) => {
                if params.iter().any(|&p| p < *min) {
                    return false;
                }
            }
            ProjectConstraint::MaxWeight(max) => {
                if params.iter().any(|&p| p > *max) {
                    return false;
                }
            }
            ProjectConstraint::DimensionRange(_name, lo, hi) => {
                if params.iter().any(|&p| p < *lo || p > *hi) {
                    return false;
                }
            }
            // Non-numeric constraints are assumed satisfied at this layer.
            _ => {}
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Self-refinement loop
// ---------------------------------------------------------------------------

/// Configuration for the self-refinement loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementConfig {
    /// Maximum number of refinement iterations.
    pub max_iterations: u32,
    /// Stop when score improvement falls below this threshold.
    pub improvement_threshold: f64,
}

impl Default for RefinementConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            improvement_threshold: 0.01,
        }
    }
}

/// Outcome of a refinement run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementResult {
    /// Number of iterations actually executed.
    pub iterations: u32,
    /// Final quality score in [0, 1].
    pub final_score: f64,
    /// Whether the loop converged before hitting the iteration cap.
    pub converged: bool,
    /// Remaining constraint violations (empty if feasible).
    pub constraint_violations: Vec<String>,
}

/// Score a latent by counting how many tokens have valid structure.
fn score_latent(latent: &CadLatent) -> f64 {
    if latent.tokens.is_empty() {
        return 0.0;
    }
    let valid = latent
        .tokens
        .iter()
        .filter(|t| !matches!(t, CadToken::Dimension(d) if *d < 0.0))
        .count();
    valid as f64 / latent.tokens.len() as f64
}

/// Check constraint violations and return descriptions.
fn check_violations(latent: &CadLatent, constraints: &[ProjectConstraint]) -> Vec<String> {
    let dims: Vec<f64> = latent
        .tokens
        .iter()
        .filter_map(|t| match t {
            CadToken::Dimension(v) => Some(*v),
            _ => None,
        })
        .collect();
    let mut violations = Vec::new();
    for c in constraints {
        match c {
            ProjectConstraint::MinWall(min) => {
                if dims.iter().any(|&d| d < *min) {
                    violations.push(format!("wall thickness below {min} mm"));
                }
            }
            ProjectConstraint::MaxWeight(max) => {
                if dims.iter().any(|&d| d > *max) {
                    violations.push(format!("weight exceeds {max} kg"));
                }
            }
            ProjectConstraint::DimensionRange(name, lo, hi) => {
                if dims.iter().any(|&d| d < *lo || d > *hi) {
                    violations.push(format!("{name} outside [{lo}, {hi}]"));
                }
            }
            _ => {}
        }
    }
    violations
}

/// Run the self-refinement loop: repeatedly fix constraint violations and improve quality.
pub fn refine(
    latent: &mut CadLatent,
    constraints: &[ProjectConstraint],
    config: &RefinementConfig,
) -> RefinementResult {
    let mut prev_score = score_latent(latent);
    let mut iterations = 0;
    let mut converged = false;

    for _ in 0..config.max_iterations {
        iterations += 1;
        // Project dimension tokens onto constraints.
        for tok in latent.tokens.iter_mut() {
            if let CadToken::Dimension(v) = tok {
                for c in constraints {
                    match c {
                        ProjectConstraint::MinWall(min) => {
                            if *v < *min {
                                *v = *min;
                            }
                        }
                        ProjectConstraint::MaxWeight(max) => {
                            if *v > *max {
                                *v = *max;
                            }
                        }
                        ProjectConstraint::DimensionRange(_, lo, hi) => {
                            *v = v.clamp(*lo, *hi);
                        }
                        _ => {}
                    }
                }
                // Clamp negative dimensions to zero.
                if *v < 0.0 {
                    *v = 0.0;
                }
            }
        }
        let new_score = score_latent(latent);
        if (new_score - prev_score).abs() < config.improvement_threshold {
            converged = true;
            prev_score = new_score;
            break;
        }
        prev_score = new_score;
    }

    let violations = check_violations(latent, constraints);
    RefinementResult {
        iterations,
        final_score: prev_score,
        converged,
        constraint_violations: violations,
    }
}

// ---------------------------------------------------------------------------
// Full generation pipeline
// ---------------------------------------------------------------------------

/// Result of the full generation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    /// The generated latent representation.
    pub latent: CadLatent,
    /// Decoded feature descriptions.
    pub features: Vec<String>,
    /// CFL code snippet (simplified).
    pub cfl_code: String,
    /// Quality score in [0, 1].
    pub score: f64,
    /// Number of flow steps actually used.
    pub steps_used: u32,
}

/// Full generation pipeline: prompt -> flow matching -> constraint projection -> output.
pub fn generate(
    prompt: &str,
    constraints: &[ProjectConstraint],
    config: &FlowConfig,
) -> GenerationResult {
    // Step 1: Seed latent from the prompt (simple keyword extraction).
    let seed_features: Vec<String> = prompt
        .split([',', '.', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut latent = encode_to_latent(&seed_features);

    // Step 2: Extract dimension parameters for flow matching.
    let mut params: Vec<f64> = latent
        .tokens
        .iter()
        .filter_map(|t| match t {
            CadToken::Dimension(v) => Some(*v),
            _ => None,
        })
        .collect();

    // Step 3: Add noise then denoise through flow steps.
    if !params.is_empty() {
        let noisy = add_noise(&params, 0.1);
        params = noisy;
        for step in 0..config.num_steps {
            params = denoise_step(&params, step, config.num_steps);
            project_onto_constraints(&mut params, constraints);
        }
        // Write params back into latent.
        let mut pi = 0;
        for tok in latent.tokens.iter_mut() {
            if let CadToken::Dimension(v) = tok {
                if pi < params.len() {
                    *v = params[pi];
                    pi += 1;
                }
            }
        }
    }

    // Step 4: Self-refinement.
    let refinement_cfg = RefinementConfig {
        max_iterations: 3,
        improvement_threshold: 0.001,
    };
    let _refinement = refine(&mut latent, constraints, &refinement_cfg);

    // Step 5: Decode.
    let features = decode_from_latent(&latent);
    let score = score_latent(&latent);

    // Step 6: Generate CFL code snippet.
    let cfl_lines: Vec<String> = features
        .iter()
        .map(|f| format!("  feature(\"{f}\");"))
        .collect();
    let cfl_code = format!("part {{\n{}\n}}", cfl_lines.join("\n"));

    GenerationResult {
        latent,
        features,
        cfl_code,
        score,
        steps_used: config.num_steps,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_step_interpolation() {
        let a = vec![0.0, 0.0];
        let b = vec![10.0, 20.0];
        let mid = flow_step(&a, &b, 0.5);
        assert!((mid[0] - 5.0).abs() < 1e-10);
        assert!((mid[1] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn flow_step_endpoints() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let start = flow_step(&a, &b, 0.0);
        let end = flow_step(&a, &b, 1.0);
        assert_eq!(start, a);
        assert_eq!(end, b);
    }

    #[test]
    fn add_noise_preserves_length() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let noisy = add_noise(&data, 0.1);
        assert_eq!(noisy.len(), data.len());
    }

    #[test]
    fn add_noise_is_deterministic() {
        let data = vec![5.0, 10.0];
        let a = add_noise(&data, 1.0);
        let b = add_noise(&data, 1.0);
        assert_eq!(a, b);
    }

    #[test]
    fn denoise_step_reduces_magnitude() {
        let noisy = vec![10.0, -10.0, 5.0];
        let denoised = denoise_step(&noisy, 4, 5);
        // At step 4/5, alpha = 1 - 5/5 = 0 -> everything goes to 0.
        for v in &denoised {
            assert!(v.abs() < 1e-10);
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let features = vec![
            "extrude 50mm".to_string(),
            "hole 8mm".to_string(),
            "fillet 2mm".to_string(),
        ];
        let latent = encode_to_latent(&features);
        assert!(!latent.tokens.is_empty());
        let decoded = decode_from_latent(&latent);
        assert!(!decoded.is_empty());
        // Should recover keywords.
        let all = decoded.join(" ");
        assert!(all.contains("extrude"));
        assert!(all.contains("hole"));
        assert!(all.contains("fillet"));
    }

    #[test]
    fn cad_token_keywords() {
        assert_eq!(CadToken::Extrude.keyword(), "extrude");
        assert_eq!(CadToken::Hole.keyword(), "hole");
        assert_eq!(CadToken::Dimension(5.0).keyword(), "dim");
    }

    #[test]
    fn project_min_wall() {
        let mut params = vec![0.5, 1.0, 3.0];
        project_onto_constraints(&mut params, &[ProjectConstraint::MinWall(1.0)]);
        assert!(params.iter().all(|&p| p >= 1.0));
    }

    #[test]
    fn is_feasible_checks_correctly() {
        let params = vec![2.0, 3.0, 4.0];
        assert!(is_feasible(&params, &[ProjectConstraint::MinWall(1.0)]));
        assert!(!is_feasible(&params, &[ProjectConstraint::MaxWeight(3.0)]));
    }

    #[test]
    fn refinement_converges() {
        let mut latent = CadLatent {
            tokens: vec![
                CadToken::Extrude,
                CadToken::Dimension(50.0),
                CadToken::Hole,
                CadToken::Dimension(8.0),
            ],
        };
        let constraints = vec![ProjectConstraint::MinWall(1.0)];
        let config = RefinementConfig::default();
        let result = refine(&mut latent, &constraints, &config);
        assert!(result.converged);
        assert!(result.final_score > 0.0);
        assert!(result.constraint_violations.is_empty());
    }

    #[test]
    fn refinement_fixes_violations() {
        let mut latent = CadLatent {
            tokens: vec![
                CadToken::Extrude,
                CadToken::Dimension(-5.0), // negative -> violation
                CadToken::Hole,
                CadToken::Dimension(0.3), // below min wall
            ],
        };
        let constraints = vec![ProjectConstraint::MinWall(1.0)];
        let config = RefinementConfig { max_iterations: 5, improvement_threshold: 0.001 };
        let result = refine(&mut latent, &constraints, &config);
        assert!(result.constraint_violations.is_empty());
    }

    #[test]
    fn generate_pipeline_produces_output() {
        let result = generate(
            "extrude 50mm, hole 8mm, fillet 2mm",
            &[ProjectConstraint::MinWall(1.0)],
            &FlowConfig::default(),
        );
        assert!(!result.features.is_empty());
        assert!(!result.cfl_code.is_empty());
        assert!(result.cfl_code.contains("part"));
        assert!(result.steps_used == 5);
        assert!(result.score > 0.0);
    }

    #[test]
    fn flow_config_default() {
        let cfg = FlowConfig::default();
        assert_eq!(cfg.num_steps, 5);
        assert!(matches!(cfg.noise_schedule, NoiseSchedule::Linear));
    }
}
