//! Tool registry — defines the MCP tool catalog.
//!
//! Each tool has a flat parameter schema that generates valid JSON Schema
//! for the MCP `tools/list` response. The schema is designed so that AI
//! models produce correct calls on the first attempt.

use serde::{Deserialize, Serialize};

/// Parameter type — kept deliberately simple for AI consumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    /// f64 number.
    Number,
    /// UTF-8 string.
    String,
    /// Integer (usize).
    Integer,
    /// Boolean flag.
    Boolean,
    /// Array of f64 (e.g., [x, y, z] for a point).
    NumberArray,
    /// Array of strings (e.g., list of solid handles).
    StringArray,
}

/// A single tool parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    /// Parameter name (snake_case, matches JSON key).
    pub name: &'static str,
    /// Human-readable description — one sentence, no jargon.
    pub description: &'static str,
    /// Type.
    pub param_type: ParamType,
    /// Required? If false, has a default.
    pub required: bool,
    /// Default value as JSON (if not required).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// A tool definition — what gets sent to the AI in `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    /// Tool name. Matches MCP tool name exactly.
    pub name: &'static str,
    /// One-line description — matches how an AI would phrase the intent.
    /// This is the MOST IMPORTANT field for AI ergonomics.
    /// Bad:  "Execute a CSG boolean union operation on two BRepSolid objects"
    /// Good: "Combine two solids into one (add material together)"
    pub description: &'static str,
    /// Longer explanation (shown in tool detail, not in listing).
    pub long_description: &'static str,
    /// Flat parameter list.
    pub params: Vec<ToolParam>,
    /// What the tool returns (human-readable).
    pub returns: &'static str,
    /// Category for grouping in UI.
    pub category: ToolCategory,
}

/// Tool categories — helps AI pick the right tool.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// Create geometry from scratch.
    Create,
    /// Modify existing geometry.
    Modify,
    /// Analyze properties of geometry.
    Analyze,
    /// Export to file formats.
    Export,
    /// 2D sketching and constraints.
    Sketch,
    /// Material and manufacturing data.
    Lookup,
    /// Parametric feature history.
    Parametric,
    /// Simulation (FEA, CFD, modal, thermal, topology optimization).
    Simulation,
    /// Manufacturing (toolpath, slicer, nesting, DFM).
    Manufacturing,
}

/// The tool registry — all available tools.
pub struct ToolRegistry {
    tools: Vec<ToolDef>,
}

impl ToolRegistry {
    /// Build the complete tool catalog.
    pub fn new() -> Self {
        Self {
            tools: build_tool_catalog(),
        }
    }

    /// List all tools (for MCP `tools/list`).
    pub fn list(&self) -> &[ToolDef] {
        &self.tools
    }

    /// Find a tool by name.
    pub fn get(&self, name: &str) -> Option<&ToolDef> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Generate MCP-compliant JSON Schema for a tool's input.
    pub fn json_schema(&self, name: &str) -> Option<serde_json::Value> {
        let tool = self.get(name)?;
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &tool.params {
            let type_str = match param.param_type {
                ParamType::Number => "number",
                ParamType::String => "string",
                ParamType::Integer => "integer",
                ParamType::Boolean => "boolean",
                ParamType::NumberArray => "array",
                ParamType::StringArray => "array",
            };

            let mut prop = serde_json::json!({
                "type": type_str,
                "description": param.description,
            });

            // Array items type
            match param.param_type {
                ParamType::NumberArray => {
                    prop["items"] = serde_json::json!({"type": "number"});
                }
                ParamType::StringArray => {
                    prop["items"] = serde_json::json!({"type": "string"});
                }
                _ => {}
            }

            if let Some(ref default) = param.default {
                prop["default"] = default.clone();
            }

            properties.insert(param.name.to_string(), prop);

            if param.required {
                required.push(serde_json::Value::String(param.name.to_string()));
            }
        }

        Some(serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }))
    }
}

// ---------------------------------------------------------------------------
// Tool catalog — the actual tool definitions
// ---------------------------------------------------------------------------

