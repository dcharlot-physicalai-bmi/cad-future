//! Linear buckling analysis — eigenvalue buckling load estimation.
//!
//! Determines critical buckling loads via the geometric stiffness approach:
//! (K + λ·Kg)·φ = 0, where Kg is the stress stiffness matrix.
//! The smallest λ multiplier on the applied load gives the buckling safety factor.

use glam::DVec3;
use crate::{FEAMesh, Tet4, BC};

/// Buckling analysis result.
#[derive(Debug, Clone)]
pub struct BucklingResult {
    /// Critical load multipliers (eigenvalues), sorted ascending.
    /// The first value is the critical buckling load factor.
    /// Values > 1.0 mean the applied load is below buckling.
    pub load_multipliers: Vec<f64>,
    /// Corresponding buckled mode shapes.
    pub mode_shapes: Vec<Vec<DVec3>>,
    /// Whether the structure is stable under the applied load.
    pub is_stable: bool,
}

/// Compute the geometric stiffness matrix for a Tet4 element.
/// The geometric stiffness accounts for the effect of pre-stress on stability.
/// Kg is based on the initial stress state from the linear analysis.
fn element_geometric_stiffness(
    mesh: &FEAMesh,
    elem: &Tet4,
    stress: &[f64; 6], // [σxx, σyy, σzz, τxy, τyz, τzx]
) -> [f64; 144] {
    let vol = crate::tet_volume(mesh, elem).abs();
    let p = [
        mesh.nodes[elem.nodes[0]].position,
        mesh.nodes[elem.nodes[1]].position,
        mesh.nodes[elem.nodes[2]].position,
        mesh.nodes[elem.nodes[3]].position,
    ];

    // Shape function gradients (same as strain-displacement)
    let vol6 = 6.0 * crate::tet_volume_from_points(&p);
    if vol6.abs() < 1e-30 {
        return [0.0; 144];
    }

    let mut grad = [[0.0f64; 3]; 4];
    for i in 0..4 {
        let (j, k, l) = match i {
            0 => (1, 2, 3),
            1 => (0, 3, 2),
            2 => (0, 1, 3),
            _ => (0, 2, 1),
        };
        let a = p[k] - p[j];
        let b_vec = p[l] - p[j];
        let n = a.cross(b_vec);
        let sign = if (p[i] - p[j]).dot(n) > 0.0 { 1.0 } else { -1.0 };
        grad[i][0] = sign * n.x / vol6;
        grad[i][1] = sign * n.y / vol6;
        grad[i][2] = sign * n.z / vol6;
    }

    // Stress tensor as 3×3 matrix
    let s = [
        [stress[0], stress[3], stress[5]],
        [stress[3], stress[1], stress[4]],
        [stress[5], stress[4], stress[2]],
    ];

    // Geometric stiffness: Kg[i,j] = V × ∇N_i · σ · ∇N_j × I₃
    let mut kg = [0.0f64; 144];
    for i in 0..4 {
        for j in 0..4 {
            // grad_i^T · S · grad_j (scalar)
            let mut gsg = 0.0;
            for a in 0..3 {
                for b in 0..3 {
                    gsg += grad[i][a] * s[a][b] * grad[j][b];
                }
            }
            gsg *= vol;
            // Place on diagonal 3×3 block
            for d in 0..3 {
                kg[(i * 3 + d) * 12 + (j * 3 + d)] = gsg;
            }
        }
    }
    kg
}

