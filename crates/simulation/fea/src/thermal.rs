//! Steady-state heat conduction FEA solver.
//!
//! Assembles the global thermal conductivity matrix for a tetrahedral mesh,
//! applies thermal boundary conditions (fixed temperature, heat flux,
//! convection), and solves for nodal temperatures. Post-processes per-element
//! heat flux vectors.

use super::{FEAMesh, Tet4};
use glam::DVec3;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Thermal boundary condition.
#[derive(Debug, Clone, Copy)]
pub enum ThermalBC {
    /// Fixed temperature at a node (Dirichlet).
    FixedTemp(usize, f64),
    /// Heat flux into a node (Neumann, in watts).
    HeatFlux(usize, f64),
    /// Convection at a node: h * A * (T - T_ambient).
    /// Fields: node index, h*A coefficient, ambient temperature.
    Convection(usize, f64, f64),
}

/// Thermal analysis result.
#[derive(Debug, Clone)]
pub struct ThermalResult {
    /// Temperature at each node.
    pub temperatures: Vec<f64>,
    /// Per-element heat flux vectors.
    pub element_fluxes: Vec<DVec3>,
    /// Maximum temperature.
    pub max_temperature: f64,
    /// Minimum temperature.
    pub min_temperature: f64,
}

// ---------------------------------------------------------------------------
// Main solver
// ---------------------------------------------------------------------------

/// Solve steady-state heat conduction: K_thermal * T = Q.
///
/// `conductivity`: thermal conductivity in W/(m*K).
pub fn solve_thermal(
    mesh: &FEAMesh,
    conductivity: f64,
    bcs: &[ThermalBC],
) -> ThermalResult {
    let n = mesh.nodes.len();

    // Assemble global conductivity matrix (dense, n x n — 1 DOF per node)
    let mut k_global = vec![0.0f64; n * n];

    for elem in &mesh.elements {
        let (b_t, vol) = thermal_gradient_matrix(mesh, elem);
        let vol_abs = vol.abs();

        // ke = k * V * B_T^T * B_T  (4x4)
        // B_T is 3x4, so B_T^T * B_T is 4x4
        for i in 0..4 {
            for j in 0..4 {
                let mut sum = 0.0;
                for d in 0..3 {
                    sum += b_t[d * 4 + i] * b_t[d * 4 + j];
                }
                let gi = elem.nodes[i];
                let gj = elem.nodes[j];
                k_global[gi * n + gj] += conductivity * vol_abs * sum;
            }
        }
    }

    // Build load vector
    let mut q = vec![0.0f64; n];

    // Apply boundary conditions
    let penalty = 1e20 * conductivity.max(1.0);

    for bc in bcs {
        match *bc {
            ThermalBC::FixedTemp(node, temp) => {
                k_global[node * n + node] += penalty;
                q[node] += penalty * temp;
            }
            ThermalBC::HeatFlux(node, flux) => {
                q[node] += flux;
            }
            ThermalBC::Convection(node, h_a, t_ambient) => {
                k_global[node * n + node] += h_a;
                q[node] += h_a * t_ambient;
            }
        }
    }

    // Solve K * T = Q
    let temperatures = solve_linear_system(&mut k_global, &mut q, n);

    // Post-process: element heat flux vectors  q = -k * B_T * T_e
    let mut element_fluxes = Vec::with_capacity(mesh.elements.len());
    for elem in &mesh.elements {
        let (b_t, _vol) = thermal_gradient_matrix(mesh, elem);

        // Gather element temperatures
        let mut t_e = [0.0f64; 4];
        for i in 0..4 {
            t_e[i] = temperatures[elem.nodes[i]];
        }

        // flux_i = -k * sum_j(B_T[i][j] * T_e[j])  for i in 0..3
        let mut fx = 0.0;
        let mut fy = 0.0;
        let mut fz = 0.0;
        for j in 0..4 {
            fx += b_t[0 * 4 + j] * t_e[j];
            fy += b_t[1 * 4 + j] * t_e[j];
            fz += b_t[2 * 4 + j] * t_e[j];
        }
        element_fluxes.push(DVec3::new(
            -conductivity * fx,
            -conductivity * fy,
            -conductivity * fz,
        ));
    }

    let max_temperature = temperatures.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_temperature = temperatures.iter().cloned().fold(f64::INFINITY, f64::min);

    ThermalResult {
        temperatures,
        element_fluxes,
        max_temperature,
        min_temperature,
    }
}

// ---------------------------------------------------------------------------
// Element computations
// ---------------------------------------------------------------------------

