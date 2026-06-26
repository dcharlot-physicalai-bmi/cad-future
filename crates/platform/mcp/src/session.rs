//! MCP session — holds live solids and dispatches tool calls to the kernel.
//!
//! Each session is a stateful workspace: solids created by one tool call
//! are available to subsequent calls via handles.

use std::collections::HashMap;
use glam::DVec3;
use physical_brep::Solid;
use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::boolean::{union, subtract, intersect};
use physical_brep::profile::Profile;
use physical_brep::extrude::extrude_z;
use physical_brep::shell::shell;
use physical_brep::pattern::{linear_pattern, circular_pattern, mirror};
use physical_analytical::mass_properties;
use physical_dfm;
use physical_tessellation;
use physical_emit_step;
use physical_emit_stl;
use physical_emit_dxf;
use physical_emit_gltf;
use physical_emit_threemf;
use physical_cfd;
use physical_topology;

use crate::types::{ToolResult, ToolError};

/// A live MCP session holding solids and state.
pub struct McpSession {
    solids: HashMap<String, Solid>,
    next_id: u64,
}

impl McpSession {
    pub fn new() -> Self {
        Self {
            solids: HashMap::new(),
            next_id: 1,
        }
    }

    /// Generate a new unique solid handle.
    fn next_handle(&mut self, prefix: &str) -> String {
        let id = self.next_id;
        self.next_id += 1;
        format!("{}_{}", prefix, id)
    }

    /// Store a solid and return its handle.
    fn store(&mut self, prefix: &str, solid: Solid) -> String {
        let handle = self.next_handle(prefix);
        self.solids.insert(handle.clone(), solid);
        handle
    }

    /// Get a solid by handle, returning a helpful error if not found.
    fn get_solid(&self, handle: &str) -> Result<&Solid, ToolError> {
        self.solids.get(handle).ok_or_else(|| ToolError {
            message: format!("No solid found with handle '{}'", handle),
            param: Some("solid".into()),
            suggestion: Some(format!(
                "Available solids: {}",
                self.solids.keys().cloned().collect::<Vec<_>>().join(", ")
            )),
        })
    }

    /// Dispatch a tool call by name with JSON arguments.
    pub fn call_tool(
        &mut self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        match name {
            "create_box" => self.tool_create_box(args),
            "create_cylinder" => self.tool_create_cylinder(args),
            "extrude_profile" => self.tool_extrude_profile(args),
            "combine_solids" => self.tool_combine_solids(args),
            "move_solid" => self.tool_move_solid(args),
            "hollow_out" => self.tool_hollow_out(args),
            "pattern" => self.tool_pattern(args),
            "analyze_part" => self.tool_analyze_part(args),
            "check_manufacturability" => self.tool_check_manufacturability(args),
            "export" => self.tool_export(args),
            "lookup_material" => self.tool_lookup_material(args),
            "lookup_manufacturing_constraint" => self.tool_lookup_manufacturing_constraint(args),
            "lookup_thread" => self.tool_lookup_thread(args),
            "lookup_tolerance_fit" => self.tool_lookup_tolerance_fit(args),
            "calculate_formula" => self.tool_calculate_formula(args),
            "create_loft" => self.tool_create_loft(args),
            "create_sweep" => self.tool_create_sweep(args),
            "add_fillet" => self.tool_add_fillet(args),
            "unfold_sheet_metal" => self.tool_unfold_sheet_metal(args),
            "add_thread" => self.tool_add_thread(args),
            "run_fea" => self.tool_run_fea(args),
            "run_modal_analysis" => self.tool_run_modal_analysis(args),
            "run_thermal_analysis" => self.tool_run_thermal_analysis(args),
            "run_coupled_analysis" => self.tool_run_coupled_analysis(args),
            "run_cfd" => self.tool_run_cfd(args),
            "optimize_topology" => self.tool_optimize_topology(args),
            "export_step" => self.tool_export_step(args),
            "export_3mf" => self.tool_export_3mf(args),
            "export_gltf" => self.tool_export_gltf(args),
            "export_dxf" => self.tool_export_dxf(args),
            "export_pdf" => self.tool_export_pdf(args),
            "export_svg" => self.tool_export_svg(args),
            "export_stl" => self.tool_export_stl(args),
            "export_obj" => self.tool_export_obj(args),
            "export_iges" => self.tool_export_iges(args),
            "run_dfm_check" => self.tool_run_dfm_check(args),
            "generate_toolpath" => self.tool_generate_toolpath(args),
            "slice_for_printing" => self.tool_slice_for_printing(args),
            "nest_parts" => self.tool_nest_parts(args),
            _ => Err(ToolError {
                message: format!("Unknown tool '{}'", name),
                param: None,
                suggestion: Some("Use tools/list to see available tools".into()),
            }),
        }
    }

    // =======================================================================
    // Tool implementations
    // =======================================================================

