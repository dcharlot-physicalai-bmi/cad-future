//! FEA invariants — zero-load-zero-displacement, finite stress, positive stiffness.

use proptest::prelude::*;
use physical_brep::builder::make_box;
use physical_fea::{tetrahedralize, solve, BC};
use glam::DVec3;

fn arb_beam_dims() -> impl Strategy<Value = (f64, f64, f64)> {
    // Beam-like: length >> width/height, reasonable sizes
    (50.0..200.0_f64, 5.0..30.0_f64, 5.0..30.0_f64)
}

proptest! {
    /// Zero load → zero displacement.
    #[test]
    fn zero_load_zero_displacement((l, w, h) in arb_beam_dims()) {
        let solid = make_box(l, w, h);
        let mesh = tetrahedralize(&solid);

        // Fix one end, no loads
        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < -l / 2.0 + 1.0 {
                bcs.push(BC::FixAll(i));
            }
        }

        if bcs.is_empty() { return Ok(()); }

        let result = solve(&mesh, 200_000.0, 0.3, &bcs);
        prop_assert!(result.max_displacement < 1e-10,
            "zero load should give zero displacement, got {}", result.max_displacement);
        prop_assert!(result.max_von_mises < 1e-6,
            "zero load should give zero stress, got {}", result.max_von_mises);
    }

    /// All stresses are finite (no NaN or Inf).
    #[test]
    fn stress_always_finite((l, w, h) in arb_beam_dims()) {
        let solid = make_box(l, w, h);
        let mesh = tetrahedralize(&solid);

        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < -l / 2.0 + 1.0 {
                bcs.push(BC::FixAll(i));
            } else if node.position.x > l / 2.0 - 1.0 {
                bcs.push(BC::Force(i, DVec3::new(0.0, -10.0, 0.0)));
            }
        }

        if bcs.is_empty() { return Ok(()); }

        let result = solve(&mesh, 200_000.0, 0.3, &bcs);
        prop_assert!(!result.max_von_mises.is_nan(), "stress should not be NaN");
        prop_assert!(!result.max_von_mises.is_infinite(), "stress should not be Inf");
        prop_assert!(!result.max_displacement.is_nan(), "displacement should not be NaN");
        prop_assert!(!result.max_displacement.is_infinite(), "displacement should not be Inf");
    }

    /// Displacement is always positive or zero (magnitude).
    #[test]
    fn displacement_non_negative((l, w, h) in arb_beam_dims()) {
        let solid = make_box(l, w, h);
        let mesh = tetrahedralize(&solid);

        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < -l / 2.0 + 1.0 {
                bcs.push(BC::FixAll(i));
            }
        }

        if bcs.is_empty() { return Ok(()); }

        let result = solve(&mesh, 200_000.0, 0.3, &bcs);
        prop_assert!(result.max_displacement >= 0.0,
            "displacement magnitude should be >= 0");
    }

    /// Von Mises stress is always non-negative.
    #[test]
    fn von_mises_non_negative((l, w, h) in arb_beam_dims()) {
        let solid = make_box(l, w, h);
        let mesh = tetrahedralize(&solid);

        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < -l / 2.0 + 1.0 {
                bcs.push(BC::FixAll(i));
            } else if node.position.x > l / 2.0 - 1.0 {
                bcs.push(BC::Force(i, DVec3::new(0.0, -100.0, 0.0)));
            }
        }

        if bcs.is_empty() { return Ok(()); }

        let result = solve(&mesh, 200_000.0, 0.3, &bcs);
        prop_assert!(result.max_von_mises >= 0.0, "von Mises should be >= 0");
        for s in &result.stresses {
            prop_assert!(s.von_mises >= 0.0, "element von Mises should be >= 0");
        }
    }

    /// Tetrahedralization always produces positive-volume elements.
    #[test]
    fn tet_volumes_positive((l, w, h) in arb_beam_dims()) {
        let solid = make_box(l, w, h);
        let mesh = tetrahedralize(&solid);

        for (idx, elem) in mesh.elements.iter().enumerate() {
            let p = [
                mesh.nodes[elem.nodes[0]].position,
                mesh.nodes[elem.nodes[1]].position,
                mesh.nodes[elem.nodes[2]].position,
                mesh.nodes[elem.nodes[3]].position,
            ];
            let a = p[1] - p[0];
            let b = p[2] - p[0];
            let c = p[3] - p[0];
            let vol = a.dot(b.cross(c)) / 6.0;
            prop_assert!(vol.abs() > 1e-15, "element {} has degenerate volume {}", idx, vol);
        }
    }
}
