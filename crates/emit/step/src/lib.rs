//! `physical-emit-step` -- STEP AP203/AP214/AP242 file writer and reader.
//!
//! Supports:
//! - AP203 basic export (planar faces)
//! - AP214 export with per-face colors and curved surface entities
//! - AP242 export with PMI (Product Manufacturing Information) and GD&T annotations
//! - Curved surfaces: cylinder, cone, sphere, torus, NURBS (B_SPLINE_SURFACE_WITH_KNOTS)
//! - Assembly export with NEXT_ASSEMBLY_USAGE_OCCURRENCE (NAUO) links

use glam::DVec3;
use physical_brep::{Solid, Surface, assembly::Placement};

// ---------------------------------------------------------------------------
// Entity ID allocator
// ---------------------------------------------------------------------------

struct IdAlloc(u64);

impl IdAlloc {
    fn new(start: u64) -> Self { Self(start) }
    fn next(&mut self) -> u64 { let id = self.0; self.0 += 1; id }
}

// ---------------------------------------------------------------------------
// Color type for AP214
// ---------------------------------------------------------------------------

/// RGB color (each component 0.0..1.0) for AP214 STYLED_ITEM annotations.
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color {
    pub fn new(r: f64, g: f64, b: f64) -> Self { Self { r, g, b } }
    pub fn silver() -> Self { Self { r: 0.75, g: 0.75, b: 0.75 } }
    pub fn red() -> Self { Self { r: 0.8, g: 0.1, b: 0.1 } }
    pub fn blue() -> Self { Self { r: 0.1, g: 0.3, b: 0.8 } }
}

// ---------------------------------------------------------------------------
// GD&T / PMI types for AP242
// ---------------------------------------------------------------------------

/// GD&T characteristic type per ASME Y14.5
#[derive(Debug, Clone, Copy)]
pub enum GdtCharacteristic {
    Flatness,
    Straightness,
    Circularity,
    Cylindricity,
    Position,
    Concentricity,
    Symmetry,
    Parallelism,
    Perpendicularity,
    Angularity,
    ProfileOfLine,
    ProfileOfSurface,
    CircularRunout,
    TotalRunout,
}

impl GdtCharacteristic {
    /// Returns the STEP entity name for this GD&T characteristic.
    fn step_entity_name(self) -> &'static str {
        match self {
            Self::Flatness => "FLATNESS_TOLERANCE",
            Self::Straightness => "STRAIGHTNESS_TOLERANCE",
            Self::Circularity => "ROUNDNESS_TOLERANCE",
            Self::Cylindricity => "CYLINDRICITY_TOLERANCE",
            Self::Position => "POSITION_TOLERANCE",
            Self::Concentricity => "CONCENTRICITY_TOLERANCE",
            Self::Symmetry => "SYMMETRY_TOLERANCE",
            Self::Parallelism => "PARALLELISM_TOLERANCE",
            Self::Perpendicularity => "PERPENDICULARITY_TOLERANCE",
            Self::Angularity => "ANGULARITY_TOLERANCE",
            Self::ProfileOfLine => "LINE_PROFILE_TOLERANCE",
            Self::ProfileOfSurface => "SURFACE_PROFILE_TOLERANCE",
            Self::CircularRunout => "CIRCULAR_RUNOUT_TOLERANCE",
            Self::TotalRunout => "TOTAL_RUNOUT_TOLERANCE",
        }
    }
}

/// A GD&T annotation attached to a face or edge.
#[derive(Debug, Clone)]
pub struct GdtAnnotation {
    pub characteristic: GdtCharacteristic,
    pub tolerance_value: f64,  // mm
    pub datum_refs: Vec<String>,  // e.g. ["A", "B", "C"]
    pub material_condition: Option<MaterialCondition>,
    pub face_index: usize,
}

/// Material condition modifier for GD&T.
#[derive(Debug, Clone, Copy)]
pub enum MaterialCondition {
    /// Maximum Material Condition (circle-M)
    Mmc,
    /// Least Material Condition (circle-L)
    Lmc,
    /// Regardless of Feature Size (default)
    Rfs,
}

impl MaterialCondition {
    fn step_label(self) -> &'static str {
        match self {
            Self::Mmc => ".MAXIMUM_MATERIAL_CONDITION.",
            Self::Lmc => ".LEAST_MATERIAL_CONDITION.",
            Self::Rfs => ".REGARDLESS_OF_FEATURE_SIZE.",
        }
    }
}

/// Dimension annotation for AP242 PMI.
pub struct DimensionAnnotation {
    pub dim_type: DimensionType,
    pub value: f64,
    pub tolerance_plus: f64,
    pub tolerance_minus: f64,
    pub unit: &'static str,
}

/// The type of dimensional measurement.
pub enum DimensionType {
    Linear,
    Radial,
    Diameter,
    Angular,
}

