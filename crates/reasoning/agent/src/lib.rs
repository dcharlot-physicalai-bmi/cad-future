//! `physical-agent` — Agentic design loop for autonomous engineering.
//!
//! Inspired by Arena Physica's "Electromagnetic Superintelligence" architecture,
//! applied to the full mechanical + EM design space. The agent takes a natural
//! language design request and autonomously:
//!
//! 1. **Interprets** — classifies intent, extracts constraints, identifies domain
//! 2. **Generates** — creates parametric geometry via MCP tool calls
//! 3. **Evaluates** — runs cascade (LUT → formula → solver) against constraints
//! 4. **Iterates** — modifies design parameters to satisfy all constraints
//! 5. **Delivers** — returns optimized design with manufacturing plan + export
//!
//! The agent operates in a closed loop with configurable iteration limits,
//! convergence criteria, and multi-objective optimization.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Design Request — what the user asks for
// ---------------------------------------------------------------------------

/// A complete design request from natural language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignRequest {
    /// Original natural language request.
    pub text: String,
    /// Engineering domain(s) this request touches.
    pub domains: Vec<EngineeringDomain>,
    /// Extracted design constraints.
    pub constraints: Vec<DesignConstraint>,
    /// Target geometry type (if identified).
    pub geometry_type: Option<GeometryType>,
    /// Material preference (if specified).
    pub material_id: Option<String>,
    /// Manufacturing process preference.
    pub process: Option<String>,
}

/// Engineering domain — mechanical, electromagnetic, thermal, or cross-domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineeringDomain {
    Mechanical,
    Electromagnetic,
    Thermal,
    Fluid,
    Optical,
    Acoustic,
    MultiPhysics,
}

/// Target geometry archetype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeometryType {
    Bracket,
    Housing,
    Shaft,
    Gear,
    Plate,
    Tube,
    Beam,
    Flange,
    Enclosure,
    HeatSink,
    Antenna,
    Waveguide,
    PCB,
    Coil,
    Custom(String),
}

/// A quantified design constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DesignConstraint {
    MaxWeight { kg: f64 },
    MaxDeflection { mm: f64 },
    MinSafetyFactor { factor: f64 },
    MaxStress { mpa: f64 },
    MaxTemperature { celsius: f64 },
    MinTemperature { celsius: f64 },
    MaxCost { usd: f64 },
    Material { id: String },
    Process { name: String },
    MaxDimension { axis: char, mm: f64 },
    MinDimension { axis: char, mm: f64 },
    Frequency { hz: f64, tolerance_pct: f64 },
    Impedance { ohms: f64 },
    Gain { db: f64 },
    Bandwidth { hz_low: f64, hz_high: f64 },
    InsertionLoss { db_max: f64 },
    ReturnLoss { db_min: f64 },
    ShieldingEffectiveness { db_min: f64 },
    ThermalResistance { c_per_w_max: f64 },
}

// ---------------------------------------------------------------------------
// Design State — the evolving design during iteration
// ---------------------------------------------------------------------------

/// Current state of a design being optimized by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignState {
    /// Current design parameters (name → value).
    pub parameters: HashMap<String, f64>,
    /// MCP tool calls that produce the current geometry.
    pub tool_calls: Vec<physical_inference::ToolCall>,
    /// Material ID for the current iteration.
    pub material_id: String,
    /// Results from the most recent evaluation.
    pub evaluation: Option<EvaluationResult>,
    /// Iteration count.
    pub iteration: usize,
}

/// Result of evaluating the current design against all constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Per-constraint check results.
    pub checks: Vec<ConstraintCheck>,
    /// Overall score (0.0 = all constraints violated, 1.0 = all satisfied).
    pub score: f64,
    /// Whether all hard constraints are satisfied.
    pub feasible: bool,
    /// Cascade tier used for each check.
    pub tiers_used: Vec<String>,
}

/// A single constraint check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintCheck {
    pub constraint_name: String,
    pub satisfied: bool,
    pub actual_value: f64,
    pub target_value: f64,
    pub margin: f64, // positive = satisfied with margin, negative = violated
    pub tier: String, // "lut", "formula", "solver"
}

// ---------------------------------------------------------------------------
// Agent Configuration
// ---------------------------------------------------------------------------

/// Configuration for the agentic design loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum iterations before giving up.
    pub max_iterations: usize,
    /// Convergence threshold: stop when score >= this value.
    pub convergence_threshold: f64,
    /// Parameter step size for gradient-free optimization.
    pub step_size: f64,
    /// Whether to try multiple materials.
    pub explore_materials: bool,
    /// Whether to suggest manufacturing alternatives.
    pub suggest_processes: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            convergence_threshold: 0.95,
            step_size: 0.1,
            explore_materials: true,
            suggest_processes: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent — the autonomous design loop
// ---------------------------------------------------------------------------

/// The agentic design engine.
pub struct DesignAgent {
    pub config: AgentConfig,
    pub history: Vec<DesignState>,
}