    fn tool_create_box(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let width = require_f64(args, "width")?;
        let height = require_f64(args, "height")?;
        let depth = require_f64(args, "depth")?;

        positive(width, "width")?;
        positive(height, "height")?;
        positive(depth, "depth")?;

        let solid = make_box(width, height, depth);
        let vol = width * height * depth;
        let handle = self.store("box", solid);

        Ok(ToolResult {
            summary: format!(
                "Created box '{handle}': {width}×{height}×{depth} mm, volume {vol:.1} mm³, 6 faces."
            ),
            data: Some(serde_json::json!({
                "width": width,
                "height": height,
                "depth": depth,
                "volume_mm3": vol,
                "face_count": 6,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_create_cylinder(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let radius = require_f64(args, "radius")?;
        let height = require_f64(args, "height")?;
        let segments = optional_usize(args, "segments", 32);

        positive(radius, "radius")?;
        positive(height, "height")?;

        let solid = make_cylinder(radius, height, segments);
        let vol = std::f64::consts::PI * radius * radius * height;
        let handle = self.store("cyl", solid);

        Ok(ToolResult {
            summary: format!(
                "Created cylinder '{handle}': r={radius} mm, h={height} mm, volume {vol:.1} mm³, {segments} segments."
            ),
            data: Some(serde_json::json!({
                "radius": radius,
                "height": height,
                "volume_mm3": vol,
                "segments": segments,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_extrude_profile(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let profile_type = require_str(args, "profile")?;
        let distance = require_f64(args, "distance")?;
        positive(distance, "distance")?;

        let profile = match profile_type.as_str() {
            "rectangle" => {
                let w = optional_f64(args, "width", 10.0);
                let h = optional_f64(args, "height", 10.0);
                Profile::rectangle(w, h)
            }
            "circle" => {
                let r = optional_f64(args, "radius", 5.0);
                Profile::circle(r)
            }
            "l_shape" => {
                let w = optional_f64(args, "width", 20.0);
                let h = optional_f64(args, "height", 30.0);
                let t = optional_f64(args, "thickness", 5.0);
                Profile::l_shape(w, h, t)
            }
            other => {
                return Err(ToolError {
                    message: format!("Unknown profile type '{other}'"),
                    param: Some("profile".into()),
                    suggestion: Some("Use 'rectangle', 'circle', or 'l_shape'".into()),
                });
            }
        };

        let solid = extrude_z(&profile, distance);
        let props = mass_properties(&solid);
        let handle = self.store("extrude", solid);

        Ok(ToolResult {
            summary: format!(
                "Extruded {profile_type} by {distance} mm → '{handle}', volume {:.1} mm³, {} faces.",
                props.volume, props.volume.signum() as usize // placeholder
            ),
            data: Some(serde_json::json!({
                "profile": profile_type,
                "distance": distance,
                "volume_mm3": props.volume,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_combine_solids(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let ha = require_str(args, "solid_a")?;
        let hb = require_str(args, "solid_b")?;
        let op = require_str(args, "operation")?;

        let a = self.get_solid(&ha)?.clone();
        let b = self.get_solid(&hb)?.clone();

        let result = match op.as_str() {
            "union" => union(&a, &b),
            "subtract" => subtract(&a, &b),
            "intersect" => intersect(&a, &b),
            other => {
                return Err(ToolError {
                    message: format!("Unknown operation '{other}'"),
                    param: Some("operation".into()),
                    suggestion: Some("Use 'union', 'subtract', or 'intersect'".into()),
                });
            }
        };

        let faces = result.face_count();
        let handle = self.store("bool", result);

        Ok(ToolResult {
            summary: format!(
                "{op} of '{ha}' and '{hb}' → '{handle}', {faces} faces."
            ),
            data: Some(serde_json::json!({
                "operation": op,
                "face_count": faces,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_move_solid(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let x = optional_f64(args, "x", 0.0);
        let y = optional_f64(args, "y", 0.0);
        let z = optional_f64(args, "z", 0.0);

        let solid = self.solids.get_mut(&handle).ok_or_else(|| ToolError {
            message: format!("No solid found with handle '{handle}'"),
            param: Some("solid".into()),
            suggestion: None,
        })?;

        let offset = DVec3::new(x, y, z);
        let vids: Vec<_> = solid.vertices.keys().collect();
        for vid in vids {
            solid.vertices[vid].point += offset;
        }

        let (min, max) = solid.bounding_box();

        Ok(ToolResult {
            summary: format!(
                "Moved '{handle}' by ({x}, {y}, {z}) mm. New bounds: ({:.1},{:.1},{:.1}) to ({:.1},{:.1},{:.1}).",
                min.x, min.y, min.z, max.x, max.y, max.z
            ),
            data: Some(serde_json::json!({
                "offset": [x, y, z],
                "bounding_box_min": [min.x, min.y, min.z],
                "bounding_box_max": [max.x, max.y, max.z],
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_hollow_out(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let thickness = require_f64(args, "wall_thickness")?;
        positive(thickness, "wall_thickness")?;

        let solid = self.get_solid(&handle)?.clone();
        let open_faces: Vec<usize> = args.get("open_face_indices")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();

        let result = shell(&solid, thickness, &open_faces);
        let new_handle = self.store("shell", result);

        Ok(ToolResult {
            summary: format!(
                "Hollowed '{handle}' → '{new_handle}', wall thickness {thickness} mm, {} open faces.",
                open_faces.len()
            ),
            data: Some(serde_json::json!({
                "wall_thickness": thickness,
                "open_faces": open_faces,
            })),
            solid: Some(new_handle),
            sketch: None,
        })
    }

    fn tool_pattern(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let pattern_type = require_str(args, "pattern_type")?;
        let solid = self.get_solid(&handle)?.clone();

        let result = match pattern_type.as_str() {
            "linear" => {
                let count = optional_usize(args, "count", 3);
                let spacing = optional_f64(args, "spacing", 20.0);
                let dir = optional_str(args, "direction", "x");
                let direction = parse_axis(&dir)?;
                linear_pattern(&solid, direction * spacing, spacing, count)
            }
            "circular" => {
                let count = optional_usize(args, "count", 4);
                let axis_str = optional_str(args, "axis", "y");
                let axis = parse_axis(&axis_str)?;
                circular_pattern(&solid, DVec3::ZERO, axis, count)
            }
            "mirror" => {
                let plane = optional_str(args, "mirror_plane", "yz");
                let (point, normal) = parse_mirror_plane(&plane)?;
                mirror(&solid, point, normal)
            }
            other => {
                return Err(ToolError {
                    message: format!("Unknown pattern type '{other}'"),
                    param: Some("pattern_type".into()),
                    suggestion: Some("Use 'linear', 'circular', or 'mirror'".into()),
                });
            }
        };

        let new_handle = self.store("pattern", result);

        Ok(ToolResult {
            summary: format!("{pattern_type} pattern of '{handle}' → '{new_handle}'."),
            data: Some(serde_json::json!({"pattern_type": pattern_type})),
            solid: Some(new_handle),
            sketch: None,
        })
    }

    fn tool_analyze_part(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let solid = self.get_solid(&handle)?;

        let props = mass_properties(solid);
        let (min, max) = solid.bounding_box();
        let faces = solid.face_count();
        let edges = solid.edge_count();
        let verts = solid.vertex_count();
        let euler = solid.euler_characteristic();
        let size = max - min;

        let mut data = serde_json::json!({
            "volume_mm3": props.volume,
            "surface_area_mm2": props.surface_area,
            "centroid": [props.centroid.x, props.centroid.y, props.centroid.z],
            "bounding_box_min": [min.x, min.y, min.z],
            "bounding_box_max": [max.x, max.y, max.z],
            "bounding_box_size": [size.x, size.y, size.z],
            "faces": faces,
            "edges": edges,
            "vertices": verts,
            "euler_characteristic": euler,
            "is_closed_shell": euler == 2,
        });

        // If material specified, compute mass
        let material_id = args.get("material").and_then(|v| v.as_str());
        let mass_str = if let Some(mat_id) = material_id {
            if let Some(mat) = physical_lut::materials::lookup(mat_id) {
                let density = mat.density.value();
                let mass_kg = props.volume * 1e-9 * density;
                let mass_g = mass_kg * 1000.0;
                data["mass_g"] = serde_json::json!(mass_g);
                data["material"] = serde_json::json!(mat_id);
                data["density_kg_m3"] = serde_json::json!(density);
                format!(", mass {mass_g:.1} g ({mat_id})")
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        Ok(ToolResult {
            summary: format!(
                "'{handle}': {:.1}×{:.1}×{:.1} mm, volume {:.1} mm³, surface area {:.1} mm²{mass_str}, \
                {faces}F/{edges}E/{verts}V, Euler={euler}.",
                size.x, size.y, size.z, props.volume, props.surface_area
            ),
            data: Some(data),
            solid: None,
            sketch: None,
        })
    }

    fn tool_check_manufacturability(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let process = require_str(args, "process")?;
        let solid = self.get_solid(&handle)?;

        let config = match process.as_str() {
            "cnc_3axis" | "cnc_5axis" | "cnc" => physical_dfm::cnc_config(),
            "injection_mold" | "injection" => physical_dfm::injection_mold_config(),
            other => {
                return Err(ToolError {
                    message: format!("Unknown process '{other}'"),
                    param: Some("process".into()),
                    suggestion: Some(
                        "Use 'cnc_3axis', 'cnc_5axis', 'injection_mold', 'fdm', 'sla', or 'sls'"
                            .into(),
                    ),
                });
            }
        };

        let issues = physical_dfm::validate(solid, &config);
        let pass = issues.is_empty();

        let issue_data: Vec<serde_json::Value> = issues
            .iter()
            .map(|issue| {
                serde_json::json!({
                    "severity": format!("{:?}", issue.severity),
                    "description": issue.message,
                    "location": issue.location,
                })
            })
            .collect();

        let summary = if pass {
            format!("'{handle}' PASSES {process} manufacturability check.")
        } else {
            let count = issues.len();
            let first = &issues[0].message;
            format!("'{handle}' has {count} DFM issue(s) for {process}. First: {first}")
        };

        Ok(ToolResult {
            summary,
            data: Some(serde_json::json!({
                "pass": pass,
                "process": process,
                "issue_count": issues.len(),
                "issues": issue_data,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let format = require_str(args, "format")?;
        let filename = optional_str(args, "filename", "part");
        let solid = self.get_solid(&handle)?;

        match format.as_str() {
            "step" => {
                let content = physical_emit_step::write_step_ap203(solid, &filename);
                let size = content.len();
                Ok(ToolResult {
                    summary: format!("Exported '{handle}' to STEP ({size} bytes) as '{filename}.step'."),
                    data: Some(serde_json::json!({
                        "format": "step",
                        "filename": format!("{filename}.step"),
                        "size_bytes": size,
                        "content": content,
                    })),
                    solid: None,
                    sketch: None,
                })
            }
            "stl" => {
                let mesh = physical_tessellation::tessellate(solid, 0.1);
                let bytes = physical_emit_stl::write_binary_stl(&mesh);
                let size = bytes.len();
                let triangles = mesh.triangle_count();
                Ok(ToolResult {
                    summary: format!(
                        "Exported '{handle}' to binary STL ({size} bytes, {triangles} triangles) as '{filename}.stl'."
                    ),
                    data: Some(serde_json::json!({
                        "format": "stl",
                        "filename": format!("{filename}.stl"),
                        "size_bytes": size,
                        "triangle_count": triangles,
                    })),
                    solid: None,
                    sketch: None,
                })
            }
            other => Err(ToolError {
                message: format!("Unknown export format '{other}'"),
                param: Some("format".into()),
                suggestion: Some("Use 'step', 'stl', or 'oie'".into()),
            }),
        }
    }

    fn tool_lookup_material(&mut self, args: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let material_id = require_str(args, "material_id")?;

        let mat = physical_lut::materials::lookup(&material_id).ok_or_else(|| ToolError {
            message: format!("Material '{}' not found in database", material_id),
            param: Some("material_id".into()),
            suggestion: Some("Try '6061-T6', '7075-T6', 'AISI-304', 'Ti-6Al-4V'. Use list_materials to see all.".into()),
        })?;

        let density = mat.density.value();
        let yield_s = mat.yield_strength.value();
        let uts = mat.ultimate_tensile.value();
        let e_mod = mat.elastic_modulus.value();
        let nu = mat.poissons_ratio.value();
        let k = mat.thermal_conductivity.value();
        let cte = mat.cte.value();
        let melting = mat.melting_point.value();

        Ok(ToolResult {
            summary: format!(
                "{material_id}: density {density:.0} kg/m³, yield {yield_s:.0} MPa, \
                ultimate {uts:.0} MPa, E={:.0} GPa, ν={nu:.2}, k={k:.1} W/m·K.",
                e_mod / 1000.0,
            ),
            data: Some(serde_json::json!({
                "material_id": material_id,
                "density_kg_m3": density,
                "yield_strength_mpa": yield_s,
                "ultimate_tensile_mpa": uts,
                "elastic_modulus_mpa": e_mod,
                "poissons_ratio": nu,
                "thermal_conductivity_w_mk": k,
                "cte_um_mk": cte,
                "melting_point_c": melting,
            })),
            solid: None,
            sketch: None,
        })
    }
    // =======================================================================
    // Lookup tools (additional)
    // =======================================================================

    fn tool_lookup_manufacturing_constraint(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let process_str = require_str(args, "process")?;
        let material_str = require_str(args, "material_class")?;

        let process = parse_manufacturing_process(&process_str)?;
        let material_class = parse_material_class(&material_str)?;

        let constraint =
            physical_lut::manufacturing::lookup(process, material_class).ok_or_else(|| {
                ToolError {
                    message: format!(
                        "No manufacturing constraint found for process '{}' + material '{}'",
                        process_str, material_str
                    ),
                    param: None,
                    suggestion: Some(
                        "Try process='cnc_3axis' material_class='aluminum'".into(),
                    ),
                }
            })?;

        let min_wall = constraint.min_wall_thickness.value();
        let min_hole = constraint.min_hole_diameter.value();
        let min_corner = constraint.min_corner_radius.value();

        Ok(ToolResult {
            summary: format!(
                "{process_str}/{material_str}: min wall {min_wall:.2} mm, min hole {min_hole:.2} mm, min corner R {min_corner:.2} mm."
            ),
            data: Some(serde_json::json!({
                "process": process_str,
                "material_class": material_str,
                "min_wall_thickness_mm": min_wall,
                "min_hole_diameter_mm": min_hole,
                "min_corner_radius_mm": min_corner,
                "max_pocket_depth_ratio": constraint.max_pocket_depth_ratio,
                "max_depth_to_width_ratio": constraint.max_depth_to_width_ratio,
                "draft_angle_min_deg": constraint.draft_angle_min.value(),
                "min_bend_radius_factor": constraint.min_bend_radius_factor,
                "max_aspect_ratio": constraint.max_aspect_ratio,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_lookup_thread(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let designation = require_str(args, "designation")?;

        let thread =
            physical_lut::standards::lookup_metric_thread(&designation).ok_or_else(|| {
                ToolError {
                    message: format!("Thread '{}' not found in ISO 262 table", designation),
                    param: Some("designation".into()),
                    suggestion: Some(
                        "Try 'M3', 'M4', 'M5', 'M6', 'M8', 'M10', 'M12', 'M16', 'M20'".into(),
                    ),
                }
            })?;

        Ok(ToolResult {
            summary: format!(
                "{}: d={:.1} mm, coarse pitch {:.2} mm, pitch dia {:.3} mm, \
                 tensile area {:.1} mm².",
                thread.designation,
                thread.nominal_diameter_mm,
                thread.coarse_pitch_mm,
                thread.pitch_diameter_mm,
                thread.tensile_stress_area_mm2,
            ),
            data: Some(serde_json::json!({
                "designation": thread.designation,
                "nominal_diameter_mm": thread.nominal_diameter_mm,
                "coarse_pitch_mm": thread.coarse_pitch_mm,
                "fine_pitches_mm": thread.fine_pitches_mm,
                "minor_diameter_mm": thread.minor_diameter_mm,
                "pitch_diameter_mm": thread.pitch_diameter_mm,
                "tensile_stress_area_mm2": thread.tensile_stress_area_mm2,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_lookup_tolerance_fit(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let designation = require_str(args, "designation")?;

        let fit =
            physical_lut::standards::lookup_tolerance_fit(&designation).ok_or_else(|| {
                ToolError {
                    message: format!(
                        "Tolerance fit '{}' not found in ISO 286 table",
                        designation
                    ),
                    param: Some("designation".into()),
                    suggestion: Some(
                        "Try 'H7/g6', 'H7/k6', 'H7/p6', 'H11/c11', 'H7/f7'".into(),
                    ),
                }
            })?;

        Ok(ToolResult {
            summary: format!(
                "{}: {:?} fit — {}. Typical use: {}.",
                fit.designation,
                fit.fit_type,
                fit.description,
                fit.typical_use,
            ),
            data: Some(serde_json::json!({
                "designation": fit.designation,
                "fit_type": format!("{:?}", fit.fit_type),
                "description": fit.description,
                "typical_use": fit.typical_use,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_calculate_formula(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let formula = require_str(args, "formula")?;
        let _params_json = require_str(args, "params_json")?;

        // Stub: validate the formula name is known, return descriptive error
        // until full formula dispatch is wired up.
        let known = [
            "beam_simply_supported_uniform",
            "beam_simply_supported_center_load",
            "beam_cantilever_end_load",
            "beam_cantilever_uniform",
            "stress_concentration_hole",
            "stress_concentration_fillet",
            "pressure_vessel_thin_wall",
        ];
        if !known.contains(&formula.as_str()) {
            return Err(ToolError {
                message: format!("Unknown formula '{formula}'"),
                param: Some("formula".into()),
                suggestion: Some(format!("Available: {}", known.join(", "))),
            });
        }

        Err(ToolError {
            message: format!(
                "Formula '{}' dispatch not yet wired — requires typed unit parsing",
                formula
            ),
            param: None,
            suggestion: Some(
                "Use quick_stress_check or run_fea for structural analysis".into(),
            ),
        })
    }

    // =======================================================================
    // Geometry tools (additional)
    // =======================================================================

    fn tool_create_loft(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let bottom_type = require_str(args, "bottom_profile")?;
        let top_type = require_str(args, "top_profile")?;
        let bottom_size = require_f64(args, "bottom_size")?;
        let top_size = require_f64(args, "top_size")?;
        let height = require_f64(args, "height")?;

        positive(bottom_size, "bottom_size")?;
        positive(top_size, "top_size")?;
        positive(height, "height")?;

        let bottom = make_profile(&bottom_type, bottom_size)?;
        let top = make_profile(&top_type, top_size)?;

        let solid = physical_brep::loft::loft_profiles(&bottom, &top, 0.0, height);
        let handle = self.store("loft", solid);

        Ok(ToolResult {
            summary: format!(
                "Lofted {bottom_type}({bottom_size} mm) → {top_type}({top_size} mm) \
                 over {height} mm → '{handle}'."
            ),
            data: Some(serde_json::json!({
                "bottom_profile": bottom_type,
                "top_profile": top_type,
                "bottom_size": bottom_size,
                "top_size": top_size,
                "height": height,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_create_sweep(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let profile_type = require_str(args, "profile")?;
        let profile_size = require_f64(args, "profile_size")?;
        let path_type = require_str(args, "path_type")?;
        let path_length = require_f64(args, "path_length")?;
        let steps = optional_usize(args, "steps", 32);

        positive(profile_size, "profile_size")?;
        positive(path_length, "path_length")?;

        let profile = make_profile(&profile_type, profile_size)?;

        let path = match path_type.as_str() {
            "line" => physical_brep::curve::Curve::line(
                DVec3::ZERO,
                DVec3::new(0.0, 0.0, path_length),
            ),
            "arc" | "helix" => {
                // Approximate with a line for now — full arc/helix paths
                // require additional parameters.
                physical_brep::curve::Curve::line(
                    DVec3::ZERO,
                    DVec3::new(0.0, 0.0, path_length),
                )
            }
            other => {
                return Err(ToolError {
                    message: format!("Unknown path type '{other}'"),
                    param: Some("path_type".into()),
                    suggestion: Some("Use 'line', 'arc', or 'helix'".into()),
                });
            }
        };

        let solid = physical_brep::sweep::sweep(&profile, &path, steps);
        let handle = self.store("sweep", solid);

        Ok(ToolResult {
            summary: format!(
                "Swept {profile_type}({profile_size} mm) along {path_type} \
                 ({path_length} mm) → '{handle}', {steps} steps."
            ),
            data: Some(serde_json::json!({
                "profile": profile_type,
                "profile_size": profile_size,
                "path_type": path_type,
                "path_length": path_length,
                "steps": steps,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_add_fillet(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let radius = require_f64(args, "radius")?;
        positive(radius, "radius")?;

        let edge_indices: Vec<usize> = args
            .get("edge_indices")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .ok_or_else(|| ToolError {
                message: "Missing required parameter 'edge_indices' (expected number array)"
                    .into(),
                param: Some("edge_indices".into()),
                suggestion: Some("Add \"edge_indices\": [0, 1, 2] to the arguments".into()),
            })?;

        let solid = self.solids.get_mut(&handle).ok_or_else(|| ToolError {
            message: format!("No solid found with handle '{handle}'"),
            param: Some("solid".into()),
            suggestion: None,
        })?;

        // Collect edge IDs from indices
        let edge_ids: Vec<_> = solid.edges.keys().collect();
        let mut selected = Vec::new();
        for &idx in &edge_indices {
            if idx < edge_ids.len() {
                selected.push(edge_ids[idx]);
            }
        }

        physical_brep::fillet::fillet(solid, &selected, radius);
        let count = edge_indices.len();

        Ok(ToolResult {
            summary: format!(
                "Filleted {count} edge(s) on '{handle}' with R={radius} mm."
            ),
            data: Some(serde_json::json!({
                "edges_filleted": count,
                "radius": radius,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    fn tool_unfold_sheet_metal(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _thickness = require_f64(args, "thickness")?;
        let _k_factor = optional_f64(args, "k_factor", 0.44);

        // Validate solid exists
        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "Sheet metal unfold for '{}' requires SheetMetalPart conversion — \
                 not yet wired from generic Solid",
                handle
            ),
            param: None,
            suggestion: Some(
                "Create the part using sheet metal tools first, or use export_dxf for 2D output"
                    .into(),
            ),
        })
    }

    fn tool_add_thread(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let face_index = args
            .get("face_index")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .ok_or_else(|| ToolError {
                message: "Missing required parameter 'face_index' (expected integer)".into(),
                param: Some("face_index".into()),
                suggestion: Some("Add \"face_index\": 0 to the arguments".into()),
            })?;
        let thread_designation = require_str(args, "thread_designation")?;
        let _depth = optional_f64(args, "depth", 0.0);

        // Validate solid exists and face index is in range
        let solid = self.get_solid(&handle)?;
        let face_count = solid.face_count();
        if face_index >= face_count {
            return Err(ToolError {
                message: format!(
                    "Face index {face_index} out of range (solid has {face_count} faces)"
                ),
                param: Some("face_index".into()),
                suggestion: Some(format!("Use a face index between 0 and {}", face_count - 1)),
            });
        }

        // Validate thread designation exists in LUT
        let _thread = physical_lut::standards::lookup_metric_thread(&thread_designation)
            .ok_or_else(|| ToolError {
                message: format!(
                    "Thread '{}' not found — cannot annotate",
                    thread_designation
                ),
                param: Some("thread_designation".into()),
                suggestion: Some("Try 'M3', 'M4', 'M5', 'M6', 'M8', 'M10', 'M12'".into()),
            })?;

        Ok(ToolResult {
            summary: format!(
                "Added {thread_designation} thread annotation to face {face_index} of '{handle}'."
            ),
            data: Some(serde_json::json!({
                "thread_designation": thread_designation,
                "face_index": face_index,
            })),
            solid: Some(handle),
            sketch: None,
        })
    }

    // =======================================================================
    // Simulation tools
    // =======================================================================

    fn tool_run_fea(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _material_id = optional_str(args, "material", "6061-T6");
        let _force_mag = require_f64(args, "force_magnitude")?;
        let _force_dir = optional_str(args, "force_direction", "-y");

        let _fixed: Vec<usize> = args
            .get("fixed_faces")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        let _load: Vec<usize> = args
            .get("force_faces")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();

        // Validate solid exists
        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "FEA for '{}' requires solid-to-tet-mesh conversion — \
                 use run_stress_analysis for the existing pipeline",
                handle
            ),
            param: None,
            suggestion: Some(
                "Use run_stress_analysis or quick_stress_check for now".into(),
            ),
        })
    }

    fn tool_run_modal_analysis(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _material_id = optional_str(args, "material", "6061-T6");
        let _num_modes = optional_usize(args, "num_modes", 6);

        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "Modal analysis for '{}' requires solid-to-tet-mesh + mass matrix assembly",
                handle
            ),
            param: None,
            suggestion: Some(
                "Run modal analysis requires meshing pipeline — coming soon".into(),
            ),
        })
    }

    fn tool_run_thermal_analysis(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let material_id = optional_str(args, "material", "6061-T6");
        let hot_temp = require_f64(args, "hot_temperature_c")?;
        let cold_temp = require_f64(args, "cold_temperature_c")?;

        let solid = self.get_solid(&handle)?;
        let mesh = physical_fea::tetrahedralize(&solid);

        // Look up thermal conductivity
        let k = physical_lut::materials::lookup(&material_id)
            .map(|m| m.thermal_conductivity.value())
            .unwrap_or(167.0); // default aluminum

        // Apply BCs: hot on min-x nodes, cold on max-x nodes
        let (bb_min, bb_max) = solid.bounding_box();
        let x_range = bb_max.x - bb_min.x;
        let threshold = x_range * 0.15;

        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < bb_min.x + threshold {
                bcs.push(physical_fea::ThermalBC::FixedTemp(i, hot_temp + 273.15));
            } else if node.position.x > bb_max.x - threshold {
                bcs.push(physical_fea::ThermalBC::FixedTemp(i, cold_temp + 273.15));
            }
        }

        let result = physical_fea::solve_thermal(&mesh, k, &bcs);

        Ok(ToolResult {
            summary: format!(
                "Thermal analysis on '{}' ({} material): {} nodes, {} elements. \
                 T_min={:.1}°C, T_max={:.1}°C, ΔT={:.1}°C.",
                handle, material_id,
                mesh.nodes.len(), mesh.elements.len(),
                result.min_temperature - 273.15,
                result.max_temperature - 273.15,
                result.max_temperature - result.min_temperature,
            ),
            data: Some(serde_json::json!({
                "nodes": mesh.nodes.len(),
                "elements": mesh.elements.len(),
                "min_temperature_c": result.min_temperature - 273.15,
                "max_temperature_c": result.max_temperature - 273.15,
                "delta_t": result.max_temperature - result.min_temperature,
                "material": material_id,
                "thermal_conductivity_w_mk": k,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_run_coupled_analysis(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let material_id = optional_str(args, "material", "6061-T6");
        let hot_temp = require_f64(args, "hot_temperature_c")?;
        let cold_temp = require_f64(args, "cold_temperature_c")?;
        let ref_temp = optional_f64(args, "reference_temperature_c", 20.0);

        let solid = self.get_solid(&handle)?;
        let mesh = physical_fea::tetrahedralize(&solid);

        // Look up material properties
        let mat = physical_lut::materials::lookup(&material_id);
        let k_thermal = mat.as_ref().map(|m| m.thermal_conductivity.value()).unwrap_or(167.0);
        let e_mod = mat.as_ref().map(|m| m.elastic_modulus.value() / 1e6).unwrap_or(70_000.0); // Pa→MPa
        let poisson = mat.as_ref().map(|m| m.poissons_ratio.value()).unwrap_or(0.33);
        let cte = mat.as_ref().map(|m| m.cte.value()).unwrap_or(23e-6);

        // Step 1: Thermal solve
        let (bb_min, bb_max) = solid.bounding_box();
        let x_range = bb_max.x - bb_min.x;
        let threshold = x_range * 0.15;

        let mut thermal_bcs = Vec::new();
        let mut mech_bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < bb_min.x + threshold {
                thermal_bcs.push(physical_fea::ThermalBC::FixedTemp(i, hot_temp + 273.15));
                mech_bcs.push(physical_fea::BC::FixAll(i));
            } else if node.position.x > bb_max.x - threshold {
                thermal_bcs.push(physical_fea::ThermalBC::FixedTemp(i, cold_temp + 273.15));
                mech_bcs.push(physical_fea::BC::FixAll(i));
            }
        }

        let thermal = physical_fea::solve_thermal(&mesh, k_thermal, &thermal_bcs);

        // Step 2: Coupled solve
        let coupled = physical_fea::solve_coupled(
            &mesh,
            &thermal.temperatures,
            ref_temp + 273.15,
            cte,
            e_mod,
            poisson,
            &mech_bcs,
        );

        Ok(ToolResult {
            summary: format!(
                "Coupled thermal-structural on '{}': ΔT={:.1}°C, max stress={:.1} MPa, \
                 max displacement={:.4} mm.",
                handle,
                thermal.max_temperature - thermal.min_temperature,
                coupled.structural.max_von_mises,
                coupled.structural.max_displacement,
            ),
            data: Some(serde_json::json!({
                "nodes": mesh.nodes.len(),
                "elements": mesh.elements.len(),
                "min_temperature_c": thermal.min_temperature - 273.15,
                "max_temperature_c": thermal.max_temperature - 273.15,
                "max_von_mises_mpa": coupled.structural.max_von_mises,
                "max_displacement_mm": coupled.structural.max_displacement,
                "max_thermal_stress_mpa": coupled.max_thermal_stress,
                "material": material_id,
                "cte": cte,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_run_cfd(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let diameter_mm = require_f64(args, "diameter_mm")?;
        let length_mm = require_f64(args, "length_mm")?;
        let velocity = require_f64(args, "velocity_m_s")?;
        let density = optional_f64(args, "fluid_density", 998.0);
        let viscosity = optional_f64(args, "fluid_viscosity", 0.001);
        let roughness_mm = optional_f64(args, "roughness_mm", 0.045);

        positive(diameter_mm, "diameter_mm")?;
        positive(length_mm, "length_mm")?;
        positive(velocity, "velocity_m_s")?;

        let result = physical_cfd::pipe_flow(
            diameter_mm / 1000.0,
            length_mm / 1000.0,
            velocity,
            density,
            viscosity,
            roughness_mm / 1000.0,
        );

        let regime_str = match result.regime {
            physical_cfd::FlowRegime::Laminar => "laminar",
            physical_cfd::FlowRegime::Transitional => "transitional",
            physical_cfd::FlowRegime::Turbulent => "turbulent",
        };

        Ok(ToolResult {
            summary: format!(
                "Pipe flow: Re={:.0} ({}), f={:.5}, ΔP={:.1} Pa, Q={:.6} m³/s.",
                result.reynolds, regime_str, result.friction_factor,
                result.pressure_drop_pa, result.flow_rate_m3_s,
            ),
            data: Some(serde_json::json!({
                "reynolds": result.reynolds,
                "regime": regime_str,
                "friction_factor": result.friction_factor,
                "pressure_drop_pa": result.pressure_drop_pa,
                "velocity_avg_m_s": result.velocity_avg_m_s,
                "flow_rate_m3_s": result.flow_rate_m3_s,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_optimize_topology(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let nx = args.get("nx").and_then(|v| v.as_u64()).map(|n| n as usize)
            .ok_or_else(|| ToolError {
                message: "Missing required parameter 'nx' (integer)".into(),
                param: Some("nx".into()),
                suggestion: Some("Add \"nx\": 60 to the arguments".into()),
            })?;
        let ny = args.get("ny").and_then(|v| v.as_u64()).map(|n| n as usize)
            .ok_or_else(|| ToolError {
                message: "Missing required parameter 'ny' (integer)".into(),
                param: Some("ny".into()),
                suggestion: Some("Add \"ny\": 30 to the arguments".into()),
            })?;
        let vf = require_f64(args, "volume_fraction")?;
        let load_node = args.get("load_node").and_then(|v| v.as_u64()).map(|n| n as usize)
            .ok_or_else(|| ToolError {
                message: "Missing required parameter 'load_node' (integer)".into(),
                param: Some("load_node".into()),
                suggestion: None,
            })?;
        let load_x = optional_f64(args, "load_x", 0.0);
        let load_y = optional_f64(args, "load_y", -1.0);
        let max_iter = optional_usize(args, "max_iterations", 100);

        if vf <= 0.0 || vf >= 1.0 {
            return Err(ToolError {
                message: format!("volume_fraction must be between 0 and 1, got {vf}"),
                param: Some("volume_fraction".into()),
                suggestion: Some("Try 0.4 (keep 40% of material)".into()),
            });
        }

        let mut problem = physical_topology::TopologyProblem::new_2d(nx, ny, 1.0);
        problem.volume_fraction = vf;
        problem.loads.push(physical_topology::Load {
            node: load_node,
            force: DVec3::new(load_x, load_y, 0.0),
        });
        // Fix left edge nodes
        for iy in 0..=ny {
            problem.supports.push(physical_topology::Support {
                node: iy * (nx + 1),
                fix_x: true,
                fix_y: true,
                fix_z: true,
            });
        }

        let result = physical_topology::optimize(&problem, max_iter, 1.5);

        Ok(ToolResult {
            summary: format!(
                "Topology optimization: {}x{} grid, vf={:.0}%, {} iterations, \
                 final compliance={:.4}.",
                nx, ny, vf * 100.0, result.iterations, result.final_compliance,
            ),
            data: Some(serde_json::json!({
                "nx": nx,
                "ny": ny,
                "volume_fraction": vf,
                "iterations": result.iterations,
                "final_compliance": result.final_compliance,
                "density_field_shape": [nx, ny],
            })),
            solid: None,
            sketch: None,
        })
    }

    // =======================================================================
    // Export tools (additional formats)
    // =======================================================================

    fn tool_export_step(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _protocol = optional_str(args, "protocol", "ap203");
        let filename = optional_str(args, "filename", "part");
        let solid = self.get_solid(&handle)?;

        let content = physical_emit_step::write_step_ap203(solid, &filename);
        let size = content.len();

        Ok(ToolResult {
            summary: format!(
                "Exported '{handle}' to STEP AP203 ({size} bytes) as '{filename}.step'."
            ),
            data: Some(serde_json::json!({
                "format": "step",
                "protocol": "ap203",
                "filename": format!("{filename}.step"),
                "size_bytes": size,
                "content": content,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_3mf(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let filename = optional_str(args, "filename", "part");
        let solid = self.get_solid(&handle)?;

        let mesh = physical_tessellation::tessellate(solid, 0.1);
        let bytes = physical_emit_threemf::write_3mf(&mesh, &filename);
        let size = bytes.len();

        Ok(ToolResult {
            summary: format!(
                "Exported '{handle}' to 3MF ({size} bytes) as '{filename}.3mf'."
            ),
            data: Some(serde_json::json!({
                "format": "3mf",
                "filename": format!("{filename}.3mf"),
                "size_bytes": size,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_gltf(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let filename = optional_str(args, "filename", "part");
        let solid = self.get_solid(&handle)?;

        let mesh = physical_tessellation::tessellate(solid, 0.1);
        let triangles = mesh.triangle_count();
        let bytes = physical_emit_gltf::write_glb(&mesh, &filename);
        let size = bytes.len();

        Ok(ToolResult {
            summary: format!(
                "Exported '{handle}' to GLB ({size} bytes, {triangles} triangles) as '{filename}.glb'."
            ),
            data: Some(serde_json::json!({
                "format": "glb",
                "filename": format!("{filename}.glb"),
                "size_bytes": size,
                "triangle_count": triangles,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_dxf(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let filename = optional_str(args, "filename", "part");
        let solid = self.get_solid(&handle)?;

        let content = physical_emit_dxf::write_dxf_3d(solid);
        let size = content.len();

        Ok(ToolResult {
            summary: format!(
                "Exported '{handle}' to DXF ({size} bytes) as '{filename}.dxf'."
            ),
            data: Some(serde_json::json!({
                "format": "dxf",
                "filename": format!("{filename}.dxf"),
                "size_bytes": size,
                "content": content,
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_pdf(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _sheet_size = optional_str(args, "sheet_size", "a3");
        let _title = optional_str(args, "title", "Part Drawing");

        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "PDF export for '{}' requires Drawing construction from solid views — \
                 use export_svg for immediate output",
                handle
            ),
            param: None,
            suggestion: Some("Use export_svg (same views, web-native format)".into()),
        })
    }

    fn tool_export_svg(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _sheet_size = optional_str(args, "sheet_size", "a3");
        let _title = optional_str(args, "title", "Part Drawing");

        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "SVG export for '{}' requires Drawing construction from solid views — \
                 projection pipeline not yet wired through MCP",
                handle
            ),
            param: None,
            suggestion: Some(
                "Use export_dxf for 2D interchange, or export for STEP/STL".into(),
            ),
        })
    }

    fn tool_export_stl(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let tolerance = optional_f64(args, "tolerance", 0.1);

        let solid = self.get_solid(&handle)?;
        let mesh = physical_tessellation::tessellate(&solid, tolerance);
        let stl_bytes = physical_emit_stl::write_binary_stl(&mesh);
        let tri_count = mesh.triangle_count();

        Ok(ToolResult {
            summary: format!(
                "Exported '{}' as binary STL: {} triangles, {} bytes.",
                handle, tri_count, stl_bytes.len()
            ),
            data: Some(serde_json::json!({
                "triangle_count": tri_count,
                "vertex_count": mesh.vertices.len(),
                "file_size_bytes": stl_bytes.len(),
                "format": "binary_stl",
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_obj(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let name = optional_str(args, "name", "part");

        let solid = self.get_solid(&handle)?;
        let mesh = physical_tessellation::tessellate(&solid, 0.1);
        let obj_text = physical_emit_obj::write_obj(&mesh, &name);

        Ok(ToolResult {
            summary: format!(
                "Exported '{}' as OBJ '{}': {} vertices, {} triangles.",
                handle, name, mesh.vertices.len(), mesh.triangle_count()
            ),
            data: Some(serde_json::json!({
                "vertex_count": mesh.vertices.len(),
                "triangle_count": mesh.triangle_count(),
                "obj_size_bytes": obj_text.len(),
                "format": "wavefront_obj",
            })),
            solid: None,
            sketch: None,
        })
    }

    fn tool_export_iges(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let name = optional_str(args, "name", "Part");

        let solid = self.get_solid(&handle)?;
        let iges_text = physical_emit_iges::write_iges(&solid, &name);

        Ok(ToolResult {
            summary: format!(
                "Exported '{}' as IGES: {} bytes.",
                handle, iges_text.len()
            ),
            data: Some(serde_json::json!({
                "format": "iges_5.3",
                "file_size_bytes": iges_text.len(),
            })),
            solid: None,
            sketch: None,
        })
    }

    // =======================================================================
    // Manufacturing tools
    // =======================================================================

    fn tool_run_dfm_check(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        // Delegates to the same DFM pipeline as check_manufacturability
        self.tool_check_manufacturability(args)
    }

    fn tool_generate_toolpath(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _material = optional_str(args, "material", "6061-T6");
        let _tool_dia = optional_f64(args, "tool_diameter", 6.0);
        let _operation = optional_str(args, "operation", "adaptive_clear");
        let _dialect = optional_str(args, "dialect", "grbl");

        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "Toolpath generation for '{}' requires solid-to-contour extraction — \
                 CNC pipeline not yet wired through MCP",
                handle
            ),
            param: None,
            suggestion: Some(
                "The toolpath crate (physical_mfg_toolpath) is ready — needs contour extraction from solid"
                    .into(),
            ),
        })
    }

    fn tool_slice_for_printing(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let handle = require_str(args, "solid")?;
        let _layer_height = optional_f64(args, "layer_height", 0.2);
        let _nozzle = optional_f64(args, "nozzle_diameter", 0.4);
        let _infill = optional_f64(args, "infill_percent", 20.0);
        let _supports = args
            .get("supports")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let _dialect = optional_str(args, "dialect", "marlin");

        let _solid = self.get_solid(&handle)?;

        Err(ToolError {
            message: format!(
                "Slicer for '{}' requires tessellation + slicing pipeline — \
                 not yet wired through MCP",
                handle
            ),
            param: None,
            suggestion: Some(
                "The slicer crate (physical_mfg_slicer) is ready — needs tessellate-then-slice pipeline"
                    .into(),
            ),
        })
    }

    fn tool_nest_parts(
        &mut self,
        args: &serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        let _parts_json = require_str(args, "parts")?;
        let _sheet_w = require_f64(args, "sheet_width")?;
        let _sheet_h = require_f64(args, "sheet_height")?;
        let _spacing = optional_f64(args, "spacing", 3.0);
        let _allow_rot = args
            .get("allow_rotation")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Err(ToolError {
            message: "Part nesting requires parsing part outlines from JSON — not yet wired".into(),
            param: None,
            suggestion: Some(
                "The nesting algorithm (physical_mfg_laser::nest_part_outlines) is ready — \
                 needs JSON-to-PartOutline conversion"
                    .into(),
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// Profile helper for loft / sweep
// ---------------------------------------------------------------------------

fn make_profile(profile_type: &str, size: f64) -> Result<Profile, ToolError> {
    match profile_type {
        "rectangle" => Ok(Profile::rectangle(size, size)),
        "circle" => Ok(Profile::circle(size)),
        other => Err(ToolError {
            message: format!("Unknown profile type '{other}'"),
            param: Some("profile".into()),
            suggestion: Some("Use 'rectangle' or 'circle'".into()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Manufacturing process / material class parsers
// ---------------------------------------------------------------------------

fn parse_manufacturing_process(
    s: &str,
) -> Result<physical_lut::manufacturing::Process, ToolError> {
    use physical_lut::manufacturing::Process;
    match s {
        "cnc_3axis" | "cnc_mill_3ax" => Ok(Process::CncMill3Ax),
        "cnc_5axis" | "cnc_mill_5ax" => Ok(Process::CncMill5Ax),
        "cnc_turn" => Ok(Process::CncTurn),
        "injection_mold" | "injection" => Ok(Process::InjectionMold),
        "sheet_metal" => Ok(Process::SheetMetal),
        "die_casting" => Ok(Process::DieCasting),
        "fdm" => Ok(Process::Fdm),
        "sla" => Ok(Process::Sla),
        "sls" => Ok(Process::Sls),
        "laser_cut" => Ok(Process::LaserCut),
        other => Err(ToolError {
            message: format!("Unknown manufacturing process '{other}'"),
            param: Some("process".into()),
            suggestion: Some(
                "Use 'cnc_3axis', 'cnc_5axis', 'injection_mold', 'fdm', 'sla', 'sls', 'sheet_metal', 'die_casting'"
                    .into(),
            ),
        }),
    }
}

fn parse_material_class(
    s: &str,
) -> Result<physical_lut::manufacturing::MaterialClass, ToolError> {
    use physical_lut::manufacturing::MaterialClass;
    match s {
        "aluminum" | "aluminium" => Ok(MaterialClass::Aluminum),
        "mild_steel" | "steel" => Ok(MaterialClass::MildSteel),
        "stainless" | "stainless_steel" => Ok(MaterialClass::Stainless),
        "titanium" => Ok(MaterialClass::Titanium),
        "copper" | "brass" | "copper_brass" => Ok(MaterialClass::CopperBrass),
        "plastic" | "polymer" => Ok(MaterialClass::Plastic),
        "cast_iron" => Ok(MaterialClass::CastIron),
        "tool_steel" => Ok(MaterialClass::ToolSteel),
        "nickel" | "nickel_alloy" => Ok(MaterialClass::NickelAlloy),
        other => Err(ToolError {
            message: format!("Unknown material class '{other}'"),
            param: Some("material_class".into()),
            suggestion: Some(
                "Use 'aluminum', 'mild_steel', 'stainless', 'titanium', 'plastic'"
                    .into(),
            ),
        }),
    }
}

// ---------------------------------------------------------------------------
// Argument helpers — produce AI-actionable errors
// ---------------------------------------------------------------------------

fn require_f64(args: &serde_json::Value, name: &str) -> Result<f64, ToolError> {
    args.get(name)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError {
            message: format!("Missing required parameter '{name}' (expected a number)"),
            param: Some(name.into()),
            suggestion: Some(format!("Add \"{name}\": <number> to the arguments")),
        })
}

fn require_str(args: &serde_json::Value, name: &str) -> Result<String, ToolError> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ToolError {
            message: format!("Missing required parameter '{name}' (expected a string)"),
            param: Some(name.into()),
            suggestion: Some(format!("Add \"{name}\": \"value\" to the arguments")),
        })
}

fn optional_f64(args: &serde_json::Value, name: &str, default: f64) -> f64 {
    args.get(name).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn optional_usize(args: &serde_json::Value, name: &str, default: usize) -> usize {
    args.get(name)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default)
}

fn optional_str(args: &serde_json::Value, name: &str, default: &str) -> String {
    args.get(name)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

fn positive(val: f64, name: &str) -> Result<(), ToolError> {
    if val <= 0.0 {
        Err(ToolError {
            message: format!("'{name}' must be positive, got {val}"),
            param: Some(name.into()),
            suggestion: Some(format!("Set {name} to a value greater than 0")),
        })
    } else {
        Ok(())
    }
}

fn parse_axis(s: &str) -> Result<DVec3, ToolError> {
    match s {
        "x" | "X" | "+x" => Ok(DVec3::X),
        "y" | "Y" | "+y" => Ok(DVec3::Y),
        "z" | "Z" | "+z" => Ok(DVec3::Z),
        "-x" => Ok(-DVec3::X),
        "-y" => Ok(-DVec3::Y),
        "-z" => Ok(-DVec3::Z),
        other => Err(ToolError {
            message: format!("Unknown axis '{other}'"),
            param: Some("direction".into()),
            suggestion: Some("Use 'x', 'y', 'z', '-x', '-y', or '-z'".into()),
        }),
    }
}

fn parse_mirror_plane(s: &str) -> Result<(DVec3, DVec3), ToolError> {
    match s {
        "xy" | "XY" => Ok((DVec3::ZERO, DVec3::Z)),
        "xz" | "XZ" => Ok((DVec3::ZERO, DVec3::Y)),
        "yz" | "YZ" => Ok((DVec3::ZERO, DVec3::X)),
        other => Err(ToolError {
            message: format!("Unknown mirror plane '{other}'"),
            param: Some("mirror_plane".into()),
            suggestion: Some("Use 'xy', 'xz', or 'yz'".into()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_box_and_analyze() {
        let mut session = McpSession::new();

        // Create a box
        let result = session.call_tool("create_box", &serde_json::json!({
            "width": 10.0,
            "height": 20.0,
            "depth": 30.0,
        })).unwrap();

        let handle = result.solid.unwrap();
        assert!(result.summary.contains("10×20×30"));

        // Analyze it
        let analysis = session.call_tool("analyze_part", &serde_json::json!({
            "solid": handle,
        })).unwrap();

        let data = analysis.data.unwrap();
        let vol = data["volume_mm3"].as_f64().unwrap();
        assert!((vol - 6000.0).abs() < 100.0, "volume should be ~6000, got {vol}");
    }

    #[test]
    fn boolean_union_via_tools() {
        let mut session = McpSession::new();

        let a = session.call_tool("create_box", &serde_json::json!({
            "width": 10.0, "height": 10.0, "depth": 10.0
        })).unwrap().solid.unwrap();

        let b = session.call_tool("create_box", &serde_json::json!({
            "width": 5.0, "height": 5.0, "depth": 5.0
        })).unwrap().solid.unwrap();

        // Move b out of the way
        session.call_tool("move_solid", &serde_json::json!({
            "solid": b, "x": 20.0
        })).unwrap();

        let result = session.call_tool("combine_solids", &serde_json::json!({
            "solid_a": a,
            "solid_b": b,
            "operation": "union"
        })).unwrap();

        assert!(result.summary.contains("union"));
        let data = result.data.unwrap();
        assert_eq!(data["face_count"], 12);
    }

    #[test]
    fn missing_param_gives_helpful_error() {
        let mut session = McpSession::new();

        let err = session.call_tool("create_box", &serde_json::json!({
            "width": 10.0,
            // missing height and depth
        })).unwrap_err();

        assert!(err.message.contains("height"), "error should mention missing param");
        assert!(err.suggestion.is_some(), "error should suggest a fix");
    }

    #[test]
    fn negative_dimension_gives_helpful_error() {
        let mut session = McpSession::new();

        let err = session.call_tool("create_box", &serde_json::json!({
            "width": -5.0,
            "height": 10.0,
            "depth": 10.0,
        })).unwrap_err();

        assert!(err.message.contains("positive"), "should say must be positive");
        assert_eq!(err.param.as_deref(), Some("width"));
    }

    #[test]
    fn unknown_tool_gives_helpful_error() {
        let mut session = McpSession::new();

        let err = session.call_tool("make_donut", &serde_json::json!({})).unwrap_err();
        assert!(err.message.contains("Unknown tool"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn invalid_handle_lists_available() {
        let mut session = McpSession::new();

        // Create one solid so the suggestion includes it
        session.call_tool("create_box", &serde_json::json!({
            "width": 10.0, "height": 10.0, "depth": 10.0,
        })).unwrap();

        let err = session.call_tool("analyze_part", &serde_json::json!({
            "solid": "nonexistent_handle"
        })).unwrap_err();

        assert!(err.suggestion.unwrap().contains("box_1"));
    }

    #[test]
    fn export_step() {
        let mut session = McpSession::new();

        let handle = session.call_tool("create_box", &serde_json::json!({
            "width": 10.0, "height": 10.0, "depth": 10.0,
        })).unwrap().solid.unwrap();

        let result = session.call_tool("export", &serde_json::json!({
            "solid": handle,
            "format": "step",
        })).unwrap();

        assert!(result.summary.contains("STEP"));
        let data = result.data.unwrap();
        assert!(data["size_bytes"].as_u64().unwrap() > 0);
    }
}
