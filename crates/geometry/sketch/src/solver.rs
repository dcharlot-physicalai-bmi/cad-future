//! Newton-Raphson constraint solver.
//!
//! Solves F(x) = 0 where F is the vector of constraint residuals
//! and x is the vector of entity parameters. Uses Jacobian + LU
//! factorization (manual implementation, no external dep).

use glam::DVec2;
use rayon::prelude::*;
use crate::entity::{SketchEntity, PointRef};
use crate::constraint::Constraint;
use crate::sketch::Sketch;

/// Result of constraint solving.
#[derive(Clone, Debug, PartialEq)]
pub enum SolveResult {
    /// All constraints satisfied.
    FullyConstrained,
    /// Under-constrained: DOF remaining.
    UnderConstrained(usize),
    /// Over-constrained (more equations than parameters).
    OverConstrained,
    /// Solver failed to converge.
    Failed,
}

const MAX_ITER: usize = 50;
const TOLERANCE: f64 = 1e-10;
const DAMPING: f64 = 1.0;

/// Solve the constraint system of a sketch in-place.
pub fn solve(sketch: &mut Sketch) -> SolveResult {
    let total_params = sketch.total_params();
    let total_equations = sketch.total_equations();

    if total_equations == 0 { return SolveResult::UnderConstrained(total_params); }
    if total_equations > total_params { return SolveResult::OverConstrained; }

    for _iter in 0..MAX_ITER {
        let x = sketch.collect_params();
        let residuals = evaluate_residuals(sketch, &x);

        // Check convergence
        let max_r = residuals.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
        if max_r < TOLERANCE {
            let dof = total_params - total_equations;
            return if dof == 0 { SolveResult::FullyConstrained }
                   else { SolveResult::UnderConstrained(dof) };
        }

        // Build Jacobian
        let jacobian = build_jacobian(sketch, &x, total_equations, total_params);

        // Solve J * delta = -residuals using least-squares (J^T J) delta = J^T (-r)
        let mut jtj = mat_mul_transpose(&jacobian, &jacobian, total_params, total_equations);
        let jtr = mat_vec_transpose(&jacobian, &residuals, total_params, total_equations);

        // Tikhonov regularization for under-constrained systems (J^T*J is singular)
        if total_equations < total_params {
            let lambda = 1e-8;
            for i in 0..total_params {
                jtj[i * total_params + i] += lambda;
            }
        }

        // Solve via Gauss elimination
        match solve_linear_system(&jtj, &jtr, total_params) {
            Some(delta) => {
                // Apply update with damping
                let mut new_x = x.clone();
                for i in 0..total_params {
                    new_x[i] += DAMPING * delta[i];
                }
                sketch.apply_params(&new_x);
            }
            None => return SolveResult::Failed,
        }
    }

    SolveResult::Failed
}

/// Get the point value from the parameter vector.
fn get_point(sketch: &Sketch, pref: &PointRef, params: &[f64]) -> DVec2 {
    let offset = sketch.param_offset(pref.entity);
    let entity = &sketch.entities[pref.entity];
    match entity {
        SketchEntity::Point { .. } => {
            DVec2::new(params[offset], params[offset + 1])
        }
        SketchEntity::Line { .. } => {
            let idx = offset + pref.point * 2;
            DVec2::new(params[idx], params[idx + 1])
        }
        SketchEntity::Circle { .. } => {
            DVec2::new(params[offset], params[offset + 1])
        }
        SketchEntity::Arc { .. } => {
            let idx = offset + pref.point * 2;
            DVec2::new(params[idx], params[idx + 1])
        }
    }
}