impl DimensionType {
    fn step_label(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Radial => "radial",
            Self::Diameter => "diameter",
            Self::Angular => "angular",
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers: emit AXIS2_PLACEMENT_3D and DIRECTION
// ---------------------------------------------------------------------------

fn emit_cartesian_point(s: &mut String, ids: &mut IdAlloc, pt: DVec3) -> u64 {
    let id = ids.next();
    s.push_str(&format!(
        "#{}=CARTESIAN_POINT('',({:.6},{:.6},{:.6}));\n",
        id, pt.x, pt.y, pt.z,
    ));
    id
}

fn emit_direction(s: &mut String, ids: &mut IdAlloc, d: DVec3) -> u64 {
    let id = ids.next();
    let n = d.normalize();
    s.push_str(&format!(
        "#{}=DIRECTION('',({:.6},{:.6},{:.6}));\n",
        id, n.x, n.y, n.z,
    ));
    id
}

/// Emit AXIS2_PLACEMENT_3D (location, axis, ref_direction).
fn emit_axis2_placement(
    s: &mut String,
    ids: &mut IdAlloc,
    origin: DVec3,
    axis: DVec3,
) -> u64 {
    let loc_id = emit_cartesian_point(s, ids, origin);
    let axis_id = emit_direction(s, ids, axis);
    // Compute a reference direction perpendicular to axis
    let ref_dir = perpendicular_to(axis);
    let ref_id = emit_direction(s, ids, ref_dir);
    let id = ids.next();
    s.push_str(&format!(
        "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});\n",
        id, loc_id, axis_id, ref_id,
    ));
    id
}

fn perpendicular_to(v: DVec3) -> DVec3 {
    let n = v.normalize();
    let candidate = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    n.cross(candidate).normalize()
}

// ---------------------------------------------------------------------------
// Surface entity emitter
// ---------------------------------------------------------------------------

/// Emit the STEP surface entity for a BRep Surface. Returns the entity ID
/// of the surface, or the edge-loop ID as a fallback for planes (matching
/// existing AP203 behavior).
fn emit_surface_entity(
    s: &mut String,
    ids: &mut IdAlloc,
    surface: &Surface,
) -> u64 {
    match surface {
        Surface::Plane { origin, normal } => {
            let ax_id = emit_axis2_placement(s, ids, *origin, *normal);
            let id = ids.next();
            s.push_str(&format!("#{}=PLANE('',#{});\n", id, ax_id));
            id
        }
        Surface::Cylinder { origin, axis, radius } => {
            let ax_id = emit_axis2_placement(s, ids, *origin, *axis);
            let id = ids.next();
            s.push_str(&format!(
                "#{}=CYLINDRICAL_SURFACE('',#{},{:.6});\n",
                id, ax_id, radius,
            ));
            id
        }
        Surface::Cone { apex, axis, half_angle } => {
            let ax_id = emit_axis2_placement(s, ids, *apex, *axis);
            let id = ids.next();
            // STEP CONICAL_SURFACE uses the base radius (0 at apex) and semi-angle
            s.push_str(&format!(
                "#{}=CONICAL_SURFACE('',#{},0.0,{:.6});\n",
                id, ax_id, half_angle,
            ));
            id
        }
        Surface::Sphere { center, radius } => {
            let ax_id = emit_axis2_placement(s, ids, *center, DVec3::Z);
            let id = ids.next();
            s.push_str(&format!(
                "#{}=SPHERICAL_SURFACE('',#{},{:.6});\n",
                id, ax_id, radius,
            ));
            id
        }
        Surface::Torus { center, axis, major_radius, minor_radius } => {
            let ax_id = emit_axis2_placement(s, ids, *center, *axis);
            let id = ids.next();
            s.push_str(&format!(
                "#{}=TOROIDAL_SURFACE('',#{},{:.6},{:.6});\n",
                id, ax_id, major_radius, minor_radius,
            ));
            id
        }
        Surface::Nurbs {
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u,
            degree_v,
        } => {
            emit_bspline_surface(
                s, ids,
                control_points, weights,
                knots_u, knots_v,
                *degree_u, *degree_v,
            )
        }
    }
}

/// Emit a B_SPLINE_SURFACE_WITH_KNOTS entity.
fn emit_bspline_surface(
    s: &mut String,
    ids: &mut IdAlloc,
    control_points: &[Vec<DVec3>],
    weights: &[Vec<f64>],
    knots_u: &[f64],
    knots_v: &[f64],
    degree_u: usize,
    degree_v: usize,
) -> u64 {
    let rows = control_points.len();
    let cols = if rows > 0 { control_points[0].len() } else { 0 };

    // Emit control point entities
    let mut cp_ids: Vec<Vec<u64>> = Vec::with_capacity(rows);
    for row in control_points {
        let mut row_ids = Vec::with_capacity(cols);
        for &pt in row {
            row_ids.push(emit_cartesian_point(s, ids, pt));
        }
        cp_ids.push(row_ids);
    }

    // Format the control-point grid as nested parenthesized lists
    let cp_grid: String = cp_ids
        .iter()
        .map(|row| {
            let inner: Vec<String> = row.iter().map(|id| format!("#{id}")).collect();
            format!("({})", inner.join(","))
        })
        .collect::<Vec<_>>()
        .join(",");

    // Compute knot multiplicities from the raw knot vectors
    let (u_knot_vals, u_mults) = compress_knots(knots_u);
    let (v_knot_vals, v_mults) = compress_knots(knots_v);

    let fmt_f64_list = |vals: &[f64]| -> String {
        vals.iter().map(|v| format!("{v:.6}")).collect::<Vec<_>>().join(",")
    };
    let fmt_int_list = |vals: &[usize]| -> String {
        vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
    };

    // Format weights grid
    let w_grid: String = weights
        .iter()
        .map(|row| {
            let inner: Vec<String> = row.iter().map(|w| format!("{w:.6}")).collect();
            format!("({})", inner.join(","))
        })
        .collect::<Vec<_>>()
        .join(",");

    let id = ids.next();
    s.push_str(&format!(
        "#{}=B_SPLINE_SURFACE_WITH_KNOTS('',{},{},({}),.UNSPECIFIED.,.F.,.F.,.F.,\
         ({}),({}),({},{}),\
         .UNSPECIFIED.);\n",
        id,
        degree_u,
        degree_v,
        cp_grid,
        fmt_int_list(&u_mults),
        fmt_int_list(&v_mults),
        fmt_f64_list(&u_knot_vals),
        fmt_f64_list(&v_knot_vals),
    ));

    // If any weight is not 1.0, also emit the RATIONAL form
    let has_rational = weights.iter().any(|row| row.iter().any(|w| (*w - 1.0).abs() > 1e-12));
    if has_rational {
        let rat_id = ids.next();
        s.push_str(&format!(
            "#{}=RATIONAL_B_SPLINE_SURFACE(#{},{},{},({}),.UNSPECIFIED.,.F.,.F.,.F.,\
             ({}),({}),({},{}),\
             .UNSPECIFIED.,({}));\n",
            rat_id,
            id,
            degree_u,
            degree_v,
            cp_grid,
            fmt_int_list(&u_mults),
            fmt_int_list(&v_mults),
            fmt_f64_list(&u_knot_vals),
            fmt_f64_list(&v_knot_vals),
            w_grid,
        ));
        return rat_id;
    }

    id
}

/// Compress a knot vector into (unique_values, multiplicities).
fn compress_knots(knots: &[f64]) -> (Vec<f64>, Vec<usize>) {
    if knots.is_empty() {
        return (vec![], vec![]);
    }
    let mut vals = vec![knots[0]];
    let mut mults = vec![1usize];
    for &k in &knots[1..] {
        if (k - *vals.last().unwrap()).abs() < 1e-14 {
            *mults.last_mut().unwrap() += 1;
        } else {
            vals.push(k);
            mults.push(1);
        }
    }
    (vals, mults)
}

// ---------------------------------------------------------------------------
// Shared face-loop emitter
// ---------------------------------------------------------------------------

/// Emit faces of a solid, returning the list of ADVANCED_FACE entity IDs.
/// When `emit_surfaces` is true, proper STEP surface entities are emitted;
/// otherwise the edge-loop ID is used as the surface ref (AP203 compat).
fn emit_faces(
    s: &mut String,
    ids: &mut IdAlloc,
    solid: &Solid,
    point_map: &std::collections::HashMap<physical_brep::VertexId, u64>,
    emit_surfaces: bool,
) -> Vec<u64> {
    let mut face_entity_ids = Vec::new();

    for fid in solid.face_ids() {
        let face_verts = solid.face_vertices(fid);
        let face_id = ids.next();
        face_entity_ids.push(face_id);

        // Emit a FACE_OUTER_BOUND referencing an EDGE_LOOP
        let bound_id = ids.next();
        let loop_id = ids.next();

        // Encode vertex point references in the edge loop
        let vert_refs: Vec<String> = face_verts.iter().map(|v| {
            let mut best_id = 0u64;
            let mut best_dist = f64::MAX;
            for (&vid, &pid) in point_map {
                let d = (solid.vertices[vid].point - *v).length();
                if d < best_dist {
                    best_dist = d;
                    best_id = pid;
                }
            }
            format!("#{best_id}")
        }).collect();

        s.push_str(&format!(
            "#{}=EDGE_LOOP('', ({}) );\n",
            loop_id,
            vert_refs.join(","),
        ));
        s.push_str(&format!(
            "#{}=FACE_OUTER_BOUND('',#{}, .T.);\n",
            bound_id, loop_id,
        ));

        // Emit the surface entity
        let surface_ref = if emit_surfaces {
            let face = &solid.faces[fid];
            emit_surface_entity(s, ids, &face.surface)
        } else {
            loop_id // AP203 compat fallback
        };

        s.push_str(&format!(
            "#{}=ADVANCED_FACE('',(#{}),#{},.T.);\n",
            face_id, bound_id, surface_ref,
        ));
    }

    face_entity_ids
}

/// Emit vertices as CARTESIAN_POINT entities. Returns the point map.
fn emit_vertices(
    s: &mut String,
    ids: &mut IdAlloc,
    solid: &Solid,
) -> std::collections::HashMap<physical_brep::VertexId, u64> {
    let mut point_map = std::collections::HashMap::new();
    let vertex_ids: Vec<_> = solid.vertices.keys().collect();
    for &vid in &vertex_ids {
        let pt = solid.vertices[vid].point;
        let id = emit_cartesian_point(s, ids, pt);
        point_map.insert(vid, id);
    }
    point_map
}

// ---------------------------------------------------------------------------
// AP203 writer (original, preserved)
// ---------------------------------------------------------------------------

/// Write a B-Rep solid as a STEP AP203 string.
///
/// `name` is used as the product identifier in the STEP header.
/// Returns the complete STEP file content as a `String`.
pub fn write_step_ap203(solid: &Solid, name: &str) -> String {
    let faces = solid.face_count();
    let edges = solid.edge_count();
    let verts = solid.vertex_count();
    let (min, max) = solid.bounding_box();

    let mut s = String::with_capacity(4096);

    s.push_str("ISO-10303-21;\n");
    s.push_str("HEADER;\n");
    s.push_str("FILE_DESCRIPTION(('Physical CAD export'),'2;1');\n");
    s.push_str(&format!(
        "FILE_NAME('{name}.step','2025-01-01',('Physical'),('OpenIE'),'physical-emit-step','Physical CAD','');\n"
    ));
    s.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\n");
    s.push_str("ENDSEC;\n");
    s.push_str("DATA;\n");

    // Application context
    s.push_str("#1=APPLICATION_CONTEXT('automotive design');\n");
    s.push_str("#2=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2001,#1);\n");
    s.push_str(&format!("#3=PRODUCT('{name}','{name}','',(#4));\n"));
    s.push_str("#4=PRODUCT_CONTEXT('',#1,'mechanical');\n");
    s.push_str("#5=PRODUCT_DEFINITION_FORMATION('','',#3);\n");
    s.push_str("#6=PRODUCT_DEFINITION('design','',#5,#7);\n");
    s.push_str("#7=PRODUCT_DEFINITION_CONTEXT('part definition',#1,'design');\n");

    let mut ids = IdAlloc::new(10);

    // Emit MANIFOLD_SOLID_BREP referencing a closed shell
    let msb_id = ids.next();
    let shell_id = ids.next();

    // Emit vertices as CARTESIAN_POINT
    let vertex_ids: Vec<_> = solid.vertices.keys().collect();
    let mut point_map = std::collections::HashMap::new();

    for &vid in &vertex_ids {
        let pt = solid.vertices[vid].point;
        s.push_str(&format!(
            "#{}=CARTESIAN_POINT('',({:.6},{:.6},{:.6}));\n",
            ids.0, pt.x, pt.y, pt.z
        ));
        point_map.insert(vid, ids.0);
        ids.0 += 1;
    }

    // Emit ADVANCED_FACE entities for each face with vertex references
    // We encode face vertex loops so the reader can reconstruct topology
    let mut face_entity_ids = Vec::new();
    for fid in solid.face_ids() {
        let face_verts = solid.face_vertices(fid);
        let face_id = ids.next();
        face_entity_ids.push(face_id);

        // Emit a FACE_OUTER_BOUND referencing an EDGE_LOOP
        let bound_id = ids.next();
        let loop_id = ids.next();

        // Encode vertex point references in the edge loop
        let vert_refs: Vec<String> = face_verts.iter().map(|v| {
            // Find the closest point entity
            let mut best_id = 0u64;
            let mut best_dist = f64::MAX;
            for (&vid, &pid) in &point_map {
                let d = (solid.vertices[vid].point - *v).length();
                if d < best_dist {
                    best_dist = d;
                    best_id = pid;
                }
            }
            format!("#{best_id}")
        }).collect();

        s.push_str(&format!(
            "#{}=EDGE_LOOP('', ({}) );\n",
            loop_id,
            vert_refs.join(",")
        ));
        s.push_str(&format!(
            "#{}=FACE_OUTER_BOUND('',#{}, .T.);\n",
            bound_id, loop_id
        ));
        s.push_str(&format!(
            "#{}=ADVANCED_FACE('',(#{}),#{},.T.);\n",
            face_id, bound_id, loop_id
        ));
    }

    // Now emit the shell and MSB
    let face_refs: Vec<String> = face_entity_ids.iter().map(|id| format!("#{id}")).collect();
    s.push_str(&format!(
        "#{}=CLOSED_SHELL('', ({}) );\n",
        shell_id,
        face_refs.join(",")
    ));
    s.push_str(&format!(
        "#{}=MANIFOLD_SOLID_BREP('{}',#{});\n",
        msb_id, name, shell_id
    ));

    // Summary comment
    s.push_str(&format!(
        "/* B-Rep: {} faces, {} edges, {} vertices, bbox ({:.2},{:.2},{:.2})-({:.2},{:.2},{:.2}) */\n",
        faces, edges, verts,
        min.x, min.y, min.z,
        max.x, max.y, max.z,
    ));

    s.push_str("ENDSEC;\n");
    s.push_str("END-ISO-10303-21;\n");

    s
}

// ---------------------------------------------------------------------------
// AP214 writer — adds colors, curved surfaces, PRODUCT_DEFINITION_FORMATION
// ---------------------------------------------------------------------------

/// Per-face color assignment for AP214 export.
pub struct FaceColor {
    /// Face index (0-based, in iteration order of `solid.face_ids()`).
    pub face_index: usize,
    /// RGB color.
    pub color: Color,
}

/// Write a B-Rep solid as a STEP AP214 string with color and surface support.
///
/// `name` is the product identifier. `face_colors` assigns optional per-face
/// colors (faces not listed get no styled item). Pass an empty slice for no
/// color annotations.
pub fn write_step_ap214(
    solid: &Solid,
    name: &str,
    face_colors: &[FaceColor],
) -> String {
    let faces_count = solid.face_count();
    let edges_count = solid.edge_count();
    let verts_count = solid.vertex_count();
    let (bb_min, bb_max) = solid.bounding_box();

    let mut s = String::with_capacity(8192);

    // --- Header ---
    s.push_str("ISO-10303-21;\n");
    s.push_str("HEADER;\n");
    s.push_str("FILE_DESCRIPTION(('Physical CAD AP214 export'),'2;1');\n");
    s.push_str(&format!(
        "FILE_NAME('{name}.step','2025-01-01',('Physical'),('OpenIE'),'physical-emit-step','Physical CAD','');\n"
    ));
    s.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\n");
    s.push_str("ENDSEC;\n");
    s.push_str("DATA;\n");

    // --- Application context ---
    s.push_str("#1=APPLICATION_CONTEXT('automotive design');\n");
    s.push_str("#2=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2003,#1);\n");
    s.push_str(&format!("#3=PRODUCT('{name}','{name}','',(#4));\n"));
    s.push_str("#4=PRODUCT_CONTEXT('',#1,'mechanical');\n");
    s.push_str("#5=PRODUCT_DEFINITION_FORMATION('1','version 1',#3);\n");
    s.push_str("#6=PRODUCT_DEFINITION('design','',#5,#7);\n");
    s.push_str("#7=PRODUCT_DEFINITION_CONTEXT('part definition',#1,'design');\n");

    // SHAPE_DEFINITION_REPRESENTATION linking product to shape
    s.push_str("#8=PRODUCT_DEFINITION_SHAPE('','',#6);\n");
    let mut ids = IdAlloc::new(20);

    // --- Geometry ---
    let msb_id = ids.next();
    let shell_id = ids.next();

    let point_map = emit_vertices(&mut s, &mut ids, solid);
    let face_entity_ids = emit_faces(&mut s, &mut ids, solid, &point_map, true);

    // Shell and MSB
    let face_refs: Vec<String> = face_entity_ids.iter().map(|id| format!("#{id}")).collect();
    s.push_str(&format!(
        "#{}=CLOSED_SHELL('', ({}) );\n",
        shell_id,
        face_refs.join(","),
    ));
    s.push_str(&format!(
        "#{}=MANIFOLD_SOLID_BREP('{}',#{});\n",
        msb_id, name, shell_id,
    ));

    // SHAPE_REPRESENTATION
    let shape_rep_id = ids.next();
    s.push_str(&format!(
        "#{}=SHAPE_REPRESENTATION('{}',(#{}),#9);\n",
        shape_rep_id, name, msb_id,
    ));
    // geometric context (unit length, right-handed)
    s.push_str("#9=( GEOMETRIC_REPRESENTATION_CONTEXT(3) \
                GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#9a)) \
                GLOBAL_UNIT_ASSIGNED_CONTEXT((#9b,#9c,#9d)) \
                REPRESENTATION_CONTEXT('','3D') );\n");
    s.push_str("#9a=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#9b,'distance_accuracy_value','');\n");
    s.push_str("#9b=( CONVERSION_BASED_UNIT('MILLIMETRE',#9e) LENGTH_UNIT() NAMED_UNIT(#9f) );\n");
    s.push_str("#9c=( NAMED_UNIT(#9g) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) );\n");
    s.push_str("#9d=( NAMED_UNIT(#9h) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() );\n");
    s.push_str("#9e=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0),#9i);\n");
    s.push_str("#9f=DIMENSIONAL_EXPONENTS(1.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9g=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9h=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9i=( LENGTH_UNIT() NAMED_UNIT(#9f) SI_UNIT(.MILLI.,.METRE.) );\n");

    // SHAPE_DEFINITION_REPRESENTATION
    let sdr_id = ids.next();
    s.push_str(&format!(
        "#{}=SHAPE_DEFINITION_REPRESENTATION(#8,#{});\n",
        sdr_id, shape_rep_id,
    ));

    // --- Per-face colors (AP214 STYLED_ITEM) ---
    for fc in face_colors {
        if fc.face_index < face_entity_ids.len() {
            let target_face_id = face_entity_ids[fc.face_index];
            emit_styled_item(&mut s, &mut ids, target_face_id, fc.color);
        }
    }

    // Summary comment
    s.push_str(&format!(
        "/* AP214 B-Rep: {} faces, {} edges, {} vertices, bbox ({:.2},{:.2},{:.2})-({:.2},{:.2},{:.2}) */\n",
        faces_count, edges_count, verts_count,
        bb_min.x, bb_min.y, bb_min.z,
        bb_max.x, bb_max.y, bb_max.z,
    ));

    s.push_str("ENDSEC;\n");
    s.push_str("END-ISO-10303-21;\n");

    s
}

/// Emit a STYLED_ITEM with COLOR_RGB for a given face entity.
fn emit_styled_item(s: &mut String, ids: &mut IdAlloc, face_entity: u64, color: Color) {
    let rgb_id = ids.next();
    s.push_str(&format!(
        "#{}=COLOUR_RGB('',{:.4},{:.4},{:.4});\n",
        rgb_id, color.r, color.g, color.b,
    ));

    let fill_area_id = ids.next();
    s.push_str(&format!(
        "#{}=FILL_AREA_STYLE_COLOUR('',#{});\n",
        fill_area_id, rgb_id,
    ));

    let fill_style_id = ids.next();
    s.push_str(&format!(
        "#{}=FILL_AREA_STYLE('',(#{}));\n",
        fill_style_id, fill_area_id,
    ));

    let surf_style_fill_id = ids.next();
    s.push_str(&format!(
        "#{}=SURFACE_STYLE_FILL_AREA(#{});\n",
        surf_style_fill_id, fill_style_id,
    ));

    let surf_side_id = ids.next();
    s.push_str(&format!(
        "#{}=SURFACE_SIDE_STYLE('',(#{}));\n",
        surf_side_id, surf_style_fill_id,
    ));

    let surf_style_id = ids.next();
    s.push_str(&format!(
        "#{}=SURFACE_STYLE_USAGE(.BOTH.,#{});\n",
        surf_style_id, surf_side_id,
    ));

    let psa_id = ids.next();
    s.push_str(&format!(
        "#{}=PRESENTATION_STYLE_ASSIGNMENT((#{}));\n",
        psa_id, surf_style_id,
    ));

    let si_id = ids.next();
    s.push_str(&format!(
        "#{}=STYLED_ITEM('',(#{}),#{});\n",
        si_id, psa_id, face_entity,
    ));
}

// ---------------------------------------------------------------------------
// Assembly writer — NAUO-based structure
// ---------------------------------------------------------------------------

/// A part entry for assembly export.
pub struct AssemblyEntry<'a> {
    /// The solid geometry.
    pub solid: &'a Solid,
    /// Part name / label.
    pub name: &'a str,
    /// Placement in assembly coordinates.
    pub placement: Placement,
}

/// Write a STEP assembly file with NEXT_ASSEMBLY_USAGE_OCCURRENCE (NAUO) links.
///
/// The assembly is modelled as a root product with children linked via
/// NAUO + CONTEXT_DEPENDENT_SHAPE_REPRESENTATION for placement.
pub fn write_step_assembly(assembly_name: &str, parts: &[AssemblyEntry<'_>]) -> String {
    let mut s = String::with_capacity(16384);

    // --- Header ---
    s.push_str("ISO-10303-21;\n");
    s.push_str("HEADER;\n");
    s.push_str("FILE_DESCRIPTION(('Physical CAD assembly export'),'2;1');\n");
    s.push_str(&format!(
        "FILE_NAME('{assembly_name}.step','2025-01-01',('Physical'),('OpenIE'),'physical-emit-step','Physical CAD','');\n"
    ));
    s.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\n");
    s.push_str("ENDSEC;\n");
    s.push_str("DATA;\n");

    // --- Application context (shared) ---
    s.push_str("#1=APPLICATION_CONTEXT('automotive design');\n");
    s.push_str("#2=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2003,#1);\n");
    s.push_str("#4=PRODUCT_CONTEXT('',#1,'mechanical');\n");
    s.push_str("#7=PRODUCT_DEFINITION_CONTEXT('part definition',#1,'design');\n");

    let mut ids = IdAlloc::new(100);

    // --- Root assembly product ---
    let root_prod_id = ids.next();
    s.push_str(&format!(
        "#{}=PRODUCT('{assembly_name}','{assembly_name}','',(#4));\n",
        root_prod_id,
    ));
    let root_pdf_id = ids.next();
    s.push_str(&format!(
        "#{}=PRODUCT_DEFINITION_FORMATION('1','',#{});\n",
        root_pdf_id, root_prod_id,
    ));
    let root_pd_id = ids.next();
    s.push_str(&format!(
        "#{}=PRODUCT_DEFINITION('design','',#{},#7);\n",
        root_pd_id, root_pdf_id,
    ));
    let root_pds_id = ids.next();
    s.push_str(&format!(
        "#{}=PRODUCT_DEFINITION_SHAPE('','',#{});\n",
        root_pds_id, root_pd_id,
    ));

    // --- Child parts ---
    struct ChildInfo {
        pd_id: u64,
        shape_rep_id: u64,
    }
    let mut children: Vec<ChildInfo> = Vec::with_capacity(parts.len());

    for entry in parts {
        // Product
        let prod_id = ids.next();
        s.push_str(&format!(
            "#{}=PRODUCT('{}','{}','',(#4));\n",
            prod_id, entry.name, entry.name,
        ));
        let pdf_id = ids.next();
        s.push_str(&format!(
            "#{}=PRODUCT_DEFINITION_FORMATION('1','',#{});\n",
            pdf_id, prod_id,
        ));
        let pd_id = ids.next();
        s.push_str(&format!(
            "#{}=PRODUCT_DEFINITION('design','',#{},#7);\n",
            pd_id, pdf_id,
        ));
        let pds_id = ids.next();
        s.push_str(&format!(
            "#{}=PRODUCT_DEFINITION_SHAPE('','',#{});\n",
            pds_id, pd_id,
        ));

        // Geometry
        let msb_id = ids.next();
        let shell_id = ids.next();

        let point_map = emit_vertices(&mut s, &mut ids, entry.solid);
        let face_entity_ids = emit_faces(&mut s, &mut ids, entry.solid, &point_map, true);

        let face_refs: Vec<String> = face_entity_ids.iter().map(|id| format!("#{id}")).collect();
        s.push_str(&format!(
            "#{}=CLOSED_SHELL('', ({}) );\n",
            shell_id,
            face_refs.join(","),
        ));
        s.push_str(&format!(
            "#{}=MANIFOLD_SOLID_BREP('{}',#{});\n",
            msb_id, entry.name, shell_id,
        ));

        // SHAPE_REPRESENTATION for this part
        let shape_rep_id = ids.next();
        s.push_str(&format!(
            "#{}=SHAPE_REPRESENTATION('{}',(#{}),#98);\n",
            shape_rep_id, entry.name, msb_id,
        ));

        // SDR
        let sdr_id = ids.next();
        s.push_str(&format!(
            "#{}=SHAPE_DEFINITION_REPRESENTATION(#{},#{});\n",
            sdr_id, pds_id, shape_rep_id,
        ));

        children.push(ChildInfo { pd_id, shape_rep_id });
    }

    // --- NAUO links + CONTEXT_DEPENDENT_SHAPE_REPRESENTATION ---
    for (i, (entry, child)) in parts.iter().zip(children.iter()).enumerate() {
        let nauo_id = ids.next();
        s.push_str(&format!(
            "#{}=NEXT_ASSEMBLY_USAGE_OCCURRENCE('{}','{}','',#{},#{});\n",
            nauo_id,
            format!("link_{i}"),
            entry.name,
            root_pd_id,
            child.pd_id,
        ));

        // Product definition shape for the NAUO
        let nauo_pds_id = ids.next();
        s.push_str(&format!(
            "#{}=PRODUCT_DEFINITION_SHAPE('','',#{});\n",
            nauo_pds_id, nauo_id,
        ));

        // Emit placement transform
        let pos = entry.placement.position;
        let rot_mat = entry.placement.rotation_matrix();
        let axis = DVec3::new(rot_mat.z_axis.x, rot_mat.z_axis.y, rot_mat.z_axis.z);
        let ref_dir = DVec3::new(rot_mat.x_axis.x, rot_mat.x_axis.y, rot_mat.x_axis.z);

        let loc_id = emit_cartesian_point(&mut s, &mut ids, pos);
        let axis_dir_id = emit_direction(&mut s, &mut ids, axis);
        let ref_dir_id = emit_direction(&mut s, &mut ids, ref_dir);
        let a2p_id = ids.next();
        s.push_str(&format!(
            "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});\n",
            a2p_id, loc_id, axis_dir_id, ref_dir_id,
        ));

        // ITEM_DEFINED_TRANSFORMATION
        let idt_id = ids.next();
        // Identity placement for source
        let src_a2p_id = emit_axis2_placement(&mut s, &mut ids, DVec3::ZERO, DVec3::Z);
        s.push_str(&format!(
            "#{}=ITEM_DEFINED_TRANSFORMATION('','',#{},#{});\n",
            idt_id, src_a2p_id, a2p_id,
        ));

        // REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION
        // Root shape rep (we'll use a simple placeholder)
        let rrwt_id = ids.next();
        s.push_str(&format!(
            "#{}=( REPRESENTATION_RELATIONSHIP('','',#{},#{}) \
             REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{}) \
             SHAPE_REPRESENTATION_RELATIONSHIP() );\n",
            rrwt_id, child.shape_rep_id, child.shape_rep_id, idt_id,
        ));

        // CONTEXT_DEPENDENT_SHAPE_REPRESENTATION
        let cdsr_id = ids.next();
        s.push_str(&format!(
            "#{}=CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{},#{});\n",
            cdsr_id, rrwt_id, nauo_pds_id,
        ));
    }

    // Geometric context (shared, using entity #98)
    s.push_str("#98=( GEOMETRIC_REPRESENTATION_CONTEXT(3) \
                GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#98a)) \
                GLOBAL_UNIT_ASSIGNED_CONTEXT((#98b,#98c,#98d)) \
                REPRESENTATION_CONTEXT('','3D') );\n");
    s.push_str("#98a=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#98b,'distance_accuracy_value','');\n");
    s.push_str("#98b=( CONVERSION_BASED_UNIT('MILLIMETRE',#98e) LENGTH_UNIT() NAMED_UNIT(#98f) );\n");
    s.push_str("#98c=( NAMED_UNIT(#98g) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) );\n");
    s.push_str("#98d=( NAMED_UNIT(#98h) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() );\n");
    s.push_str("#98e=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0),#98i);\n");
    s.push_str("#98f=DIMENSIONAL_EXPONENTS(1.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#98g=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#98h=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#98i=( LENGTH_UNIT() NAMED_UNIT(#98f) SI_UNIT(.MILLI.,.METRE.) );\n");

    s.push_str(&format!(
        "/* Assembly '{}': {} parts */\n",
        assembly_name,
        parts.len(),
    ));

    s.push_str("ENDSEC;\n");
    s.push_str("END-ISO-10303-21;\n");

    s
}

// ---------------------------------------------------------------------------
// AP242 writer — PMI / GD&T annotations
// ---------------------------------------------------------------------------

/// Write a B-Rep solid as a STEP AP242 string with GD&T annotations.
///
/// `name` is the product identifier. `annotations` contains GD&T callouts
/// linked to face indices. Pass an empty slice for no annotations (the output
/// is still valid AP242).
pub fn write_step_ap242(
    solid: &Solid,
    name: &str,
    annotations: &[GdtAnnotation],
) -> String {
    write_step_ap242_with_dims(solid, name, annotations, &[])
}

/// Write a B-Rep solid as a STEP AP242 string with GD&T annotations and
/// dimensional annotations.
pub fn write_step_ap242_with_dims(
    solid: &Solid,
    name: &str,
    annotations: &[GdtAnnotation],
    dimensions: &[DimensionAnnotation],
) -> String {
    let faces_count = solid.face_count();
    let edges_count = solid.edge_count();
    let verts_count = solid.vertex_count();
    let (bb_min, bb_max) = solid.bounding_box();

    let mut s = String::with_capacity(16384);

    // --- Header ---
    s.push_str("ISO-10303-21;\n");
    s.push_str("HEADER;\n");
    s.push_str("FILE_DESCRIPTION(('Physical CAD AP242 export'),'2;1');\n");
    s.push_str(&format!(
        "FILE_NAME('{name}.step','2025-01-01',('Physical'),('OpenIE'),'physical-emit-step','Physical CAD','');\n"
    ));
    s.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 442 1 1 4 }'));\n");
    s.push_str("ENDSEC;\n");
    s.push_str("DATA;\n");

    // --- Application context ---
    s.push_str("#1=APPLICATION_CONTEXT('automotive design');\n");
    s.push_str("#2=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2011,#1);\n");
    s.push_str(&format!("#3=PRODUCT('{name}','{name}','',(#4));\n"));
    s.push_str("#4=PRODUCT_CONTEXT('',#1,'mechanical');\n");
    s.push_str("#5=PRODUCT_DEFINITION_FORMATION('1','version 1',#3);\n");
    s.push_str("#6=PRODUCT_DEFINITION('design','',#5,#7);\n");
    s.push_str("#7=PRODUCT_DEFINITION_CONTEXT('part definition',#1,'design');\n");

    // SHAPE_DEFINITION_REPRESENTATION linking product to shape
    s.push_str("#8=PRODUCT_DEFINITION_SHAPE('','',#6);\n");
    let mut ids = IdAlloc::new(20);

    // --- Geometry ---
    let msb_id = ids.next();
    let shell_id = ids.next();

    let point_map = emit_vertices(&mut s, &mut ids, solid);
    let face_entity_ids = emit_faces(&mut s, &mut ids, solid, &point_map, true);

    // Shell and MSB
    let face_refs: Vec<String> = face_entity_ids.iter().map(|id| format!("#{id}")).collect();
    s.push_str(&format!(
        "#{}=CLOSED_SHELL('', ({}) );\n",
        shell_id,
        face_refs.join(","),
    ));
    s.push_str(&format!(
        "#{}=MANIFOLD_SOLID_BREP('{}',#{});\n",
        msb_id, name, shell_id,
    ));

    // SHAPE_REPRESENTATION
    let shape_rep_id = ids.next();
    s.push_str(&format!(
        "#{}=SHAPE_REPRESENTATION('{}',(#{}),#9);\n",
        shape_rep_id, name, msb_id,
    ));

    // Geometric context (unit length, right-handed)
    s.push_str("#9=( GEOMETRIC_REPRESENTATION_CONTEXT(3) \
                GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#9a)) \
                GLOBAL_UNIT_ASSIGNED_CONTEXT((#9b,#9c,#9d)) \
                REPRESENTATION_CONTEXT('','3D') );\n");
    s.push_str("#9a=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#9b,'distance_accuracy_value','');\n");
    s.push_str("#9b=( CONVERSION_BASED_UNIT('MILLIMETRE',#9e) LENGTH_UNIT() NAMED_UNIT(#9f) );\n");
    s.push_str("#9c=( NAMED_UNIT(#9g) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) );\n");
    s.push_str("#9d=( NAMED_UNIT(#9h) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() );\n");
    s.push_str("#9e=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0),#9i);\n");
    s.push_str("#9f=DIMENSIONAL_EXPONENTS(1.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9g=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9h=DIMENSIONAL_EXPONENTS(0.0,0.0,0.0,0.0,0.0,0.0,0.0);\n");
    s.push_str("#9i=( LENGTH_UNIT() NAMED_UNIT(#9f) SI_UNIT(.MILLI.,.METRE.) );\n");

    // SHAPE_DEFINITION_REPRESENTATION
    let sdr_id = ids.next();
    s.push_str(&format!(
        "#{}=SHAPE_DEFINITION_REPRESENTATION(#8,#{});\n",
        sdr_id, shape_rep_id,
    ));

    // --- GD&T Annotations (AP242 PMI) ---
    for ann in annotations {
        if ann.face_index >= face_entity_ids.len() {
            continue;
        }
        let target_face_id = face_entity_ids[ann.face_index];

        // SHAPE_ASPECT referencing the product definition shape and target face
        let shape_aspect_id = ids.next();
        s.push_str(&format!(
            "#{}=SHAPE_ASPECT('gdt_target','',#8,.F.);\n",
            shape_aspect_id,
        ));

        // Link SHAPE_ASPECT to the face via SHAPE_ASPECT_RELATIONSHIP
        let sar_id = ids.next();
        s.push_str(&format!(
            "#{}=SHAPE_ASPECT_RELATIONSHIP('','',#{},#{});\n",
            sar_id, shape_aspect_id, target_face_id,
        ));

        // Emit DATUM and DATUM_REFERENCE entities for datum refs
        let mut datum_ref_ids: Vec<u64> = Vec::new();
        for (idx, datum_label) in ann.datum_refs.iter().enumerate() {
            let datum_id = ids.next();
            s.push_str(&format!(
                "#{}=DATUM('{}','{}',#8);\n",
                datum_id, datum_label, datum_label,
            ));

            let datum_ref_id = ids.next();
            // precedence_order is 1-based index
            s.push_str(&format!(
                "#{}=DATUM_REFERENCE(#{},{});\n",
                datum_ref_id, datum_id, idx + 1,
            ));
            datum_ref_ids.push(datum_ref_id);
        }

        // TOLERANCE_ZONE
        let tz_id = ids.next();
        s.push_str(&format!(
            "#{}=TOLERANCE_ZONE('',LENGTH_MEASURE({:.6}),#{});\n",
            tz_id, ann.tolerance_value, shape_aspect_id,
        ));

        // Always emit the characteristic-specific tolerance entity
        let entity_name = ann.characteristic.step_entity_name();
        let gt_id = ids.next();
        s.push_str(&format!(
            "#{}={}('','{:.6}',#{},#{});\n",
            gt_id,
            entity_name,
            ann.tolerance_value,
            tz_id,
            shape_aspect_id,
        ));

        // If there are datum references, emit GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE
        if !datum_ref_ids.is_empty() {
            let datum_refs_str: Vec<String> =
                datum_ref_ids.iter().map(|id| format!("#{id}")).collect();
            let gtdr_id = ids.next();
            s.push_str(&format!(
                "#{}=GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE(#{},\
                 ({}));\n",
                gtdr_id,
                gt_id,
                datum_refs_str.join(","),
            ));
        }

        // Material condition modifier (if present)
        if let Some(mc) = ann.material_condition {
            let mcm_id = ids.next();
            s.push_str(&format!(
                "#{}=GEOMETRIC_TOLERANCE_WITH_MODIFIERS(#{},{});\n",
                mcm_id, gt_id, mc.step_label(),
            ));
        }
    }

    // --- Dimensional annotations ---
    for dim in dimensions {
        let shape_aspect_id = ids.next();
        s.push_str(&format!(
            "#{}=SHAPE_ASPECT('dimension','',#8,.F.);\n",
            shape_aspect_id,
        ));

        // DIMENSIONAL_SIZE or DIMENSIONAL_LOCATION
        let ds_id = ids.next();
        s.push_str(&format!(
            "#{}=DIMENSIONAL_SIZE(#{},'{}');\n",
            ds_id, shape_aspect_id, dim.dim_type.step_label(),
        ));

        // Value with tolerance
        let mv_id = ids.next();
        s.push_str(&format!(
            "#{}=MEASURE_WITH_UNIT(LENGTH_MEASURE({:.6}),#9b);\n",
            mv_id, dim.value,
        ));

        // Plus/minus tolerance
        if dim.tolerance_plus.abs() > 1e-15 || dim.tolerance_minus.abs() > 1e-15 {
            let plus_id = ids.next();
            s.push_str(&format!(
                "#{}=MEASURE_WITH_UNIT(LENGTH_MEASURE({:.6}),#9b);\n",
                plus_id, dim.tolerance_plus,
            ));
            let minus_id = ids.next();
            s.push_str(&format!(
                "#{}=MEASURE_WITH_UNIT(LENGTH_MEASURE({:.6}),#9b);\n",
                minus_id, dim.tolerance_minus,
            ));
            let tol_id = ids.next();
            s.push_str(&format!(
                "#{}=PLUS_MINUS_TOLERANCE(#{},#{},#{});\n",
                tol_id, mv_id, plus_id, minus_id,
            ));
        }
    }

    // Summary comment
    s.push_str(&format!(
        "/* AP242 B-Rep: {} faces, {} edges, {} vertices, {} GD&T, {} dims, \
         bbox ({:.2},{:.2},{:.2})-({:.2},{:.2},{:.2}) */\n",
        faces_count, edges_count, verts_count,
        annotations.len(), dimensions.len(),
        bb_min.x, bb_min.y, bb_min.z,
        bb_max.x, bb_max.y, bb_max.z,
    ));

    s.push_str("ENDSEC;\n");
    s.push_str("END-ISO-10303-21;\n");

    s
}

// ---------------------------------------------------------------------------
// Reader (supports AP203, AP214, AP242)
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Per-face color extracted from AP214 STYLED_ITEM / COLOUR_RGB.
#[derive(Clone, Copy, Debug)]
pub struct FaceColorInfo {
    /// Face index (0-based, in order parsed).
    pub face_index: usize,
    /// RGB color.
    pub color: Color,
}

/// Read a STEP file (AP203, AP214, or AP242) and reconstruct a `Solid`.
///
/// This is a convenience wrapper around [`read_step_solid`] that discards the
/// part name.
pub fn read_step(text: &str) -> Option<Solid> {
    read_step_solid(text).map(|(solid, _name)| solid)
}

/// Read a STEP file and reconstruct a [`Solid`] together with the part name
/// extracted from `PRODUCT_DEFINITION`.
///
/// The parser builds an entity index (`#ID -> entity text`) and walks the
/// ADVANCED_FACE -> FACE_OUTER_BOUND -> EDGE_LOOP topology. For each face it
/// resolves the face-geometry reference to determine the proper [`Surface`]
/// variant (plane, cylinder, sphere, cone, torus, or NURBS).
///
/// Returns `None` when the input is not a recognisable STEP file or contains
/// no geometry.
pub fn read_step_solid(text: &str) -> Option<(Solid, String)> {
    // Basic validation
    if !text.contains("ISO-10303-21") {
        return None;
    }
    if !text.contains("DATA;") || !text.contains("ENDSEC;") {
        return None;
    }

    // ------------------------------------------------------------------
    // 1. Build entity map: #ID -> entity body text
    // ------------------------------------------------------------------
    let entities = build_entity_map(text);
    if entities.is_empty() {
        return None;
    }

    // ------------------------------------------------------------------
    // 2. Parse primitive entity types into typed maps
    // ------------------------------------------------------------------
    let mut points: HashMap<u64, DVec3> = HashMap::new();
    let mut directions: HashMap<u64, DVec3> = HashMap::new();
    let mut axis_placements: HashMap<u64, (DVec3, DVec3, DVec3)> = HashMap::new(); // origin, axis, ref_dir
    let mut colours: HashMap<u64, Color> = HashMap::new();

    for (&id, body) in &entities {
        let upper = entity_type_name(body);
        match upper.as_str() {
            "CARTESIAN_POINT" => {
                if let Some(pt) = parse_cartesian_point(body) {
                    points.insert(id, pt);
                }
            }
            "DIRECTION" => {
                if let Some(d) = parse_direction_entity(body) {
                    directions.insert(id, d);
                }
            }
            "COLOUR_RGB" => {
                if let Some(c) = parse_colour_rgb(body) {
                    colours.insert(id, c);
                }
            }
            _ => {}
        }
    }

    // Resolve AXIS2_PLACEMENT_3D (needs points + directions already parsed)
    for (&id, body) in &entities {
        if entity_type_name(body) == "AXIS2_PLACEMENT_3D" {
            if let Some(placement) = parse_axis2_placement_3d(body, &points, &directions) {
                axis_placements.insert(id, placement);
            }
        }
    }

    // ------------------------------------------------------------------
    // 3. Build surface map: entity_id -> Surface
    // ------------------------------------------------------------------
    let mut surfaces: HashMap<u64, Surface> = HashMap::new();
    for (&id, body) in &entities {
        let tn = entity_type_name(body);
        match tn.as_str() {
            "PLANE" => {
                if let Some(surf) = parse_plane_surface(body, &axis_placements) {
                    surfaces.insert(id, surf);
                }
            }
            "CYLINDRICAL_SURFACE" => {
                if let Some(surf) = parse_cylindrical_surface(body, &axis_placements) {
                    surfaces.insert(id, surf);
                }
            }
            "SPHERICAL_SURFACE" => {
                if let Some(surf) = parse_spherical_surface(body, &axis_placements) {
                    surfaces.insert(id, surf);
                }
            }
            "CONICAL_SURFACE" => {
                if let Some(surf) = parse_conical_surface(body, &axis_placements) {
                    surfaces.insert(id, surf);
                }
            }
            "TOROIDAL_SURFACE" => {
                if let Some(surf) = parse_toroidal_surface(body, &axis_placements) {
                    surfaces.insert(id, surf);
                }
            }
            "B_SPLINE_SURFACE_WITH_KNOTS" => {
                if let Some(surf) = parse_bspline_surface(body, &points) {
                    surfaces.insert(id, surf);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // 4. Extract part name from PRODUCT_DEFINITION (first one wins)
    // ------------------------------------------------------------------
    let part_name = extract_product_name(&entities);

    // ------------------------------------------------------------------
    // 5. Extract STYLED_ITEM -> face colour mapping (AP214)
    // ------------------------------------------------------------------
    let styled_colours = extract_styled_colours(&entities, &colours);

    // ------------------------------------------------------------------
    // 6. Walk ADVANCED_FACE topology
    // ------------------------------------------------------------------

    // FACE_OUTER_BOUND -> edge-loop ref
    let mut face_bounds: HashMap<u64, u64> = HashMap::new();
    for (&id, body) in &entities {
        if entity_type_name(body) == "FACE_OUTER_BOUND" {
            if let Some(refs) = parse_entity_refs(body) {
                if let Some(&loop_ref) = refs.first() {
                    face_bounds.insert(id, loop_ref);
                }
            }
        }
    }

    // EDGE_LOOP -> list of point refs
    let mut edge_loops: HashMap<u64, Vec<u64>> = HashMap::new();
    for (&id, body) in &entities {
        if entity_type_name(body) == "EDGE_LOOP" {
            if let Some(refs) = parse_entity_refs(body) {
                edge_loops.insert(id, refs);
            }
        }
    }

    // Collect ADVANCED_FACE entries: (bound_ref, surface_ref)
    // The refs list for ADVANCED_FACE is: first = FACE_OUTER_BOUND, last = surface
    struct FaceInfo {
        point_refs: Vec<u64>,
        surface_id: Option<u64>,
    }

    let mut face_infos: Vec<FaceInfo> = Vec::new();
    for body in entities.values() {
        if entity_type_name(body) == "ADVANCED_FACE" {
            if let Some(refs) = parse_entity_refs(body) {
                let bound_id = refs.first().copied();
                // The last ref is the surface geometry reference
                let surface_id = if refs.len() >= 2 { Some(refs[refs.len() - 1]) } else { None };

                if let Some(bid) = bound_id {
                    if let Some(&loop_id) = face_bounds.get(&bid) {
                        if let Some(pt_refs) = edge_loops.get(&loop_id) {
                            face_infos.push(FaceInfo {
                                point_refs: pt_refs.clone(),
                                surface_id,
                            });
                        }
                    }
                }
            }
        }
    }

    if points.is_empty() {
        return None;
    }

    // If we found face topology, reconstruct with actual faces
    if !face_infos.is_empty() {
        let mut solid = Solid::new();

        // Create vertices for each unique point
        let mut point_to_vertex: HashMap<u64, physical_brep::VertexId> = HashMap::new();
        for (&pid, &pt) in &points {
            let vid = solid.add_vertex(pt);
            point_to_vertex.insert(pid, vid);
        }

        // Create faces
        for (_face_idx, fi) in face_infos.iter().enumerate() {
            let vertex_ids: Vec<physical_brep::VertexId> = fi
                .point_refs
                .iter()
                .filter_map(|pid| point_to_vertex.get(pid).copied())
                .collect();

            if vertex_ids.len() >= 3 {
                // Try to resolve the surface from the entity map
                let surface = if let Some(sid) = fi.surface_id {
                    surfaces.get(&sid).cloned()
                } else {
                    None
                };

                let surface = surface.unwrap_or_else(|| {
                    // Fallback: compute planar surface from vertices
                    let p0 = solid.vertices[vertex_ids[0]].point;
                    let p1 = solid.vertices[vertex_ids[1]].point;
                    let p2 = solid.vertices[vertex_ids[2]].point;
                    let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
                    Surface::plane(p0, normal)
                });

                solid.add_face_from_vertices(surface, &vertex_ids, true);
            }
        }

        solid.link_twins();
        let _ = styled_colours; // available for future per-face colour application
        return Some((solid, part_name));
    }

    // Fallback: reconstruct from bounding box of all points
    let all_pts: Vec<DVec3> = points.values().copied().collect();
    let mut min = all_pts[0];
    let mut max = all_pts[0];
    for p in &all_pts {
        min = min.min(*p);
        max = max.max(*p);
    }

    let size = max - min;
    if size.x < 1e-10 && size.y < 1e-10 && size.z < 1e-10 {
        return None;
    }

    Some((physical_brep::builder::make_box(size.x, size.y, size.z), part_name))
}

// ---------------------------------------------------------------------------
// Entity map builder
// ---------------------------------------------------------------------------

/// Split STEP DATA section into entity lines and build `#ID -> body` map.
///
/// Handles multi-line entities by joining continuation lines.
fn build_entity_map(text: &str) -> HashMap<u64, String> {
    let mut map = HashMap::new();

    // Extract the DATA section
    let data_start = match text.find("DATA;") {
        Some(pos) => pos + 5,
        None => return map,
    };
    let data_end = match text[data_start..].find("ENDSEC;") {
        Some(pos) => data_start + pos,
        None => return map,
    };
    let data_section = &text[data_start..data_end];

    // Join continuation lines and split on `;\n` boundaries
    let mut current = String::new();
    for line in data_section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("/*") {
            continue;
        }
        current.push_str(trimmed);
        if trimmed.ends_with(';') {
            // Complete entity line
            let entity_line = current.trim_end_matches(';').to_string();
            if let Some(id) = parse_entity_id_from_str(&entity_line) {
                if let Some(eq_pos) = entity_line.find('=') {
                    map.insert(id, entity_line[eq_pos + 1..].to_string());
                }
            }
            current.clear();
        }
    }

    map
}

/// Extract entity type name from the body text (everything before the first `(`).
fn entity_type_name(body: &str) -> String {
    let trimmed = body.trim();
    if let Some(paren) = trimmed.find('(') {
        trimmed[..paren].trim().to_uppercase()
    } else {
        trimmed.to_uppercase()
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse entity ID from a string like "#42=ENTITY_NAME(...);"
fn parse_entity_id_from_str(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if !trimmed.starts_with('#') { return None; }
    let eq_pos = trimmed.find('=')?;
    trimmed[1..eq_pos].parse().ok()
}

/// Parse a CARTESIAN_POINT body and extract (x, y, z).
fn parse_cartesian_point(body: &str) -> Option<DVec3> {
    parse_inner_floats(body, 3).map(|v| DVec3::new(v[0], v[1], v[2]))
}

/// Parse a DIRECTION body and extract the direction vector.
fn parse_direction_entity(body: &str) -> Option<DVec3> {
    parse_inner_floats(body, 3).map(|v| DVec3::new(v[0], v[1], v[2]))
}

/// Parse COLOUR_RGB body: COLOUR_RGB('',r,g,b)
fn parse_colour_rgb(body: &str) -> Option<Color> {
    // Extract floats after the name string
    let paren_start = body.find('(')?;
    let paren_end = body.rfind(')')?;
    let inner = &body[paren_start + 1..paren_end];
    // Skip the name string (first element)
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() >= 4 {
        let r: f64 = parts[1].trim().parse().ok()?;
        let g: f64 = parts[2].trim().parse().ok()?;
        let b: f64 = parts[3].trim().parse().ok()?;
        Some(Color::new(r, g, b))
    } else {
        None
    }
}

/// Parse AXIS2_PLACEMENT_3D body: AXIS2_PLACEMENT_3D('',#loc,#axis,#ref_dir)
fn parse_axis2_placement_3d(
    body: &str,
    points: &HashMap<u64, DVec3>,
    directions: &HashMap<u64, DVec3>,
) -> Option<(DVec3, DVec3, DVec3)> {
    let refs = parse_entity_refs_from_body(body)?;
    if refs.len() < 3 { return None; }
    let origin = *points.get(&refs[0])?;
    let axis = *directions.get(&refs[1])?;
    let ref_dir = *directions.get(&refs[2])?;
    Some((origin, axis.normalize(), ref_dir.normalize()))
}

/// Parse PLANE('',#axis2_placement) -> Surface::Plane
fn parse_plane_surface(
    body: &str,
    placements: &HashMap<u64, (DVec3, DVec3, DVec3)>,
) -> Option<Surface> {
    let refs = parse_entity_refs_from_body(body)?;
    let &(origin, axis, _ref_dir) = placements.get(refs.first()?)?;
    Some(Surface::Plane { origin, normal: axis })
}

/// Parse CYLINDRICAL_SURFACE('',#axis2,radius) -> Surface::Cylinder
fn parse_cylindrical_surface(
    body: &str,
    placements: &HashMap<u64, (DVec3, DVec3, DVec3)>,
) -> Option<Surface> {
    let refs = parse_entity_refs_from_body(body)?;
    let &(origin, axis, _) = placements.get(refs.first()?)?;
    let radius = parse_trailing_float(body)?;
    Some(Surface::Cylinder { origin, axis, radius })
}

/// Parse SPHERICAL_SURFACE('',#axis2,radius) -> Surface::Sphere
fn parse_spherical_surface(
    body: &str,
    placements: &HashMap<u64, (DVec3, DVec3, DVec3)>,
) -> Option<Surface> {
    let refs = parse_entity_refs_from_body(body)?;
    let &(center, _, _) = placements.get(refs.first()?)?;
    let radius = parse_trailing_float(body)?;
    Some(Surface::Sphere { center, radius })
}

/// Parse CONICAL_SURFACE('',#axis2,base_radius,half_angle) -> Surface::Cone
fn parse_conical_surface(
    body: &str,
    placements: &HashMap<u64, (DVec3, DVec3, DVec3)>,
) -> Option<Surface> {
    let refs = parse_entity_refs_from_body(body)?;
    let &(apex, axis, _) = placements.get(refs.first()?)?;
    let floats = parse_all_trailing_floats(body);
    // CONICAL_SURFACE has base_radius and half_angle as trailing floats
    let half_angle = if floats.len() >= 2 { floats[1] } else { return None; };
    Some(Surface::Cone { apex, axis, half_angle })
}

/// Parse TOROIDAL_SURFACE('',#axis2,major_r,minor_r) -> Surface::Torus
fn parse_toroidal_surface(
    body: &str,
    placements: &HashMap<u64, (DVec3, DVec3, DVec3)>,
) -> Option<Surface> {
    let refs = parse_entity_refs_from_body(body)?;
    let &(center, axis, _) = placements.get(refs.first()?)?;
    let floats = parse_all_trailing_floats(body);
    if floats.len() < 2 { return None; }
    Some(Surface::Torus {
        center,
        axis,
        major_radius: floats[0],
        minor_radius: floats[1],
    })
}

/// Parse B_SPLINE_SURFACE_WITH_KNOTS -> Surface::Nurbs
///
/// Format: B_SPLINE_SURFACE_WITH_KNOTS('',deg_u,deg_v,(cp_grid),
///   .UNSPECIFIED.,.F.,.F.,.F.,(u_mults),(v_mults),(u_knots,v_knots),.UNSPECIFIED.)
fn parse_bspline_surface(
    body: &str,
    points: &HashMap<u64, DVec3>,
) -> Option<Surface> {
    let paren_start = body.find('(')?;
    let inner = &body[paren_start + 1..];

    // Parse degrees: first two integer tokens after the name string
    let tokens = tokenize_step_args(inner);
    // tokens[0] = name string (''), tokens[1] = degree_u, tokens[2] = degree_v,
    // tokens[3] = control-point grid, ...
    if tokens.len() < 4 { return None; }

    let degree_u: usize = tokens[1].trim().parse().ok()?;
    let degree_v: usize = tokens[2].trim().parse().ok()?;

    // Parse control point grid (nested parenthesized lists of #refs)
    let cp_grid_str = &tokens[3];
    let control_points = parse_cp_grid(cp_grid_str, points)?;

    let rows = control_points.len();
    let cols = if rows > 0 { control_points[0].len() } else { 0 };

    // Find the knot multiplicity and knot value lists
    // After the cp grid and the 4 enum/bool flags, we have (u_mults),(v_mults),(u_knots,v_knots)
    // We look for them by index in the token list
    // tokens[4..7] are .UNSPECIFIED.,.F.,.F.,.F.
    // tokens[8] = u_mults, tokens[9] = v_mults
    // tokens[10] contains both u_knots and v_knots
    let u_mults_idx = 8;
    let v_mults_idx = 9;
    let knots_idx = 10;

    if tokens.len() <= knots_idx { return None; }

    let u_mults: Vec<usize> = parse_int_list(&tokens[u_mults_idx]);
    let v_mults: Vec<usize> = parse_int_list(&tokens[v_mults_idx]);

    // The knots token may contain both u and v knot vectors separated by comma
    // between the closing and opening parens of the two vectors.
    let knot_floats = parse_two_float_lists(&tokens[knots_idx]);
    let (knots_u_unique, knots_v_unique) = if let Some((ku, kv)) = knot_floats {
        (ku, kv)
    } else {
        return None;
    };

    // Expand knots from (unique, multiplicity) pairs
    let knots_u = expand_knots(&knots_u_unique, &u_mults);
    let knots_v = expand_knots(&knots_v_unique, &v_mults);

    // Build uniform weights (all 1.0)
    let weights = vec![vec![1.0; cols]; rows];

    Some(Surface::Nurbs {
        control_points,
        weights,
        knots_u,
        knots_v,
        degree_u,
        degree_v,
    })
}

/// Extract part name from the first PRODUCT_DEFINITION entity.
fn extract_product_name(entities: &HashMap<u64, String>) -> String {
    // Look for PRODUCT('name', ...) — the name is the first quoted argument
    for body in entities.values() {
        if entity_type_name(body) == "PRODUCT" {
            if let Some(name) = extract_first_quoted_string(body) {
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }
    String::from("Unnamed")
}

/// Extract per-face colour assignments from STYLED_ITEM entities.
fn extract_styled_colours(
    entities: &HashMap<u64, String>,
    colours: &HashMap<u64, Color>,
) -> Vec<(u64, Color)> {
    // STYLED_ITEM('', (#psa), #face_entity)
    // Walk STYLED_ITEM -> PSA -> SURFACE_STYLE_USAGE -> ... -> COLOUR_RGB
    let mut result = Vec::new();

    for body in entities.values() {
        if entity_type_name(body) != "STYLED_ITEM" { continue; }
        let refs = match parse_entity_refs_from_body(body) {
            Some(r) => r,
            None => continue,
        };
        if refs.len() < 2 { continue; }
        let face_ref = refs[refs.len() - 1];
        // Walk the chain to find a COLOUR_RGB
        // PSA -> SURFACE_STYLE_USAGE -> SURFACE_SIDE_STYLE -> SURFACE_STYLE_FILL_AREA
        //     -> FILL_AREA_STYLE -> FILL_AREA_STYLE_COLOUR -> COLOUR_RGB
        let psa_ref = refs[0];
        if let Some(color) = walk_style_chain(entities, colours, psa_ref) {
            result.push((face_ref, color));
        }
    }

    result
}

/// Walk the AP214 style chain from a PSA ref down to a COLOUR_RGB.
fn walk_style_chain(
    entities: &HashMap<u64, String>,
    colours: &HashMap<u64, Color>,
    start_id: u64,
) -> Option<Color> {
    // Follow up to 8 hops of entity refs looking for a COLOUR_RGB
    let mut current = start_id;
    for _ in 0..8 {
        if let Some(c) = colours.get(&current) {
            return Some(*c);
        }
        if let Some(body) = entities.get(&current) {
            if let Some(refs) = parse_entity_refs_from_body(body) {
                if let Some(&next) = refs.first() {
                    current = next;
                    continue;
                }
            }
        }
        break;
    }
    None
}

// ---------------------------------------------------------------------------
// Low-level parsing utilities
// ---------------------------------------------------------------------------

/// Parse entity references from a body string (after the `=`).
fn parse_entity_refs(line: &str) -> Option<Vec<u64>> {
    parse_entity_refs_from_body(line)
}

/// Parse #N references from entity body text.
fn parse_entity_refs_from_body(body: &str) -> Option<Vec<u64>> {
    let refs: Vec<u64> = body
        .split(|c: char| !c.is_ascii_digit() && c != '#')
        .filter_map(|s| {
            let s = s.trim();
            if s.starts_with('#') {
                s[1..].parse().ok()
            } else {
                None
            }
        })
        .collect();

    if refs.is_empty() { None } else { Some(refs) }
}

/// Extract floats from the innermost parenthesized group.
fn parse_inner_floats(body: &str, expected: usize) -> Option<Vec<f64>> {
    // Find the inner parenthesized coordinates: ENTITY('', (x,y,z))
    let paren_start = body.find('(')?;
    let after = &body[paren_start..];
    let inner_start = after[1..].find('(')?;
    let inner = &after[inner_start + 2..];
    let end = inner.find(')')?;
    let coords: Vec<f64> = inner[..end]
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if coords.len() == expected { Some(coords) } else { None }
}

/// Parse the last float value from a body string.
/// Used for entities like CYLINDRICAL_SURFACE('',#ref,radius)
fn parse_trailing_float(body: &str) -> Option<f64> {
    let floats = parse_all_trailing_floats(body);
    floats.last().copied()
}

/// Parse all trailing float values (after the last #ref) from a body string.
fn parse_all_trailing_floats(body: &str) -> Vec<f64> {
    let paren_start = match body.find('(') {
        Some(p) => p,
        None => return vec![],
    };
    let paren_end = match body.rfind(')') {
        Some(p) => p,
        None => return vec![],
    };
    let inner = &body[paren_start + 1..paren_end];

    // Split by comma, skip name and #refs, collect floats
    let mut floats = Vec::new();
    for part in inner.split(',') {
        let trimmed = part.trim();
        if trimmed.starts_with('#') || trimmed.starts_with('\'') || trimmed.starts_with('(')
            || trimmed.starts_with('.') || trimmed.ends_with(')')
        {
            continue;
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            floats.push(f);
        }
    }
    floats
}

/// Extract the first single-quoted string from entity body.
fn extract_first_quoted_string(body: &str) -> Option<String> {
    let start = body.find('\'')?;
    let end = body[start + 1..].find('\'')?;
    Some(body[start + 1..start + 1 + end].to_string())
}

/// Tokenize STEP arguments at the top level (respecting nested parentheses).
fn tokenize_step_args(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                if depth > 0 {
                    depth -= 1;
                    current.push(ch);
                }
                // depth == 0 means we hit the closing paren of the outer entity
            }
            ',' if depth == 0 => {
                tokens.push(std::mem::take(&mut current));
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Parse a control-point grid string like `((#10,#11),(#12,#13))` into
/// `Vec<Vec<DVec3>>`.
fn parse_cp_grid(grid_str: &str, points: &HashMap<u64, DVec3>) -> Option<Vec<Vec<DVec3>>> {
    // Remove outer parens
    let inner = grid_str.trim();
    let inner = inner.strip_prefix('(')?.strip_suffix(')')?;

    let mut rows = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in inner.chars() {
        match ch {
            '(' => {
                depth += 1;
                if depth > 1 { current.push(ch); }
            }
            ')' => {
                depth -= 1;
                if depth > 0 {
                    current.push(ch);
                } else {
                    // End of a row
                    let row: Vec<DVec3> = current
                        .split(',')
                        .filter_map(|s| {
                            let s = s.trim().strip_prefix('#')?;
                            let id: u64 = s.parse().ok()?;
                            points.get(&id).copied()
                        })
                        .collect();
                    if !row.is_empty() {
                        rows.push(row);
                    }
                    current.clear();
                }
            }
            ',' if depth == 0 => {
                // separator between row groups, skip
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if rows.is_empty() { None } else { Some(rows) }
}

/// Parse a parenthesized integer list like `(3,1,3)`.
fn parse_int_list(s: &str) -> Vec<usize> {
    let inner = s.trim().trim_start_matches('(').trim_end_matches(')');
    inner
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect()
}

/// Parse two float lists from a combined token like `(0.0,0.5,1.0),(0.0,1.0)`.
/// This handles the format used in B_SPLINE_SURFACE_WITH_KNOTS for knot vectors.
fn parse_two_float_lists(s: &str) -> Option<(Vec<f64>, Vec<f64>)> {
    let trimmed = s.trim();
    // Find the boundary between the two lists: `),(`
    if let Some(split_pos) = trimmed.find("),(") {
        let first = &trimmed[..split_pos + 1]; // includes closing )
        let second = &trimmed[split_pos + 2..]; // starts with (
        let a = parse_float_list(first);
        let b = parse_float_list(second);
        if !a.is_empty() && !b.is_empty() {
            return Some((a, b));
        }
    }
    // If only one list, try splitting floats evenly (fallback)
    let all = parse_float_list(trimmed);
    if all.len() >= 2 {
        let mid = all.len() / 2;
        return Some((all[..mid].to_vec(), all[mid..].to_vec()));
    }
    None
}

/// Parse a parenthesized float list like `(0.0,0.5,1.0)`.
fn parse_float_list(s: &str) -> Vec<f64> {
    let inner = s.trim().trim_start_matches('(').trim_end_matches(')');
    inner
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect()
}

/// Expand compressed knots (unique values + multiplicities) into a full knot vector.
fn expand_knots(values: &[f64], multiplicities: &[usize]) -> Vec<f64> {
    let mut result = Vec::new();
    for (val, &mult) in values.iter().zip(multiplicities.iter()) {
        for _ in 0..mult {
            result.push(*val);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- STEP reader unit tests ---

    #[test]
    fn parse_cartesian_point_test() {
        let body = "CARTESIAN_POINT('',(1.500000,2.300000,-4.100000))";
        let pt = parse_cartesian_point(body).unwrap();
        assert!((pt.x - 1.5).abs() < 1e-9);
        assert!((pt.y - 2.3).abs() < 1e-9);
        assert!((pt.z - (-4.1)).abs() < 1e-9);
    }

    #[test]
    fn parse_direction_test() {
        let body = "DIRECTION('',(0.000000,0.000000,1.000000))";
        let d = parse_direction_entity(body).unwrap();
        assert!((d.x).abs() < 1e-9);
        assert!((d.y).abs() < 1e-9);
        assert!((d.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_axis2_placement_test() {
        let mut points = HashMap::new();
        let mut dirs = HashMap::new();
        points.insert(10, DVec3::new(1.0, 2.0, 3.0));
        dirs.insert(11, DVec3::new(0.0, 0.0, 1.0));
        dirs.insert(12, DVec3::new(1.0, 0.0, 0.0));

        let body = "AXIS2_PLACEMENT_3D('',#10,#11,#12)";
        let (origin, axis, ref_dir) = parse_axis2_placement_3d(body, &points, &dirs).unwrap();
        assert!((origin - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
        assert!((axis - DVec3::Z).length() < 1e-9);
        assert!((ref_dir - DVec3::X).length() < 1e-9);
    }

    #[test]
    fn parse_cylindrical_surface_test() {
        let mut placements = HashMap::new();
        placements.insert(20, (DVec3::ZERO, DVec3::Z, DVec3::X));

        let body = "CYLINDRICAL_SURFACE('',#20,5.000000)";
        let surf = parse_cylindrical_surface(body, &placements).unwrap();
        match surf {
            Surface::Cylinder { origin, axis, radius } => {
                assert!((origin - DVec3::ZERO).length() < 1e-9);
                assert!((axis - DVec3::Z).length() < 1e-9);
                assert!((radius - 5.0).abs() < 1e-9);
            }
            _ => panic!("expected Cylinder surface"),
        }
    }

    #[test]
    fn parse_spherical_surface_test() {
        let mut placements = HashMap::new();
        placements.insert(30, (DVec3::new(1.0, 2.0, 3.0), DVec3::Z, DVec3::X));

        let body = "SPHERICAL_SURFACE('',#30,7.500000)";
        let surf = parse_spherical_surface(body, &placements).unwrap();
        match surf {
            Surface::Sphere { center, radius } => {
                assert!((center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
                assert!((radius - 7.5).abs() < 1e-9);
            }
            _ => panic!("expected Sphere surface"),
        }
    }

    #[test]
    fn read_step_solid_box() {
        // Minimal STEP text for a box with 6 planar faces
        let step = "\
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('test'),'2;1');
FILE_NAME('test.step','2025-01-01',('T'),('T'),'test','test','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.0,0.0,0.0));
#2=CARTESIAN_POINT('',(10.0,0.0,0.0));
#3=CARTESIAN_POINT('',(10.0,10.0,0.0));
#4=CARTESIAN_POINT('',(0.0,10.0,0.0));
#5=CARTESIAN_POINT('',(0.0,0.0,10.0));
#6=CARTESIAN_POINT('',(10.0,0.0,10.0));
#7=CARTESIAN_POINT('',(10.0,10.0,10.0));
#8=CARTESIAN_POINT('',(0.0,10.0,10.0));
#9=DIRECTION('',(0.0,0.0,-1.0));
#10=DIRECTION('',(1.0,0.0,0.0));
#11=AXIS2_PLACEMENT_3D('',#1,#9,#10);
#12=PLANE('',#11);
#20=EDGE_LOOP('',(#1,#2,#3,#4));
#21=FACE_OUTER_BOUND('',#20,.T.);
#22=ADVANCED_FACE('',(#21),#12,.T.);
#30=EDGE_LOOP('',(#5,#8,#7,#6));
#31=FACE_OUTER_BOUND('',#30,.T.);
#32=ADVANCED_FACE('',(#31),#12,.T.);
#40=EDGE_LOOP('',(#1,#5,#6,#2));
#41=FACE_OUTER_BOUND('',#40,.T.);
#42=ADVANCED_FACE('',(#41),#12,.T.);
#50=EDGE_LOOP('',(#2,#6,#7,#3));
#51=FACE_OUTER_BOUND('',#50,.T.);
#52=ADVANCED_FACE('',(#51),#12,.T.);
#60=EDGE_LOOP('',(#3,#7,#8,#4));
#61=FACE_OUTER_BOUND('',#60,.T.);
#62=ADVANCED_FACE('',(#61),#12,.T.);
#70=EDGE_LOOP('',(#4,#8,#5,#1));
#71=FACE_OUTER_BOUND('',#70,.T.);
#72=ADVANCED_FACE('',(#71),#12,.T.);
#80=PRODUCT('TestBox','TestBox','',(#81));
#81=PRODUCT_CONTEXT('',#82,'mechanical');
#82=APPLICATION_CONTEXT('automotive design');
ENDSEC;
END-ISO-10303-21;
";
        let (solid, name) = read_step_solid(step).unwrap();
        assert_eq!(solid.face_count(), 6, "box should have 6 faces");
        assert_eq!(name, "TestBox");
    }

    #[test]
    fn read_step_preserves_vertex_count() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let step = write_step_ap203(&solid, "Verts");
        let (reimported, _name) = read_step_solid(&step).unwrap();
        // The reimported solid should have at least the original 8 vertices
        assert!(
            reimported.vertex_count() >= 8,
            "expected >= 8 vertices, got {}",
            reimported.vertex_count()
        );
    }

    #[test]
    fn read_step_returns_none_for_empty() {
        assert!(read_step_solid("").is_none());
        assert!(read_step_solid("not a step file").is_none());
        assert!(read_step_solid("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;").is_none());
    }

    // --- Existing AP203 tests (preserved) ---

    #[test]
    fn roundtrip_l_shape() {
        let profile = physical_brep::Profile::l_shape(20.0, 30.0, 5.0);
        let solid = physical_brep::extrude::extrude_z(&profile, 15.0);
        let step = write_step_ap203(&solid, "L");
        let reimported = read_step(&step).unwrap();
        assert_eq!(reimported.face_count(), solid.face_count());
        assert_eq!(reimported.vertex_count(), solid.vertex_count());
    }

    #[test]
    fn roundtrip_box() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let step = write_step_ap203(&solid, "Box");
        let reimported = read_step(&step).unwrap();
        assert_eq!(reimported.face_count(), 6);
        assert_eq!(reimported.vertex_count(), 8);
    }

    // --- AP214 tests ---

    #[test]
    fn ap214_basic_box() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let step = write_step_ap214(&solid, "Box214", &[]);
        assert!(step.contains("ISO-10303-21"));
        assert!(step.contains("AP214"));
        assert!(step.contains("PRODUCT_DEFINITION_FORMATION('1','version 1'"));
        assert!(step.contains("SHAPE_REPRESENTATION"));
        assert!(step.contains("SHAPE_DEFINITION_REPRESENTATION"));
        assert!(step.contains("PLANE("));
    }

    #[test]
    fn ap214_with_colors() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let colors = vec![
            FaceColor { face_index: 0, color: Color::red() },
            FaceColor { face_index: 2, color: Color::blue() },
        ];
        let step = write_step_ap214(&solid, "ColorBox", &colors);
        assert!(step.contains("COLOUR_RGB"));
        assert!(step.contains("STYLED_ITEM"));
        assert!(step.contains("SURFACE_STYLE_USAGE"));
        // Should have 2 STYLED_ITEM entries
        let styled_count = step.matches("STYLED_ITEM").count();
        assert_eq!(styled_count, 2, "expected 2 STYLED_ITEMs, got {styled_count}");
    }

    // --- Curved surface tests ---

    #[test]
    fn ap214_cylinder_surface() {
        let solid = physical_brep::make_cylinder(5.0, 20.0, 32);
        let step = write_step_ap214(&solid, "Cyl", &[]);
        assert!(step.contains("CYLINDRICAL_SURFACE"));
        // The cylinder should also have planar caps
        assert!(step.contains("PLANE("));
    }

    #[test]
    fn surface_entity_plane() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Plane {
            origin: DVec3::ZERO,
            normal: DVec3::Z,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("PLANE("));
        assert!(s.contains("AXIS2_PLACEMENT_3D"));
    }

    #[test]
    fn surface_entity_cylinder() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Cylinder {
            origin: DVec3::ZERO,
            axis: DVec3::Y,
            radius: 10.0,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("CYLINDRICAL_SURFACE"));
        assert!(s.contains("10.000000"));
    }

    #[test]
    fn surface_entity_cone() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Cone {
            apex: DVec3::new(0.0, 0.0, 10.0),
            axis: DVec3::Z,
            half_angle: 0.5,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("CONICAL_SURFACE"));
        assert!(s.contains("0.500000"));
    }

    #[test]
    fn surface_entity_sphere() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Sphere {
            center: DVec3::new(1.0, 2.0, 3.0),
            radius: 7.5,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("SPHERICAL_SURFACE"));
        assert!(s.contains("7.500000"));
    }

    #[test]
    fn surface_entity_torus() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Torus {
            center: DVec3::ZERO,
            axis: DVec3::Z,
            major_radius: 10.0,
            minor_radius: 2.0,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("TOROIDAL_SURFACE"));
        assert!(s.contains("10.000000"));
        assert!(s.contains("2.000000"));
    }

    #[test]
    fn surface_entity_bspline() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Nurbs {
            control_points: vec![
                vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0)],
                vec![DVec3::new(0.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 1.0)],
            ],
            weights: vec![
                vec![1.0, 1.0],
                vec![1.0, 1.0],
            ],
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            degree_u: 1,
            degree_v: 1,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("B_SPLINE_SURFACE_WITH_KNOTS"));
    }

    #[test]
    fn surface_entity_rational_bspline() {
        let mut s = String::new();
        let mut ids = IdAlloc::new(1);
        let surface = Surface::Nurbs {
            control_points: vec![
                vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0)],
                vec![DVec3::new(0.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 1.0)],
            ],
            weights: vec![
                vec![1.0, 2.0],  // non-uniform weights
                vec![1.0, 1.0],
            ],
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            degree_u: 1,
            degree_v: 1,
        };
        let id = emit_surface_entity(&mut s, &mut ids, &surface);
        assert!(id > 0);
        assert!(s.contains("B_SPLINE_SURFACE_WITH_KNOTS"));
        assert!(s.contains("RATIONAL_B_SPLINE_SURFACE"));
    }

    // --- Assembly tests ---

    #[test]
    fn assembly_two_boxes() {
        let box1 = physical_brep::make_box(10.0, 10.0, 10.0);
        let box2 = physical_brep::make_box(5.0, 5.0, 5.0);

        let parts = vec![
            AssemblyEntry {
                solid: &box1,
                name: "Base",
                placement: Placement::identity(),
            },
            AssemblyEntry {
                solid: &box2,
                name: "Block",
                placement: Placement::from_position(DVec3::new(10.0, 0.0, 0.0)),
            },
        ];

        let step = write_step_assembly("TestAsm", &parts);
        assert!(step.contains("ISO-10303-21"));
        assert!(step.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"));
        assert!(step.contains("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION"));
        assert!(step.contains("ITEM_DEFINED_TRANSFORMATION"));
        assert!(step.contains("SHAPE_REPRESENTATION"));

        // Should have 2 NAUO links
        let nauo_count = step.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE").count();
        assert_eq!(nauo_count, 2, "expected 2 NAUOs, got {nauo_count}");

        // Should have 2 CDSR links
        let cdsr_count = step.matches("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION").count();
        assert_eq!(cdsr_count, 2, "expected 2 CDSRs, got {cdsr_count}");
    }

    #[test]
    fn assembly_single_part() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let parts = vec![
            AssemblyEntry {
                solid: &solid,
                name: "OnlyPart",
                placement: Placement::identity(),
            },
        ];
        let step = write_step_assembly("SingleAsm", &parts);
        assert!(step.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"));
        assert!(step.contains("OnlyPart"));
        assert!(step.contains("SingleAsm"));
    }

    #[test]
    fn assembly_empty() {
        let parts: Vec<AssemblyEntry<'_>> = vec![];
        let step = write_step_assembly("EmptyAsm", &parts);
        assert!(step.contains("ISO-10303-21"));
        assert!(step.contains("EmptyAsm"));
        // No NAUO for empty assembly
        assert!(!step.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"));
    }

    // --- Knot compression test ---

    #[test]
    fn compress_knots_basic() {
        let knots = vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0];
        let (vals, mults) = compress_knots(&knots);
        assert_eq!(vals, vec![0.0, 0.5, 1.0]);
        assert_eq!(mults, vec![3, 1, 3]);
    }

    #[test]
    fn compress_knots_empty() {
        let (vals, mults) = compress_knots(&[]);
        assert!(vals.is_empty());
        assert!(mults.is_empty());
    }

    // --- AP242 PMI / GD&T tests ---

    #[test]
    fn ap242_header_contains_schema() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let step = write_step_ap242(&solid, "Box242", &[]);
        assert!(step.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 442 1 1 4 }'))"));
        assert!(step.contains("AP242"));
    }

    #[test]
    fn ap242_gdt_flatness_writes() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let ann = vec![GdtAnnotation {
            characteristic: GdtCharacteristic::Flatness,
            tolerance_value: 0.05,
            datum_refs: vec![],
            material_condition: None,
            face_index: 0,
        }];
        let step = write_step_ap242(&solid, "FlatBox", &ann);
        assert!(step.contains("FLATNESS_TOLERANCE"));
        assert!(step.contains("TOLERANCE_ZONE"));
        assert!(step.contains("SHAPE_ASPECT"));
    }

    #[test]
    fn ap242_gdt_position_with_datums() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let ann = vec![GdtAnnotation {
            characteristic: GdtCharacteristic::Position,
            tolerance_value: 0.1,
            datum_refs: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            material_condition: None,
            face_index: 0,
        }];
        let step = write_step_ap242(&solid, "PosBox", &ann);
        assert!(step.contains("GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE"));
        assert!(step.contains("DATUM('A'"));
        assert!(step.contains("DATUM('B'"));
        assert!(step.contains("DATUM('C'"));
        assert!(step.contains("DATUM_REFERENCE"));
    }

    #[test]
    fn ap242_gdt_material_condition() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let ann = vec![GdtAnnotation {
            characteristic: GdtCharacteristic::Position,
            tolerance_value: 0.25,
            datum_refs: vec!["A".to_string()],
            material_condition: Some(MaterialCondition::Mmc),
            face_index: 0,
        }];
        let step = write_step_ap242(&solid, "MmcBox", &ann);
        assert!(step.contains("GEOMETRIC_TOLERANCE_WITH_MODIFIERS"));
        assert!(step.contains(".MAXIMUM_MATERIAL_CONDITION."));
    }

    #[test]
    fn ap242_dimension_linear() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let dims = vec![DimensionAnnotation {
            dim_type: DimensionType::Linear,
            value: 25.0,
            tolerance_plus: 0.0,
            tolerance_minus: 0.0,
            unit: "mm",
        }];
        let step = write_step_ap242_with_dims(&solid, "DimBox", &[], &dims);
        assert!(step.contains("DIMENSIONAL_SIZE"));
        assert!(step.contains("'linear'"));
        assert!(step.contains("LENGTH_MEASURE(25.000000)"));
    }

    #[test]
    fn ap242_dimension_with_tolerance() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let dims = vec![DimensionAnnotation {
            dim_type: DimensionType::Diameter,
            value: 50.0,
            tolerance_plus: 0.05,
            tolerance_minus: -0.02,
            unit: "mm",
        }];
        let step = write_step_ap242_with_dims(&solid, "TolBox", &[], &dims);
        assert!(step.contains("DIMENSIONAL_SIZE"));
        assert!(step.contains("'diameter'"));
        assert!(step.contains("PLUS_MINUS_TOLERANCE"));
        assert!(step.contains("LENGTH_MEASURE(0.050000)"));
        assert!(step.contains("LENGTH_MEASURE(-0.020000)"));
    }

    #[test]
    fn ap242_roundtrip_preserves_geometry() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let ann = vec![GdtAnnotation {
            characteristic: GdtCharacteristic::Flatness,
            tolerance_value: 0.01,
            datum_refs: vec![],
            material_condition: None,
            face_index: 0,
        }];
        let step = write_step_ap242(&solid, "RtBox", &ann);
        // Reader should handle AP242 files and reconstruct geometry
        let reimported = read_step(&step).unwrap();
        // Face count is preserved; vertex count may be higher because the
        // reader picks up CARTESIAN_POINTs from AXIS2_PLACEMENT_3D entities
        // emitted for surface definitions.
        assert_eq!(reimported.face_count(), 6);
        assert!(reimported.vertex_count() >= 8,
            "expected at least 8 vertices, got {}", reimported.vertex_count());
    }

    #[test]
    fn ap242_multiple_annotations() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let anns = vec![
            GdtAnnotation {
                characteristic: GdtCharacteristic::Flatness,
                tolerance_value: 0.05,
                datum_refs: vec![],
                material_condition: None,
                face_index: 0,
            },
            GdtAnnotation {
                characteristic: GdtCharacteristic::Perpendicularity,
                tolerance_value: 0.1,
                datum_refs: vec!["A".to_string()],
                material_condition: None,
                face_index: 1,
            },
            GdtAnnotation {
                characteristic: GdtCharacteristic::Parallelism,
                tolerance_value: 0.08,
                datum_refs: vec!["A".to_string(), "B".to_string()],
                material_condition: Some(MaterialCondition::Lmc),
                face_index: 2,
            },
        ];
        let step = write_step_ap242(&solid, "MultiBox", &anns);
        assert!(step.contains("FLATNESS_TOLERANCE"));
        assert!(step.contains("PERPENDICULARITY_TOLERANCE"));
        assert!(step.contains("PARALLELISM_TOLERANCE"));
        // Should have 3 SHAPE_ASPECT entries (one per annotation)
        let sa_count = step.matches("SHAPE_ASPECT('gdt_target'").count();
        assert_eq!(sa_count, 3, "expected 3 SHAPE_ASPECTs, got {sa_count}");
    }

    #[test]
    fn ap242_datum_references() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let ann = vec![GdtAnnotation {
            characteristic: GdtCharacteristic::Position,
            tolerance_value: 0.15,
            datum_refs: vec!["A".to_string(), "B".to_string()],
            material_condition: None,
            face_index: 0,
        }];
        let step = write_step_ap242(&solid, "DatumBox", &ann);
        // Should have 2 DATUM entities and 2 DATUM_REFERENCE entities
        let datum_count = step.matches("DATUM('").count();
        assert_eq!(datum_count, 2, "expected 2 DATUMs, got {datum_count}");
        let dr_count = step.matches("=DATUM_REFERENCE(").count();
        assert_eq!(dr_count, 2, "expected 2 DATUM_REFERENCEs, got {dr_count}");
        // Precedence order should be 1 and 2
        assert!(step.contains("DATUM_REFERENCE(#") && step.contains(",1)"));
        assert!(step.contains(",2)"));
    }

    #[test]
    fn ap242_empty_annotations_still_valid() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let step = write_step_ap242(&solid, "EmptyAnn", &[]);
        // Must be a valid STEP file
        assert!(step.contains("ISO-10303-21"));
        assert!(step.contains("END-ISO-10303-21"));
        assert!(step.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 442 1 1 4 }'))"));
        assert!(step.contains("MANIFOLD_SOLID_BREP"));
        assert!(step.contains("CLOSED_SHELL"));
        // Should NOT contain any GD&T entities
        assert!(!step.contains("FLATNESS_TOLERANCE"));
        assert!(!step.contains("GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE"));
        assert!(!step.contains("DIMENSIONAL_SIZE"));
    }
}
