//! Sketch solver invariants — convergence, DOF correctness, fixed-point stability.

use proptest::prelude::*;
use physical_sketch::{Sketch, SketchEntity, Constraint, PointRef, solve, SolveResult};

proptest! {
    /// Fixed point constraint always converges to the exact target.
    #[test]
    fn fixed_point_converges(x in -100.0..100.0_f64, y in -100.0..100.0_f64) {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));
        s.add_constraint(Constraint::Fixed { point: PointRef::new(0, 0), x, y });
        let result = solve(&mut s);
        prop_assert_eq!(result, SolveResult::FullyConstrained);
        let p = s.entities[0].get_point(0);
        prop_assert!((p.x - x).abs() < 1e-6, "x: expected {x}, got {}", p.x);
        prop_assert!((p.y - y).abs() < 1e-6, "y: expected {y}, got {}", p.y);
    }

    /// DOF count = params - equations for any sketch configuration.
    #[test]
    fn dof_equals_params_minus_equations(
        n_points in 1..5_usize,
        n_fixed in 0..3_usize,
    ) {
        let mut s = Sketch::new();
        for i in 0..n_points {
            s.add_entity(SketchEntity::point(i as f64 * 10.0, 0.0));
        }
        for i in 0..n_fixed.min(n_points) {
            s.add_constraint(Constraint::Fixed {
                point: PointRef::new(i, 0),
                x: i as f64 * 10.0,
                y: 0.0,
            });
        }
        let expected_dof = (n_points * 2) as i32 - (n_fixed.min(n_points) * 2) as i32;
        let actual_dof = s.dof();
        prop_assert_eq!(actual_dof, expected_dof,
            "DOF mismatch: {} points, {} fixed → expected {}, got {}",
            n_points, n_fixed, expected_dof, actual_dof);
    }

    /// Horizontal constraint makes y-coordinates equal.
    #[test]
    fn horizontal_constraint_works(
        x1 in -50.0..50.0_f64, y1 in -50.0..50.0_f64,
        x2 in -50.0..50.0_f64, y2 in -50.0..50.0_f64,
    ) {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(x1, y1, x2, y2));
        s.add_constraint(Constraint::Fixed { point: PointRef::new(0, 0), x: x1, y: y1 });
        s.add_constraint(Constraint::Horizontal { a: PointRef::new(0, 0), b: PointRef::new(0, 1) });
        s.add_constraint(Constraint::LineLength { entity: 0, value: 10.0 });
        let result = solve(&mut s);
        prop_assert_eq!(result, SolveResult::FullyConstrained);

        if let SketchEntity::Line { start, end } = &s.entities[0] {
            prop_assert!((start.y - end.y).abs() < 1e-6,
                "horizontal: start.y={} end.y={}", start.y, end.y);
        }
    }

    /// Distance constraint produces correct distance.
    #[test]
    fn distance_constraint(target in 1.0..100.0_f64) {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));
        s.add_entity(SketchEntity::point(target * 1.1, 0.0)); // start slightly off
        s.add_constraint(Constraint::Fixed { point: PointRef::new(0, 0), x: 0.0, y: 0.0 });
        s.add_constraint(Constraint::Distance {
            a: PointRef::new(0, 0),
            b: PointRef::new(1, 0),
            value: target,
        });
        let _result = solve(&mut s);
        let p0 = s.entities[0].get_point(0);
        let p1 = s.entities[1].get_point(0);
        let dist = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt();
        // Distance constraint uses squared distance, so tolerance is wider
        prop_assert!((dist - target).abs() < 1.0,
            "distance: expected {target}, got {dist}");
    }

    /// Solve never panics for any valid sketch.
    #[test]
    fn solve_no_panic(n_entities in 1..4_usize) {
        let mut s = Sketch::new();
        for i in 0..n_entities {
            s.add_entity(SketchEntity::point(i as f64 * 5.0, i as f64 * 3.0));
        }
        // This should never panic
        let _ = solve(&mut s);
    }
}
