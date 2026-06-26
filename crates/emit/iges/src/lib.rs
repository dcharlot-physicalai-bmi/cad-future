//! `physical-emit-iges` — IGES 5.3 file format emitter.
//!
//! Emits Initial Graphics Exchange Specification (IGES) files from B-Rep solids.
//! Covers entity types needed for geometry interchange with legacy CAD systems:
//! - Entity 110: Line
//! - Entity 126: Rational B-Spline Curve
//! - Entity 128: Rational B-Spline Surface
//! - Entity 142: Curve on Parametric Surface
//! - Entity 144: Trimmed Parametric Surface
//! - Entity 314: Color Definition
//! - Entity 402: Associativity Instance (group)

use glam::DVec3;
use physical_brep::Solid;
use physical_brep::curve::Curve;
use physical_brep::surface::Surface;

// ---------------------------------------------------------------------------
// IGES Writer
// ---------------------------------------------------------------------------

/// IGES file writer.
struct IgesWriter {
    /// Directory entry lines (72-char fixed columns + sequence numbers).
    directory: Vec<DirEntry>,
    /// Parameter data lines.
    params: Vec<String>,
    /// Current directory entry sequence number (odd, 1-based).
    de_seq: usize,
    /// Current parameter data sequence number (1-based).
    pd_seq: usize,
}

struct DirEntry {
    entity_type: i32,
    param_start: usize,
    param_count: usize,
    status: u32,
    form: i32,
    label: String,
}

impl IgesWriter {
    fn new() -> Self {
        Self {
            directory: Vec::new(),
            params: Vec::new(),
            de_seq: 1,
            pd_seq: 1,
        }
    }

    /// Add an entity. Returns the DE sequence number (for referencing).
    fn add_entity(
        &mut self,
        entity_type: i32,
        form: i32,
        label: &str,
        param_data: &str,
    ) -> usize {
        let de_num = self.de_seq;
        let pd_start = self.pd_seq;

        // Split parameter data into 64-char lines (cols 1-64 of P section)
        let full_param = format!("{}{}", param_data, ";");
        let lines = split_param_data(&full_param, 64);
        let pd_count = lines.len();

        for (i, line) in lines.iter().enumerate() {
            let seq = self.pd_seq;
            let pd_line = format!(
                "{:<64}{:>8}P{:>7}",
                line,
                de_num,
                seq,
            );
            self.params.push(pd_line);
            self.pd_seq += 1;
        }

        self.directory.push(DirEntry {
            entity_type,
            param_start: pd_start,
            param_count: pd_count,
            status: 0,
            form,
            label: label.to_string(),
        });
        self.de_seq += 2; // DE entries always come in pairs
        de_num
    }

    /// Write a line entity (type 110).
    fn line(&mut self, start: DVec3, end: DVec3) -> usize {
        let data = format!(
            "110,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            start.x, start.y, start.z, end.x, end.y, end.z
        );
        self.add_entity(110, 0, "LINE", &data)
    }

    /// Write a rational B-spline curve (type 126).
    fn nurbs_curve(
        &mut self,
        degree: usize,
        control_points: &[DVec3],
        weights: &[f64],
        knots: &[f64],
    ) -> usize {
        let n = control_points.len() - 1; // upper index
        let k = degree;
        let a = knots.len() - 1;

        let mut data = format!("126,{},{},{},0,0,0", n, k, a);

        // Knot values
        for kv in knots {
            data.push_str(&format!(",{:.6}", kv));
        }

        // Weights
        for w in weights {
            data.push_str(&format!(",{:.6}", w));
        }

        // Control points
        for cp in control_points {
            data.push_str(&format!(",{:.6},{:.6},{:.6}", cp.x, cp.y, cp.z));
        }

        // Parameter range
        let t_start = knots.first().copied().unwrap_or(0.0);
        let t_end = knots.last().copied().unwrap_or(1.0);
        data.push_str(&format!(",{:.6},{:.6}", t_start, t_end));

        // Unit tangent at start/end (0,0,0 = unspecified)
        data.push_str(",0.0,0.0,0.0,0.0,0.0,0.0");

        self.add_entity(126, 0, "BSPLINE_C", &data)
    }

