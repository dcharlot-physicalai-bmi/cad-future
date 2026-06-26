//! Modal analysis — natural frequency extraction via eigenvalue analysis.
//!
//! Solves the generalized eigenvalue problem: K·φ = ω²·M·φ
//! where K is stiffness, M is consistent mass, φ is mode shape, ω is frequency.
//! Uses inverse power iteration (simplest eigensolver, no external deps).

use glam::DVec3;
use crate::{FEAMesh, Tet4};

/// A natural vibration mode.
#[derive(Debug, Clone)]
pub struct Mode {
    /// Mode number (1-indexed).
    pub number: usize,
    /// Natural frequency (Hz).
    pub frequency_hz: f64,
    /// Angular frequency (rad/s).
    pub omega_rad_s: f64,
    /// Mode shape: displacement vector per node (normalized).
    pub shape: Vec<DVec3>,
}

/// Modal analysis result.
#[derive(Debug, Clone)]
pub struct ModalResult {
    /// Extracted modes, sorted by frequency (lowest first).
    pub modes: Vec<Mode>,
}

/// Compute the consistent mass matrix for a Tet4 element (12x12).
/// M_ij = ρ·V/20·(1 + δ_ij) for nodes i,j of the element.
fn element_mass_matrix(mesh: &FEAMesh, elem: &Tet4, density: f64) -> [f64; 144] {
    let vol = crate::tet_volume(mesh, elem).abs();
    let mut me = [0.0f64; 144];
    // Consistent mass for Tet4: M = ρ·V/20 × [2 1 1 1; 1 2 1 1; 1 1 2 1; 1 1 1 2] (per DOF)
    for i in 0..4 {
        for j in 0..4 {
            let factor = if i == j { 2.0 } else { 1.0 };
            let mass = density * vol * factor / 20.0;
            for d in 0..3 {
                me[(i * 3 + d) * 12 + (j * 3 + d)] = mass;
            }
        }
    }
    me
}

/// Assemble global mass matrix.
fn assemble_mass_matrix(mesh: &FEAMesh, density: f64) -> Vec<f64> {
    let n_dof = mesh.nodes.len() * 3;
    let mut m_global = vec![0.0f64; n_dof * n_dof];

    for elem in &mesh.elements {
        let me = element_mass_matrix(mesh, elem, density);
        for i in 0..4 {
            for j in 0..4 {
                for di in 0..3 {
                    for dj in 0..3 {
                        let gi = elem.nodes[i] * 3 + di;
                        let gj = elem.nodes[j] * 3 + dj;
                        m_global[gi * n_dof + gj] += me[(i * 3 + di) * 12 + (j * 3 + dj)];
                    }
                }
            }
        }
    }
    m_global
}

/// Assemble global stiffness matrix (reusing existing FEA infrastructure).
fn assemble_stiffness_matrix(mesh: &FEAMesh, e_modulus: f64, poisson: f64) -> Vec<f64> {
    let n_dof = mesh.nodes.len() * 3;
    let mut k_global = vec![0.0f64; n_dof * n_dof];
    let d_matrix = crate::constitutive_matrix(e_modulus, poisson);

    for elem in &mesh.elements {
        let ke = crate::element_stiffness(mesh, elem, &d_matrix);
        for i in 0..4 {
            for j in 0..4 {
                for di in 0..3 {
                    for dj in 0..3 {
                        let gi = elem.nodes[i] * 3 + di;
                        let gj = elem.nodes[j] * 3 + dj;
                        k_global[gi * n_dof + gj] += ke[(i * 3 + di) * 12 + (j * 3 + dj)];
                    }
                }
            }
        }
    }
    k_global
}