fn build_tool_catalog() -> Vec<ToolDef> {
    vec![
        // ===== CREATE =====
        ToolDef {
            name: "create_box",
            description: "Create a rectangular box (specify width, height, depth in mm)",
            long_description: "Creates a B-Rep box solid centered at the origin. \
                All dimensions are in millimeters. The box is axis-aligned with \
                width along X, height along Y, depth along Z.",
            params: vec![
                ToolParam {
                    name: "width",
                    description: "Width in mm (X dimension)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "height",
                    description: "Height in mm (Y dimension)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "depth",
                    description: "Depth in mm (Z dimension)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "name",
                    description: "Optional name for the solid",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("box")),
                },
            ],
            returns: "A solid handle and summary with dimensions, volume, and face count.",
            category: ToolCategory::Create,
        },

        ToolDef {
            name: "create_cylinder",
            description: "Create a cylinder (specify radius and height in mm)",
            long_description: "Creates a B-Rep cylinder centered at the origin with \
                axis along Y. Radius and height in millimeters.",
            params: vec![
                ToolParam {
                    name: "radius",
                    description: "Radius in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "height",
                    description: "Height in mm (Y dimension)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "segments",
                    description: "Number of circumferential segments (more = smoother)",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(32)),
                },
            ],
            returns: "A solid handle and summary with dimensions, volume, and face count.",
            category: ToolCategory::Create,
        },

        ToolDef {
            name: "extrude_profile",
            description: "Extrude a 2D shape into a 3D solid (pull a flat shape upward)",
            long_description: "Takes a 2D profile (rectangle, circle, L-shape, or custom) \
                and extrudes it along Z to create a solid. This is the most common way \
                to create prismatic parts like brackets, beams, and housings.",
            params: vec![
                ToolParam {
                    name: "profile",
                    description: "Profile type: 'rectangle', 'circle', 'l_shape', or a sketch handle",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "distance",
                    description: "Extrusion distance in mm (how far to pull)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "width",
                    description: "Profile width in mm (for rectangle/l_shape)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(10.0)),
                },
                ToolParam {
                    name: "height",
                    description: "Profile height in mm (for rectangle/l_shape)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(10.0)),
                },
                ToolParam {
                    name: "radius",
                    description: "Radius in mm (for circle profile)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(5.0)),
                },
                ToolParam {
                    name: "thickness",
                    description: "Wall thickness in mm (for l_shape profile)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(2.0)),
                },
                ToolParam {
                    name: "symmetric",
                    description: "Extrude equally in both directions?",
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some(serde_json::json!(false)),
                },
            ],
            returns: "A solid handle and summary with dimensions and volume.",
            category: ToolCategory::Create,
        },

        ToolDef {
            name: "revolve_profile",
            description: "Spin a 2D shape around an axis to create a solid of revolution",
            long_description: "Creates solids like wheels, rings, cups, and vases by \
                revolving a 2D profile around an axis. Full 360° by default.",
            params: vec![
                ToolParam {
                    name: "profile",
                    description: "Profile type: 'rectangle', 'circle', 'l_shape', or a sketch handle",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "angle_degrees",
                    description: "Angle of revolution in degrees (360 = full rotation)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(360.0)),
                },
                ToolParam {
                    name: "axis",
                    description: "Axis of revolution: 'x', 'y', or 'z'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("y")),
                },
                ToolParam {
                    name: "width",
                    description: "Profile width in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(10.0)),
                },
                ToolParam {
                    name: "height",
                    description: "Profile height in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(10.0)),
                },
                ToolParam {
                    name: "segments",
                    description: "Number of angular segments (more = smoother)",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(32)),
                },
            ],
            returns: "A solid handle and summary with dimensions.",
            category: ToolCategory::Create,
        },

        // ===== MODIFY =====
        ToolDef {
            name: "combine_solids",
            description: "Combine, subtract, or intersect two solids (boolean operation)",
            long_description: "Boolean CSG operations on two solids:\n\
                - 'union': merge both solids into one (add material)\n\
                - 'subtract': cut solid_b out of solid_a (remove material)\n\
                - 'intersect': keep only the overlapping region\n\
                \n\
                The AI CANNOT fake this — geometry is computed exactly.",
            params: vec![
                ToolParam {
                    name: "solid_a",
                    description: "Handle of the first solid",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "solid_b",
                    description: "Handle of the second solid",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "operation",
                    description: "Boolean operation: 'union', 'subtract', or 'intersect'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "A new solid handle and summary with face count and operation performed.",
            category: ToolCategory::Modify,
        },

        ToolDef {
            name: "move_solid",
            description: "Move a solid by an offset (translate in X, Y, Z)",
            long_description: "Translates all vertices of a solid by the given offset. \
                Units are millimeters. Modifies the solid in place.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to move",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "x",
                    description: "Move distance in X (mm)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.0)),
                },
                ToolParam {
                    name: "y",
                    description: "Move distance in Y (mm)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.0)),
                },
                ToolParam {
                    name: "z",
                    description: "Move distance in Z (mm)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.0)),
                },
            ],
            returns: "Updated solid handle and new bounding box.",
            category: ToolCategory::Modify,
        },

        ToolDef {
            name: "hollow_out",
            description: "Hollow out a solid to create a shell with uniform wall thickness",
            long_description: "Removes interior material, leaving a shell of the given \
                wall thickness. Optionally leave faces open (e.g., top of a box = open container).",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to hollow",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "wall_thickness",
                    description: "Wall thickness in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "open_face_indices",
                    description: "Face indices to leave open (0-based), e.g. [0] for top face",
                    param_type: ParamType::NumberArray,
                    required: false,
                    default: Some(serde_json::json!([])),
                },
            ],
            returns: "A new solid handle and wall thickness confirmation.",
            category: ToolCategory::Modify,
        },

        ToolDef {
            name: "pattern",
            description: "Create a repeating pattern of a solid (linear array or circular array)",
            long_description: "Duplicates a solid in a pattern:\n\
                - 'linear': copies along a direction with equal spacing\n\
                - 'circular': copies around an axis at equal angles\n\
                - 'mirror': single mirror copy across a plane",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to pattern",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "pattern_type",
                    description: "Pattern type: 'linear', 'circular', or 'mirror'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "count",
                    description: "Number of copies (for linear/circular)",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(3)),
                },
                ToolParam {
                    name: "spacing",
                    description: "Distance between copies in mm (for linear)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(20.0)),
                },
                ToolParam {
                    name: "direction",
                    description: "Direction: 'x', 'y', or 'z' (for linear)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("x")),
                },
                ToolParam {
                    name: "axis",
                    description: "Axis of rotation: 'x', 'y', or 'z' (for circular)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("y")),
                },
                ToolParam {
                    name: "mirror_plane",
                    description: "Mirror plane: 'xy', 'xz', or 'yz' (for mirror)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("yz")),
                },
            ],
            returns: "A new solid handle containing all copies merged together.",
            category: ToolCategory::Modify,
        },

        // ===== ANALYZE (one call returns everything) =====
        ToolDef {
            name: "analyze_part",
            description: "Get all physical properties of a solid (mass, volume, bounding box, surface area, centroid)",
            long_description: "Returns mass properties, bounding box, face/edge/vertex counts, \
                and Euler characteristic in ONE call. The AI SHOULD use this instead of \
                estimating — the result is exact and includes a ready-to-relay summary.\n\
                \n\
                Optionally specify a material to get mass (not just volume).",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for mass calculation (e.g., '6061-T6', 'AISI-304')",
                    param_type: ParamType::String,
                    required: false,
                    default: None,
                },
            ],
            returns: "Volume (mm³), surface area (mm²), centroid [x,y,z], bounding box, face/edge/vertex counts, mass (g, if material specified), and a human-readable summary.",
            category: ToolCategory::Analyze,
        },

        ToolDef {
            name: "check_manufacturability",
            description: "Check if a part can be manufactured with a given process (CNC, injection mold, 3D print)",
            long_description: "Runs DFM (Design for Manufacturability) validation against \
                process-specific rules. Returns specific issues with locations and fix suggestions.\n\
                \n\
                This is CRITICAL for the AI to use rather than guess — DFM rules vary by \
                process and material, and the tool checks actual geometry, not assumptions.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to check",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "process",
                    description: "Manufacturing process: 'cnc_3axis', 'cnc_5axis', 'injection_mold', 'fdm', 'sla', 'sls', 'sheet_metal'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID (affects tolerances and constraints)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
            ],
            returns: "List of DFM issues (each with severity, location, description, and fix suggestion), or 'PASS' if manufacturable. Includes human-readable summary.",
            category: ToolCategory::Analyze,
        },

        ToolDef {
            name: "run_stress_analysis",
            description: "Run FEA stress analysis on a solid (apply loads and constraints, get stress/displacement)",
            long_description: "Finite element analysis: tetrahedralizes the solid, applies \
                boundary conditions, and solves for stress and displacement.\n\
                \n\
                The AI CANNOT fake FEA results — this tool is the only way to get \
                physically meaningful stress data for complex geometry.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID (for elastic modulus and Poisson's ratio)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "fixed_faces",
                    description: "Face indices to fix in place (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "force_magnitude",
                    description: "Applied force in Newtons",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "force_direction",
                    description: "Force direction: 'x', 'y', 'z', '-x', '-y', or '-z'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("-y")),
                },
                ToolParam {
                    name: "force_faces",
                    description: "Face indices where force is applied (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
            ],
            returns: "Max von Mises stress (MPa), max displacement (mm), safety factor (if material specified), and per-element stress summary.",
            category: ToolCategory::Analyze,
        },

        ToolDef {
            name: "quick_stress_check",
            description: "Quick analytical stress estimate (no FEA needed — beam theory)",
            long_description: "Uses Roark's beam formulas for quick stress/deflection estimates. \
                MUCH faster than FEA, good for sizing and sanity checks.\n\
                \n\
                This tool is EASIER for the AI to use than computing beam formulas manually, \
                and it uses the correct formulas from Roark's (not approximations).",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid (used to extract beam dimensions automatically)",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "load_newtons",
                    description: "Applied load in Newtons",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "support",
                    description: "Support condition: 'cantilever' or 'simply_supported'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("simply_supported")),
                },
                ToolParam {
                    name: "material",
                    description: "Material ID (for elastic modulus and yield strength)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
            ],
            returns: "Max bending stress (MPa), max deflection (mm), safety factor, and pass/fail verdict with human-readable summary.",
            category: ToolCategory::Analyze,
        },

        // ===== EXPORT =====
        ToolDef {
            name: "export",
            description: "Export a solid to a file format (STEP, STL, or OIE)",
            long_description: "Converts the solid to an industry-standard file format.\n\
                - STEP: for CAD interchange (other CAD software)\n\
                - STL: for 3D printing and visualization\n\
                - OIE: native format (preserves full feature tree)",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "format",
                    description: "Export format: 'step', 'stl', or 'oie'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "filename",
                    description: "Output filename (without extension — added automatically)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "File content (base64 for binary formats, text for STEP) and file size.",
            category: ToolCategory::Export,
        },

        // ===== LOOKUP (LUT cascade) =====
        ToolDef {
            name: "lookup_material",
            description: "Look up material properties (density, strength, thermal, etc.)",
            long_description: "Returns engineering properties from the material database. \
                The AI SHOULD use this instead of recalling properties from memory — \
                the database has verified values from handbooks, not LLM approximations.\n\
                \n\
                Supports: aluminum alloys, steels, stainless steels, titanium, polymers, composites.",
            params: vec![
                ToolParam {
                    name: "material_id",
                    description: "Material identifier (e.g., '6061-T6', 'AISI-304', 'Ti-6Al-4V', 'ABS')",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "Full property table: density, yield/ultimate strength, elastic modulus, Poisson's ratio, thermal conductivity, CTE, melting point, machinability index. All in SI units with a human-readable summary.",
            category: ToolCategory::Lookup,
        },

        ToolDef {
            name: "list_materials",
            description: "List available materials, optionally filtered by category",
            long_description: "Returns available material IDs from the database. \
                Use this to discover what materials are available before looking up properties.",
            params: vec![
                ToolParam {
                    name: "category",
                    description: "Filter by category: 'aluminum', 'steel', 'stainless', 'titanium', 'polymer', 'composite', or 'all'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("all")),
                },
            ],
            returns: "List of material IDs with short descriptions.",
            category: ToolCategory::Lookup,
        },

        // ===== SKETCH =====
        ToolDef {
            name: "create_sketch",
            description: "Create a 2D sketch with lines, arcs, and geometric constraints",
            long_description: "Creates a constrained 2D sketch that can be extruded or revolved. \
                Add entities (lines, circles, arcs) and constraints (distance, angle, tangent, \
                parallel, perpendicular, coincident) in one call.\n\
                \n\
                The solver automatically resolves constraint positions.",
            params: vec![
                ToolParam {
                    name: "entities",
                    description: "JSON array of entities. Each: {\"type\": \"line\", \"x0\": 0, \"y0\": 0, \"x1\": 10, \"y1\": 0} or {\"type\": \"circle\", \"cx\": 5, \"cy\": 5, \"r\": 3}",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "constraints",
                    description: "JSON array of constraints. Each: {\"type\": \"distance\", \"entity\": 0, \"value\": 10.0} or {\"type\": \"parallel\", \"entity_a\": 0, \"entity_b\": 1}",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("[]")),
                },
                ToolParam {
                    name: "plane",
                    description: "Sketch plane: 'xy', 'xz', or 'yz'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("xy")),
                },
            ],
            returns: "Sketch handle, solved entity positions, DOF count, and solve status.",
            category: ToolCategory::Sketch,
        },

        // ===== LOOKUP (additional) =====
        ToolDef {
            name: "lookup_manufacturing_constraint",
            description: "Get manufacturing constraints for a process + material class",
            long_description: "Returns min wall thickness, min hole diameter, min corner radius, \
                max pocket depth ratio, draft angle, and other process-specific limits from \
                the handbook-backed constraint database.\n\
                \n\
                Use this BEFORE designing — knowing the limits prevents DFM failures.",
            params: vec![
                ToolParam {
                    name: "process",
                    description: "Process: 'cnc_3axis', 'cnc_5axis', 'injection_mold', 'fdm', 'sla', 'sls', 'sheet_metal', 'die_casting'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material_class",
                    description: "Material class: 'aluminum', 'mild_steel', 'stainless', 'titanium', 'plastic'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "Min wall thickness, min hole diameter, min corner radius, max pocket depth ratio, draft angle, and more. All in mm/degrees with summary.",
            category: ToolCategory::Lookup,
        },

        ToolDef {
            name: "lookup_thread",
            description: "Look up metric thread data by size (M3, M8, etc.)",
            long_description: "Returns ISO 262 metric thread geometry: nominal diameter, coarse \
                pitch, fine pitches, minor diameter, pitch diameter, and tensile stress area.\n\
                \n\
                The AI SHOULD use this instead of computing thread dimensions — the table has \
                exact values from ISO 724.",
            params: vec![
                ToolParam {
                    name: "designation",
                    description: "Thread designation, e.g. 'M3', 'M8', 'M12', 'M20'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "Nominal diameter, coarse pitch, fine pitches, minor diameter, pitch diameter, tensile stress area. All in mm/mm².",
            category: ToolCategory::Lookup,
        },

        ToolDef {
            name: "lookup_tolerance_fit",
            description: "Look up ISO 286 tolerance fit data (H7/g6, H7/p6, etc.)",
            long_description: "Returns shaft and hole tolerance bands for standard ISO 286 fits.\n\
                \n\
                Covers clearance fits (H7/f6, H7/g6), transition fits (H7/k6, H7/m6), \
                and interference fits (H7/p6, H7/s6). The AI SHOULD use this for any \
                tolerance question — guessing fit classes causes manufacturing failures.",
            params: vec![
                ToolParam {
                    name: "designation",
                    description: "Fit designation, e.g. 'H7/g6', 'H7/p6', 'H11/c11'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "Fit type (clearance/transition/interference), shaft tolerance, hole tolerance, min/max clearance or interference. All in micrometers.",
            category: ToolCategory::Lookup,
        },

        ToolDef {
            name: "calculate_formula",
            description: "Evaluate an engineering formula (beam deflection, stress, etc.)",
            long_description: "Evaluates formulas from Roark's, Peterson's, and Shigley's.\n\
                \n\
                Supported formulas: 'beam_simply_supported_uniform', \
                'beam_simply_supported_center_load', 'beam_cantilever_end_load', \
                'beam_cantilever_uniform', 'stress_concentration_hole', \
                'stress_concentration_fillet', 'pressure_vessel_thin_wall'.\n\
                \n\
                EASIER than computing by hand, and uses the correct textbook equations.",
            params: vec![
                ToolParam {
                    name: "formula",
                    description: "Formula name: 'beam_simply_supported_uniform', 'beam_cantilever_end_load', etc.",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "params_json",
                    description: "JSON object with formula-specific parameters (see formula docs)",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
            ],
            returns: "Computed result (deflection, stress, etc.) with units and a summary sentence.",
            category: ToolCategory::Lookup,
        },

        // ===== GEOMETRY (additional) =====
        ToolDef {
            name: "create_loft",
            description: "Loft between two or more profiles to create a smooth solid",
            long_description: "Creates a solid by connecting two or more cross-section profiles. \
                Useful for aerodynamic shapes, bottle forms, and transitions between \
                different cross-sections (e.g., round-to-square).",
            params: vec![
                ToolParam {
                    name: "bottom_profile",
                    description: "Bottom profile type: 'rectangle' or 'circle'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "top_profile",
                    description: "Top profile type: 'rectangle' or 'circle'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "bottom_size",
                    description: "Bottom profile size in mm (width for rect, radius for circle)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "top_size",
                    description: "Top profile size in mm (width for rect, radius for circle)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "height",
                    description: "Distance between profiles in mm (Z direction)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
            ],
            returns: "A solid handle and summary with dimensions and face count.",
            category: ToolCategory::Create,
        },

        ToolDef {
            name: "create_sweep",
            description: "Sweep a profile along a path to create a solid",
            long_description: "Creates a solid by sweeping a 2D profile along a 3D curve path. \
                Useful for pipes, rails, wires, and any constant-section shape along a curve.",
            params: vec![
                ToolParam {
                    name: "profile",
                    description: "Profile type: 'rectangle' or 'circle'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "profile_size",
                    description: "Profile size in mm (width for rect, radius for circle)",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "path_type",
                    description: "Path type: 'line', 'arc', or 'helix'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "path_length",
                    description: "Path length or extent in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "steps",
                    description: "Number of sweep steps (more = smoother)",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(32)),
                },
            ],
            returns: "A solid handle and summary with dimensions.",
            category: ToolCategory::Create,
        },

        ToolDef {
            name: "add_fillet",
            description: "Round off edges of a solid with a fillet radius",
            long_description: "Adds a smooth blend (fillet) to selected edges of a solid. \
                Specify edge indices and a radius in mm. Commonly used to remove sharp \
                edges for strength, aesthetics, or moldability.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to fillet",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "edge_indices",
                    description: "Edge indices to fillet (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "radius",
                    description: "Fillet radius in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
            ],
            returns: "Updated solid handle and count of edges filleted.",
            category: ToolCategory::Modify,
        },

        ToolDef {
            name: "unfold_sheet_metal",
            description: "Unfold a sheet metal part into a flat pattern",
            long_description: "Computes the flat pattern from a sheet metal part using k-factor \
                bend allowance from the LUT. Returns bend lines, overall blank dimensions, \
                and flat contour for laser/punch cutting.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the sheet metal solid",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "thickness",
                    description: "Sheet thickness in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "k_factor",
                    description: "Bend k-factor (0.0-1.0, typically 0.33-0.5)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.44)),
                },
            ],
            returns: "Flat pattern dimensions, bend line positions, and blank size.",
            category: ToolCategory::Modify,
        },

        ToolDef {
            name: "add_thread",
            description: "Add a cosmetic thread annotation to a cylindrical face",
            long_description: "Adds ISO metric thread annotation to a cylindrical hole or boss. \
                The thread is cosmetic (visual + metadata) — geometry is not cut. \
                Use this for documentation and manufacturing intent.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "face_index",
                    description: "Index of the cylindrical face to thread (0-based)",
                    param_type: ParamType::Integer,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "thread_designation",
                    description: "Thread size, e.g. 'M6', 'M8x1.0', 'M10'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "depth",
                    description: "Thread depth in mm (0 = through)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.0)),
                },
            ],
            returns: "Confirmation with thread designation and face reference.",
            category: ToolCategory::Modify,
        },

        // ===== SIMULATION =====
        ToolDef {
            name: "run_fea",
            description: "Run static FEA stress analysis on a solid (loads + constraints)",
            long_description: "Meshes the solid into tetrahedra and solves for stress and \
                displacement under applied loads. Returns von Mises stress, max displacement, \
                and safety factor.\n\
                \n\
                The AI CANNOT fake FEA — this is the only path to real stress data.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for E, nu, yield strength",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "fixed_faces",
                    description: "Face indices to fix in place (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "force_magnitude",
                    description: "Applied force in Newtons",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "force_direction",
                    description: "Force direction: 'x', 'y', 'z', '-x', '-y', '-z'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("-y")),
                },
                ToolParam {
                    name: "force_faces",
                    description: "Face indices where force is applied (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
            ],
            returns: "Max von Mises stress (MPa), max displacement (mm), safety factor, node/element counts.",
            category: ToolCategory::Simulation,
        },

        ToolDef {
            name: "run_modal_analysis",
            description: "Find natural frequencies of a solid (vibration modes)",
            long_description: "Extracts natural frequencies and mode shapes via eigenvalue analysis. \
                Use this to check for resonance issues — if a natural frequency is near an \
                operating frequency, the part will vibrate excessively.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for E, density",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "fixed_faces",
                    description: "Face indices to fix (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "num_modes",
                    description: "Number of modes to extract",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(6)),
                },
            ],
            returns: "Natural frequencies (Hz) for each mode, mode shapes, and participation factors.",
            category: ToolCategory::Simulation,
        },

        ToolDef {
            name: "run_thermal_analysis",
            description: "Run steady-state thermal analysis (heat conduction)",
            long_description: "Solves the heat equation on a tetrahedral mesh with fixed-temperature, \
                heat-flux, and convection boundary conditions. Returns temperature distribution.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for thermal conductivity",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "hot_faces",
                    description: "Face indices at fixed high temperature (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "hot_temperature_c",
                    description: "Temperature at hot faces in degrees C",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "cold_faces",
                    description: "Face indices at fixed low temperature (0-based)",
                    param_type: ParamType::NumberArray,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "cold_temperature_c",
                    description: "Temperature at cold faces in degrees C",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
            ],
            returns: "Min/max temperature, heat flux, temperature distribution summary.",
            category: ToolCategory::Simulation,
        },

        ToolDef {
            name: "run_coupled_analysis",
            description: "Run coupled thermal-structural analysis",
            long_description: "Solves thermal conduction first, then feeds the temperature field \
                into a structural solver to compute thermal stresses. Returns temperature \
                distribution, von Mises stress, and displacement from combined thermal + \
                mechanical loads.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to analyze",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for properties (E, ν, α, k)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "hot_temperature_c",
                    description: "Temperature at hot boundary in °C",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "cold_temperature_c",
                    description: "Temperature at cold boundary in °C",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "reference_temperature_c",
                    description: "Stress-free reference temperature in °C",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(20.0)),
                },
            ],
            returns: "Temperature distribution, von Mises stress, thermal stress, displacement.",
            category: ToolCategory::Simulation,
        },

        ToolDef {
            name: "run_cfd",
            description: "Run pipe flow analysis (pressure drop, Reynolds number, velocity)",
            long_description: "Solves pipe flow using Darcy-Weisbach + Moody chart (LUT-first). \
                Returns pressure drop, friction factor, Reynolds number, and flow regime.\n\
                \n\
                For simple pipe geometries, this returns instant analytical results.",
            params: vec![
                ToolParam {
                    name: "diameter_mm",
                    description: "Pipe inner diameter in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "length_mm",
                    description: "Pipe length in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "velocity_m_s",
                    description: "Flow velocity in m/s",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "fluid_density",
                    description: "Fluid density in kg/m3",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(998.0)),
                },
                ToolParam {
                    name: "fluid_viscosity",
                    description: "Dynamic viscosity in Pa-s",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.001)),
                },
                ToolParam {
                    name: "roughness_mm",
                    description: "Pipe wall roughness in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.045)),
                },
            ],
            returns: "Reynolds number, flow regime, friction factor, pressure drop (Pa), volume flow rate.",
            category: ToolCategory::Simulation,
        },

        ToolDef {
            name: "optimize_topology",
            description: "Find optimal material layout for a design space (minimize weight)",
            long_description: "Given a design space, loads, and a volume fraction target, \
                optimizes material distribution to maximize stiffness. Returns density field \
                and compliance history.\n\
                \n\
                The AI CANNOT fake topology optimization — the result is non-intuitive \
                and requires iterative numerical computation.",
            params: vec![
                ToolParam {
                    name: "nx",
                    description: "Grid resolution in X (number of elements)",
                    param_type: ParamType::Integer,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "ny",
                    description: "Grid resolution in Y (number of elements)",
                    param_type: ParamType::Integer,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "volume_fraction",
                    description: "Target volume fraction (0.0-1.0), e.g. 0.4 = keep 40% material",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "load_node",
                    description: "Node index where load is applied",
                    param_type: ParamType::Integer,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "load_x",
                    description: "Load X component in N",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.0)),
                },
                ToolParam {
                    name: "load_y",
                    description: "Load Y component in N",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(-1.0)),
                },
                ToolParam {
                    name: "max_iterations",
                    description: "Maximum optimization iterations",
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(100)),
                },
            ],
            returns: "Density field, final compliance, volume fraction achieved, iteration count.",
            category: ToolCategory::Simulation,
        },

        // ===== EXPORT (additional formats) =====
        ToolDef {
            name: "export_step",
            description: "Export a solid to STEP format (AP203 or AP214)",
            long_description: "Writes an ISO 10303 STEP file for CAD interchange. AP203 is the \
                most compatible; AP214 includes colors and layers.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "protocol",
                    description: "STEP application protocol: 'ap203' or 'ap214'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("ap203")),
                },
                ToolParam {
                    name: "filename",
                    description: "Output filename (without extension)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "STEP file content and file size in bytes.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_3mf",
            description: "Export a solid to 3MF format for 3D printing",
            long_description: "Writes a 3MF (3D Manufacturing Format) ZIP archive. 3MF is the \
                modern replacement for STL — includes mesh, units, and metadata in one file.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "filename",
                    description: "Output filename (without extension)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "3MF file bytes (base64) and file size.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_gltf",
            description: "Export a solid to GLB (binary glTF) for 3D visualization",
            long_description: "Writes a GLB file suitable for web viewers, AR/VR, and \
                real-time rendering. Includes mesh with normals and optional PBR material.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "filename",
                    description: "Output filename (without extension)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "GLB file bytes (base64), triangle count, and file size.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_dxf",
            description: "Export solid edges to DXF for 2D/3D CAD interchange",
            long_description: "Writes a DXF R12 file containing the solid's wireframe edges. \
                Compatible with virtually all CAD/CAM software for 2D drawing interchange.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "filename",
                    description: "Output filename (without extension)",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "DXF file content and file size in bytes.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_pdf",
            description: "Export a technical drawing as PDF",
            long_description: "Generates orthographic projection views of the solid and \
                renders them as a dimensioned technical drawing in PDF format.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to draw",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "views",
                    description: "View directions to include: 'front', 'top', 'right', 'iso'",
                    param_type: ParamType::StringArray,
                    required: false,
                    default: Some(serde_json::json!(["front", "top", "right"])),
                },
                ToolParam {
                    name: "sheet_size",
                    description: "Sheet size: 'a4', 'a3', 'a2', 'a1'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("a3")),
                },
                ToolParam {
                    name: "title",
                    description: "Drawing title",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("Part Drawing")),
                },
            ],
            returns: "PDF file bytes (base64) and file size.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_svg",
            description: "Export a technical drawing as SVG",
            long_description: "Generates orthographic projection views of the solid and \
                renders them as a dimensioned technical drawing in SVG format. \
                SVG is web-native and scalable — good for embedding in reports.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to draw",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "views",
                    description: "View directions to include: 'front', 'top', 'right', 'iso'",
                    param_type: ParamType::StringArray,
                    required: false,
                    default: Some(serde_json::json!(["front", "top", "right"])),
                },
                ToolParam {
                    name: "sheet_size",
                    description: "Sheet size: 'a4', 'a3', 'a2', 'a1'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("a3")),
                },
                ToolParam {
                    name: "title",
                    description: "Drawing title",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("Part Drawing")),
                },
            ],
            returns: "SVG markup string and estimated dimensions.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_stl",
            description: "Export a solid as binary STL mesh",
            long_description: "Tessellates the solid and writes a binary STL file. \
                STL is universally supported by 3D printers and mesh tools.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "tolerance",
                    description: "Mesh tolerance in mm (smaller = finer mesh)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.1)),
                },
            ],
            returns: "Binary STL bytes (base64-encoded) and triangle count.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_obj",
            description: "Export a solid as Wavefront OBJ",
            long_description: "Tessellates the solid and writes an ASCII OBJ file with \
                vertices, normals, and texture coordinates. OBJ is widely supported by \
                3D modeling and rendering tools.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "name",
                    description: "Object name in OBJ file",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("part")),
                },
            ],
            returns: "OBJ text content, vertex count, and triangle count.",
            category: ToolCategory::Export,
        },

        ToolDef {
            name: "export_iges",
            description: "Export a solid as IGES 5.3",
            long_description: "Writes an IGES file for legacy CAD system compatibility. \
                Supports lines, NURBS curves, NURBS surfaces, trimmed surfaces, and colors.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to export",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "name",
                    description: "Part name in IGES file",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("Part")),
                },
            ],
            returns: "IGES text content and entity count.",
            category: ToolCategory::Export,
        },

        // ===== MANUFACTURING =====
        ToolDef {
            name: "run_dfm_check",
            description: "Check a part for manufacturing issues with a specific process",
            long_description: "Validates geometry against process-specific DFM rules. \
                Returns actionable issues with severity, location, and fix suggestions. \
                More detailed than check_manufacturability — includes constraint values.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to check",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "process",
                    description: "Process: 'cnc_3axis', 'cnc_5axis', 'injection_mold', 'fdm', 'sla', 'sls', 'sheet_metal'",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Material ID for process-specific constraints",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
            ],
            returns: "Pass/fail verdict, issue list with severity/location/fix, constraint values used.",
            category: ToolCategory::Manufacturing,
        },

        ToolDef {
            name: "generate_toolpath",
            description: "Generate CNC toolpath with feeds and speeds",
            long_description: "Creates a CNC toolpath for a solid part. Computes adaptive clearing, \
                rest machining, and finishing passes with LUT-backed feeds and speeds.\n\
                \n\
                Returns G-code and machining time estimate.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to machine",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "material",
                    description: "Work material ID for feeds/speeds lookup",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("6061-T6")),
                },
                ToolParam {
                    name: "tool_diameter",
                    description: "Cutter diameter in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(6.0)),
                },
                ToolParam {
                    name: "operation",
                    description: "Operation: 'adaptive_clear', 'contour', 'pocket', 'drill'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("adaptive_clear")),
                },
                ToolParam {
                    name: "dialect",
                    description: "G-code dialect: 'grbl', 'marlin', 'fanuc', 'linuxcnc'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("grbl")),
                },
            ],
            returns: "G-code program, estimated machining time, line count, and feeds/speeds used.",
            category: ToolCategory::Manufacturing,
        },

        ToolDef {
            name: "slice_for_printing",
            description: "Slice a solid for FDM 3D printing (generate G-code)",
            long_description: "Tessellates the solid, slices into layers, generates perimeters \
                and infill, and emits printer-ready G-code. Includes support generation.",
            params: vec![
                ToolParam {
                    name: "solid",
                    description: "Handle of the solid to slice",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "layer_height",
                    description: "Layer height in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.2)),
                },
                ToolParam {
                    name: "nozzle_diameter",
                    description: "Nozzle diameter in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(0.4)),
                },
                ToolParam {
                    name: "infill_percent",
                    description: "Infill percentage (0-100)",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(20.0)),
                },
                ToolParam {
                    name: "supports",
                    description: "Generate support structures?",
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some(serde_json::json!(true)),
                },
                ToolParam {
                    name: "dialect",
                    description: "G-code dialect: 'marlin', 'klipper', 'grbl'",
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("marlin")),
                },
            ],
            returns: "G-code program, layer count, estimated print time, estimated filament use.",
            category: ToolCategory::Manufacturing,
        },

        ToolDef {
            name: "nest_parts",
            description: "Nest 2D parts on a sheet for cutting (minimize waste)",
            long_description: "Arranges 2D part outlines on a rectangular sheet to minimize \
                material waste. Supports rotation and multiple copies. Returns positions \
                and material utilization percentage.",
            params: vec![
                ToolParam {
                    name: "parts",
                    description: "JSON array of part outlines, each a list of [x,y] points",
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "sheet_width",
                    description: "Sheet width in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "sheet_height",
                    description: "Sheet height in mm",
                    param_type: ParamType::Number,
                    required: true,
                    default: None,
                },
                ToolParam {
                    name: "spacing",
                    description: "Minimum spacing between parts in mm",
                    param_type: ParamType::Number,
                    required: false,
                    default: Some(serde_json::json!(3.0)),
                },
                ToolParam {
                    name: "allow_rotation",
                    description: "Allow 90-degree rotation of parts?",
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some(serde_json::json!(true)),
                },
            ],
            returns: "Part positions and rotations, material utilization percentage, waste area.",
            category: ToolCategory::Manufacturing,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_tools() {
        let reg = ToolRegistry::new();
        assert!(reg.list().len() >= 38, "should have at least 38 tools, got {}", reg.list().len());
    }

    #[test]
    fn all_tools_have_descriptions() {
        let reg = ToolRegistry::new();
        for tool in reg.list() {
            assert!(!tool.description.is_empty(), "tool {} has no description", tool.name);
            assert!(!tool.long_description.is_empty(), "tool {} has no long description", tool.name);
            assert!(!tool.returns.is_empty(), "tool {} has no return description", tool.name);
        }
    }

    #[test]
    fn all_tools_have_unique_names() {
        let reg = ToolRegistry::new();
        let mut names: Vec<&str> = reg.list().iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), reg.list().len(), "duplicate tool names found");
    }

    #[test]
    fn descriptions_are_ai_friendly() {
        let reg = ToolRegistry::new();
        for tool in reg.list() {
            // Descriptions should be short (< 100 chars)
            assert!(
                tool.description.len() < 100,
                "tool {} description too long ({} chars): {}",
                tool.name, tool.description.len(), tool.description
            );
            // No jargon in top-level descriptions
            let jargon = ["BRep", "B-Rep", "CSG", "topology", "manifold", "half-edge"];
            for word in &jargon {
                assert!(
                    !tool.description.contains(word),
                    "tool {} description contains jargon '{}': {}",
                    tool.name, word, tool.description
                );
            }
        }
    }

    #[test]
    fn required_params_have_no_defaults() {
        let reg = ToolRegistry::new();
        for tool in reg.list() {
            for param in &tool.params {
                if param.required {
                    assert!(
                        param.default.is_none(),
                        "tool {} param {} is required but has a default",
                        tool.name, param.name
                    );
                }
            }
        }
    }

    #[test]
    fn optional_params_have_defaults_or_are_truly_optional() {
        let reg = ToolRegistry::new();
        // Params that are genuinely optional with no default (omission is meaningful)
        let allow_no_default = [("analyze_part", "material")];
        for tool in reg.list() {
            for param in &tool.params {
                if !param.required && param.default.is_none() {
                    assert!(
                        allow_no_default.contains(&(tool.name, param.name)),
                        "tool {} param {} is optional with no default — add to allow list or add a default",
                        tool.name, param.name
                    );
                }
            }
        }
    }

    #[test]
    fn json_schema_generates() {
        let reg = ToolRegistry::new();
        for tool in reg.list() {
            let schema = reg.json_schema(tool.name);
            assert!(schema.is_some(), "tool {} has no JSON schema", tool.name);
            let schema = schema.unwrap();
            assert!(schema.get("type").is_some(), "schema has no type field");
            assert!(schema.get("properties").is_some(), "schema has no properties");
        }
    }

    #[test]
    fn create_box_schema_is_minimal() {
        let reg = ToolRegistry::new();
        let tool = reg.get("create_box").unwrap();
        let required: Vec<&str> = tool.params.iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        // Only 3 required params — width, height, depth. That's it.
        assert_eq!(required, vec!["width", "height", "depth"]);
    }

    #[test]
    fn analyze_part_is_single_tool() {
        let reg = ToolRegistry::new();
        let tool = reg.get("analyze_part").unwrap();
        // Only 1 required param — the solid handle.
        let required: Vec<&str> = tool.params.iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        assert_eq!(required, vec!["solid"]);
    }

    // --- New tool registration tests ---

    #[test]
    fn all_new_lookup_tools_registered() {
        let reg = ToolRegistry::new();
        for name in &[
            "lookup_material",
            "lookup_manufacturing_constraint",
            "lookup_thread",
            "lookup_tolerance_fit",
            "calculate_formula",
            "list_materials",
        ] {
            assert!(reg.get(name).is_some(), "missing lookup tool: {name}");
        }
    }

    #[test]
    fn all_new_geometry_tools_registered() {
        let reg = ToolRegistry::new();
        for name in &[
            "create_loft",
            "create_sweep",
            "add_fillet",
            "unfold_sheet_metal",
            "add_thread",
        ] {
            assert!(reg.get(name).is_some(), "missing geometry tool: {name}");
        }
    }

    #[test]
    fn all_new_simulation_tools_registered() {
        let reg = ToolRegistry::new();
        for name in &[
            "run_fea",
            "run_modal_analysis",
            "run_thermal_analysis",
            "run_coupled_analysis",
            "run_cfd",
            "optimize_topology",
        ] {
            let tool = reg.get(name);
            assert!(tool.is_some(), "missing simulation tool: {name}");
            assert_eq!(tool.unwrap().category, ToolCategory::Simulation);
        }
    }

    #[test]
    fn all_new_export_tools_registered() {
        let reg = ToolRegistry::new();
        for name in &[
            "export_step",
            "export_3mf",
            "export_gltf",
            "export_dxf",
            "export_pdf",
            "export_svg",
            "export_stl",
            "export_obj",
            "export_iges",
        ] {
            let tool = reg.get(name);
            assert!(tool.is_some(), "missing export tool: {name}");
            assert_eq!(tool.unwrap().category, ToolCategory::Export);
        }
    }

    #[test]
    fn all_new_manufacturing_tools_registered() {
        let reg = ToolRegistry::new();
        for name in &[
            "run_dfm_check",
            "generate_toolpath",
            "slice_for_printing",
            "nest_parts",
        ] {
            let tool = reg.get(name);
            assert!(tool.is_some(), "missing manufacturing tool: {name}");
            assert_eq!(tool.unwrap().category, ToolCategory::Manufacturing);
        }
    }

    #[test]
    fn run_cfd_schema_has_correct_required_params() {
        let reg = ToolRegistry::new();
        let tool = reg.get("run_cfd").unwrap();
        let required: Vec<&str> = tool.params.iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        assert_eq!(required, vec!["diameter_mm", "length_mm", "velocity_m_s"]);
    }

    #[test]
    fn optimize_topology_schema_has_correct_required_params() {
        let reg = ToolRegistry::new();
        let tool = reg.get("optimize_topology").unwrap();
        let required: Vec<&str> = tool.params.iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        assert_eq!(required, vec!["nx", "ny", "volume_fraction", "load_node"]);
    }

    #[test]
    fn export_step_schema_has_protocol_default() {
        let reg = ToolRegistry::new();
        let tool = reg.get("export_step").unwrap();
        let protocol_param = tool.params.iter().find(|p| p.name == "protocol").unwrap();
        assert!(!protocol_param.required);
        assert_eq!(protocol_param.default, Some(serde_json::json!("ap203")));
    }

    #[test]
    fn lookup_thread_schema_validates() {
        let reg = ToolRegistry::new();
        let schema = reg.json_schema("lookup_thread").unwrap();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str().unwrap(), "designation");
    }

    #[test]
    fn slice_for_printing_schema_validates() {
        let reg = ToolRegistry::new();
        let schema = reg.json_schema("slice_for_printing").unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("solid"));
        assert!(props.contains_key("layer_height"));
        assert!(props.contains_key("nozzle_diameter"));
        assert!(props.contains_key("infill_percent"));
        assert!(props.contains_key("supports"));
        // supports should be boolean type
        assert_eq!(props["supports"]["type"].as_str().unwrap(), "boolean");
    }
}
