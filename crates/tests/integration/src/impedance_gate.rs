//! Impedance regression gate.
//!
//! Runs the full `physical-impedance` audit against the registered cad-future
//! operations and fails if:
//!
//! 1. Any operation exceeds its per-op impedance budget (gap in orders of
//!    magnitude). Previously-closed offenders are held to a tight budget so
//!    an accidental revert surfaces as a test failure.
//! 2. The geometric-mean speedup of the whole registry regresses above the
//!    established baseline. This catches cumulative drift that doesn't trip
//!    any single per-op budget.
//!
//! Budgets are intentionally slightly looser than current state so
//! near-threshold noise doesn't flap. Tighten them deliberately when new
//! closures land.

use physical_impedance::{
    analyze, audit, audit_total_speedup, cad_future_operations, AbstractionLevel,
    ConcurrencyTopology, InformationTopology, Operation, Primitive,
};

/// Per-operation impedance budget, in orders of magnitude above the floor.
///
/// The gate fails if an operation's gap exceeds the listed value.
/// Operations not listed fall back to `DEFAULT_MAX_GAP_ORDERS`.
const PER_OP_BUDGETS: &[(&str, f64)] = &[
    // Cache-closed ops — revert-guards. Anything above ~2.5 means the LUT
    // layer has been bypassed or the registry coordinates drifted back.
    ("tessellate_face_uv_grid", 2.5),
    ("boolean_point_in_solid", 2.5),
    ("gen_bridge_render_depth", 2.5),

    // Cache-closed: FEA stiffness assembly was moved to sparse CSR + rayon
    // in Phase B. Natural coordinates now — hold it at 0.5 to catch any
    // revert that sends assembly back through the dense path.
    ("fea_stiffness_assembly", 0.5),

    // Cache-closed: sketch Jacobian finite differences + J^T·J are now
    // rayon-parallel across columns/rows (`build_jacobian`, `mat_mul_transpose`).
    // The remaining ~2 orders are "we don't have SIMD intrinsics" which is
    // out-of-scope for this phase; budget holds the parallel path steady.
    ("sketch_solver_jacobian", 2.5),

    // Cache-closed: region-grow now runs on precomputed per-triangle
    // normal/centroid/area arrays and dense adjacency; classification is
    // rayon-parallel per region. The remaining 1 order is the inherent
    // serial-BFS floor.
    ("inverse_segment_region_grow", 1.5),

    // Known-hard reductive work. These are the next targets for Phase B.
    // Budget is set slightly above current state so the gate catches
    // regressions without forcing us to close them today.
    ("fea_solve_pcg", 4.0),
    ("lut_material_lookup", 4.0),
];

/// Default per-operation budget for ops without an explicit entry.
const DEFAULT_MAX_GAP_ORDERS: f64 = 5.5;

/// Aggregate budget: the geometric-mean speedup across all registered ops.
/// After Phase B (FEA sparse + sketch Jacobian parallelized + inverse
/// region-grow cached) the audit reports ~72×. We allow a little slack to
/// avoid flapping on numeric tweaks; tighten as new offenders are closed.
const MAX_GEOMETRIC_MEAN_SPEEDUP: f64 = 90.0;

#[test]
fn per_op_impedance_gaps_within_budget() {
    let ops = cad_future_operations();
    let reports = audit(&ops);

    let lookup = |name: &str| -> f64 {
        PER_OP_BUDGETS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
            .unwrap_or(DEFAULT_MAX_GAP_ORDERS)
    };

    let mut violations: Vec<String> = Vec::new();
    for r in &reports {
        let budget = lookup(&r.operation);
        if r.gap_orders > budget + 1e-9 {
            violations.push(format!(
                "  {}: gap {:.2} orders > budget {:.2} orders (≈{:.0}× waste)",
                r.operation, r.gap_orders, budget, r.estimated_speedup
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "impedance gate: {} operation(s) exceeded per-op budgets:\n{}\n\
         If this regression is intentional, raise the budget in \
         crates/tests/integration/src/impedance_gate.rs.",
        violations.len(),
        violations.join("\n"),
    );
}

#[test]
fn aggregate_geometric_mean_speedup_within_budget() {
    let ops = cad_future_operations();
    let speedup = audit_total_speedup(&ops);
    assert!(
        speedup <= MAX_GEOMETRIC_MEAN_SPEEDUP,
        "impedance gate: aggregate geometric-mean speedup is {:.0}×, \
         exceeding the {:.0}× budget. Something in the registry regressed — \
         check `cargo run --example audit -p physical-impedance` for details.",
        speedup,
        MAX_GEOMETRIC_MEAN_SPEEDUP,
    );
}

#[test]
fn every_registered_op_appears_in_audit() {
    // Guards against silently dropping an operation from the registry and
    // letting it go unmeasured.
    let ops = cad_future_operations();
    let reports = audit(&ops);
    assert_eq!(
        reports.len(),
        ops.len(),
        "audit report count must match registered operations"
    );
    for op in &ops {
        assert!(
            reports.iter().any(|r| r.operation == op.name),
            "operation {} registered but missing from audit",
            op.name
        );
    }
}

#[test]
fn gate_catches_a_synthetic_regression() {
    // Sanity: if we hand the analyzer the worst-case coordinates
    // (L0 op at B4 sequential), it must report a gap that would blow past
    // any reasonable budget. A gate that never fails is a gate that
    // doesn't protect anything.
    let bad = Operation {
        name: "synthetic_worst_case".into(),
        primitive: Primitive::EmbeddingLookup,
        abstraction: AbstractionLevel::B4Software,
        topology: ConcurrencyTopology::D5Sequential,
        info_topology: InformationTopology::Bijective,
    };
    let report = analyze(&bad);
    assert!(
        report.gap_orders > DEFAULT_MAX_GAP_ORDERS,
        "synthetic L0-at-B4-sequential op should exceed the default budget \
         ({:.1} ≤ {:.1}) — the analyzer is broken or DEFAULT_MAX_GAP_ORDERS \
         has drifted too loose",
        report.gap_orders,
        DEFAULT_MAX_GAP_ORDERS,
    );
}

#[test]
fn closed_offenders_are_no_longer_top_of_audit() {
    // Explicit guard: the three caches we closed in the April-2026 sprint
    // must NOT appear in the top 3 of the sorted audit. If one does, the
    // cache has regressed (wrong coordinates or someone deleted the LUT).
    let ops = cad_future_operations();
    let reports = audit(&ops);
    let top_3: Vec<&str> = reports.iter().take(3).map(|r| r.operation.as_str()).collect();

    for closed in ["tessellate_face_uv_grid", "boolean_point_in_solid", "gen_bridge_render_depth"]
    {
        assert!(
            !top_3.contains(&closed),
            "{} is back in the top 3 offenders ({top_3:?}) — \
             its LUT/cache layer has regressed",
            closed
        );
    }
}