/// Perform linear buckling analysis.
///
/// 1. Run a linear static analysis to get the stress state.
/// 2. Build the geometric stiffness matrix from those stresses.
/// 3. Solve the eigenvalue problem (K + λ·Kg)·φ = 0.
///
/// Returns the critical load multiplier: if > 1, the structure doesn't buckle
/// under the applied load.
pub fn buckling_analysis(
    mesh: &FEAMesh,
    e_modulus: f64,
    poisson: f64,
    bcs: &[BC],
    n_modes: usize,
) -> BucklingResult {
    // Step 1: Linear static solve
    let fea_result = crate::solve(mesh, e_modulus, poisson, bcs);
    let n_nodes = mesh.nodes.len();
    let n_dof = n_nodes * 3;

    // Step 2: Assemble global stiffness and geometric stiffness
    let d_matrix = crate::constitutive_matrix(e_modulus, poisson);
    let mut k_global = vec![0.0f64; n_dof * n_dof];
    let mut kg_global = vec![0.0f64; n_dof * n_dof];

    for (idx, elem) in mesh.elements.iter().enumerate() {
        let ke = crate::element_stiffness(mesh, elem, &d_matrix);
        let stress = fea_result.stresses[idx].stress;
        let kge = element_geometric_stiffness(mesh, elem, &stress);

        for i in 0..4 {
            for j in 0..4 {
                for di in 0..3 {
                    for dj in 0..3 {
                        let gi = elem.nodes[i] * 3 + di;
                        let gj = elem.nodes[j] * 3 + dj;
                        k_global[gi * n_dof + gj] += ke[(i * 3 + di) * 12 + (j * 3 + dj)];
                        kg_global[gi * n_dof + gj] += kge[(i * 3 + di) * 12 + (j * 3 + dj)];
                    }
                }
            }
        }
    }

    // Apply BCs (penalty on stiffness diagonal)
    let penalty = 1e20 * e_modulus;
    let mut fixed_dofs = vec![false; n_dof];
    for bc in bcs {
        match *bc {
            BC::FixAll(node) => {
                for d in 0..3 { fixed_dofs[node * 3 + d] = true; }
            }
            BC::Fix(node, dof) => { fixed_dofs[node * 3 + dof] = true; }
            _ => {}
        }
    }
    for i in 0..n_dof {
        if fixed_dofs[i] {
            k_global[i * n_dof + i] += penalty;
            kg_global[i * n_dof + i] = 0.0; // no geometric stiffness on fixed DOFs
        }
    }

    // Step 3: Inverse iteration for eigenvalue of K^-1 · Kg
    // Solves K·x = Kg·x_prev → eigenvalue λ of (K + λ·Kg) = 0
    let mut load_multipliers = Vec::new();
    let mut mode_shapes = Vec::new();

    for mode_num in 0..n_modes {
        let mut x = vec![0.0f64; n_dof];
        for i in 0..n_dof {
            x[i] = 1.0 + 0.1 * ((i * 7 + mode_num * 13) % 17) as f64 / 17.0;
        }

        let mut eigenvalue = 0.0;

        for _iter in 0..100 {
            // rhs = Kg · x
            let mut rhs = vec![0.0f64; n_dof];
            for i in 0..n_dof {
                for j in 0..n_dof {
                    rhs[i] += kg_global[i * n_dof + j] * x[j];
                }
            }

            // Solve K · x_new = rhs
            let mut k_copy = k_global.clone();
            let x_new = crate::solve_linear_system(&mut k_copy, &mut rhs, n_dof);

            // Rayleigh quotient: λ = x^T·Kg·x / x^T·K^-1·Kg·x = x^T·rhs / x^T·x_new
            let num: f64 = x.iter().zip(rhs.iter()).map(|(a, b)| a * b).sum();
            let den: f64 = x.iter().zip(x_new.iter()).map(|(a, b)| a * b).sum();
            eigenvalue = if den.abs() > 1e-30 { num / den } else { f64::MAX };

            // Normalize
            let norm: f64 = x_new.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-30);
            for i in 0..n_dof {
                x[i] = x_new[i] / norm;
            }
        }

        // The buckling load multiplier = 1/eigenvalue (since we solved K^-1·Kg)
        let multiplier = if eigenvalue.abs() > 1e-30 { 1.0 / eigenvalue } else { f64::MAX };

        let shape: Vec<DVec3> = (0..n_nodes)
            .map(|i| DVec3::new(x[i * 3], x[i * 3 + 1], x[i * 3 + 2]))
            .collect();

        load_multipliers.push(multiplier.abs());
        mode_shapes.push(shape);
    }

    // Sort by load multiplier
    let mut indices: Vec<usize> = (0..n_modes).collect();
    indices.sort_by(|&a, &b| load_multipliers[a].partial_cmp(&load_multipliers[b]).unwrap());

    let sorted_multipliers: Vec<f64> = indices.iter().map(|&i| load_multipliers[i]).collect();
    let sorted_shapes: Vec<Vec<DVec3>> = indices.iter().map(|&i| mode_shapes[i].clone()).collect();

    let is_stable = sorted_multipliers.first().map_or(true, |&m| m > 1.0);

    BucklingResult {
        load_multipliers: sorted_multipliers,
        mode_shapes: sorted_shapes,
        is_stable,
    }
}

/// Quick Euler column buckling estimate (analytical, no FEA needed).
/// Returns critical load in the same force units as E and area.
pub fn euler_column_buckling(
    e_modulus: f64,
    moment_of_inertia: f64,
    length: f64,
    end_condition: EndCondition,
) -> f64 {
    let k = match end_condition {
        EndCondition::PinnedPinned => 1.0,
        EndCondition::FixedFree => 0.25,
        EndCondition::FixedPinned => 2.046,
        EndCondition::FixedFixed => 4.0,
    };
    k * std::f64::consts::PI.powi(2) * e_modulus * moment_of_inertia / (length * length)
}

/// Column end conditions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndCondition {
    PinnedPinned,
    FixedFree,
    FixedPinned,
    FixedFixed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::builder::make_box;
    use crate::tetrahedralize;

    #[test]
    fn euler_column_known() {
        // Steel column: E=200GPa, I=100cm^4, L=2m, pinned-pinned
        // P_cr = π²·EI/L² = π²·200e3·100e4/2000² = 493,480 N
        let pcr = euler_column_buckling(200_000.0, 1_000_000.0, 2000.0, EndCondition::PinnedPinned);
        let expected = std::f64::consts::PI.powi(2) * 200_000.0 * 1_000_000.0 / (2000.0 * 2000.0);
        assert!((pcr - expected).abs() < 1.0, "pcr={}", pcr);
    }

    #[test]
    fn euler_fixed_free_weakest() {
        let e = 200_000.0;
        let i = 1_000_000.0;
        let l = 1000.0;
        let pp = euler_column_buckling(e, i, l, EndCondition::PinnedPinned);
        let ff = euler_column_buckling(e, i, l, EndCondition::FixedFree);
        let fp = euler_column_buckling(e, i, l, EndCondition::FixedPinned);
        let fxfx = euler_column_buckling(e, i, l, EndCondition::FixedFixed);

        assert!(ff < pp, "fixed-free should be weakest");
        assert!(pp < fp, "pinned-pinned < fixed-pinned");
        assert!(fp < fxfx, "fixed-pinned < fixed-fixed");
    }

    #[test]
    fn buckling_analysis_runs() {
        let solid = make_box(5.0, 50.0, 5.0); // tall thin column
        let mesh = tetrahedralize(&solid);

        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.y < -20.0 {
                bcs.push(BC::FixAll(i));
            } else if node.position.y > 20.0 {
                bcs.push(BC::Force(i, DVec3::new(0.0, -10.0, 0.0)));
            }
        }

        let result = buckling_analysis(&mesh, 200_000.0, 0.3, &bcs, 1);
        assert_eq!(result.load_multipliers.len(), 1);
        assert!(result.load_multipliers[0] > 0.0);
        assert_eq!(result.mode_shapes[0].len(), mesh.nodes.len());
    }
}