/// Evaluate all constraint residuals.
fn evaluate_residuals(sketch: &Sketch, params: &[f64]) -> Vec<f64> {
    let mut residuals = Vec::new();

    for constraint in &sketch.constraints {
        match constraint {
            Constraint::Fixed { point, x, y } => {
                let p = get_point(sketch, point, params);
                residuals.push(p.x - x);
                residuals.push(p.y - y);
            }
            Constraint::Coincident { a, b } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                residuals.push(pa.x - pb.x);
                residuals.push(pa.y - pb.y);
            }
            Constraint::Horizontal { a, b } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                residuals.push(pa.y - pb.y);
            }
            Constraint::Vertical { a, b } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                residuals.push(pa.x - pb.x);
            }
            Constraint::Distance { a, b, value } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                let dist_sq = (pa - pb).length_squared();
                residuals.push(dist_sq - value * value);
            }
            Constraint::LineLength { entity, value } => {
                let off = sketch.param_offset(*entity);
                let dx = params[off + 2] - params[off];
                let dy = params[off + 3] - params[off + 1];
                residuals.push(dx * dx + dy * dy - value * value);
            }
            Constraint::Radius { entity, value } => {
                let off = sketch.param_offset(*entity);
                match &sketch.entities[*entity] {
                    SketchEntity::Circle { .. } => {
                        residuals.push(params[off + 2] - value);
                    }
                    SketchEntity::Arc { .. } => {
                        residuals.push(params[off + 6] - value);
                    }
                    _ => residuals.push(0.0),
                }
            }
            Constraint::LineAngle { entity, angle } => {
                let off = sketch.param_offset(*entity);
                let dx = params[off + 2] - params[off];
                let dy = params[off + 3] - params[off + 1];
                let actual = dy.atan2(dx);
                residuals.push(actual - angle);
            }
            Constraint::Perpendicular { line_a, line_b } => {
                let oa = sketch.param_offset(*line_a);
                let ob = sketch.param_offset(*line_b);
                let da = DVec2::new(params[oa + 2] - params[oa], params[oa + 3] - params[oa + 1]);
                let db = DVec2::new(params[ob + 2] - params[ob], params[ob + 3] - params[ob + 1]);
                residuals.push(da.dot(db));
            }
            Constraint::Parallel { line_a, line_b } => {
                let oa = sketch.param_offset(*line_a);
                let ob = sketch.param_offset(*line_b);
                let da = DVec2::new(params[oa + 2] - params[oa], params[oa + 3] - params[oa + 1]);
                let db = DVec2::new(params[ob + 2] - params[ob], params[ob + 3] - params[ob + 1]);
                // Cross product = 0 for parallel
                residuals.push(da.x * db.y - da.y * db.x);
            }
            Constraint::PointOnCircle { point, circle } => {
                let p = get_point(sketch, point, params);
                let oc = sketch.param_offset(*circle);
                let cx = params[oc];
                let cy = params[oc + 1];
                let r = params[oc + 2];
                let dist_sq = (p.x - cx).powi(2) + (p.y - cy).powi(2);
                residuals.push(dist_sq - r * r);
            }
            Constraint::Equal { a, b } => {
                let la = sketch.entities[*a].length();
                let lb = sketch.entities[*b].length();
                residuals.push(la - lb);
            }
            Constraint::SymmetricX { a, b } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                residuals.push(pa.x - pb.x);
                residuals.push(pa.y + pb.y);
            }
            Constraint::SymmetricY { a, b } => {
                let pa = get_point(sketch, a, params);
                let pb = get_point(sketch, b, params);
                residuals.push(pa.x + pb.x);
                residuals.push(pa.y - pb.y);
            }
            Constraint::PointOnLine { point, line } => {
                let p = get_point(sketch, point, params);
                let ol = sketch.param_offset(*line);
                let ls = DVec2::new(params[ol], params[ol + 1]);
                let le = DVec2::new(params[ol + 2], params[ol + 3]);
                let d = le - ls;
                let v = p - ls;
                // Cross product (point-to-line distance)
                residuals.push(d.x * v.y - d.y * v.x);
            }
            Constraint::Midpoint { point, line } => {
                let p = get_point(sketch, point, params);
                let ol = sketch.param_offset(*line);
                let mx = (params[ol] + params[ol + 2]) / 2.0;
                let my = (params[ol + 1] + params[ol + 3]) / 2.0;
                residuals.push(p.x - mx);
                residuals.push(p.y - my);
            }
            Constraint::Tangent { entity_a, entity_b } => {
                // Line params: x1, y1, x2, y2
                let ol = sketch.param_offset(*entity_a);
                let x1 = params[ol];
                let y1 = params[ol + 1];
                let x2 = params[ol + 2];
                let y2 = params[ol + 3];
                // Circle/arc center and radius
                let oc = sketch.param_offset(*entity_b);
                let cx = params[oc];
                let cy = params[oc + 1];
                let r = match &sketch.entities[*entity_b] {
                    SketchEntity::Circle { .. } => params[oc + 2],
                    SketchEntity::Arc { .. } => params[oc + 6],
                    _ => 0.0,
                };
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt();
                // Signed distance from center to line minus radius
                let signed_dist = (-(dy) * (cx - x1) + dx * (cy - y1)) / len;
                residuals.push(signed_dist - r);
            }
        }
    }
    residuals
}

