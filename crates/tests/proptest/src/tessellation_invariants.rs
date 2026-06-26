//! Tessellation invariants — bounding box containment, no degenerate triangles.

use proptest::prelude::*;
use physical_brep::builder::{make_box, make_cylinder};
use physical_tessellation::tessellate;

fn arb_box_and_tol() -> impl Strategy<Value = (f64, f64, f64, f64)> {
    (5.0..200.0_f64, 5.0..200.0_f64, 5.0..200.0_f64, 0.1..10.0_f64)
}

proptest! {
    /// Tessellated mesh bounding box is contained within the solid's bounding box (with tolerance).
    #[test]
    fn tess_bbox_contained((w, h, d, tol) in arb_box_and_tol()) {
        let solid = make_box(w, h, d);
        let mesh = tessellate(&solid, tol);
        let (s_min, s_max) = solid.bounding_box();
        let (m_min, m_max) = mesh.bounding_box();

        // Mesh bbox should be within solid bbox ± tolerance
        prop_assert!(m_min[0] as f64 >= s_min.x - tol, "mesh min_x {} < solid min_x {}", m_min[0], s_min.x);
        prop_assert!(m_min[1] as f64 >= s_min.y - tol, "mesh min_y {} < solid min_y {}", m_min[1], s_min.y);
        prop_assert!(m_min[2] as f64 >= s_min.z - tol, "mesh min_z {} < solid min_z {}", m_min[2], s_min.z);
        prop_assert!((m_max[0] as f64) <= s_max.x + tol, "mesh max_x {} > solid max_x {}", m_max[0], s_max.x);
        prop_assert!((m_max[1] as f64) <= s_max.y + tol, "mesh max_y {} > solid max_y {}", m_max[1], s_max.y);
        prop_assert!((m_max[2] as f64) <= s_max.z + tol, "mesh max_z {} > solid max_z {}", m_max[2], s_max.z);
    }

    /// No degenerate triangles (all triangles have positive area).
    #[test]
    fn tess_no_degenerate_triangles((w, h, d, tol) in arb_box_and_tol()) {
        let solid = make_box(w, h, d);
        let mesh = tessellate(&solid, tol);

        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let v0 = &mesh.vertices[tri[0] as usize];
            let v1 = &mesh.vertices[tri[1] as usize];
            let v2 = &mesh.vertices[tri[2] as usize];

            let e1 = [v1.position[0] - v0.position[0], v1.position[1] - v0.position[1], v1.position[2] - v0.position[2]];
            let e2 = [v2.position[0] - v0.position[0], v2.position[1] - v0.position[1], v2.position[2] - v0.position[2]];
            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let area = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() * 0.5;
            prop_assert!(area > 1e-10, "degenerate triangle with area {}", area);
        }
    }

    /// Finer tolerance produces at least as many triangles (monotonic).
    #[test]
    fn tess_finer_more_triangles((w, h, d) in (10.0..100.0_f64, 10.0..100.0_f64, 10.0..100.0_f64)) {
        let solid = make_box(w, h, d);
        let coarse = tessellate(&solid, 5.0);
        let fine = tessellate(&solid, 0.5);
        prop_assert!(fine.triangle_count() >= coarse.triangle_count(),
            "fine ({}) should have >= coarse ({}) triangles",
            fine.triangle_count(), coarse.triangle_count());
    }

    /// Tessellation always produces at least 12 triangles for a box (2 per face).
    #[test]
    fn tess_minimum_triangles((w, h, d) in (1.0..500.0_f64, 1.0..500.0_f64, 1.0..500.0_f64)) {
        let solid = make_box(w, h, d);
        let mesh = tessellate(&solid, 1.0);
        prop_assert!(mesh.triangle_count() >= 12, "box should have >= 12 triangles, got {}", mesh.triangle_count());
    }

    /// Cylinder tessellation produces more triangles than a box.
    #[test]
    fn tess_cylinder_more_than_box((r, h) in (2.0..50.0_f64, 5.0..100.0_f64)) {
        let cyl = make_cylinder(r, h, 16);
        let bx = make_box(r * 2.0, r * 2.0, h);
        let mesh_cyl = tessellate(&cyl, 1.0);
        let mesh_box = tessellate(&bx, 1.0);
        prop_assert!(mesh_cyl.triangle_count() >= mesh_box.triangle_count(),
            "cylinder ({}) should have >= box ({}) triangles",
            mesh_cyl.triangle_count(), mesh_box.triangle_count());
    }
}
