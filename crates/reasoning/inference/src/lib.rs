//! `physical-inference` — Rule-based intent classification and MCP tool-call generation.
//!
//! Bridges natural language to MCP tool calls via keyword matching (v1)
//! and template-based design pattern recognition.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Intent classification
// ---------------------------------------------------------------------------

/// The kind of design intent extracted from natural language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentKind {
    CreatePrimitive,
    ModifyFeature,
    QueryMaterial,
    RunSimulation,
    ExportFile,
    AssemblyOp,
    ManufacturingOp,
    MeasureQuery,
    Unknown,
}

/// A classified intent with confidence and extracted parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub kind: IntentKind,
    /// Confidence score in 0.0..=1.0.
    pub confidence: f64,
    /// Extracted key-value parameters from the input text.
    pub parameters: HashMap<String, String>,
    /// The original input text.
    pub raw_text: String,
}

/// An MCP tool call with a tool name and JSON arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Pipeline types
// ---------------------------------------------------------------------------

/// A multi-step pipeline of ordered tool calls parsed from complex requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub steps: Vec<PipelineStep>,
    pub confidence: f64,
}

/// A single step in a pipeline with dependency tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub description: String,
    pub tool_call: ToolCall,
    pub depends_on: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Design constraints
// ---------------------------------------------------------------------------

/// A constraint extracted from natural language design requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DesignConstraint {
    MaxWeight { kg: f64 },
    MaxDeflection { mm: f64 },
    MinSafetyFactor { factor: f64 },
    Material { id: String },
    Process { name: String },
    MaxCost { usd: f64 },
}

// ---------------------------------------------------------------------------
// Keyword rules
// ---------------------------------------------------------------------------

struct Rule {
    kind: IntentKind,
    keywords: &'static [&'static str],
    confidence: f64,
}

const RULES: &[Rule] = &[
    Rule { kind: IntentKind::CreatePrimitive, keywords: &[
        "make a box", "create a box", "create a cube", "make a cube",
        "make a cylinder", "create a cylinder", "make a sphere", "create a sphere",
        "box", "cube", "cylinder", "sphere", "cone", "prism",
    ], confidence: 0.85 },
    Rule { kind: IntentKind::ModifyFeature, keywords: &[
        "extrude", "revolve", "loft", "sweep", "fillet", "chamfer",
        "shell", "hollow", "pattern", "mirror", "cut", "trim", "offset",
    ], confidence: 0.85 },
    Rule { kind: IntentKind::QueryMaterial, keywords: &[
        "what material", "yield strength", "density of", "tensile",
        "material", "aluminum", "steel", "titanium", "modulus",
        "hardness", "poisson",
    ], confidence: 0.80 },
    Rule { kind: IntentKind::RunSimulation, keywords: &[
        "stress", "deflection", "fea", "simulate", "modal", "thermal",
        "buckling", "fatigue", "vibration", "load", "force",
    ], confidence: 0.80 },
    Rule { kind: IntentKind::ExportFile, keywords: &[
        "export", "save as", "step", "stl", "dxf", "3mf", "iges",
        "gltf", "obj", "download",
    ], confidence: 0.90 },
    Rule { kind: IntentKind::AssemblyOp, keywords: &[
        "assemble", "mate", "fit", "join", "attach", "align",
        "constrain", "bolt", "fastener",
    ], confidence: 0.80 },
    Rule { kind: IntentKind::ManufacturingOp, keywords: &[
        "slice", "toolpath", "gcode", "print", "mill", "turn",
        "lathe", "cnc", "laser", "engrave", "etch",
    ], confidence: 0.80 },
    Rule { kind: IntentKind::MeasureQuery, keywords: &[
        "measure", "distance", "volume", "area", "weight", "mass",
        "length", "angle", "radius of", "diameter",
    ], confidence: 0.85 },
];

/// Classify natural language text into a design intent using keyword matching.
pub fn classify_intent(text: &str) -> Intent {
    let lower = text.to_lowercase();
    let mut best_kind = IntentKind::Unknown;
    let mut best_confidence = 0.0_f64;
    let mut best_match_count = 0_usize;

    for rule in RULES {
        let match_count = rule.keywords.iter()
            .filter(|kw| lower.contains(*kw))
            .count();
        if match_count > best_match_count || (match_count == best_match_count && rule.confidence > best_confidence) {
            if match_count > 0 {
                best_kind = rule.kind.clone();
                best_confidence = rule.confidence;
                best_match_count = match_count;
            }
        }
    }

    // Boost confidence for multi-keyword matches
    if best_match_count > 1 {
        best_confidence = (best_confidence + 0.05).min(1.0);
    }

    let parameters = extract_parameters(&lower, &best_kind);

    Intent {
        kind: best_kind,
        confidence: best_confidence,
        parameters,
        raw_text: text.to_string(),
    }
}