/// Build the Jacobian matrix using finite differences.
fn build_jacobian(sketch: &Sketch, params: &[f64], neq: usize, npar: usize) -> Vec<f64> {
    // Forward-difference Jacobian. Each column `j` perturbs a single parameter
    // and re-evaluates the residual vector — completely independent across
    // columns, which makes this a textbook D4 embarrassingly-parallel workload.
    // Previously ran as a serial loop (D5) with a fresh sketch clone per
    // column; we now parallelize via rayon and each task owns its own clone
    // (required because `Constraint::Equal` reads entity lengths from the
    // sketch's stored state, not from the params slice).
    let eps = 1e-7;
    let r0 = evaluate_residuals(sketch, params);

    // Per-column closure: clone the sketch, apply the perturbed params,
    // evaluate residuals, emit a length-`neq` Jacobian column.
    let columns: Vec<Vec<f64>> = (0..npar)
        .into_par_iter()
        .map(|j| {
            let mut perturbed = params.to_vec();
            perturbed[j] += eps;
            let mut temp = sketch.clone();
            temp.apply_params(&perturbed);
            let r1 = evaluate_residuals(&temp, &perturbed);
            let mut col = vec![0.0f64; neq];
            for i in 0..neq {
                col[i] = (r1[i] - r0[i]) / eps;
            }
            col
        })
        .collect();

    // Transpose columns into row-major `jac[i*npar + j]`.
    let mut jac = vec![0.0f64; neq * npar];
    for j in 0..npar {
        for i in 0..neq {
            jac[i * npar + j] = columns[j][i];
        }
    }
    jac
}

/// J^T * J (result is npar × npar).
///
/// Each row of the output is an independent dot-product column-sweep —
/// parallelize over rows so the matmul runs on D4 embarrassingly-parallel
/// hardware instead of a serial loop.
fn mat_mul_transpose(j: &[f64], _j2: &[f64], npar: usize, neq: usize) -> Vec<f64> {
    (0..npar)
        .into_par_iter()
        .flat_map_iter(|r| {
            let mut row = vec![0.0f64; npar];
            for c in 0..npar {
                let mut sum = 0.0;
                for k in 0..neq {
                    sum += j[k * npar + r] * j[k * npar + c];
                }
                row[c] = sum;
            }
            row.into_iter()
        })
        .collect()
}

/// J^T * vec (result is npar).
fn mat_vec_transpose(j: &[f64], v: &[f64], npar: usize, neq: usize) -> Vec<f64> {
    let mut result = vec![0.0; npar];
    for r in 0..npar {
        let mut sum = 0.0;
        for k in 0..neq {
            sum += j[k * npar + r] * (-v[k]);
        }
        result[r] = sum;
    }
    result
}

