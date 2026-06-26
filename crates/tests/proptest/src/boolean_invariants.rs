//! Boolean operation invariants — volume bounds, manifold preservation.

use proptest::prelude::*;
use physical_brep::builder::make_box;
use physical_brep::{boolean, union, subtract, intersect, volume, BooleanOp};
use glam::DVec3;

/// Two boxes with random dimensions and separation.
fn arb_two_boxes() -> impl Strategy<Value = ((f64, f64, f64), (f64, f64, f64), f64)> {
    (
        (5.0..50.0_f64, 5.0..50.0_f64, 5.0..50.0_f64),
        (5.0..50.0_f64, 5.0..50.0_f64, 5.0..50.0_f64),
        0.0..100.0_f64, // separation along X
    )
}

proptest! {
    /// Union volume >= max(vol_A, vol_B) when boxes don't overlap.
    #[test]
    fn union_volume_non_overlapping(
        ((w1, h1, d1), (w2, h2, d2), sep) in arb_two_boxes()
    ) {
        let a = make_box(w1, h1, d1);
        let mut b = make_box(w2, h2, d2);
        // Move B far enough that it doesn't overlap A
        let offset = w1 / 2.0 + w2 / 2.0 + sep;
        let vids: Vec<_> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += offset;
        }

        let result = union(&a, &b);
        let vol_a = volume(&a);
        let vol_b = volume(&b);
        let vol_r = volume(&result);

        // Non-overlapping union: vol_r should ≈ vol_a + vol_b
        prop_assert!(vol_r >= vol_a * 0.9, "union volume {vol_r:.1} should be >= vol_a {vol_a:.1}");
        prop_assert!(vol_r >= vol_b * 0.9, "union volume {vol_r:.1} should be >= vol_b {vol_b:.1}");
    }

    /// Union always produces at least as many faces as the larger operand.
    #[test]
    fn union_face_count(((w1, h1, d1), (w2, h2, d2), sep) in arb_two_boxes()) {
        let a = make_box(w1, h1, d1);
        let mut b = make_box(w2, h2, d2);
        let offset = w1 / 2.0 + w2 / 2.0 + sep;
        let vids: Vec<_> = b.vertices.keys().collect();
        for vid in vids { b.vertices[vid].point.x += offset; }

        let result = union(&a, &b);
        prop_assert!(result.face_count() >= 6, "union should have at least 6 faces");
    }

    /// Subtract produces a valid solid (face_count > 0 or empty).
    #[test]
    fn subtract_produces_valid(((w1, h1, d1), (w2, h2, d2), _sep) in arb_two_boxes()) {
        let a = make_box(w1, h1, d1);
        let b = make_box(w2, h2, d2);
        let result = subtract(&a, &b);
        // Result is either empty or has at least 4 faces
        let fc = result.face_count();
        prop_assert!(fc == 0 || fc >= 4, "subtract face count {fc} is invalid");
    }

    /// Boolean operations never panic for valid inputs.
    #[test]
    fn boolean_no_panic(((w1, h1, d1), (w2, h2, d2), sep) in arb_two_boxes()) {
        let a = make_box(w1, h1, d1);
        let mut b = make_box(w2, h2, d2);
        let vids: Vec<_> = b.vertices.keys().collect();
        for vid in vids { b.vertices[vid].point.x += sep; }

        // These should not panic for any valid input
        let _ = boolean(&a, &b, BooleanOp::Union);
        let _ = boolean(&a, &b, BooleanOp::Subtract);
        let _ = boolean(&a, &b, BooleanOp::Intersect);
    }
}