/// Extract relevant parameters from text based on intent kind.
fn extract_parameters(text: &str, kind: &IntentKind) -> HashMap<String, String> {
    let mut params = HashMap::new();

    match kind {
        IntentKind::CreatePrimitive => {
            // Try to extract dimensions: "10x20x30", "10 by 20 by 30", "10mm x 20mm"
            extract_dimensions(text, &mut params);
            // Detect primitive type
            if text.contains("box") || text.contains("cube") {
                params.insert("primitive".into(), "box".into());
            } else if text.contains("cylinder") {
                params.insert("primitive".into(), "cylinder".into());
            } else if text.contains("sphere") {
                params.insert("primitive".into(), "sphere".into());
            } else if text.contains("cone") {
                params.insert("primitive".into(), "cone".into());
            }
        }
        IntentKind::ModifyFeature => {
            if text.contains("extrude") { params.insert("operation".into(), "extrude".into()); }
            else if text.contains("revolve") { params.insert("operation".into(), "revolve".into()); }
            else if text.contains("loft") { params.insert("operation".into(), "loft".into()); }
            else if text.contains("fillet") { params.insert("operation".into(), "fillet".into()); }
            else if text.contains("chamfer") { params.insert("operation".into(), "chamfer".into()); }
            else if text.contains("shell") || text.contains("hollow") {
                params.insert("operation".into(), "shell".into());
            }
            else if text.contains("mirror") { params.insert("operation".into(), "mirror".into()); }
            extract_number(text, "by", &mut params, "distance");
            extract_number(text, "radius", &mut params, "radius");
        }
        IntentKind::QueryMaterial => {
            if let Some(mat_id) = extract_material(text) {
                params.insert("material_id".into(), mat_id);
            }
        }
        IntentKind::ExportFile => {
            for fmt in &["step", "stl", "dxf", "3mf", "iges", "gltf", "obj"] {
                if text.contains(fmt) {
                    params.insert("format".into(), fmt.to_string());
                    break;
                }
            }
        }
        _ => {}
    }

    params
}

/// Try to extract dimensions like "10x20x30" or "10 by 20 by 30".
fn extract_dimensions(text: &str, params: &mut HashMap<String, String>) {
    // Pattern: NxNxN or N x N x N
    let nums: Vec<f64> = text.split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    if nums.len() >= 3 {
        params.insert("width".into(), nums[0].to_string());
        params.insert("height".into(), nums[1].to_string());
        params.insert("depth".into(), nums[2].to_string());
    } else if nums.len() == 2 {
        params.insert("width".into(), nums[0].to_string());
        params.insert("height".into(), nums[1].to_string());
    } else if nums.len() == 1 {
        params.insert("size".into(), nums[0].to_string());
    }
}

/// Extract a number following a keyword.
fn extract_number(text: &str, keyword: &str, params: &mut HashMap<String, String>, param_name: &str) {
    if let Some(pos) = text.find(keyword) {
        let after = &text[pos + keyword.len()..];
        let num_str: String = after.chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '.')
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if !num_str.is_empty() {
            params.insert(param_name.into(), num_str);
        }
    }
}

// ---------------------------------------------------------------------------
// Enhanced dimension parser
// ---------------------------------------------------------------------------

/// Parse a dimension string and return the value in millimeters.
///
/// Handles various formats:
/// - "10mm", "10 mm"
/// - "2 inch", "2in", "2\""
/// - "1/4 inch" (fractions)
/// - "radius 5mm", "r=5"
/// - "diameter 10"
/// - Plain numbers (assumed mm)
pub fn parse_dimension(text: &str) -> Option<f64> {
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }

    // Try fraction patterns first: "1/4 inch", "3/8 in", "1/2\""
    if let Some(mm) = try_parse_fraction(&text) {
        return Some(mm);
    }

    // Extract numeric value and unit
    let mut num_str = String::new();
    let mut unit_str = String::new();
    let mut in_num = false;
    let mut past_num = false;

    for c in text.chars() {
        if !past_num && (c.is_ascii_digit() || c == '.' || (c == '-' && num_str.is_empty())) {
            num_str.push(c);
            in_num = true;
        } else if in_num && c == ' ' {
            past_num = true;
        } else if in_num {
            past_num = true;
            unit_str.push(c);
        } else if past_num {
            unit_str.push(c);
        }
    }

    let value: f64 = num_str.parse().ok()?;
    let unit = unit_str.trim();

    // Convert to mm based on unit
    let mm = match unit {
        "" | "mm" => value,
        "cm" => value * 10.0,
        "m" => value * 1000.0,
        "in" | "inch" | "inches" | "\"" => value * 25.4,
        "ft" | "feet" | "foot" => value * 304.8,
        _ => value, // assume mm for unknown units
    };

    Some(mm)
}