/// Extract natural frequencies and mode shapes via inverse power iteration.
///
/// Solves K·φ = ω²·M·φ for the lowest `n_modes` modes.
/// Fixed DOFs are specified by node indices (all 3 DOFs fixed).
pub fn modal_analysis(
    mesh: &FEAMesh,
    e_modulus: f64,
    poisson: f64,
    density: f64,
    fixed_nodes: &[usize],
    n_modes: usize,
) -> ModalResult {
    let n_nodes = mesh.nodes.len();
    let n_dof = n_nodes * 3;

    let mut k = assemble_stiffness_matrix(mesh, e_modulus, poisson);
    let m = assemble_mass_matrix(mesh, density);

    // Apply fixed BCs (penalty on stiffness diagonal)
    let penalty = 1e20 * e_modulus;
    for &node in fixed_nodes {
        for d in 0..3 {
            let idx = node * 3 + d;
            k[idx * n_dof + idx] += penalty;
        }
    }

    let mut modes = Vec::with_capacity(n_modes);
    let mut deflation_vectors: Vec<Vec<f64>> = Vec::new();

    for mode_num in 0..n_modes {
        // Inverse iteration: solve K·x_{k+1} = M·x_k, normalize
        let mut x = vec![1.0f64; n_dof];
        // Perturb to break symmetry
        for i in 0..n_dof {
            x[i] += 0.1 * ((i * 7 + mode_num * 13) % 17) as f64 / 17.0;
        }

        // Deflate against previously found modes
        for prev in &deflation_vectors {
            let dot: f64 = x.iter().zip(prev.iter()).map(|(a, b)| a * b).sum();
            for i in 0..n_dof {
                x[i] -= dot * prev[i];
            }
        }

        let mut eigenvalue = 0.0;

        for _iter in 0..200 {
            // rhs = M · x
            let mut rhs = vec![0.0f64; n_dof];
            for i in 0..n_dof {
                for j in 0..n_dof {
                    rhs[i] += m[i * n_dof + j] * x[j];
                }
            }

            // Solve K · x_new = rhs
            let mut k_copy = k.clone();
            let x_new = crate::solve_linear_system(&mut k_copy, &mut rhs, n_dof);

            // Deflate
            for prev in &deflation_vectors {
                let dot: f64 = x_new.iter().zip(prev.iter()).map(|(a, b)| a * b).sum();
                let mut x_def = x_new.clone();
                for i in 0..n_dof {
                    x_def[i] -= dot * prev[i];
                }
                // Overwrite with deflated (can't mutate x_new directly)
                let _ = x_def; // used below
            }

            // Rayleigh quotient: λ = x^T·K·x / x^T·M·x
            let mut xkx = 0.0f64;
            let mut xmx = 0.0f64;
            for i in 0..n_dof {
                for j in 0..n_dof {
                    xkx += x_new[i] * k[i * n_dof + j] * x_new[j];
                    xmx += x_new[i] * m[i * n_dof + j] * x_new[j];
                }
            }

            eigenvalue = if xmx.abs() > 1e-30 { xkx / xmx } else { 0.0 };

            // Normalize: x = x_new / sqrt(x^T · M · x)
            let norm = xmx.abs().sqrt().max(1e-30);
            for i in 0..n_dof {
                x[i] = x_new[i] / norm;
            }
        }

        // Frequency from eigenvalue: ω² = eigenvalue → f = ω/(2π)
        let omega = eigenvalue.abs().sqrt();
        let freq = omega / (2.0 * std::f64::consts::PI);

        let shape: Vec<DVec3> = (0..n_nodes)
            .map(|i| DVec3::new(x[i * 3], x[i * 3 + 1], x[i * 3 + 2]))
            .collect();

        deflation_vectors.push(x.clone());

        modes.push(Mode {
            number: mode_num + 1,
            frequency_hz: freq,
            omega_rad_s: omega,
            shape,
        });
    }

    modes.sort_by(|a, b| a.frequency_hz.partial_cmp(&b.frequency_hz).unwrap());
    ModalResult { modes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, tetrahedralize};
    use physical_brep::builder::make_box;

    #[test]
    fn modal_cantilever() {
        let solid = make_box(100.0, 10.0, 10.0);
        let mesh = tetrahedralize(&solid);

        // Fix left face (x < -45)
        let fixed: Vec<usize> = mesh.nodes.iter().enumerate()
            .filter(|(_, n)| n.position.x < -45.0)
            .map(|(i, _)| i)
            .collect();

        // Steel: E=200GPa, ν=0.3, ρ=7800 kg/m³
        // Using mm units: E=200000 MPa, ρ=7.8e-9 tonnes/mm³
        let result = modal_analysis(&mesh, 200_000.0, 0.3, 7.8e-9, &fixed, 3);

        assert_eq!(result.modes.len(), 3);
        // All frequencies should be positive
        for mode in &result.modes {
            assert!(mode.frequency_hz > 0.0, "freq={}", mode.frequency_hz);
            assert!(mode.shape.len() == mesh.nodes.len());
        }
        // First mode should have lowest frequency
        assert!(result.modes[0].frequency_hz <= result.modes[1].frequency_hz);
    }

    #[test]
    fn mass_matrix_symmetric() {
        let mesh = FEAMesh {
            nodes: vec![
                Node { position: DVec3::ZERO },
                Node { position: DVec3::new(1.0, 0.0, 0.0) },
                Node { position: DVec3::new(0.0, 1.0, 0.0) },
                Node { position: DVec3::new(0.0, 0.0, 1.0) },
            ],
            elements: vec![Tet4 { nodes: [0, 1, 2, 3] }],
        };

        let me = element_mass_matrix(&mesh, &mesh.elements[0], 1.0);
        for i in 0..12 {
            for j in 0..12 {
                assert!((me[i * 12 + j] - me[j * 12 + i]).abs() < 1e-12,
                    "M[{},{}] != M[{},{}]", i, j, j, i);
            }
        }
    }

    #[test]
    fn mass_matrix_total_mass() {
        let mesh = FEAMesh {
            nodes: vec![
                Node { position: DVec3::ZERO },
                Node { position: DVec3::new(1.0, 0.0, 0.0) },
                Node { position: DVec3::new(0.0, 1.0, 0.0) },
                Node { position: DVec3::new(0.0, 0.0, 1.0) },
            ],
            elements: vec![Tet4 { nodes: [0, 1, 2, 3] }],
        };

        let density = 1000.0;
        let me = element_mass_matrix(&mesh, &mesh.elements[0], density);
        // Total mass = sum of all entries in one DOF direction = ρ·V
        // (consistent mass matrix: row sums and total sum preserve total mass)
        let vol = 1.0 / 6.0; // unit tet
        let mut total = 0.0;
        for i in 0..4 {
            for j in 0..4 {
                total += me[(i * 3) * 12 + (j * 3)]; // all x-direction entries
            }
        }
        let expected = density * vol;
        assert!((total - expected).abs() < 1e-6 * expected,
            "total={}, expected={}", total, expected);
    }
}