/// Compute the thermal gradient matrix B_T (3x4) and volume for a Tet4 element.
///
/// B_T[i*4+j] = dN_j / dx_i  (shape function gradient of node j w.r.t. direction i).
/// Returns (B_T as 12 values in row-major order, signed volume).
fn thermal_gradient_matrix(mesh: &FEAMesh, elem: &Tet4) -> ([f64; 12], f64) {
    let p = [
        mesh.nodes[elem.nodes[0]].position,
        mesh.nodes[elem.nodes[1]].position,
        mesh.nodes[elem.nodes[2]].position,
        mesh.nodes[elem.nodes[3]].position,
    ];

    let vol = tet_volume_from_points(&p);
    let vol6 = 6.0 * vol;

    if vol6.abs() < 1e-30 {
        return ([0.0; 12], 0.0);
    }

    let mut b_t = [0.0f64; 12]; // 3 rows x 4 cols

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

        b_t[0 * 4 + i] = sign * n.x / vol6; // dN_i/dx
        b_t[1 * 4 + i] = sign * n.y / vol6; // dN_i/dy
        b_t[2 * 4 + i] = sign * n.z / vol6; // dN_i/dz
    }

    (b_t, vol)
}

/// Signed volume of a tetrahedron from four corner points.
fn tet_volume_from_points(p: &[DVec3; 4]) -> f64 {
    let a = p[1] - p[0];
    let b = p[2] - p[0];
    let c = p[3] - p[0];
    a.dot(b.cross(c)) / 6.0
}

// ---------------------------------------------------------------------------
// Linear solver (Gaussian elimination with partial pivoting)
// ---------------------------------------------------------------------------