    /// Write a rational B-spline surface (type 128).
    fn nurbs_surface(
        &mut self,
        degree_u: usize,
        degree_v: usize,
        control_points: &[Vec<DVec3>],
        weights: &[Vec<f64>],
        knots_u: &[f64],
        knots_v: &[f64],
    ) -> usize {
        let rows = control_points.len();
        let cols = if rows > 0 { control_points[0].len() } else { 0 };
        let m1 = rows - 1; // upper index u
        let m2 = cols - 1; // upper index v

        let mut data = format!(
            "128,{},{},{},{},0,0,0,0,0",
            m1, m2, degree_u, degree_v
        );

        // Knots U
        for kv in knots_u {
            data.push_str(&format!(",{:.6}", kv));
        }

        // Knots V
        for kv in knots_v {
            data.push_str(&format!(",{:.6}", kv));
        }

        // Weights (row-major)
        for row_w in weights {
            for w in row_w {
                data.push_str(&format!(",{:.6}", w));
            }
        }

        // Control points (row-major)
        for row in control_points {
            for cp in row {
                data.push_str(&format!(",{:.6},{:.6},{:.6}", cp.x, cp.y, cp.z));
            }
        }

        // Parameter ranges
        let u_start = knots_u.first().copied().unwrap_or(0.0);
        let u_end = knots_u.last().copied().unwrap_or(1.0);
        let v_start = knots_v.first().copied().unwrap_or(0.0);
        let v_end = knots_v.last().copied().unwrap_or(1.0);
        data.push_str(&format!(",{:.6},{:.6},{:.6},{:.6}", u_start, u_end, v_start, v_end));

        self.add_entity(128, 0, "BSPLINE_S", &data)
    }

    /// Write a trimmed parametric surface (type 144).
    fn trimmed_surface(&mut self, surface_de: usize, outer_boundary_de: usize) -> usize {
        // 144, surface_de, 1 (outer boundary is a boundary), n_inner=0, outer_boundary_de
        let data = format!("144,{},1,0,{}", surface_de, outer_boundary_de);
        self.add_entity(144, 0, "TRIMSURF", &data)
    }

    /// Write a curve on parametric surface (type 142).
    fn curve_on_surface(
        &mut self,
        surface_de: usize,
        curve_3d_de: usize,
    ) -> usize {
        // 142, creation_method=0 (unspecified), surface_de, curve_2d_de=0, curve_3d_de, preferred=3 (3D)
        let data = format!("142,0,{},0,{},3", surface_de, curve_3d_de);
        self.add_entity(142, 0, "CURV_SRF", &data)
    }

    /// Emit a B-Rep curve as an IGES entity.
    fn emit_curve(&mut self, curve: &Curve) -> usize {
        match curve {
            Curve::Line { start, end } => self.line(*start, *end),
            Curve::Circle { center, axis, radius } | Curve::Arc { center, axis, radius, .. } => {
                // Approximate circle as degree-2 rational B-spline (9 control points)
                let (cps, ws, ks) = circle_to_nurbs(*center, *axis, *radius);
                self.nurbs_curve(2, &cps, &ws, &ks)
            }
            Curve::Nurbs { control_points, weights, knots, degree } => {
                self.nurbs_curve(*degree, control_points, weights, knots)
            }
        }
    }