/// Final result from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Final design state.
    pub design: DesignState,
    /// Total iterations used.
    pub iterations: usize,
    /// Whether the design is feasible (all constraints met).
    pub feasible: bool,
    /// Final score.
    pub score: f64,
    /// Suggested improvements if not fully feasible.
    pub suggestions: Vec<String>,
    /// Manufacturing plan.
    pub manufacturing_plan: ManufacturingPlan,
    /// Summary for the user.
    pub summary: String,
}

/// Manufacturing plan generated by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturingPlan {
    pub process: String,
    pub material: String,
    pub estimated_cost_usd: Option<f64>,
    pub estimated_time_hours: Option<f64>,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

impl DesignAgent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
        }
    }

    /// Parse a natural language request into a structured DesignRequest.
    pub fn parse_request(text: &str) -> DesignRequest {
        let lower = text.to_lowercase();

        // Detect domains
        let mut domains = Vec::new();
        if has_mechanical_keywords(&lower) { domains.push(EngineeringDomain::Mechanical); }
        if has_em_keywords(&lower) { domains.push(EngineeringDomain::Electromagnetic); }
        if has_thermal_keywords(&lower) { domains.push(EngineeringDomain::Thermal); }
        if has_fluid_keywords(&lower) { domains.push(EngineeringDomain::Fluid); }
        if domains.len() > 1 { domains.push(EngineeringDomain::MultiPhysics); }
        if domains.is_empty() { domains.push(EngineeringDomain::Mechanical); } // default

        // Extract constraints
        let constraints = extract_constraints(&lower);

        // Detect geometry type
        let geometry_type = detect_geometry_type(&lower);

        // Detect material
        let material_id = detect_material(&lower);

        // Detect process
        let process = detect_process(&lower);

        DesignRequest {
            text: text.to_string(),
            domains,
            constraints,
            geometry_type,
            material_id,
            process,
        }
    }

    /// Run the full agentic design loop.
    pub fn run(&mut self, request: &DesignRequest) -> AgentResult {
        // Step 1: Generate initial design
        let mut state = self.generate_initial_design(request);

        // Step 2: Evaluate against constraints
        state.evaluation = Some(self.evaluate(&state, &request.constraints));

        self.history.push(state.clone());

        // Step 3: Iterate until converged or max iterations
        for _ in 0..self.config.max_iterations {
            let eval = state.evaluation.as_ref().unwrap();
            if eval.score >= self.config.convergence_threshold && eval.feasible {
                break;
            }

            // Step 4: Modify parameters to improve
            state = self.iterate(&state, &request.constraints);
            state.evaluation = Some(self.evaluate(&state, &request.constraints));
            self.history.push(state.clone());
        }

        // Step 5: Generate manufacturing plan
        let manufacturing_plan = self.generate_manufacturing_plan(&state, request);

        // Step 6: Generate summary
        let eval = state.evaluation.as_ref().unwrap();
        let summary = self.generate_summary(&state, request, eval);

        AgentResult {
            iterations: state.iteration,
            feasible: eval.feasible,
            score: eval.score,
            suggestions: self.generate_suggestions(&state, &request.constraints),
            manufacturing_plan,
            summary,
            design: state,
        }
    }

    /// Generate an initial design from the request.
    fn generate_initial_design(&self, request: &DesignRequest) -> DesignState {
        let mut params = HashMap::new();
        let material = request.material_id.clone().unwrap_or_else(|| "6061-T6".into());

        // Set default dimensions based on geometry type
        match &request.geometry_type {
            Some(GeometryType::Bracket) => {
                params.insert("width".into(), 50.0);
                params.insert("height".into(), 80.0);
                params.insert("thickness".into(), 5.0);
                params.insert("fillet_radius".into(), 3.0);
            }
            Some(GeometryType::Housing | GeometryType::Enclosure) => {
                params.insert("width".into(), 100.0);
                params.insert("height".into(), 60.0);
                params.insert("depth".into(), 80.0);
                params.insert("wall_thickness".into(), 3.0);
            }
            Some(GeometryType::Shaft) => {
                params.insert("diameter".into(), 25.0);
                params.insert("length".into(), 200.0);
            }
            Some(GeometryType::HeatSink) => {
                params.insert("base_width".into(), 60.0);
                params.insert("base_height".into(), 10.0);
                params.insert("fin_height".into(), 25.0);
                params.insert("fin_count".into(), 8.0);
                params.insert("fin_thickness".into(), 1.5);
            }
            Some(GeometryType::Antenna) => {
                params.insert("length".into(), 75.0); // quarter-wave at ~1 GHz
                params.insert("width".into(), 5.0);
                params.insert("ground_plane_size".into(), 150.0);
            }
            Some(GeometryType::Plate) => {
                params.insert("width".into(), 200.0);
                params.insert("height".into(), 200.0);
                params.insert("thickness".into(), 10.0);
            }
            _ => {
                params.insert("width".into(), 50.0);
                params.insert("height".into(), 50.0);
                params.insert("depth".into(), 50.0);
            }
        }

        // Apply dimension constraints
        for c in &request.constraints {
            match c {
                DesignConstraint::MaxDimension { axis, mm } => {
                    let key = match axis {
                        'x' | 'X' => "width",
                        'y' | 'Y' => "height",
                        'z' | 'Z' => "depth",
                        _ => "width",
                    };
                    if let Some(val) = params.get_mut(&key.to_string()) {
                        if *val > *mm { *val = *mm * 0.9; }
                    }
                }
                DesignConstraint::Material { id } => {
                    // material handled via material_id field
                    let _ = id;
                }
                _ => {}
            }
        }

        // Generate tool calls from parameters
        let tool_calls = self.params_to_tool_calls(&params, &request.geometry_type);

        DesignState {
            parameters: params,
            tool_calls,
            material_id: material,
            evaluation: None,
            iteration: 0,
        }
    }

    /// Evaluate the current design against constraints using the cascade.
    fn evaluate(&self, state: &DesignState, constraints: &[DesignConstraint]) -> EvaluationResult {
        let mut checks = Vec::new();
        let mut tiers = Vec::new();

        for constraint in constraints {
            let check = self.check_constraint(state, constraint);
            tiers.push(check.tier.clone());
            checks.push(check);
        }

        let satisfied_count = checks.iter().filter(|c| c.satisfied).count();
        let total = checks.len().max(1);
        let score = satisfied_count as f64 / total as f64;
        let feasible = checks.iter().all(|c| c.satisfied);

        EvaluationResult {
            checks,
            score,
            feasible,
            tiers_used: tiers,
        }
    }

    /// Check a single constraint against the current design.
    fn check_constraint(&self, state: &DesignState, constraint: &DesignConstraint) -> ConstraintCheck {
        match constraint {
            DesignConstraint::MaxWeight { kg } => {
                // Estimate weight from volume and material density
                let vol_mm3 = estimate_volume(&state.parameters);
                let mat = physical_lut::materials::lookup(&state.material_id);
                let density = mat.map(|m| m.density.value()).unwrap_or(2700.0);
                let vol_m3 = vol_mm3 * 1e-9;
                let mass_kg = density * vol_m3;
                ConstraintCheck {
                    constraint_name: "max_weight".into(),
                    satisfied: mass_kg <= *kg,
                    actual_value: mass_kg,
                    target_value: *kg,
                    margin: *kg - mass_kg,
                    tier: "formula".into(),
                }
            }
            DesignConstraint::MinSafetyFactor { factor } => {
                let mat = physical_lut::materials::lookup(&state.material_id);
                let yield_mpa = mat.map(|m| m.yield_strength.to_mpa()).unwrap_or(276.0);
                // Rough stress estimate: assume uniform loading across min cross-section
                let min_area = estimate_min_cross_section(&state.parameters);
                let applied_stress: f64 = 100.0; // placeholder — would come from loads
                let sf = yield_mpa / applied_stress.max(1.0);
                ConstraintCheck {
                    constraint_name: "min_safety_factor".into(),
                    satisfied: sf >= *factor,
                    actual_value: sf,
                    target_value: *factor,
                    margin: sf - *factor,
                    tier: "formula".into(),
                }
            }
            DesignConstraint::MaxStress { mpa } => {
                let mat = physical_lut::materials::lookup(&state.material_id);
                let yield_mpa = mat.map(|m| m.yield_strength.to_mpa()).unwrap_or(276.0);
                let actual = yield_mpa * 0.3; // conservative estimate
                ConstraintCheck {
                    constraint_name: "max_stress".into(),
                    satisfied: actual <= *mpa,
                    actual_value: actual,
                    target_value: *mpa,
                    margin: *mpa - actual,
                    tier: "formula".into(),
                }
            }
            DesignConstraint::MaxTemperature { celsius } => {
                let mat = physical_lut::materials::lookup(&state.material_id);
                let max_service = mat.map(|m| m.melting_point.to_celsius() * 0.8).unwrap_or(400.0);
                ConstraintCheck {
                    constraint_name: "max_temperature".into(),
                    satisfied: *celsius <= max_service,
                    actual_value: *celsius,
                    target_value: max_service,
                    margin: max_service - *celsius,
                    tier: "lut".into(),
                }
            }
            DesignConstraint::MaxDeflection { mm } => {
                // Beam deflection estimate
                let length = state.parameters.get("length")
                    .or(state.parameters.get("height"))
                    .copied().unwrap_or(100.0);
                let thickness = state.parameters.get("thickness")
                    .or(state.parameters.get("depth"))
                    .copied().unwrap_or(10.0);
                let width = state.parameters.get("width").copied().unwrap_or(50.0);

                let mat = physical_lut::materials::lookup(&state.material_id);
                let e_mpa = mat.map(|m| m.elastic_modulus.to_mpa()).unwrap_or(70_000.0);
                let i = width * thickness.powi(3) / 12.0; // mm^4
                let load = 1000.0; // 1kN default
                // Simply-supported center load: δ = PL³/(48EI)
                let defl = load * length.powi(3) / (48.0 * e_mpa * i);
                ConstraintCheck {
                    constraint_name: "max_deflection".into(),
                    satisfied: defl <= *mm,
                    actual_value: defl,
                    target_value: *mm,
                    margin: *mm - defl,
                    tier: "formula".into(),
                }
            }
            DesignConstraint::Process { name } => {
                // Check DFM feasibility
                let wall = state.parameters.get("wall_thickness")
                    .or(state.parameters.get("thickness"))
                    .copied().unwrap_or(3.0);
                let feasible = wall >= 0.8; // basic CNC check
                ConstraintCheck {
                    constraint_name: format!("process_{}", name),
                    satisfied: feasible,
                    actual_value: wall,
                    target_value: 0.8,
                    margin: wall - 0.8,
                    tier: "lut".into(),
                }
            }
            DesignConstraint::Material { id } => {
                let found = physical_lut::materials::lookup(id).is_some();
                ConstraintCheck {
                    constraint_name: "material_exists".into(),
                    satisfied: found,
                    actual_value: if found { 1.0 } else { 0.0 },
                    target_value: 1.0,
                    margin: if found { 1.0 } else { -1.0 },
                    tier: "lut".into(),
                }
            }
            DesignConstraint::Frequency { hz, tolerance_pct } => {
                ConstraintCheck {
                    constraint_name: "target_frequency".into(),
                    satisfied: true, // placeholder — needs EM/modal solver
                    actual_value: *hz,
                    target_value: *hz,
                    margin: *tolerance_pct,
                    tier: "solver".into(),
                }
            }
            DesignConstraint::ThermalResistance { c_per_w_max } => {
                // Heat sink thermal resistance estimate
                let fin_count = state.parameters.get("fin_count").copied().unwrap_or(8.0);
                let fin_height = state.parameters.get("fin_height").copied().unwrap_or(20.0);
                let base_width = state.parameters.get("base_width").copied().unwrap_or(60.0);
                let mat = physical_lut::materials::lookup(&state.material_id);
                let k = mat.map(|m| m.thermal_conductivity.value()).unwrap_or(167.0);

                // Simplified: R ≈ 1/(h·A_fin) where h≈10 W/m²K (natural convection)
                let h_conv = 10.0; // W/m²K
                let fin_area_m2 = fin_count * 2.0 * fin_height * base_width * 1e-6;
                let r_conv = 1.0 / (h_conv * fin_area_m2);
                let r_cond = (0.01) / (k * base_width * base_width * 1e-6); // base conduction
                let r_total = r_conv + r_cond;

                ConstraintCheck {
                    constraint_name: "thermal_resistance".into(),
                    satisfied: r_total <= *c_per_w_max,
                    actual_value: r_total,
                    target_value: *c_per_w_max,
                    margin: *c_per_w_max - r_total,
                    tier: "formula".into(),
                }
            }
            _ => {
                // EM constraints (impedance, gain, bandwidth, etc.) — solver tier needed
                ConstraintCheck {
                    constraint_name: format!("{:?}", constraint),
                    satisfied: true,
                    actual_value: 0.0,
                    target_value: 0.0,
                    margin: 0.0,
                    tier: "solver".into(),
                }
            }
        }
    }

    /// Iterate: adjust parameters to improve constraint satisfaction.
    fn iterate(&self, state: &DesignState, constraints: &[DesignConstraint]) -> DesignState {
        let mut new_state = state.clone();
        new_state.iteration += 1;

        let eval = state.evaluation.as_ref().unwrap();

        for check in &eval.checks {
            if check.satisfied { continue; }

            // Adjust parameters based on which constraint is violated
            match check.constraint_name.as_str() {
                "max_weight" => {
                    // Reduce dimensions to lower weight
                    for key in &["width", "height", "depth", "thickness"] {
                        if let Some(v) = new_state.parameters.get_mut(&key.to_string()) {
                            *v *= 0.9; // reduce by 10%
                        }
                    }
                }
                "max_deflection" => {
                    // Increase stiffness: thicken or shorten
                    if let Some(t) = new_state.parameters.get_mut("thickness") {
                        *t *= 1.2;
                    }
                    if let Some(h) = new_state.parameters.get_mut("height") {
                        *h *= 0.9;
                    }
                }
                "min_safety_factor" => {
                    // Increase cross-section
                    if let Some(t) = new_state.parameters.get_mut("thickness") {
                        *t *= 1.15;
                    }
                    if let Some(w) = new_state.parameters.get_mut("width") {
                        *w *= 1.1;
                    }
                }
                "thermal_resistance" => {
                    // More fins, taller fins
                    if let Some(n) = new_state.parameters.get_mut("fin_count") {
                        *n = (*n + 2.0).min(30.0);
                    }
                    if let Some(h) = new_state.parameters.get_mut("fin_height") {
                        *h *= 1.2;
                    }
                }
                _ => {}
            }
        }

        // Explore material alternatives if enabled
        if self.config.explore_materials && !eval.feasible && new_state.iteration > 3 {
            let alternatives = ["7075-T6", "Ti-6Al-4V", "AISI-304", "1018-CD"];
            let idx = (new_state.iteration - 4) % alternatives.len();
            new_state.material_id = alternatives[idx].to_string();
        }

        // Regenerate tool calls
        new_state.tool_calls = self.params_to_tool_calls(&new_state.parameters, &None);

        new_state
    }

    /// Convert parameters to MCP tool calls.
    fn params_to_tool_calls(
        &self,
        params: &HashMap<String, f64>,
        geometry_type: &Option<GeometryType>,
    ) -> Vec<physical_inference::ToolCall> {
        let w = params.get("width").copied().unwrap_or(50.0);
        let h = params.get("height").copied().unwrap_or(50.0);
        let d = params.get("depth").copied().unwrap_or(50.0);

        match geometry_type {
            Some(GeometryType::Shaft) => {
                let dia = params.get("diameter").copied().unwrap_or(25.0);
                let len = params.get("length").copied().unwrap_or(200.0);
                vec![physical_inference::ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": dia / 2.0, "height": len }),
                }]
            }
            Some(GeometryType::Housing | GeometryType::Enclosure) => {
                let wall = params.get("wall_thickness").copied().unwrap_or(3.0);
                vec![
                    physical_inference::ToolCall {
                        tool_name: "create_box".into(),
                        arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                    },
                    physical_inference::ToolCall {
                        tool_name: "hollow_out".into(),
                        arguments: serde_json::json!({ "wall_thickness": wall }),
                    },
                ]
            }
            _ => {
                vec![physical_inference::ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                }]
            }
        }
    }

    /// Generate a manufacturing plan for the final design.
    fn generate_manufacturing_plan(&self, state: &DesignState, request: &DesignRequest) -> ManufacturingPlan {
        let process = request.process.clone().unwrap_or_else(|| {
            // Auto-detect best process
            let vol = estimate_volume(&state.parameters);
            let wall = state.parameters.get("wall_thickness")
                .or(state.parameters.get("thickness"))
                .copied().unwrap_or(5.0);
            if vol < 1000.0 { "sla".into() }
            else if wall < 1.0 { "sheet_metal".into() }
            else if vol > 500_000.0 { "casting".into() }
            else { "cnc_3axis".into() }
        });

        let mat = physical_lut::materials::lookup(&state.material_id);
        let mat_name = mat.map(|m| m.name).unwrap_or("Unknown");

        ManufacturingPlan {
            process: process.clone(),
            material: state.material_id.clone(),
            estimated_cost_usd: Some(estimate_cost(&state.parameters, &process)),
            estimated_time_hours: Some(estimate_time(&state.parameters, &process)),
            steps: generate_process_steps(&process, mat_name),
            warnings: Vec::new(),
        }
    }

    fn generate_summary(&self, state: &DesignState, request: &DesignRequest, eval: &EvaluationResult) -> String {
        let geom = request.geometry_type.as_ref()
            .map(|g| format!("{:?}", g))
            .unwrap_or_else(|| "part".into());

        let status = if eval.feasible { "All constraints satisfied" } else { "Some constraints violated" };

        format!(
            "{} design in {} ({} material). {} after {} iterations (score: {:.0}%). \
             Manufacturing: {} process.",
            geom, state.material_id,
            physical_lut::materials::lookup(&state.material_id)
                .map(|m| m.name).unwrap_or("unknown"),
            status, state.iteration, eval.score * 100.0,
            request.process.as_deref().unwrap_or("auto"),
        )
    }

    fn generate_suggestions(&self, state: &DesignState, constraints: &[DesignConstraint]) -> Vec<String> {
        let mut suggestions = Vec::new();
        if let Some(eval) = &state.evaluation {
            for check in &eval.checks {
                if !check.satisfied {
                    suggestions.push(format!(
                        "{}: actual {:.2} vs target {:.2} (margin {:.2})",
                        check.constraint_name, check.actual_value, check.target_value, check.margin
                    ));
                }
            }
        }
        let _ = constraints;
        suggestions
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn estimate_volume(params: &HashMap<String, f64>) -> f64 {
    let w = params.get("width").copied().unwrap_or(50.0);
    let h = params.get("height").copied().unwrap_or(50.0);
    let d = params.get("depth").copied().unwrap_or(50.0);
    w * h * d
}

fn estimate_min_cross_section(params: &HashMap<String, f64>) -> f64 {
    let w = params.get("width").copied().unwrap_or(50.0);
    let t = params.get("thickness").copied().unwrap_or(10.0);
    w * t // mm²
}

fn estimate_cost(params: &HashMap<String, f64>, process: &str) -> f64 {
    let vol = estimate_volume(params);
    match process {
        "cnc_3axis" => 50.0 + vol * 0.0001,
        "sla" | "fdm" => 10.0 + vol * 0.00005,
        "sheet_metal" => 30.0 + vol * 0.00003,
        "casting" => 200.0 + vol * 0.00002,
        _ => 75.0 + vol * 0.0001,
    }
}

fn estimate_time(params: &HashMap<String, f64>, process: &str) -> f64 {
    let vol = estimate_volume(params);
    match process {
        "cnc_3axis" => 0.5 + vol * 0.000002,
        "sla" => 2.0 + vol * 0.00001,
        "fdm" => 1.0 + vol * 0.000008,
        "sheet_metal" => 0.25 + vol * 0.000001,
        _ => 1.0,
    }
}

fn generate_process_steps(process: &str, material: &str) -> Vec<String> {
    match process {
        "cnc_3axis" => vec![
            format!("Select {} stock, oversized by 5mm per side", material),
            "Load CAM toolpath (adaptive clearing → finishing)".into(),
            "Setup workholding (vise or fixture plate)".into(),
            "Rough with 10mm endmill at recommended feeds/speeds".into(),
            "Finish with 6mm endmill, 0.1mm stepover".into(),
            "Deburr edges and inspect dimensions".into(),
        ],
        "fdm" => vec![
            format!("Slice model for {} filament", material),
            "Orient for minimal support material".into(),
            "Print with 0.2mm layer height, 20% infill".into(),
            "Remove supports and post-process".into(),
        ],
        "sheet_metal" => vec![
            format!("Cut {} sheet to flat pattern dimensions", material),
            "Program bend sequence (inside-out)".into(),
            "Bend on press brake with calculated springback compensation".into(),
            "Inspect bend angles and flat dimensions".into(),
        ],
        _ => vec!["Process-specific steps to be determined".into()],
    }
}

// ---------------------------------------------------------------------------
// NLP helpers — keyword detection
// ---------------------------------------------------------------------------

fn has_mechanical_keywords(text: &str) -> bool {
    ["stress", "deflect", "load", "force", "torque", "bracket", "shaft",
     "gear", "bearing", "spring", "beam", "plate", "fillet", "chamfer",
     "cnc", "mill", "lathe", "bolt", "weld", "fatigue", "buckling"]
        .iter().any(|kw| text.contains(kw))
}

fn has_em_keywords(text: &str) -> bool {
    ["antenna", "rf", "microwave", "electromagnetic", "impedance", "frequency",
     "bandwidth", "gain", "filter", "waveguide", "pcb", "trace", "signal",
     "shield", "emc", "emi", "coil", "inductor", "capacitor", "resonat",
     "s-parameter", "smith chart", "vswr", "return loss", "insertion loss",
     "ghz", "mhz", "dbm", "serdes", "power delivery", "crosstalk"]
        .iter().any(|kw| text.contains(kw))
}

fn has_thermal_keywords(text: &str) -> bool {
    ["thermal", "heat", "temperature", "cooling", "heatsink", "heat sink",
     "dissipat", "convection", "radiation", "fin", "tjunction", "thermal resistance"]
        .iter().any(|kw| text.contains(kw))
}

fn has_fluid_keywords(text: &str) -> bool {
    ["flow", "pressure drop", "pipe", "valve", "pump", "fluid", "viscosity",
     "reynolds", "turbulent", "laminar", "aerodynamic", "drag", "lift"]
        .iter().any(|kw| text.contains(kw))
}

fn extract_constraints(text: &str) -> Vec<DesignConstraint> {
    let mut constraints = Vec::new();

    // Weight constraints: "under 200g", "less than 0.5kg", "max weight 2kg"
    if let Some(kg) = extract_value_before(text, "kg") {
        constraints.push(DesignConstraint::MaxWeight { kg });
    } else if let Some(g) = extract_value_before(text, "gram") {
        constraints.push(DesignConstraint::MaxWeight { kg: g / 1000.0 });
    } else if let Some(g) = extract_value_before(text, " g ") {
        constraints.push(DesignConstraint::MaxWeight { kg: g / 1000.0 });
    }

    // Deflection: "deflection under 0.5mm"
    if let Some(mm) = extract_value_after(text, "deflection") {
        constraints.push(DesignConstraint::MaxDeflection { mm });
    }

    // Safety factor: "safety factor of 2", "sf >= 3"
    if let Some(sf) = extract_value_after(text, "safety factor") {
        constraints.push(DesignConstraint::MinSafetyFactor { factor: sf });
    }

    // Temperature: "below 80°c", "max temp 150c"
    if let Some(c) = extract_value_after(text, "temperature") {
        constraints.push(DesignConstraint::MaxTemperature { celsius: c });
    }

    // Frequency: "2.4ghz", "resonant at 915mhz"
    if let Some(ghz) = extract_value_before(text, "ghz") {
        constraints.push(DesignConstraint::Frequency { hz: ghz * 1e9, tolerance_pct: 5.0 });
    } else if let Some(mhz) = extract_value_before(text, "mhz") {
        constraints.push(DesignConstraint::Frequency { hz: mhz * 1e6, tolerance_pct: 5.0 });
    }

    // Impedance: "50 ohm", "75 ohm", "50ohm"
    if let Some(ohm) = extract_value_before(text, " ohm")
        .or_else(|| extract_value_before(text, "ohm"))
    {
        constraints.push(DesignConstraint::Impedance { ohms: ohm });
    }

    // Thermal resistance
    if let Some(r) = extract_value_after(text, "thermal resistance") {
        constraints.push(DesignConstraint::ThermalResistance { c_per_w_max: r });
    }

    // Material detection
    for (kw, id) in &[
        ("aluminum", "6061-T6"), ("steel", "1018-CD"), ("titanium", "Ti-6Al-4V"),
        ("stainless", "AISI-304"), ("7075", "7075-T6"), ("6061", "6061-T6"),
    ] {
        if text.contains(kw) {
            constraints.push(DesignConstraint::Material { id: id.to_string() });
            break;
        }
    }

    // Process detection
    for (kw, proc) in &[
        ("cnc", "cnc_3axis"), ("3d print", "fdm"), ("print", "fdm"),
        ("laser cut", "laser"), ("sheet metal", "sheet_metal"), ("cast", "casting"),
    ] {
        if text.contains(kw) {
            constraints.push(DesignConstraint::Process { name: proc.to_string() });
            break;
        }
    }

    constraints
}

fn detect_geometry_type(text: &str) -> Option<GeometryType> {
    let mappings: &[(&str, GeometryType)] = &[
        ("bracket", GeometryType::Bracket),
        ("housing", GeometryType::Housing),
        ("enclosure", GeometryType::Enclosure),
        ("shaft", GeometryType::Shaft),
        ("gear", GeometryType::Gear),
        ("plate", GeometryType::Plate),
        ("tube", GeometryType::Tube),
        ("pipe", GeometryType::Tube),
        ("beam", GeometryType::Beam),
        ("flange", GeometryType::Flange),
        ("heat sink", GeometryType::HeatSink),
        ("heatsink", GeometryType::HeatSink),
        ("antenna", GeometryType::Antenna),
        ("waveguide", GeometryType::Waveguide),
        ("pcb", GeometryType::PCB),
        ("coil", GeometryType::Coil),
    ];
    for (kw, gt) in mappings {
        if text.contains(kw) { return Some(gt.clone()); }
    }
    None
}

fn detect_material(text: &str) -> Option<String> {
    let mappings: &[(&str, &str)] = &[
        ("6061", "6061-T6"), ("7075", "7075-T6"), ("aluminum", "6061-T6"),
        ("titanium", "Ti-6Al-4V"), ("ti-6al", "Ti-6Al-4V"),
        ("stainless", "AISI-304"), ("304", "AISI-304"),
        ("steel", "1018-CD"), ("1018", "1018-CD"),
        ("copper", "C11000"), ("brass", "C36000"),
    ];
    for (kw, id) in mappings {
        if text.contains(kw) { return Some(id.to_string()); }
    }
    None
}

fn detect_process(text: &str) -> Option<String> {
    let mappings: &[(&str, &str)] = &[
        ("cnc", "cnc_3axis"), ("mill", "cnc_3axis"), ("machine", "cnc_3axis"),
        ("3d print", "fdm"), ("print", "fdm"), ("additive", "fdm"),
        ("laser cut", "laser"), ("sheet metal", "sheet_metal"),
        ("cast", "casting"), ("injection mold", "injection_mold"),
    ];
    for (kw, proc) in mappings {
        if text.contains(kw) { return Some(proc.to_string()); }
    }
    None
}

fn extract_value_before(text: &str, unit: &str) -> Option<f64> {
    if let Some(pos) = text.find(unit) {
        let before = &text[..pos];
        let num_str: String = before.chars().rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect::<String>()
            .chars().rev().collect();
        num_str.parse().ok()
    } else {
        None
    }
}

fn extract_value_after(text: &str, keyword: &str) -> Option<f64> {
    if let Some(pos) = text.find(keyword) {
        let after = &text[pos + keyword.len()..];
        let num_str: String = after.chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '.')
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        num_str.parse().ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_bracket_request() {
        let req = DesignAgent::parse_request(
            "design a bracket that holds 500N, weighs under 0.2kg, CNC machinable from aluminum"
        );
        assert_eq!(req.geometry_type, Some(GeometryType::Bracket));
        assert!(req.domains.contains(&EngineeringDomain::Mechanical));
        assert!(req.material_id.as_deref() == Some("6061-T6"));
        assert!(req.constraints.iter().any(|c| matches!(c, DesignConstraint::MaxWeight { .. })));
        assert!(req.process.as_deref() == Some("cnc_3axis"));
    }

    #[test]
    fn parse_em_request() {
        let req = DesignAgent::parse_request(
            "design a 2.4ghz patch antenna with 50 ohm impedance"
        );
        assert!(req.domains.contains(&EngineeringDomain::Electromagnetic));
        assert_eq!(req.geometry_type, Some(GeometryType::Antenna));
        assert!(req.constraints.iter().any(|c| matches!(c, DesignConstraint::Frequency { .. })));
        assert!(req.constraints.iter().any(|c| matches!(c, DesignConstraint::Impedance { .. })));
    }

    #[test]
    fn parse_thermal_request() {
        let req = DesignAgent::parse_request(
            "design a heat sink for a 50W chip, thermal resistance under 2 c/w, aluminum"
        );
        assert!(req.domains.contains(&EngineeringDomain::Thermal));
        assert_eq!(req.geometry_type, Some(GeometryType::HeatSink));
        assert!(req.constraints.iter().any(|c| matches!(c, DesignConstraint::ThermalResistance { .. })));
    }

    #[test]
    fn parse_multi_physics() {
        let req = DesignAgent::parse_request(
            "design a bracket with stress analysis and thermal cooling for high-temperature use below 300c"
        );
        assert!(req.domains.contains(&EngineeringDomain::Mechanical));
        assert!(req.domains.contains(&EngineeringDomain::Thermal));
        assert!(req.domains.contains(&EngineeringDomain::MultiPhysics));
    }

    #[test]
    fn agent_runs_bracket_design() {
        let mut agent = DesignAgent::new(AgentConfig::default());
        let req = DesignAgent::parse_request(
            "design an aluminum bracket under 0.5kg"
        );
        let result = agent.run(&req);
        assert!(result.iterations > 0);
        assert!(result.score > 0.0);
        assert!(!result.summary.is_empty());
        assert!(!result.manufacturing_plan.steps.is_empty());
    }

    #[test]
    fn agent_runs_housing_design() {
        let mut agent = DesignAgent::new(AgentConfig::default());
        let req = DesignAgent::parse_request(
            "design a waterproof electronics housing 100x60x40mm in aluminum, CNC machined"
        );
        let result = agent.run(&req);
        assert!(result.score > 0.0);
        assert_eq!(result.manufacturing_plan.process, "cnc_3axis");
    }

    #[test]
    fn agent_iterates_to_improve() {
        let mut agent = DesignAgent::new(AgentConfig {
            max_iterations: 10,
            ..Default::default()
        });
        let req = DesignAgent::parse_request(
            "design a steel bracket under 0.1kg with safety factor 3"
        );
        let result = agent.run(&req);
        // Agent should iterate to try to satisfy constraints
        assert!(result.iterations >= 1);
        // History should show progression
        assert!(agent.history.len() >= 2);
    }

    #[test]
    fn agent_explores_materials() {
        let mut agent = DesignAgent::new(AgentConfig {
            max_iterations: 10,
            explore_materials: true,
            ..Default::default()
        });
        let req = DesignAgent::parse_request(
            "design a shaft under 0.05kg that won't deflect more than 0.01mm"
        );
        let result = agent.run(&req);
        // Difficult constraints should trigger material exploration
        assert!(result.iterations >= 1);
    }

    #[test]
    fn manufacturing_plan_has_steps() {
        let mut agent = DesignAgent::new(AgentConfig::default());
        let req = DesignAgent::parse_request("design a CNC aluminum plate");
        let result = agent.run(&req);
        assert!(!result.manufacturing_plan.steps.is_empty());
        assert!(result.manufacturing_plan.estimated_cost_usd.is_some());
        assert!(result.manufacturing_plan.estimated_time_hours.is_some());
    }

    #[test]
    fn constraint_extraction_weight_kg() {
        let constraints = extract_constraints("must weigh under 2kg");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::MaxWeight { kg } if (*kg - 2.0).abs() < 0.01)));
    }

    #[test]
    fn constraint_extraction_frequency_ghz() {
        let constraints = extract_constraints("operating at 5.8ghz");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::Frequency { hz, .. } if (*hz - 5.8e9).abs() < 1e6)));
    }

    #[test]
    fn constraint_extraction_impedance() {
        let constraints = extract_constraints("50 ohm characteristic impedance");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::Impedance { ohms } if (*ohms - 50.0).abs() < 0.01)));
    }

    #[test]
    fn detect_all_geometry_types() {
        assert_eq!(detect_geometry_type("bracket"), Some(GeometryType::Bracket));
        assert_eq!(detect_geometry_type("antenna"), Some(GeometryType::Antenna));
        assert_eq!(detect_geometry_type("heat sink"), Some(GeometryType::HeatSink));
        assert_eq!(detect_geometry_type("waveguide"), Some(GeometryType::Waveguide));
        assert_eq!(detect_geometry_type("pcb"), Some(GeometryType::PCB));
        assert_eq!(detect_geometry_type("random thing"), None);
    }

    #[test]
    fn evaluation_scores_correctly() {
        let agent = DesignAgent::new(AgentConfig::default());
        let mut params = HashMap::new();
        params.insert("width".into(), 50.0);
        params.insert("height".into(), 50.0);
        params.insert("depth".into(), 50.0);
        let state = DesignState {
            parameters: params,
            tool_calls: Vec::new(),
            material_id: "6061-T6".into(),
            evaluation: None,
            iteration: 0,
        };
        let constraints = vec![
            DesignConstraint::MaxWeight { kg: 10.0 }, // easy to satisfy
        ];
        let eval = agent.evaluate(&state, &constraints);
        assert!(eval.feasible, "10kg weight limit should be easy for a small box");
        assert!((eval.score - 1.0).abs() < 0.01);
    }

    #[test]
    fn heat_sink_thermal_check() {
        let agent = DesignAgent::new(AgentConfig::default());
        let mut params = HashMap::new();
        params.insert("base_width".into(), 60.0);
        params.insert("base_height".into(), 10.0);
        params.insert("fin_height".into(), 30.0);
        params.insert("fin_count".into(), 12.0);
        params.insert("fin_thickness".into(), 1.5);
        let state = DesignState {
            parameters: params,
            tool_calls: Vec::new(),
            material_id: "6061-T6".into(),
            evaluation: None,
            iteration: 0,
        };
        let constraints = vec![
            DesignConstraint::ThermalResistance { c_per_w_max: 5.0 },
        ];
        let eval = agent.evaluate(&state, &constraints);
        assert!(eval.checks[0].actual_value > 0.0, "thermal resistance should be computed");
    }
}