/// Solve A * x = b via Gaussian elimination with partial pivoting.
///
/// `a` is an n x n matrix in row-major order (mutated in place).
/// `b` is the right-hand side vector of length n (mutated in place).
fn solve_linear_system(a: &mut [f64], b: &mut [f64], n: usize) -> Vec<f64> {
    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_val = a[col * n + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = a[row * n + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        // Swap rows
        if max_row != col {
            for j in 0..n {
                let tmp = a[col * n + j];
                a[col * n + j] = a[max_row * n + j];
                a[max_row * n + j] = tmp;
            }
            b.swap(col, max_row);
        }

        let pivot = a[col * n + col];
        if pivot.abs() < 1e-30 {
            continue;
        }

        // Eliminate below
        for row in (col + 1)..n {
            let factor = a[row * n + col] / pivot;
            for j in col..n {
                a[row * n + j] -= factor * a[col * n + j];
            }
            b[row] -= factor * b[col];
        }
    }

    // Back substitution
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for j in (i + 1)..n {
            sum -= a[i * n + j] * x[j];
        }
        let diag = a[i * n + i];
        x[i] = if diag.abs() > 1e-30 { sum / diag } else { 0.0 };
    }
    x
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Node;

    /// Helper: build a simple two-tet mesh along the x-axis.
    ///
    /// Five nodes arranged so that two tetrahedra share a triangular face:
    ///   Node 0: (0, 0, 0)
    ///   Node 1: (1, 0, 0)
    ///   Node 2: (0.5, 1, 0)
    ///   Node 3: (0.5, 0.5, 1)
    ///   Node 4: (2, 0, 0)
    ///
    /// Element 0: [0, 1, 2, 3]
    /// Element 1: [1, 4, 2, 3]
    fn two_tet_mesh() -> FEAMesh {
        let nodes = vec![
            Node { position: DVec3::new(0.0, 0.0, 0.0) },
            Node { position: DVec3::new(1.0, 0.0, 0.0) },
            Node { position: DVec3::new(0.5, 1.0, 0.0) },
            Node { position: DVec3::new(0.5, 0.5, 1.0) },
            Node { position: DVec3::new(2.0, 0.0, 0.0) },
        ];
        let elements = vec![
            Tet4 { nodes: [0, 1, 2, 3] },
            Tet4 { nodes: [1, 4, 2, 3] },
        ];
        FEAMesh { nodes, elements }
    }

    /// Helper: single tetrahedron mesh.
    fn single_tet_mesh() -> FEAMesh {
        let nodes = vec![
            Node { position: DVec3::new(0.0, 0.0, 0.0) },
            Node { position: DVec3::new(1.0, 0.0, 0.0) },
            Node { position: DVec3::new(0.0, 1.0, 0.0) },
            Node { position: DVec3::new(0.0, 0.0, 1.0) },
        ];
        let elements = vec![Tet4 { nodes: [0, 1, 2, 3] }];
        FEAMesh { nodes, elements }
    }

    #[test]
    fn thermal_uniform_temp() {
        // All nodes fixed at the same temperature -> zero flux everywhere.
        let mesh = single_tet_mesh();
        let bcs: Vec<ThermalBC> = (0..4)
            .map(|i| ThermalBC::FixedTemp(i, 100.0))
            .collect();

        let result = solve_thermal(&mesh, 50.0, &bcs);

        for &t in &result.temperatures {
            assert!(
                (t - 100.0).abs() < 1e-6,
                "expected uniform 100 K, got {}",
                t
            );
        }
        for flux in &result.element_fluxes {
            assert!(
                flux.length() < 1e-6,
                "expected zero flux, got {:?}",
                flux
            );
        }
    }

    #[test]
    fn thermal_gradient() {
        // Fix node 0 at T=0 (x=0) and node 4 at T=100 (x=2).
        // Interior nodes should have temperatures between 0 and 100.
        let mesh = two_tet_mesh();
        let bcs = vec![
            ThermalBC::FixedTemp(0, 0.0),
            ThermalBC::FixedTemp(4, 100.0),
        ];

        let result = solve_thermal(&mesh, 1.0, &bcs);

        assert!(
            (result.temperatures[0] - 0.0).abs() < 1e-3,
            "node 0 should be ~0, got {}",
            result.temperatures[0]
        );
        assert!(
            (result.temperatures[4] - 100.0).abs() < 1e-3,
            "node 4 should be ~100, got {}",
            result.temperatures[4]
        );

        // Interior nodes should be between 0 and 100
        for i in 1..4 {
            let t = result.temperatures[i];
            assert!(
                t > -1.0 && t < 101.0,
                "node {} temperature {} should be between 0 and 100",
                i,
                t
            );
        }
    }

    #[test]
    fn thermal_convection() {
        // Fix node 0 at 200 K, apply convection on node 4 (h*A=10, T_ambient=20).
        // Node 4 temperature should be between 20 and 200.
        let mesh = two_tet_mesh();
        let bcs = vec![
            ThermalBC::FixedTemp(0, 200.0),
            ThermalBC::Convection(4, 10.0, 20.0),
        ];

        let result = solve_thermal(&mesh, 1.0, &bcs);

        assert!(
            (result.temperatures[0] - 200.0).abs() < 1e-3,
            "fixed node should stay at 200"
        );
        let t4 = result.temperatures[4];
        assert!(
            t4 > 20.0 && t4 < 200.0,
            "convection node temperature {} should be between ambient (20) and fixed (200)",
            t4
        );
    }

    #[test]
    fn thermal_flux_direction() {
        // Heat flows from hot to cold: fix node 0 hot (T=500), node 4 cold (T=0).
        // Flux should generally point from hot toward cold (positive x direction
        // since hot is at x=0 and cold is at x=2: flux = -k * dT/dx, dT/dx < 0
        // means flux is in +x direction).
        let mesh = two_tet_mesh();
        let bcs = vec![
            ThermalBC::FixedTemp(0, 500.0),
            ThermalBC::FixedTemp(4, 0.0),
        ];

        let result = solve_thermal(&mesh, 10.0, &bcs);

        // At least one element should have a non-trivial flux with positive x component
        // (heat flowing from x=0 toward x=2).
        let has_positive_x_flux = result
            .element_fluxes
            .iter()
            .any(|f| f.x > 1.0);
        assert!(
            has_positive_x_flux,
            "expected heat flux in +x direction (hot at x=0 to cold at x=2), fluxes: {:?}",
            result.element_fluxes
        );
    }

    #[test]
    fn thermal_conservation() {
        // With only FixedTemp BCs, all node temperatures must be bounded by the
        // BC values (minimum principle for the Laplace equation).
        let mesh = two_tet_mesh();
        let t_cold = 10.0;
        let t_hot = 90.0;
        let bcs = vec![
            ThermalBC::FixedTemp(0, t_cold),
            ThermalBC::FixedTemp(4, t_hot),
        ];

        let result = solve_thermal(&mesh, 5.0, &bcs);

        for (i, &t) in result.temperatures.iter().enumerate() {
            assert!(
                t >= t_cold - 1e-6 && t <= t_hot + 1e-6,
                "node {} temperature {} violates min/max principle [{}, {}]",
                i,
                t,
                t_cold,
                t_hot
            );
        }
        assert!(
            (result.max_temperature - t_hot).abs() < 1e-3,
            "max temp should be {}",
            t_hot
        );
        assert!(
            (result.min_temperature - t_cold).abs() < 1e-3,
            "min temp should be {}",
            t_cold
        );
    }
}