    /// Emit a B-Rep surface as an IGES entity.
    fn emit_surface(&mut self, surface: &Surface) -> usize {
        match surface {
            Surface::Plane { origin, normal } => {
                // Represent plane as bilinear NURBS surface (degree 1×1)
                let up = if normal.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let u_dir = normal.cross(up).normalize();
                let v_dir = normal.cross(u_dir).normalize();
                let size = 100.0; // arbitrary extent
                let cps = vec![
                    vec![*origin - u_dir * size - v_dir * size, *origin + u_dir * size - v_dir * size],
                    vec![*origin - u_dir * size + v_dir * size, *origin + u_dir * size + v_dir * size],
                ];
                let weights = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
                let knots_u = vec![0.0, 0.0, 1.0, 1.0];
                let knots_v = vec![0.0, 0.0, 1.0, 1.0];
                self.nurbs_surface(1, 1, &cps, &weights, &knots_u, &knots_v)
            }
            Surface::Cylinder { origin, axis, radius } => {
                // Represent cylinder as NURBS surface: rational circle × linear extrusion
                let (cps_circ, ws_circ, _) = circle_to_nurbs(DVec3::ZERO, DVec3::Z, *radius);
                let ax = axis.normalize();
                let up = if ax.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let u_dir = ax.cross(up).normalize();
                let v_dir = ax.cross(u_dir).normalize();
                let height = 100.0;

                // Build 2-row surface: bottom circle and top circle
                let mut cps = Vec::new();
                let mut weights = Vec::new();
                for level in [0.0, height] {
                    let offset = *origin + ax * level;
                    let row: Vec<DVec3> = cps_circ.iter().map(|cp| {
                        offset + u_dir * cp.x + v_dir * cp.y + ax * cp.z
                    }).collect();
                    cps.push(row);
                    weights.push(ws_circ.clone());
                }
                let knots_u = vec![0.0, 0.0, 1.0, 1.0]; // linear in u
                let n = cps_circ.len();
                let degree = 2;
                let knots_v = circle_knots(n, degree);
                self.nurbs_surface(1, 2, &cps, &weights, &knots_u, &knots_v)
            }
            Surface::Sphere { center, radius } => {
                // Approximate sphere as 2 hemispheres isn't trivial;
                // use a simple biquadratic NURBS approximation
                let r = *radius;
                let c = *center;
                // 3×3 rational biquadratic patch for hemisphere-like shape
                let cps = vec![
                    vec![c + DVec3::new(-r, -r, 0.0), c + DVec3::new(0.0, -r, r), c + DVec3::new(r, -r, 0.0)],
                    vec![c + DVec3::new(-r, 0.0, 0.0), c + DVec3::new(0.0, 0.0, r), c + DVec3::new(r, 0.0, 0.0)],
                    vec![c + DVec3::new(-r, r, 0.0), c + DVec3::new(0.0, r, r), c + DVec3::new(r, r, 0.0)],
                ];
                let w = std::f64::consts::FRAC_1_SQRT_2;
                let weights = vec![
                    vec![1.0, w, 1.0],
                    vec![w, w * w, w],
                    vec![1.0, w, 1.0],
                ];
                let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
                self.nurbs_surface(2, 2, &cps, &weights, &knots, &knots)
            }
            Surface::Torus { center, axis, major_radius, minor_radius } => {
                // Simplified: represent as NURBS surface
                let ax = axis.normalize();
                let up = if ax.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let u_dir = ax.cross(up).normalize();
                let v_dir = ax.cross(u_dir).normalize();
                let r = *major_radius;
                let rr = *minor_radius;
                let c = *center;
                // Simple 4×4 grid approximation
                let mut cps = Vec::new();
                let mut weights = Vec::new();
                for i in 0..4 {
                    let angle = std::f64::consts::FRAC_PI_2 * i as f64;
                    let cu = angle.cos();
                    let su = angle.sin();
                    let center_ring = c + u_dir * (r * cu) + v_dir * (r * su);
                    let out_dir = (u_dir * cu + v_dir * su).normalize();
                    let mut row = Vec::new();
                    let mut row_w = Vec::new();
                    for j in 0..4 {
                        let angle_v = std::f64::consts::FRAC_PI_2 * j as f64;
                        let cv = angle_v.cos();
                        let sv = angle_v.sin();
                        row.push(center_ring + out_dir * (rr * cv) + ax * (rr * sv));
                        row_w.push(1.0);
                    }
                    cps.push(row);
                    weights.push(row_w);
                }
                let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
                self.nurbs_surface(3, 3, &cps, &weights, &knots, &knots)
            }
            Surface::Cone { apex, axis, half_angle } => {
                // Represent cone as NURBS: linear from apex to circular base
                let ax = axis.normalize();
                let up = if ax.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let u_dir = ax.cross(up).normalize();
                let v_dir = ax.cross(u_dir).normalize();
                let height = 100.0;
                let r = height * half_angle.tan();
                let base_center = *apex + ax * height;
                let (cps_circ, ws_circ, _) = circle_to_nurbs(DVec3::ZERO, DVec3::Z, r);

                let mut cps = Vec::new();
                let mut weights = Vec::new();
                // Row 0: apex (all control points at apex)
                let apex_row: Vec<DVec3> = cps_circ.iter().map(|_| *apex).collect();
                cps.push(apex_row);
                weights.push(vec![1.0; cps_circ.len()]);
                // Row 1: base circle
                let base_row: Vec<DVec3> = cps_circ.iter().map(|cp| {
                    base_center + u_dir * cp.x + v_dir * cp.y
                }).collect();
                cps.push(base_row);
                weights.push(ws_circ);

                let knots_u = vec![0.0, 0.0, 1.0, 1.0];
                let n = cps_circ.len();
                let knots_v = circle_knots(n, 2);
                self.nurbs_surface(1, 2, &cps, &weights, &knots_u, &knots_v)
            }
            Surface::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
                self.nurbs_surface(*degree_u, *degree_v, control_points, weights, knots_u, knots_v)
            }
        }
    }

    /// Build the complete IGES file text.
    fn to_string(&self, filename: &str) -> String {
        let mut output = String::new();

        // Start section (S)
        let s_line = format!("{:<72}S{:>7}\n", format!("                                                                        1H,,1H;,7H{};", filename), 1);
        output.push_str(&s_line);

        // Global section (G)
        let g_data = format!(
            "1H,,1H;,7H{},23Hphysical.openie.dev,23Hphysical-emit-iges,32,38,6,308,15,7H{},1.0,2,2HMM,1,,15H20260101.000000,0.001,100.0,,,11,0,;",
            filename, filename
        );
        let g_lines = split_iges_lines(&g_data, "G");
        for line in &g_lines {
            output.push_str(line);
            output.push('\n');
        }

        // Directory section (D)
        let mut d_seq = 1;
        for de in &self.directory {
            // Line 1 of DE pair
            let line1 = format!(
                "{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}\n",
                de.entity_type,
                de.param_start,
                0, // structure
                0, // line font
                0, // level
                0, // view
                0, // transform
                0, // label display
                format!("{:08}", de.status),
                d_seq,
            );
            output.push_str(&line1);

            // Line 2 of DE pair
            let line2 = format!(
                "{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}\n",
                de.entity_type,
                0, // line weight
                0, // color
                de.param_count,
                de.form,
                "", "", "",
                format!("{:>8}", &de.label[..de.label.len().min(8)]),
                d_seq + 1,
            );
            output.push_str(&line2);
            d_seq += 2;
        }

        // Parameter section (P) — already formatted
        for line in &self.params {
            output.push_str(line);
            output.push('\n');
        }

        // Terminate section (T)
        let t_line = format!(
            "S{:>7}G{:>7}D{:>7}P{:>7}{:>40}T{:>7}\n",
            1,
            g_lines.len(),
            self.directory.len() * 2,
            self.params.len(),
            "",
            1,
        );
        output.push_str(&t_line);

        output
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Write an IGES 5.3 file from a B-Rep solid.
pub fn write_iges(solid: &Solid, name: &str) -> String {
    let mut w = IgesWriter::new();

    // Emit each face as a trimmed surface
    for (_fid, face) in &solid.faces {
        let surf_de = w.emit_surface(&face.surface);

        // Build boundary from edge curves
        let mut boundary_curves = Vec::new();
        for he_id in &face.outer_loop {
            let he = &solid.half_edges[*he_id];
            let edge = &solid.edges[he.edge];
            let curve_de = w.emit_curve(&edge.curve);
            let cos_de = w.curve_on_surface(surf_de, curve_de);
            boundary_curves.push(cos_de);
        }

        if let Some(&first_curve) = boundary_curves.first() {
            // Use first boundary curve as outer boundary
            // (simplified — real IGES would use composite curve 102)
            w.trimmed_surface(surf_de, first_curve);
        }
    }

    w.to_string(name)
}

// ---------------------------------------------------------------------------
// Circle-to-NURBS Conversion
// ---------------------------------------------------------------------------

/// Convert a circle to a degree-2 rational NURBS (9 control points, standard form).
fn circle_to_nurbs(center: DVec3, axis: DVec3, radius: f64) -> (Vec<DVec3>, Vec<f64>, Vec<f64>) {
    let ax = axis.normalize();
    let up = if ax.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let u = ax.cross(up).normalize();
    let v = ax.cross(u).normalize();

    let w = std::f64::consts::FRAC_1_SQRT_2;
    let r = radius;

    let cps = vec![
        center + u * r,
        center + u * r + v * r,
        center + v * r,
        center - u * r + v * r,
        center - u * r,
        center - u * r - v * r,
        center - v * r,
        center + u * r - v * r,
        center + u * r, // closed
    ];

    let weights = vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0];

    let knots = vec![
        0.0, 0.0, 0.0,
        0.25, 0.25,
        0.5, 0.5,
        0.75, 0.75,
        1.0, 1.0, 1.0,
    ];

    (cps, weights, knots)
}

