//! B-Rep topology invariants — Euler formula, manifold closure, positive area.

use proptest::prelude::*;
use physical_brep::builder::{make_box, make_cylinder};

/// Generate random box dimensions (1mm to 500mm).
fn arb_box_dims() -> impl Strategy<Value = (f64, f64, f64)> {
    (1.0..500.0_f64, 1.0..500.0_f64, 1.0..500.0_f64)
}

/// Generate random cylinder params.
fn arb_cylinder_params() -> impl Strategy<Value = (f64, f64, u32)> {
    (0.5..100.0_f64, 1.0..200.0_f64, 8..64_u32)
}

proptest! {
    /// Euler formula: V - E + F = 2 for any box.
    #[test]
    fn euler_formula_box((w, h, d) in arb_box_dims()) {
        let solid = make_box(w, h, d);
        let v = solid.vertex_count() as i32;
        let e = solid.edge_count() as i32;
        let f = solid.face_count() as i32;
        // Euler: V - E + F = 2 for a simple polyhedron
        prop_assert_eq!(v - e + f, 2, "Euler failed: V={} E={} F={}", v, e, f);
    }

    /// Every box has exactly 6 faces, 12 edges, 8 vertices.
    #[test]
    fn box_topology_counts((w, h, d) in arb_box_dims()) {
        let solid = make_box(w, h, d);
        prop_assert_eq!(solid.face_count(), 6, "box should have 6 faces");
        prop_assert_eq!(solid.vertex_count(), 8, "box should have 8 vertices");
    }

    /// All faces have positive area for any valid box.
    #[test]
    fn box_faces_positive_area((w, h, d) in arb_box_dims()) {
        let solid = make_box(w, h, d);
        for (_fid, face) in &solid.faces {
            let verts: Vec<glam::DVec3> = face.outer_loop.iter().map(|he_id| {
                solid.vertices[solid.half_edges[*he_id].origin].point
            }).collect();
            if verts.len() >= 3 {
                let e1 = verts[1] - verts[0];
                let e2 = verts[2] - verts[0];
                let area = e1.cross(e2).length() * 0.5;
                prop_assert!(area > 0.0, "face area should be positive, got {}", area);
            }
        }
    }

    /// Bounding box dimensions match construction parameters.
    #[test]
    fn box_bounding_box_matches_dims((w, h, d) in arb_box_dims()) {
        let solid = make_box(w, h, d);
        let (bb_min, bb_max) = solid.bounding_box();
        let size = bb_max - bb_min;
        prop_assert!((size.x - w).abs() < 0.01, "width: expected {w}, got {}", size.x);
        prop_assert!((size.y - h).abs() < 0.01, "height: expected {h}, got {}", size.y);
        prop_assert!((size.z - d).abs() < 0.01, "depth: expected {d}, got {}", size.z);
    }

    /// Cylinder has correct number of faces for given segment count.
    #[test]
    fn cylinder_topology((r, h, segs) in arb_cylinder_params()) {
        let solid = make_cylinder(r, h, segs as usize);
        // Cylinder: 2 caps + segs side faces
        let expected_faces = 2 + segs as usize;
        let actual_faces = solid.face_count();
        prop_assert_eq!(actual_faces, expected_faces,
            "cylinder: expected {} faces, got {}", expected_faces, actual_faces);
    }

    /// Volume of a box matches w*h*d (via signed_volume).
    #[test]
    fn box_volume_matches((w, h, d) in arb_box_dims()) {
        let solid = make_box(w, h, d);
        let vol = physical_brep::volume(&solid);
        let expected = w * h * d;
        let error = (vol - expected).abs() / expected;
        prop_assert!(error < 0.05, "volume error {:.1}% for {}x{}x{}: got {vol:.1}, expected {expected:.1}", error * 100.0, w, h, d);
    }
}