/// Try to parse a fractional dimension like "1/4 inch".
fn try_parse_fraction(text: &str) -> Option<f64> {
    // Pattern: N/N [unit]
    let parts: Vec<&str> = text.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if let Some(slash_pos) = part.find('/') {
            let num: f64 = part[..slash_pos].parse().ok()?;
            let den: f64 = part[slash_pos + 1..].parse().ok()?;
            if den == 0.0 {
                return None;
            }
            let value = num / den;
            // Check for unit in next part
            let unit = parts.get(i + 1).copied().unwrap_or("");
            let mm = match unit {
                "in" | "inch" | "inches" | "\"" => value * 25.4,
                "mm" => value,
                "cm" => value * 10.0,
                _ => value * 25.4, // fractions default to inches
            };
            return Some(mm);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Material extraction
// ---------------------------------------------------------------------------

/// Specific material ID lookup table.
const SPECIFIC_MATERIALS: &[(&str, &str)] = &[
    ("6061-t6", "6061-T6"),
    ("7075-t6", "7075-T6"),
    ("aisi-304", "AISI-304"),
    ("aisi-1018", "AISI-1018"),
    ("ti-6al-4v", "Ti-6Al-4V"),
    ("304 stainless", "AISI-304"),
    ("316 stainless", "AISI-316"),
    ("1018 steel", "AISI-1018"),
    ("4140 steel", "AISI-4140"),
];

/// Generic material to default ID mapping.
const GENERIC_MATERIALS: &[(&str, &str)] = &[
    ("aluminum", "6061-T6"),
    ("aluminium", "6061-T6"),
    ("steel", "AISI-1018"),
    ("stainless", "AISI-304"),
    ("titanium", "Ti-6Al-4V"),
    ("plastic", "ABS"),
    ("nylon", "Nylon-6"),
    ("abs", "ABS"),
    ("pla", "PLA"),
    ("copper", "C110"),
    ("brass", "C360"),
    ("bronze", "C932"),
];

/// Extract a material ID from text, resolving generic names to specific IDs.
pub fn extract_material(text: &str) -> Option<String> {
    let lower = text.to_lowercase();

    // Check specific materials first (higher priority)
    for &(pattern, id) in SPECIFIC_MATERIALS {
        if lower.contains(pattern) {
            return Some(id.to_string());
        }
    }

    // Then check generic materials
    for &(pattern, id) in GENERIC_MATERIALS {
        if lower.contains(pattern) {
            return Some(id.to_string());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Constraint extraction
// ---------------------------------------------------------------------------

/// Extract design constraints from natural language.
///
/// Parses phrases like:
/// - "must weigh less than 2kg"
/// - "deflection under 0.5mm"
/// - "safety factor of at least 2"
/// - "in aluminum" / "made from steel"
/// - "CNC machined" / "3D printed"
/// - "cost under $50"
pub fn extract_constraints(text: &str) -> Vec<DesignConstraint> {
    let lower = text.to_lowercase();
    let mut constraints = Vec::new();

    // Weight constraints: "weigh less than Nkg", "under Nkg", "max weight N kg"
    if let Some(kg) = extract_constraint_value(&lower, &[
        "weigh less than", "weigh under", "max weight", "weight under",
        "lighter than", "no more than",
    ], &["kg", "kilogram", "kilograms"]) {
        constraints.push(DesignConstraint::MaxWeight { kg });
    } else if let Some(g) = extract_constraint_value(&lower, &[
        "weigh less than", "weigh under", "max weight", "weight under",
        "lighter than",
    ], &["g", "gram", "grams"]) {
        constraints.push(DesignConstraint::MaxWeight { kg: g / 1000.0 });
    }

    // Deflection constraints
    if let Some(mm) = extract_constraint_value(&lower, &[
        "deflection under", "deflection less than", "deflect less than",
        "max deflection", "deflection below",
    ], &["mm", "millimeter", "millimeters", ""]) {
        constraints.push(DesignConstraint::MaxDeflection { mm });
    }

    // Safety factor constraints
    if let Some(factor) = extract_safety_factor(&lower) {
        constraints.push(DesignConstraint::MinSafetyFactor { factor });
    }

    // Material constraints
    if let Some(mat_id) = extract_material(&lower) {
        // Only add if text indicates material choice (not just mentioning it)
        let has_material_context = lower.contains("in ") || lower.contains("from ")
            || lower.contains("made of") || lower.contains("using ")
            || lower.contains("material");
        if has_material_context {
            constraints.push(DesignConstraint::Material { id: mat_id });
        }
    }

    // Manufacturing process constraints
    for (kw, process) in &[
        ("cnc machin", "CNC"),
        ("3d print", "3D_printing"),
        ("injection mold", "injection_molding"),
        ("sheet metal", "sheet_metal"),
        ("laser cut", "laser_cutting"),
        ("cast", "casting"),
    ] {
        if lower.contains(kw) {
            constraints.push(DesignConstraint::Process { name: process.to_string() });
            break;
        }
    }

    // Cost constraints
    if let Some(usd) = extract_cost_value(&lower) {
        constraints.push(DesignConstraint::MaxCost { usd });
    }

    constraints
}

/// Extract a numeric value associated with constraint keywords and a unit.
fn extract_constraint_value(text: &str, prefixes: &[&str], units: &[&str]) -> Option<f64> {
    for prefix in prefixes {
        if let Some(pos) = text.find(prefix) {
            let after = &text[pos + prefix.len()..];
            let num = extract_first_number(after)?;
            // Verify the unit matches (if units list includes "" we accept any)
            if units.contains(&"") {
                return Some(num);
            }
            let after_trimmed = after.trim();
            for unit in units {
                if !unit.is_empty() && after_trimmed.contains(unit) {
                    return Some(num);
                }
            }
        }
    }
    None
}

/// Extract safety factor from text.
fn extract_safety_factor(text: &str) -> Option<f64> {
    let patterns = [
        "safety factor of at least",
        "safety factor of",
        "safety factor above",
        "safety factor greater than",
        "minimum safety factor",
        "min safety factor",
        "sf of",
        "sf >",
        "sf >=",
    ];
    for pattern in &patterns {
        if let Some(pos) = text.find(pattern) {
            let after = &text[pos + pattern.len()..];
            if let Some(num) = extract_first_number(after) {
                return Some(num);
            }
        }
    }
    None
}

/// Extract a cost value from text like "cost under $50", "under $100", "budget $30".
fn extract_cost_value(text: &str) -> Option<f64> {
    let patterns = [
        "cost under", "cost less than", "cost below",
        "budget", "under $", "less than $", "max cost",
        "cheaper than",
    ];
    for pattern in &patterns {
        if let Some(pos) = text.find(pattern) {
            let after = &text[pos + pattern.len()..];
            if let Some(num) = extract_first_number(after) {
                return Some(num);
            }
        }
    }
    // Also try finding "$N" pattern
    if let Some(dollar_pos) = text.find('$') {
        let after = &text[dollar_pos + 1..];
        if let Some(num) = extract_first_number(after) {
            return Some(num);
        }
    }
    None
}

/// Extract the first number found in a string.
fn extract_first_number(text: &str) -> Option<f64> {
    let num_str: String = text.chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '.')
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}

// ---------------------------------------------------------------------------
// Multi-step pipeline generation
// ---------------------------------------------------------------------------

/// Pipeline phrase patterns for splitting complex requests.
const PIPELINE_CONNECTORS: &[&str] = &[
    ", then ", " then ", " and then ",
    ", and ", " and ", ", ",
];

/// Parse complex text into an ordered pipeline of tool calls.
///
/// Splits on connectors like "and", "then", "," and classifies each sub-phrase
/// into a tool call with dependency tracking.
pub fn generate_pipeline(text: &str) -> Pipeline {
    let lower = text.to_lowercase();

    // First try template-based pipeline matching for known compound patterns
    if let Some(pipeline) = try_compound_template(&lower, text) {
        return pipeline;
    }

    // Split text into sub-phrases using connectors
    let phrases = split_pipeline_phrases(&lower);

    if phrases.len() <= 1 {
        // Single intent, still wrap it in a pipeline
        let intent = classify_intent(text);
        let calls = intent_to_tool_calls(&intent);
        let steps: Vec<PipelineStep> = calls.into_iter().enumerate().map(|(i, call)| {
            PipelineStep {
                description: format!("{}: {}", intent.kind_label(), call.tool_name),
                tool_call: call,
                depends_on: if i > 0 { vec![i - 1] } else { vec![] },
            }
        }).collect();
        return Pipeline {
            confidence: intent.confidence,
            steps,
        };
    }

    // Classify each sub-phrase and build pipeline steps
    let mut steps = Vec::new();
    let mut total_confidence = 0.0;

    for (i, phrase) in phrases.iter().enumerate() {
        let intent = classify_intent(phrase.trim());
        let calls = intent_to_tool_calls(&intent);
        total_confidence += intent.confidence;

        for call in calls {
            let depends_on = if steps.is_empty() { vec![] } else { vec![steps.len() - 1] };
            steps.push(PipelineStep {
                description: format!("Step {}: {}", i + 1, call.tool_name),
                tool_call: call,
                depends_on,
            });
        }
    }

    let avg_confidence = if phrases.is_empty() { 0.0 } else { total_confidence / phrases.len() as f64 };
    // Multi-step pipelines from clear text get a confidence boost
    let confidence = if phrases.len() >= 2 && avg_confidence > 0.7 {
        (avg_confidence + 0.05).min(1.0)
    } else {
        avg_confidence
    };

    Pipeline { steps, confidence }
}

/// Split text into pipeline sub-phrases using common connectors.
fn split_pipeline_phrases(text: &str) -> Vec<String> {
    // Try connectors from most specific to least specific
    for connector in PIPELINE_CONNECTORS {
        if text.contains(connector) {
            let parts: Vec<String> = text.split(connector)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() > 1 {
                return parts;
            }
        }
    }
    vec![text.to_string()]
}

/// Try to match compound template patterns that map to known tool sequences.
fn try_compound_template(lower: &str, _original: &str) -> Option<Pipeline> {
    // "make a box and hollow it out"
    if (lower.contains("box") || lower.contains("cube")) && (lower.contains("hollow") || lower.contains("shell")) {
        let mut params = HashMap::new();
        extract_dimensions(lower, &mut params);
        let w = param_f64(&params, "width", 50.0);
        let h = param_f64(&params, "height", 30.0);
        let d = param_f64(&params, "depth", 40.0);
        return Some(Pipeline {
            steps: vec![
                PipelineStep {
                    description: "Create box".into(),
                    tool_call: ToolCall {
                        tool_name: "create_box".into(),
                        arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                    },
                    depends_on: vec![],
                },
                PipelineStep {
                    description: "Hollow out the box".into(),
                    tool_call: ToolCall {
                        tool_name: "hollow_out".into(),
                        arguments: serde_json::json!({ "wall_thickness": 3.0 }),
                    },
                    depends_on: vec![0],
                },
            ],
            confidence: 0.90,
        });
    }

    // "create a cylinder, fillet the edges, then export as STEP"
    if lower.contains("cylinder") && lower.contains("fillet") && lower.contains("export") {
        let format = if lower.contains("stl") { "stl" }
            else if lower.contains("3mf") { "3mf" }
            else { "step" };
        return Some(Pipeline {
            steps: vec![
                PipelineStep {
                    description: "Create cylinder".into(),
                    tool_call: ToolCall {
                        tool_name: "create_cylinder".into(),
                        arguments: serde_json::json!({ "radius": 10.0, "height": 20.0 }),
                    },
                    depends_on: vec![],
                },
                PipelineStep {
                    description: "Fillet edges".into(),
                    tool_call: ToolCall {
                        tool_name: "fillet".into(),
                        arguments: serde_json::json!({ "radius": 1.0 }),
                    },
                    depends_on: vec![0],
                },
                PipelineStep {
                    description: format!("Export as {}", format.to_uppercase()),
                    tool_call: ToolCall {
                        tool_name: "export".into(),
                        arguments: serde_json::json!({ "format": format }),
                    },
                    depends_on: vec![1],
                },
            ],
            confidence: 0.92,
        });
    }

    // "design a bracket in aluminum and check if it can be CNC machined"
    if (lower.contains("bracket") || lower.contains("design")) &&
       (lower.contains("check") || lower.contains("manufacturab")) &&
       (lower.contains("cnc") || lower.contains("machin")) {
        let material = extract_material(lower).unwrap_or_else(|| "6061-T6".to_string());
        return Some(Pipeline {
            steps: vec![
                PipelineStep {
                    description: "Extrude bracket profile".into(),
                    tool_call: ToolCall {
                        tool_name: "extrude_profile".into(),
                        arguments: serde_json::json!({
                            "profile": "l_shape",
                            "width": 20.0,
                            "height": 30.0,
                            "thickness": 5.0,
                            "distance": 40.0,
                        }),
                    },
                    depends_on: vec![],
                },
                PipelineStep {
                    description: format!("Set material to {}", material),
                    tool_call: ToolCall {
                        tool_name: "set_material".into(),
                        arguments: serde_json::json!({ "material_id": material }),
                    },
                    depends_on: vec![0],
                },
                PipelineStep {
                    description: "Check CNC manufacturability".into(),
                    tool_call: ToolCall {
                        tool_name: "check_manufacturability".into(),
                        arguments: serde_json::json!({ "process": "cnc_3axis" }),
                    },
                    depends_on: vec![1],
                },
            ],
            confidence: 0.90,
        });
    }

    None
}

impl Intent {
    fn kind_label(&self) -> &'static str {
        match self.kind {
            IntentKind::CreatePrimitive => "Create",
            IntentKind::ModifyFeature => "Modify",
            IntentKind::QueryMaterial => "Material",
            IntentKind::RunSimulation => "Simulate",
            IntentKind::ExportFile => "Export",
            IntentKind::AssemblyOp => "Assembly",
            IntentKind::ManufacturingOp => "Manufacturing",
            IntentKind::MeasureQuery => "Measure",
            IntentKind::Unknown => "Unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Intent → MCP tool calls
// ---------------------------------------------------------------------------

/// Convert a classified intent into a sequence of MCP tool calls.
pub fn intent_to_tool_calls(intent: &Intent) -> Vec<ToolCall> {
    match &intent.kind {
        IntentKind::CreatePrimitive => {
            let primitive = intent.parameters.get("primitive").map(|s| s.as_str()).unwrap_or("box");
            match primitive {
                "box" | "cube" => {
                    let w = param_f64(&intent.parameters, "width", 10.0);
                    let h = param_f64(&intent.parameters, "height", 10.0);
                    let d = param_f64(&intent.parameters, "depth", 10.0);
                    vec![ToolCall {
                        tool_name: "create_box".into(),
                        arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                    }]
                }
                "cylinder" => {
                    let r = param_f64(&intent.parameters, "width", 5.0) / 2.0;
                    let h = param_f64(&intent.parameters, "height", 20.0);
                    vec![ToolCall {
                        tool_name: "create_cylinder".into(),
                        arguments: serde_json::json!({ "radius": r, "height": h }),
                    }]
                }
                _ => vec![ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": 10.0, "height": 10.0, "depth": 10.0 }),
                }],
            }
        }
        IntentKind::ModifyFeature => {
            let op = intent.parameters.get("operation").map(|s| s.as_str()).unwrap_or("extrude");
            match op {
                "extrude" => {
                    let dist = param_f64(&intent.parameters, "distance", 10.0);
                    vec![ToolCall {
                        tool_name: "extrude_profile".into(),
                        arguments: serde_json::json!({
                            "profile": "rectangle",
                            "distance": dist,
                        }),
                    }]
                }
                "shell" => {
                    let thickness = param_f64(&intent.parameters, "distance", 2.0);
                    vec![ToolCall {
                        tool_name: "hollow_out".into(),
                        arguments: serde_json::json!({ "wall_thickness": thickness }),
                    }]
                }
                "fillet" | "chamfer" => {
                    let r = param_f64(&intent.parameters, "radius", 1.0);
                    vec![ToolCall {
                        tool_name: "fillet".into(),
                        arguments: serde_json::json!({ "radius": r }),
                    }]
                }
                "mirror" => {
                    vec![ToolCall {
                        tool_name: "pattern".into(),
                        arguments: serde_json::json!({ "pattern_type": "mirror", "mirror_plane": "yz" }),
                    }]
                }
                _ => vec![],
            }
        }
        IntentKind::QueryMaterial => {
            let mat = intent.parameters.get("material_id")
                .cloned()
                .unwrap_or_else(|| "6061-T6".into());
            vec![ToolCall {
                tool_name: "lookup_material".into(),
                arguments: serde_json::json!({ "material_id": mat }),
            }]
        }
        IntentKind::RunSimulation => {
            vec![ToolCall {
                tool_name: "analyze_part".into(),
                arguments: serde_json::json!({}),
            }]
        }
        IntentKind::ExportFile => {
            let fmt = intent.parameters.get("format")
                .cloned()
                .unwrap_or_else(|| "step".into());
            vec![ToolCall {
                tool_name: "export".into(),
                arguments: serde_json::json!({ "format": fmt }),
            }]
        }
        IntentKind::AssemblyOp => {
            vec![ToolCall {
                tool_name: "combine_solids".into(),
                arguments: serde_json::json!({ "operation": "union" }),
            }]
        }
        IntentKind::ManufacturingOp => {
            vec![ToolCall {
                tool_name: "check_manufacturability".into(),
                arguments: serde_json::json!({ "process": "cnc_3axis" }),
            }]
        }
        IntentKind::MeasureQuery => {
            vec![ToolCall {
                tool_name: "analyze_part".into(),
                arguments: serde_json::json!({}),
            }]
        }
        IntentKind::Unknown => vec![],
    }
}

fn param_f64(params: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    params.get(key).and_then(|s| s.parse().ok()).unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Design templates
// ---------------------------------------------------------------------------

/// A reusable design pattern expressed as a sequence of tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub tool_calls: Vec<ToolCall>,
}

/// Built-in design templates for common mechanical parts.
pub static TEMPLATES: &[DesignTemplate] = &[
    DesignTemplate {
        name: "bracket",
        description: "L-shaped mounting bracket with holes",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "housing",
        description: "Rectangular housing with hollowed interior",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "flange",
        description: "Circular flange plate with bolt holes",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "spacer",
        description: "Simple cylindrical spacer",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "washer",
        description: "Flat washer with center hole",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "l-bracket",
        description: "L-bracket extruded from L-profile",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "gear",
        description: "Gear approximation from cylinder with thread",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "shaft",
        description: "Cylindrical shaft with length greater than diameter",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "plate",
        description: "Flat plate with one thin dimension",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "tube",
        description: "Hollow cylindrical tube or pipe",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "pipe",
        description: "Hollow cylindrical pipe",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "beam",
        description: "Structural beam extruded from I/L/T/C profile",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "enclosure",
        description: "Box enclosure with hollow interior and mounting holes",
        tool_calls: vec![],
    },
    DesignTemplate {
        name: "mount",
        description: "Mounting plate with bolt pattern holes",
        tool_calls: vec![],
    },
];

/// Fuzzy match user text against known design templates.
/// Returns a template with populated tool calls if matched.
pub fn match_template(text: &str) -> Option<&'static DesignTemplate> {
    let lower = text.to_lowercase();
    TEMPLATES.iter().find(|t| {
        lower.contains(t.name) || lower.contains(&t.name.replace('-', " "))
    })
}

/// Generate tool calls for a matched template with optional dimensions.
pub fn template_tool_calls(template: &DesignTemplate, text: &str) -> Vec<ToolCall> {
    let lower = text.to_lowercase();
    let mut params = HashMap::new();
    extract_dimensions(&lower, &mut params);

    match template.name {
        "bracket" | "l-bracket" => {
            let w = param_f64(&params, "width", 20.0);
            let h = param_f64(&params, "height", 30.0);
            let d = param_f64(&params, "depth", 40.0);
            let t = 5.0;
            vec![
                ToolCall {
                    tool_name: "extrude_profile".into(),
                    arguments: serde_json::json!({
                        "profile": "l_shape",
                        "width": w,
                        "height": h,
                        "thickness": t,
                        "distance": d,
                    }),
                },
            ]
        }
        "housing" => {
            let w = param_f64(&params, "width", 50.0);
            let h = param_f64(&params, "height", 30.0);
            let d = param_f64(&params, "depth", 40.0);
            vec![
                ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                },
                ToolCall {
                    tool_name: "hollow_out".into(),
                    arguments: serde_json::json!({ "wall_thickness": 3.0, "open_face_indices": [4] }),
                },
            ]
        }
        "flange" => {
            let r = param_f64(&params, "size", 25.0);
            let h = param_f64(&params, "height", 5.0);
            vec![
                ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": r, "height": h }),
                },
            ]
        }
        "spacer" | "washer" => {
            let r = param_f64(&params, "size", 10.0);
            let h = param_f64(&params, "height", 3.0);
            vec![
                ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": r, "height": h }),
                },
            ]
        }
        "gear" => {
            let r = param_f64(&params, "size", 15.0);
            let h = param_f64(&params, "height", 10.0);
            vec![
                ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": r, "height": h }),
                },
                ToolCall {
                    tool_name: "add_thread".into(),
                    arguments: serde_json::json!({ "pitch": 2.0, "depth": 1.5 }),
                },
            ]
        }
        "shaft" => {
            let r = param_f64(&params, "size", 5.0);
            let h = param_f64(&params, "height", 100.0);
            vec![
                ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": r, "height": h }),
                },
            ]
        }
        "plate" => {
            let w = param_f64(&params, "width", 100.0);
            let h = param_f64(&params, "height", 5.0);
            let d = param_f64(&params, "depth", 100.0);
            vec![
                ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                },
            ]
        }
        "tube" | "pipe" => {
            let r = param_f64(&params, "size", 10.0);
            let h = param_f64(&params, "height", 50.0);
            vec![
                ToolCall {
                    tool_name: "create_cylinder".into(),
                    arguments: serde_json::json!({ "radius": r, "height": h }),
                },
                ToolCall {
                    tool_name: "hollow_out".into(),
                    arguments: serde_json::json!({ "wall_thickness": 2.0 }),
                },
            ]
        }
        "beam" => {
            let d = param_f64(&params, "depth", 200.0);
            let w = param_f64(&params, "width", 20.0);
            let h = param_f64(&params, "height", 40.0);
            vec![
                ToolCall {
                    tool_name: "extrude_profile".into(),
                    arguments: serde_json::json!({
                        "profile": "i_beam",
                        "width": w,
                        "height": h,
                        "thickness": 3.0,
                        "distance": d,
                    }),
                },
            ]
        }
        "enclosure" => {
            let w = param_f64(&params, "width", 80.0);
            let h = param_f64(&params, "height", 40.0);
            let d = param_f64(&params, "depth", 60.0);
            vec![
                ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                },
                ToolCall {
                    tool_name: "hollow_out".into(),
                    arguments: serde_json::json!({ "wall_thickness": 2.5, "open_face_indices": [4] }),
                },
                ToolCall {
                    tool_name: "add_holes".into(),
                    arguments: serde_json::json!({
                        "pattern": "corners",
                        "diameter": 3.2,
                        "count": 4,
                    }),
                },
            ]
        }
        "mount" => {
            let w = param_f64(&params, "width", 40.0);
            let h = param_f64(&params, "height", 5.0);
            let d = param_f64(&params, "depth", 40.0);
            vec![
                ToolCall {
                    tool_name: "create_box".into(),
                    arguments: serde_json::json!({ "width": w, "height": h, "depth": d }),
                },
                ToolCall {
                    tool_name: "add_holes".into(),
                    arguments: serde_json::json!({
                        "pattern": "bolt_pattern",
                        "diameter": 5.0,
                        "count": 4,
                    }),
                },
            ]
        }
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Original tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn classify_create_box() {
        let intent = classify_intent("make a box 10x20x30");
        assert_eq!(intent.kind, IntentKind::CreatePrimitive);
        assert!(intent.confidence >= 0.80);
        assert_eq!(intent.parameters.get("width").unwrap(), "10");
        assert_eq!(intent.parameters.get("height").unwrap(), "20");
        assert_eq!(intent.parameters.get("depth").unwrap(), "30");
        assert_eq!(intent.parameters.get("primitive").unwrap(), "box");
    }

    #[test]
    fn classify_create_cube() {
        let intent = classify_intent("create a cube");
        assert_eq!(intent.kind, IntentKind::CreatePrimitive);
        assert_eq!(intent.parameters.get("primitive").unwrap(), "box");
    }

    #[test]
    fn classify_modify_extrude() {
        let intent = classify_intent("extrude this face by 15mm");
        assert_eq!(intent.kind, IntentKind::ModifyFeature);
        assert_eq!(intent.parameters.get("operation").unwrap(), "extrude");
    }

    #[test]
    fn classify_query_material() {
        let intent = classify_intent("what is the yield strength of 6061-T6?");
        assert_eq!(intent.kind, IntentKind::QueryMaterial);
        assert!(intent.parameters.contains_key("material_id"));
    }

    #[test]
    fn classify_simulation() {
        let intent = classify_intent("run a stress simulation on this part");
        assert_eq!(intent.kind, IntentKind::RunSimulation);
    }

    #[test]
    fn classify_export_step() {
        let intent = classify_intent("export as STEP");
        assert_eq!(intent.kind, IntentKind::ExportFile);
        assert_eq!(intent.parameters.get("format").unwrap(), "step");
    }

    #[test]
    fn classify_export_stl() {
        let intent = classify_intent("save as STL");
        assert_eq!(intent.kind, IntentKind::ExportFile);
        assert_eq!(intent.parameters.get("format").unwrap(), "stl");
    }

    #[test]
    fn classify_assembly() {
        let intent = classify_intent("mate these two parts together");
        assert_eq!(intent.kind, IntentKind::AssemblyOp);
    }

    #[test]
    fn classify_manufacturing() {
        let intent = classify_intent("generate gcode for 3D printing");
        assert_eq!(intent.kind, IntentKind::ManufacturingOp);
    }

    #[test]
    fn classify_measure() {
        let intent = classify_intent("measure the distance between these edges");
        assert_eq!(intent.kind, IntentKind::MeasureQuery);
    }

    #[test]
    fn classify_unknown() {
        let intent = classify_intent("hello world");
        assert_eq!(intent.kind, IntentKind::Unknown);
    }

    #[test]
    fn intent_to_box_tool_call() {
        let intent = classify_intent("make a box 20x30x40");
        let calls = intent_to_tool_calls(&intent);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "create_box");
        assert_eq!(calls[0].arguments["width"], 20.0);
        assert_eq!(calls[0].arguments["height"], 30.0);
        assert_eq!(calls[0].arguments["depth"], 40.0);
    }

    #[test]
    fn intent_to_export_tool_call() {
        let intent = classify_intent("export as stl");
        let calls = intent_to_tool_calls(&intent);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "export");
        assert_eq!(calls[0].arguments["format"], "stl");
    }

    #[test]
    fn intent_to_material_tool_call() {
        let intent = classify_intent("what is the density of steel?");
        let calls = intent_to_tool_calls(&intent);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "lookup_material");
    }

    #[test]
    fn match_bracket_template() {
        let t = match_template("make me a bracket").unwrap();
        assert_eq!(t.name, "bracket");
    }

    #[test]
    fn match_housing_template() {
        let t = match_template("design a housing for electronics").unwrap();
        assert_eq!(t.name, "housing");
    }

    #[test]
    fn match_l_bracket_template() {
        let t = match_template("I need an l-bracket").unwrap();
        assert_eq!(t.name, "bracket"); // "bracket" matches first since "l-bracket" contains "bracket"
    }

    #[test]
    fn no_template_match() {
        assert!(match_template("do something random").is_none());
    }

    #[test]
    fn template_tool_calls_housing() {
        let t = match_template("housing").unwrap();
        let calls = template_tool_calls(t, "make a housing 50x30x40");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "create_box");
        assert_eq!(calls[1].tool_name, "hollow_out");
    }

    #[test]
    fn unknown_intent_gives_no_tool_calls() {
        let intent = classify_intent("hello world");
        let calls = intent_to_tool_calls(&intent);
        assert!(calls.is_empty());
    }

    // -----------------------------------------------------------------------
    // New tests: Pipeline generation
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_multi_step_box_shell() {
        let pipeline = generate_pipeline("make a box and hollow it out");
        assert_eq!(pipeline.steps.len(), 2);
        assert_eq!(pipeline.steps[0].tool_call.tool_name, "create_box");
        assert_eq!(pipeline.steps[1].tool_call.tool_name, "hollow_out");
        assert_eq!(pipeline.steps[1].depends_on, vec![0]);
        assert!(pipeline.confidence >= 0.85);
    }

    #[test]
    fn pipeline_with_export() {
        let pipeline = generate_pipeline("create a cylinder, fillet the edges, then export as STEP");
        assert_eq!(pipeline.steps.len(), 3);
        assert_eq!(pipeline.steps[0].tool_call.tool_name, "create_cylinder");
        assert_eq!(pipeline.steps[1].tool_call.tool_name, "fillet");
        assert_eq!(pipeline.steps[2].tool_call.tool_name, "export");
        assert_eq!(pipeline.steps[2].tool_call.arguments["format"], "step");
    }

    #[test]
    fn pipeline_design_and_check() {
        let pipeline = generate_pipeline("design a bracket in aluminum and check if it can be CNC machined");
        assert_eq!(pipeline.steps.len(), 3);
        assert_eq!(pipeline.steps[0].tool_call.tool_name, "extrude_profile");
        assert_eq!(pipeline.steps[1].tool_call.tool_name, "set_material");
        assert_eq!(pipeline.steps[1].tool_call.arguments["material_id"], "6061-T6");
        assert_eq!(pipeline.steps[2].tool_call.tool_name, "check_manufacturability");
    }

    #[test]
    fn pipeline_confidence_higher_for_clear_text() {
        let clear = generate_pipeline("make a box and hollow it out");
        let vague = generate_pipeline("do something weird");
        assert!(clear.confidence > vague.confidence,
            "Clear pipeline ({}) should have higher confidence than vague ({})",
            clear.confidence, vague.confidence);
    }

    // -----------------------------------------------------------------------
    // New tests: Dimension parsing
    // -----------------------------------------------------------------------

    #[test]
    fn dimension_parse_mm() {
        let mm = parse_dimension("10mm").unwrap();
        assert!((mm - 10.0).abs() < 0.001);

        let mm2 = parse_dimension("25.4 mm").unwrap();
        assert!((mm2 - 25.4).abs() < 0.001);
    }

    #[test]
    fn dimension_parse_inches() {
        let mm = parse_dimension("1 inch").unwrap();
        assert!((mm - 25.4).abs() < 0.001);

        let mm2 = parse_dimension("2in").unwrap();
        assert!((mm2 - 50.8).abs() < 0.001);
    }

    #[test]
    fn dimension_parse_fractions() {
        let mm = parse_dimension("1/4 inch").unwrap();
        assert!((mm - 6.35).abs() < 0.01, "1/4 inch should be 6.35mm, got {}", mm);

        let mm2 = parse_dimension("1/2 inch").unwrap();
        assert!((mm2 - 12.7).abs() < 0.01, "1/2 inch should be 12.7mm, got {}", mm2);
    }

    // -----------------------------------------------------------------------
    // New tests: Material extraction
    // -----------------------------------------------------------------------

    #[test]
    fn material_extract_generic() {
        assert_eq!(extract_material("use aluminum"), Some("6061-T6".to_string()));
        assert_eq!(extract_material("made from steel"), Some("AISI-1018".to_string()));
        assert_eq!(extract_material("titanium part"), Some("Ti-6Al-4V".to_string()));
        assert_eq!(extract_material("plastic housing"), Some("ABS".to_string()));
        assert_eq!(extract_material("stainless flange"), Some("AISI-304".to_string()));
    }

    #[test]
    fn material_extract_specific() {
        assert_eq!(extract_material("7075-T6 aluminum"), Some("7075-T6".to_string()));
        assert_eq!(extract_material("AISI-304 stainless"), Some("AISI-304".to_string()));
        assert_eq!(extract_material("Ti-6Al-4V alloy"), Some("Ti-6Al-4V".to_string()));
    }

    // -----------------------------------------------------------------------
    // New tests: Constraint extraction
    // -----------------------------------------------------------------------

    #[test]
    fn constraint_extract_weight() {
        let constraints = extract_constraints("must weigh less than 2kg");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::MaxWeight { kg } if (*kg - 2.0).abs() < 0.001)),
            "Expected MaxWeight 2.0, got {:?}", constraints);
    }

    #[test]
    fn constraint_extract_deflection() {
        let constraints = extract_constraints("deflection under 0.5mm");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::MaxDeflection { mm } if (*mm - 0.5).abs() < 0.001)),
            "Expected MaxDeflection 0.5, got {:?}", constraints);
    }

    #[test]
    fn constraint_extract_safety_factor() {
        let constraints = extract_constraints("safety factor of at least 2");
        assert!(constraints.iter().any(|c| matches!(c, DesignConstraint::MinSafetyFactor { factor } if (*factor - 2.0).abs() < 0.001)),
            "Expected MinSafetyFactor 2.0, got {:?}", constraints);
    }

    // -----------------------------------------------------------------------
    // New tests: Expanded templates
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Mechanical Parts Vocabulary (Leo AI "parts as tokens" concept)
    // -----------------------------------------------------------------------

    #[test]
    fn part_token_bolt() {
        let calls = intent_to_tool_calls(&classify_intent("add an M8 bolt"));
        assert!(!calls.is_empty());
    }

    #[test]
    fn template_shaft() {
        let t = match_template("create a shaft").unwrap();
        assert_eq!(t.name, "shaft");
        let calls = template_tool_calls(t, "create a shaft");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "create_cylinder");
        // Default shaft should be long (height > diameter)
        let h = calls[0].arguments["height"].as_f64().unwrap();
        let r = calls[0].arguments["radius"].as_f64().unwrap();
        assert!(h > r * 2.0, "Shaft should be longer than its diameter");
    }

    #[test]
    fn template_tube() {
        let t = match_template("make a tube").unwrap();
        assert_eq!(t.name, "tube");
        let calls = template_tool_calls(t, "make a tube");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "create_cylinder");
        assert_eq!(calls[1].tool_name, "hollow_out");
    }

    #[test]
    fn template_enclosure() {
        let t = match_template("design an enclosure").unwrap();
        assert_eq!(t.name, "enclosure");
        let calls = template_tool_calls(t, "design an enclosure 80x40x60");
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].tool_name, "create_box");
        assert_eq!(calls[1].tool_name, "hollow_out");
        assert_eq!(calls[2].tool_name, "add_holes");
    }
}