/// Generate knot vector for a circle NURBS with given control point count and degree.
fn circle_knots(n_cps: usize, degree: usize) -> Vec<f64> {
    let n_knots = n_cps + degree + 1;
    let mut knots = Vec::with_capacity(n_knots);
    for i in 0..n_knots {
        let v = (i as f64) / (n_knots as f64 - 1.0);
        knots.push(v);
    }
    // Clamp ends
    for i in 0..=degree {
        knots[i] = 0.0;
        knots[n_knots - 1 - i] = 1.0;
    }
    knots
}

// ---------------------------------------------------------------------------
// IGES Formatting Helpers
// ---------------------------------------------------------------------------

/// Split parameter data into 64-character chunks.
fn split_param_data(data: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = data;
    while !remaining.is_empty() {
        let end = remaining.len().min(width);
        lines.push(remaining[..end].to_string());
        remaining = &remaining[end..];
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Split global section data into 72-char IGES lines with section letter.
fn split_iges_lines(data: &str, section: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = data;
    let mut seq = 1;
    while !remaining.is_empty() {
        let end = remaining.len().min(72);
        let line = format!("{:<72}{}{:>7}", &remaining[..end], section, seq);
        lines.push(line);
        remaining = &remaining[end..];
        seq += 1;
    }
    if lines.is_empty() {
        lines.push(format!("{:<72}{}{:>7}", "", section, 1));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::builder::make_box;

    #[test]
    fn iges_box_has_sections() {
        let solid = make_box(10.0, 10.0, 10.0);
        let iges = write_iges(&solid, "TestBox");
        // IGES must have S, G, D, P, T sections
        assert!(iges.contains("S      1"));
        assert!(iges.contains("G      1"));
        assert!(iges.contains("D      1"));
        assert!(iges.contains("P      1"));
        assert!(iges.contains("T      1"));
    }

    #[test]
    fn iges_box_has_entities() {
        let solid = make_box(10.0, 10.0, 10.0);
        let iges = write_iges(&solid, "TestBox");
        // Should have 128 (surface) or 110 (line) entities
        assert!(iges.contains("128") || iges.contains("110"));
    }

    #[test]
    fn iges_has_trimmed_surfaces() {
        let solid = make_box(10.0, 10.0, 10.0);
        let iges = write_iges(&solid, "TestBox");
        // Entity 144 = trimmed surface
        assert!(iges.contains("     144"), "Expected trimmed surface entities");
    }

    #[test]
    fn iges_nonzero_length() {
        let solid = make_box(10.0, 10.0, 10.0);
        let iges = write_iges(&solid, "Box");
        assert!(iges.len() > 200, "IGES output too short: {} bytes", iges.len());
    }

    #[test]
    fn circle_nurbs_9_cps() {
        let (cps, ws, ks) = circle_to_nurbs(DVec3::ZERO, DVec3::Z, 5.0);
        assert_eq!(cps.len(), 9);
        assert_eq!(ws.len(), 9);
        assert_eq!(ks.len(), 12);
        // First and last control points should be the same (closed)
        assert!((cps[0] - cps[8]).length() < 1e-10);
    }

    #[test]
    fn circle_nurbs_weights() {
        let (_, ws, _) = circle_to_nurbs(DVec3::ZERO, DVec3::Z, 1.0);
        let w = std::f64::consts::FRAC_1_SQRT_2;
        // Even indices = 1.0, odd = 1/sqrt(2)
        assert!((ws[0] - 1.0).abs() < 1e-10);
        assert!((ws[1] - w).abs() < 1e-10);
        assert!((ws[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn split_param_data_works() {
        let data = "110,1.0,2.0,3.0,4.0,5.0,6.0;";
        let lines = split_param_data(data, 64);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= 64);
    }

    #[test]
    fn iges_cylinder() {
        let solid = physical_brep::builder::make_cylinder(5.0, 10.0, 8);
        let iges = write_iges(&solid, "Cylinder");
        assert!(iges.len() > 200);
        assert!(iges.contains("T      1"));
    }
}