/// Solve A*x = b via Gaussian elimination with partial pivoting.
fn solve_linear_system(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut aug = vec![0.0; n * (n + 1)];
    for r in 0..n {
        for c in 0..n {
            aug[r * (n + 1) + c] = a[r * n + c];
        }
        aug[r * (n + 1) + n] = b[r];
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_val = aug[col * (n + 1) + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = aug[row * (n + 1) + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-14 { return None; } // Singular

        // Swap rows
        if max_row != col {
            for c in 0..=n {
                let tmp = aug[col * (n + 1) + c];
                aug[col * (n + 1) + c] = aug[max_row * (n + 1) + c];
                aug[max_row * (n + 1) + c] = tmp;
            }
        }

        // Eliminate below
        let pivot = aug[col * (n + 1) + col];
        for row in (col + 1)..n {
            let factor = aug[row * (n + 1) + col] / pivot;
            for c in col..=n {
                aug[row * (n + 1) + c] -= factor * aug[col * (n + 1) + c];
            }
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = aug[row * (n + 1) + n];
        for c in (row + 1)..n {
            sum -= aug[row * (n + 1) + c] * x[c];
        }
        let diag = aug[row * (n + 1) + row];
        if diag.abs() < 1e-14 { return None; }
        x[row] = sum / diag;
    }
    Some(x)
}

// ---------------------------------------------------------------------------
// Diagnostics — DOF counting, over-constraint detection, suggestions
// ---------------------------------------------------------------------------

/// Detailed solver result with diagnostics.
#[derive(Clone, Debug)]
pub struct SolverDiagnostics {
    pub result: SolveResult,
    pub iterations: usize,
    pub max_residual: f64,
    pub total_params: usize,
    pub total_equations: usize,
    pub dof_remaining: usize,
}

/// Sketch health status.
#[derive(Clone, Debug, PartialEq)]
pub enum SketchStatus {
    FullyConstrained,
    UnderConstrained { remaining_dof: usize },
    OverConstrained,
    Inconsistent,
}

/// A suggested auto-constraint.
#[derive(Clone, Debug)]
pub struct SuggestedConstraint {
    pub constraint: Constraint,
    pub confidence: f64,
    pub description: String,
}

/// Diagnose a sketch's constraint status without modifying it.
pub fn diagnose(sketch: &Sketch) -> (SketchStatus, usize) {
    let total_params = sketch.total_params();
    let total_equations = sketch.total_equations();

    if total_equations > total_params {
        (SketchStatus::OverConstrained, 0)
    } else if total_equations == total_params {
        (SketchStatus::FullyConstrained, 0)
    } else {
        let dof = total_params - total_equations;
        (SketchStatus::UnderConstrained { remaining_dof: dof }, dof)
    }
}

/// Solve with diagnostics — returns detailed information about the solve.
pub fn solve_with_diagnostics(sketch: &mut Sketch) -> SolverDiagnostics {
    let total_params = sketch.total_params();
    let total_equations = sketch.total_equations();

    if total_equations == 0 {
        return SolverDiagnostics {
            result: SolveResult::UnderConstrained(total_params),
            iterations: 0,
            max_residual: 0.0,
            total_params,
            total_equations,
            dof_remaining: total_params,
        };
    }
    if total_equations > total_params {
        return SolverDiagnostics {
            result: SolveResult::OverConstrained,
            iterations: 0,
            max_residual: f64::INFINITY,
            total_params,
            total_equations,
            dof_remaining: 0,
        };
    }

    let mut iterations = 0;
    let mut max_residual = f64::INFINITY;

    for iter in 0..MAX_ITER {
        iterations = iter + 1;
        let x = sketch.collect_params();
        let residuals = evaluate_residuals(sketch, &x);
        max_residual = residuals.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);

        if max_residual < TOLERANCE {
            let dof = total_params - total_equations;
            return SolverDiagnostics {
                result: if dof == 0 { SolveResult::FullyConstrained }
                        else { SolveResult::UnderConstrained(dof) },
                iterations,
                max_residual,
                total_params,
                total_equations,
                dof_remaining: dof,
            };
        }

        let jacobian = build_jacobian(sketch, &x, total_equations, total_params);
        let mut jtj = mat_mul_transpose(&jacobian, &jacobian, total_params, total_equations);
        let jtr = mat_vec_transpose(&jacobian, &residuals, total_params, total_equations);

        if total_equations < total_params {
            let lambda = 1e-8;
            for i in 0..total_params {
                jtj[i * total_params + i] += lambda;
            }
        }

        match solve_linear_system(&jtj, &jtr, total_params) {
            Some(delta) => {
                let mut new_x = x.clone();
                for i in 0..total_params {
                    new_x[i] += DAMPING * delta[i];
                }
                sketch.apply_params(&new_x);
            }
            None => {
                return SolverDiagnostics {
                    result: SolveResult::Failed,
                    iterations,
                    max_residual,
                    total_params,
                    total_equations,
                    dof_remaining: total_params.saturating_sub(total_equations),
                };
            }
        }
    }

    SolverDiagnostics {
        result: SolveResult::Failed,
        iterations,
        max_residual,
        total_params,
        total_equations,
        dof_remaining: total_params.saturating_sub(total_equations),
    }
}

/// Suggest auto-constraints based on geometric proximity.
pub fn suggest_constraints(sketch: &Sketch, tolerance: f64) -> Vec<SuggestedConstraint> {
    let mut suggestions = Vec::new();
    let params = sketch.collect_params();

    // Check for near-horizontal lines
    for (idx, entity) in sketch.entities.iter().enumerate() {
        if let SketchEntity::Line { start, end } = entity {
            let dy = (end.y - start.y).abs();
            let dx = (end.x - start.x).abs();
            if dy < tolerance && dx > tolerance {
                suggestions.push(SuggestedConstraint {
                    constraint: Constraint::Horizontal {
                        a: PointRef::new(idx, 0),
                        b: PointRef::new(idx, 1),
                    },
                    confidence: 1.0 - dy / tolerance,
                    description: format!("Line {} is nearly horizontal (Δy={:.4})", idx, dy),
                });
            }
            if dx < tolerance && dy > tolerance {
                suggestions.push(SuggestedConstraint {
                    constraint: Constraint::Vertical {
                        a: PointRef::new(idx, 0),
                        b: PointRef::new(idx, 1),
                    },
                    confidence: 1.0 - dx / tolerance,
                    description: format!("Line {} is nearly vertical (Δx={:.4})", idx, dx),
                });
            }
        }
    }

    // Check for near-coincident points
    let point_refs = sketch.all_point_refs();
    for i in 0..point_refs.len() {
        for j in (i + 1)..point_refs.len() {
            let pi = get_point(sketch, &point_refs[i], &params);
            let pj = get_point(sketch, &point_refs[j], &params);
            let dist = (pi - pj).length();
            if dist < tolerance && dist > 0.0 {
                suggestions.push(SuggestedConstraint {
                    constraint: Constraint::Coincident {
                        a: point_refs[i],
                        b: point_refs[j],
                    },
                    confidence: 1.0 - dist / tolerance,
                    description: format!(
                        "Points ({},{}) and ({},{}) are {:.4} apart",
                        point_refs[i].entity, point_refs[i].point,
                        point_refs[j].entity, point_refs[j].point,
                        dist
                    ),
                });
            }
        }
    }

    // Check for near-equal line lengths
    let line_indices: Vec<usize> = sketch.entities.iter().enumerate()
        .filter(|(_, e)| matches!(e, SketchEntity::Line { .. }))
        .map(|(i, _)| i)
        .collect();

    for i in 0..line_indices.len() {
        for j in (i + 1)..line_indices.len() {
            let la = sketch.entities[line_indices[i]].length();
            let lb = sketch.entities[line_indices[j]].length();
            if (la - lb).abs() < tolerance && la > tolerance {
                suggestions.push(SuggestedConstraint {
                    constraint: Constraint::Equal {
                        a: line_indices[i],
                        b: line_indices[j],
                    },
                    confidence: 1.0 - (la - lb).abs() / tolerance,
                    description: format!(
                        "Lines {} and {} have nearly equal lengths ({:.2} vs {:.2})",
                        line_indices[i], line_indices[j], la, lb
                    ),
                });
            }
        }
    }

    // Check for near-perpendicular lines
    for i in 0..line_indices.len() {
        for j in (i + 1)..line_indices.len() {
            let oi = sketch.param_offset(line_indices[i]);
            let oj = sketch.param_offset(line_indices[j]);
            let di = DVec2::new(params[oi + 2] - params[oi], params[oi + 3] - params[oi + 1]);
            let dj = DVec2::new(params[oj + 2] - params[oj], params[oj + 3] - params[oj + 1]);
            let dot = di.dot(dj);
            let li = di.length();
            let lj = dj.length();
            if li > 1e-10 && lj > 1e-10 {
                let cos_angle = dot / (li * lj);
                if cos_angle.abs() < tolerance / li.max(lj) {
                    suggestions.push(SuggestedConstraint {
                        constraint: Constraint::Perpendicular {
                            line_a: line_indices[i],
                            line_b: line_indices[j],
                        },
                        confidence: 1.0 - cos_angle.abs(),
                        description: format!(
                            "Lines {} and {} are nearly perpendicular",
                            line_indices[i], line_indices[j]
                        ),
                    });
                }
            }
        }
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::PointRef;
    use crate::sketch::Sketch;

    #[test]
    fn solve_linear_3x3() {
        // x + y + z = 6
        // 2x + y - z = 1
        // x - y + z = 2
        let a = vec![1.0, 1.0, 1.0, 2.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        let b = vec![6.0, 1.0, 2.0];
        let x = solve_linear_system(&a, &b, 3).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
        assert!((x[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn solve_fixed_point() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(1.0, 2.0));
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 5.0, y: 7.0,
        });
        let result = solve(&mut s);
        assert_eq!(result, SolveResult::FullyConstrained);
        let p = s.entities[0].get_point(0);
        assert!((p.x - 5.0).abs() < 1e-8);
        assert!((p.y - 7.0).abs() < 1e-8);
    }

    #[test]
    fn solve_coincident_points() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));
        s.add_entity(SketchEntity::point(3.0, 4.0));
        // Fix first point
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 0.0, y: 0.0,
        });
        // Coincident
        s.add_constraint(Constraint::Coincident {
            a: PointRef::new(0, 0), b: PointRef::new(1, 0),
        });
        let result = solve(&mut s);
        assert_eq!(result, SolveResult::FullyConstrained);
        let p1 = s.entities[1].get_point(0);
        assert!(p1.length() < 1e-8);
    }

    #[test]
    fn solve_horizontal_line() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 3.0));
        // Fix start
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 0.0, y: 0.0,
        });
        // Horizontal
        s.add_constraint(Constraint::Horizontal {
            a: PointRef::new(0, 0), b: PointRef::new(0, 1),
        });
        // Length
        s.add_constraint(Constraint::LineLength { entity: 0, value: 10.0 });
        let result = solve(&mut s);
        assert_eq!(result, SolveResult::FullyConstrained);

        if let SketchEntity::Line { start, end } = &s.entities[0] {
            assert!((start.y).abs() < 1e-8);
            assert!((end.y).abs() < 1e-8);
            assert!(((end.x - start.x).abs() - 10.0).abs() < 1e-6);
        }
    }

    #[test]
    fn solve_rectangle_dimensions() {
        let mut s = Sketch::new();
        // 4 lines forming a rectangle (approximate initial positions)
        s.add_entity(SketchEntity::line(0.0, 0.0, 8.0, 0.0));  // bottom
        s.add_entity(SketchEntity::line(8.0, 0.0, 8.0, 4.0));  // right
        s.add_entity(SketchEntity::line(8.0, 4.0, 0.0, 4.0));  // top
        s.add_entity(SketchEntity::line(0.0, 4.0, 0.0, 0.0));  // left

        // Fix bottom-left corner
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 0.0, y: 0.0,
        });
        // Connect corners
        s.add_constraint(Constraint::Coincident { a: PointRef::new(0, 1), b: PointRef::new(1, 0) });
        s.add_constraint(Constraint::Coincident { a: PointRef::new(1, 1), b: PointRef::new(2, 0) });
        s.add_constraint(Constraint::Coincident { a: PointRef::new(2, 1), b: PointRef::new(3, 0) });
        s.add_constraint(Constraint::Coincident { a: PointRef::new(3, 1), b: PointRef::new(0, 0) });
        // Horizontal/Vertical
        s.add_constraint(Constraint::Horizontal { a: PointRef::new(0, 0), b: PointRef::new(0, 1) });
        s.add_constraint(Constraint::Horizontal { a: PointRef::new(2, 0), b: PointRef::new(2, 1) });
        s.add_constraint(Constraint::Vertical { a: PointRef::new(1, 0), b: PointRef::new(1, 1) });
        s.add_constraint(Constraint::Vertical { a: PointRef::new(3, 0), b: PointRef::new(3, 1) });
        // Dimensions: width=10, height=5
        s.add_constraint(Constraint::LineLength { entity: 0, value: 10.0 });
        s.add_constraint(Constraint::LineLength { entity: 1, value: 5.0 });

        let result = solve(&mut s);
        assert_eq!(result, SolveResult::FullyConstrained);

        // Verify dimensions
        if let SketchEntity::Line { start, end } = &s.entities[0] {
            assert!((end.x - start.x - 10.0).abs() < 1e-6, "width: {}", end.x - start.x);
        }
        if let SketchEntity::Line { start, end } = &s.entities[1] {
            assert!((end.y - start.y - 5.0).abs() < 1e-6, "height: {}", end.y - start.y);
        }
    }

    #[test]
    fn under_constrained() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(1.0, 2.0));
        // No constraints → under-constrained with 2 DOF
        let result = solve(&mut s);
        assert_eq!(result, SolveResult::UnderConstrained(2));
    }

    #[test]
    fn tangent_line_circle() {
        // Circle at origin, radius 5. A horizontal line at y=5 is tangent.
        // Use helper points to pin line x-coordinates, then horizontal + tangent
        // determine the y-position.
        let mut sk = Sketch::new();
        let c = sk.add_entity(SketchEntity::circle(0.0, 0.0, 5.0));
        let l = sk.add_entity(SketchEntity::line(-10.0, 5.5, 10.0, 5.5)); // slightly off
        let p0 = sk.add_entity(SketchEntity::point(-10.0, 0.0));
        let p1 = sk.add_entity(SketchEntity::point(10.0, 0.0));

        // Fix circle center and radius (3 eqs)
        sk.add_constraint(Constraint::Fixed {
            point: PointRef::new(c, 0), x: 0.0, y: 0.0,
        });
        sk.add_constraint(Constraint::Radius { entity: c, value: 5.0 });

        // Fix helper points (4 eqs)
        sk.add_constraint(Constraint::Fixed {
            point: PointRef::new(p0, 0), x: -10.0, y: 0.0,
        });
        sk.add_constraint(Constraint::Fixed {
            point: PointRef::new(p1, 0), x: 10.0, y: 0.0,
        });

        // Vertical alignment: line start.x == p0.x, line end.x == p1.x (2 eqs)
        sk.add_constraint(Constraint::Vertical {
            a: PointRef::new(l, 0), b: PointRef::new(p0, 0),
        });
        sk.add_constraint(Constraint::Vertical {
            a: PointRef::new(l, 1), b: PointRef::new(p1, 0),
        });

        // Horizontal line + tangent (2 eqs)
        sk.add_constraint(Constraint::Horizontal {
            a: PointRef::new(l, 0), b: PointRef::new(l, 1),
        });
        sk.add_constraint(Constraint::Tangent { entity_a: l, entity_b: c });

        // Params: circle(3) + line(4) + p0(2) + p1(2) = 11
        // Eqs: 3 + 4 + 2 + 2 = 11 → fully constrained
        let result = solve(&mut sk);
        assert_eq!(result, SolveResult::FullyConstrained,
            "expected FullyConstrained, got {:?}", result);

        // Line should now be at y = +5 or y = -5 (tangent to circle of radius 5)
        let params = sk.entities[l].params();
        let y1 = params[1];
        let y2 = params[3];
        assert!((y1 - 5.0).abs() < 0.01 || (y1 + 5.0).abs() < 0.01, "y1={}", y1);
        assert!((y2 - 5.0).abs() < 0.01 || (y2 + 5.0).abs() < 0.01, "y2={}", y2);
    }

    // ---- Diagnostics tests ----

    #[test]
    fn diagnose_fully_constrained() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(1.0, 2.0));
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 5.0, y: 7.0,
        });
        let (status, dof) = diagnose(&s);
        assert_eq!(status, SketchStatus::FullyConstrained);
        assert_eq!(dof, 0);
    }

    #[test]
    fn diagnose_under_constrained() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));
        s.add_entity(SketchEntity::line(0.0, 0.0, 5.0, 0.0));
        // Only fix the point (2 params consumed out of 6 total)
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 0.0, y: 0.0,
        });
        let (status, dof) = diagnose(&s);
        assert!(matches!(status, SketchStatus::UnderConstrained { .. }));
        assert_eq!(dof, 4); // 6 params - 2 equations
    }

    #[test]
    fn diagnose_over_constrained() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0)); // 2 params
        // 3 equations on 2 params → over-constrained
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 0.0, y: 0.0,
        }); // 2 eqs
        s.add_constraint(Constraint::Distance {
            a: PointRef::new(0, 0), b: PointRef::new(0, 0), value: 0.0,
        }); // 1 eq
        let (status, _) = diagnose(&s);
        assert_eq!(status, SketchStatus::OverConstrained);
    }

    #[test]
    fn solve_with_diagnostics_basic() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(1.0, 2.0));
        s.add_constraint(Constraint::Fixed {
            point: PointRef::new(0, 0), x: 5.0, y: 7.0,
        });
        let diag = solve_with_diagnostics(&mut s);
        assert_eq!(diag.result, SolveResult::FullyConstrained);
        assert!(diag.iterations > 0);
        assert!(diag.max_residual < 1e-8);
        assert_eq!(diag.total_params, 2);
        assert_eq!(diag.total_equations, 2);
        assert_eq!(diag.dof_remaining, 0);
    }

    #[test]
    fn suggest_horizontal_line() {
        let mut s = Sketch::new();
        // Nearly horizontal line (Δy = 0.001)
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.001));
        let suggestions = suggest_constraints(&s, 0.01);
        assert!(
            suggestions.iter().any(|s| matches!(s.constraint, Constraint::Horizontal { .. })),
            "should suggest horizontal constraint"
        );
    }

    #[test]
    fn suggest_vertical_line() {
        let mut s = Sketch::new();
        // Nearly vertical line (Δx = 0.002)
        s.add_entity(SketchEntity::line(0.002, 0.0, 0.0, 10.0));
        let suggestions = suggest_constraints(&s, 0.01);
        assert!(
            suggestions.iter().any(|s| matches!(s.constraint, Constraint::Vertical { .. })),
            "should suggest vertical constraint"
        );
    }

    #[test]
    fn suggest_coincident_points() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(5.0, 5.0));
        s.add_entity(SketchEntity::point(5.001, 5.002));
        let suggestions = suggest_constraints(&s, 0.01);
        assert!(
            suggestions.iter().any(|s| matches!(s.constraint, Constraint::Coincident { .. })),
            "should suggest coincident constraint"
        );
    }

    #[test]
    fn suggest_equal_length() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));   // length 10
        s.add_entity(SketchEntity::line(0.0, 5.0, 10.001, 5.0));  // length ~10.001
        let suggestions = suggest_constraints(&s, 0.01);
        assert!(
            suggestions.iter().any(|s| matches!(s.constraint, Constraint::Equal { .. })),
            "should suggest equal length constraint"
        );
    }

    #[test]
    fn suggest_perpendicular() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));       // horizontal
        s.add_entity(SketchEntity::line(5.0, -5.0, 5.001, 5.0));     // nearly vertical
        let suggestions = suggest_constraints(&s, 0.01);
        assert!(
            suggestions.iter().any(|s| matches!(s.constraint, Constraint::Perpendicular { .. })),
            "should suggest perpendicular constraint"
        );
    }

    #[test]
    fn no_suggestions_when_nothing_is_close() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 5.0)); // diagonal
        s.add_entity(SketchEntity::line(20.0, 20.0, 30.0, 25.0)); // far away diagonal
        let suggestions = suggest_constraints(&s, 0.001); // very tight tolerance
        // Diagonals at ~26.6° — not horizontal, vertical, perpendicular, or parallel
        assert!(
            !suggestions.iter().any(|s| matches!(s.constraint, Constraint::Horizontal { .. })),
            "should NOT suggest horizontal"
        );
    }

    #[test]
    fn all_point_refs_count() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));    // 1 ref
        s.add_entity(SketchEntity::line(0.0, 0.0, 1.0, 1.0));  // 2 refs
        s.add_entity(SketchEntity::circle(5.0, 5.0, 3.0));      // 1 ref (center)
        let refs = s.all_point_refs();
        assert_eq!(refs.len(), 4);
    }

    #[test]
    fn perpendicular_constraint_solves() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));  // horizontal
        s.add_entity(SketchEntity::line(0.0, 0.0, 0.0, 8.0));   // vertical

        // Fix start of both
        s.add_constraint(Constraint::Fixed { point: PointRef::new(0, 0), x: 0.0, y: 0.0 });
        s.add_constraint(Constraint::Fixed { point: PointRef::new(1, 0), x: 0.0, y: 0.0 });
        // Fix lengths
        s.add_constraint(Constraint::LineLength { entity: 0, value: 10.0 });
        s.add_constraint(Constraint::LineLength { entity: 1, value: 8.0 });
        // Horizontal first line
        s.add_constraint(Constraint::Horizontal { a: PointRef::new(0, 0), b: PointRef::new(0, 1) });
        // Perpendicular
        s.add_constraint(Constraint::Perpendicular { line_a: 0, line_b: 1 });

        let result = solve(&mut s);
        assert_eq!(result, SolveResult::FullyConstrained);

        // Second line should be vertical
        if let SketchEntity::Line { start, end } = &s.entities[1] {
            assert!((end.x - start.x).abs() < 0.01, "should be vertical: dx={}", end.x - start.x);
        }
    }
}
