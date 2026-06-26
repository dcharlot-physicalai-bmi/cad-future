//! Sketch-to-solid pipeline tests — create a constrained 2D sketch,
//! solve constraints, extract profiles, extrude to 3D, and verify
//! the resulting solid geometry.

use physical_sketch::{Sketch, SketchEntity, Constraint, solve, SolveResult, extract_profiles};
use physical_sketch::entity::PointRef;
use physical_parametric::{ModelDocument, FeatureOp};
use physical_analytical::mass_properties;

/// Full sketch→extrude pipeline: constrain a rectangle in sketch,
/// extrude, verify volume matches.
#[test]
fn constrained_rectangle_extrude() {
    let mut sk = Sketch::new();

    // Draw a rectangle (lines form a closed loop)
    let l0 = sk.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));
    let l1 = sk.add_entity(SketchEntity::line(10.0, 0.0, 10.0, 5.0));
    let l2 = sk.add_entity(SketchEntity::line(10.0, 5.0, 0.0, 5.0));
    let l3 = sk.add_entity(SketchEntity::line(0.0, 5.0, 0.0, 0.0));

    // Constrain: fix the origin corner
    sk.add_constraint(Constraint::Fixed {
        point: PointRef::new(l0, 0),
        x: 0.0, y: 0.0,
    });

    let result = solve(&mut sk);
    assert!(
        matches!(result, SolveResult::FullyConstrained | SolveResult::UnderConstrained(_)),
        "sketch should solve: {result:?}"
    );

    // Extract profiles
    let profiles = extract_profiles(&sk);
    assert!(!profiles.is_empty(), "should extract at least one profile");

    // Extrude via parametric model
    let mut doc = ModelDocument::new("SketchPart");
    doc.add("Extrude", FeatureOp::ExtrudeSketch {
        sketch: sk,
        direction: [0.0, 0.0, 1.0],
        distance: 15.0,
    });

    let solid = doc.rebuild().unwrap();
    assert!(solid.vertex_count() >= 8, "extruded rectangle should have ≥8 vertices");

    let props = mass_properties(&solid);
    let expected = 10.0 * 5.0 * 15.0;
    let err = (props.volume - expected).abs() / expected;
    assert!(
        err < 0.05,
        "volume {:.1} should be ~{:.1} (err {:.1}%)",
        props.volume, expected, err * 100.0
    );
}

/// Tangent constraint: a line tangent to a circle should be exactly
/// radius distance from center after solving.
#[test]
fn tangent_constraint_geometric_accuracy() {
    let mut sk = Sketch::new();
    let c = sk.add_entity(SketchEntity::circle(0.0, 0.0, 10.0));
    let l = sk.add_entity(SketchEntity::line(-20.0, 12.0, 20.0, 12.0)); // above circle

    // Fix circle position
    sk.add_constraint(Constraint::Fixed {
        point: PointRef::new(c, 0),
        x: 0.0, y: 0.0,
    });
    sk.add_constraint(Constraint::Radius { entity: c, value: 10.0 });
    sk.add_constraint(Constraint::Tangent { entity_a: l, entity_b: c });

    let result = solve(&mut sk);
    assert!(
        matches!(result, SolveResult::FullyConstrained | SolveResult::UnderConstrained(_)),
        "tangent sketch should solve: {result:?}"
    );

    // After solving, the line should be at distance = radius from center
    let params = sk.entities[l].params();
    let x1 = params[0]; let y1 = params[1];
    let x2 = params[2]; let y2 = params[3];

    // Distance from (0,0) to line
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    let dist = ((-dy * (0.0 - x1) + dx * (0.0 - y1)) / len).abs();

    assert!(
        (dist - 10.0).abs() < 0.1,
        "line distance from center {dist:.2} should be ~10.0 (radius)"
    );
}

/// Coincident constraint should merge endpoints.
#[test]
fn coincident_constraint_merges_points() {
    let mut sk = Sketch::new();
    let l0 = sk.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));
    let l1 = sk.add_entity(SketchEntity::line(10.5, 0.5, 20.0, 5.0)); // slightly off

    // Constrain end of l0 (point 1) to start of l1 (point 0)
    sk.add_constraint(Constraint::Coincident {
        a: PointRef::new(l0, 1),
        b: PointRef::new(l1, 0),
    });

    let result = solve(&mut sk);
    assert!(
        matches!(result, SolveResult::FullyConstrained | SolveResult::UnderConstrained(_)),
        "should solve: {result:?}"
    );

    // After solving, endpoint of l0 should match startpoint of l1
    let p0 = sk.entities[l0].params();
    let p1 = sk.entities[l1].params();
    let end_l0 = (p0[2], p0[3]);
    let start_l1 = (p1[0], p1[1]);

    let dist = ((end_l0.0 - start_l1.0).powi(2) + (end_l0.1 - start_l1.1).powi(2)).sqrt();
    assert!(
        dist < 0.01,
        "coincident points should overlap: end_l0=({:.2},{:.2}) start_l1=({:.2},{:.2}) dist={dist:.4}",
        end_l0.0, end_l0.1, start_l1.0, start_l1.1
    );
}

/// Perpendicular constraint should produce 90° angle.
#[test]
fn perpendicular_constraint_right_angle() {
    let mut sk = Sketch::new();
    let l0 = sk.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));
    let l1 = sk.add_entity(SketchEntity::line(5.0, -5.0, 5.0, 5.0));

    sk.add_constraint(Constraint::Perpendicular { line_a: l0, line_b: l1 });

    let result = solve(&mut sk);
    assert!(
        matches!(result, SolveResult::FullyConstrained | SolveResult::UnderConstrained(_)),
        "should solve: {result:?}"
    );

    // Compute dot product of line directions
    let p0 = sk.entities[l0].params();
    let p1 = sk.entities[l1].params();
    let d0 = (p0[2] - p0[0], p0[3] - p0[1]);
    let d1 = (p1[2] - p1[0], p1[3] - p1[1]);
    let dot = d0.0 * d1.0 + d0.1 * d1.1;

    assert!(
        dot.abs() < 0.01,
        "perpendicular lines should have dot product ~0, got {dot:.4}"
    );
}

/// Parallel constraint should produce parallel directions.
#[test]
fn parallel_constraint_directions() {
    let mut sk = Sketch::new();
    let l0 = sk.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 3.0));
    let l1 = sk.add_entity(SketchEntity::line(0.0, 5.0, 8.0, 6.0)); // not parallel initially

    sk.add_constraint(Constraint::Parallel { line_a: l0, line_b: l1 });

    let result = solve(&mut sk);
    assert!(
        matches!(result, SolveResult::FullyConstrained | SolveResult::UnderConstrained(_)),
        "should solve: {result:?}"
    );

    let p0 = sk.entities[l0].params();
    let p1 = sk.entities[l1].params();
    let d0 = (p0[2] - p0[0], p0[3] - p0[1]);
    let d1 = (p1[2] - p1[0], p1[3] - p1[1]);

    // Cross product should be ~0 for parallel lines
    let cross = d0.0 * d1.1 - d0.1 * d1.0;
    let len0 = (d0.0 * d0.0 + d0.1 * d0.1).sqrt();
    let len1 = (d1.0 * d1.0 + d1.1 * d1.1).sqrt();
    let sin_angle = cross / (len0 * len1);

    assert!(
        sin_angle.abs() < 0.01,
        "parallel lines should have sin(angle) ~0, got {sin_angle:.4}"
    );
}
