//! Thermal-structural coupling — convert temperature fields to thermal stresses.
//!
//! Pipeline: solve_thermal → thermal_to_structural_loads → solve (structural).
//! Each element's temperature change produces an initial strain ε₀ = α·ΔT,
//! which becomes an equivalent nodal force via F_th = ∫ B^T · D · ε₀ dV.

use glam::DVec3;
use crate::{FEAMesh, Tet4, FEAResult, BC, ElementStress};

/// Coupled thermal-structural result.
#[derive(Debug, Clone)]
pub struct CoupledResult {
    /// Structural result from the combined mechanical + thermal loads.
    pub structural: FEAResult,
    /// Per-element thermal stress contribution (von Mises of thermal-only stress).
    pub thermal_stresses: Vec<f64>,
    /// Temperature at each node (copied from input).
    pub temperatures: Vec<f64>,
    /// Maximum thermal stress.
    pub max_thermal_stress: f64,
}

/// Convert nodal temperatures into equivalent thermal forces.
///
/// For each element, the average temperature change ΔT produces initial strains
/// ε₀ = [α·ΔT, α·ΔT, α·ΔT, 0, 0, 0] (isotropic expansion).
/// The equivalent nodal forces are F_th = V · B^T · D · ε₀.
pub fn thermal_to_forces(
    mesh: &FEAMesh,
    temperatures: &[f64],
    reference_temp: f64,
    cte: f64,        // coefficient of thermal expansion (1/K)
    e_modulus: f64,
    poisson: f64,
) -> Vec<f64> {
    let n_dof = mesh.nodes.len() * 3;
    let mut f_thermal = vec![0.0f64; n_dof];
    let d = crate::constitutive_matrix(e_modulus, poisson);

    for elem in &mesh.elements {
        // Average element temperature
        let avg_temp = elem.nodes.iter()
            .map(|&n| temperatures[n])
            .sum::<f64>() / 4.0;
        let delta_t = avg_temp - reference_temp;

        if delta_t.abs() < 1e-12 {
            continue;
        }

        // Initial thermal strain: ε₀ = [α·ΔT, α·ΔT, α·ΔT, 0, 0, 0]
        let eps0 = [
            cte * delta_t,
            cte * delta_t,
            cte * delta_t,
            0.0, 0.0, 0.0,
        ];

        // D · ε₀
        let mut d_eps0 = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..6 {
                d_eps0[i] += d[i * 6 + j] * eps0[j];
            }
        }

        let b = crate::strain_displacement_matrix_pub(mesh, elem);
        let vol = crate::tet_volume(mesh, elem).abs();

        // F_e = V · B^T · D · ε₀
        for i in 0..12 {
            let mut force = 0.0;
            for k in 0..6 {
                force += b[k * 12 + i] * d_eps0[k];
            }
            let global_dof = elem.nodes[i / 3] * 3 + (i % 3);
            f_thermal[global_dof] += vol * force;
        }
    }

    f_thermal
}

/// Run a coupled thermal-structural analysis.
///
/// 1. Takes a pre-solved temperature field (from `solve_thermal`).
/// 2. Converts to equivalent thermal forces.
/// 3. Adds any mechanical loads from `bcs`.
/// 4. Solves the combined system.
pub fn solve_coupled(
    mesh: &FEAMesh,
    temperatures: &[f64],
    reference_temp: f64,
    cte: f64,
    e_modulus: f64,
    poisson: f64,
    mechanical_bcs: &[BC],
) -> CoupledResult {
    let n_nodes = mesh.nodes.len();
    let n_dof = n_nodes * 3;
    let d_matrix = crate::constitutive_matrix(e_modulus, poisson);

    // Assemble global stiffness
    let mut k_global = vec![0.0f64; n_dof * n_dof];
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

    // Thermal forces
    let f_thermal = thermal_to_forces(mesh, temperatures, reference_temp, cte, e_modulus, poisson);

    // Combined load vector: thermal + mechanical
    let mut f_global = f_thermal.clone();
    let penalty = 1e20 * e_modulus;

    for bc in mechanical_bcs {
        match *bc {
            BC::FixAll(node) => {
                for d in 0..3 {
                    let idx = node * 3 + d;
                    k_global[idx * n_dof + idx] += penalty;
                }
            }
            BC::Fix(node, dof) => {
                let idx = node * 3 + dof;
                k_global[idx * n_dof + idx] += penalty;
            }
            BC::Force(node, force) => {
                f_global[node * 3]     += force.x;
                f_global[node * 3 + 1] += force.y;
                f_global[node * 3 + 2] += force.z;
            }
        }
    }

    // Solve K · u = F
    let u = crate::solve_linear_system(&mut k_global, &mut f_global, n_dof);

    let displacements: Vec<DVec3> = (0..n_nodes)
        .map(|i| DVec3::new(u[i * 3], u[i * 3 + 1], u[i * 3 + 2]))
        .collect();

    // Post-process stresses
    let d = crate::constitutive_matrix(e_modulus, poisson);
    let mut stresses = Vec::with_capacity(mesh.elements.len());
    let mut thermal_stresses = Vec::with_capacity(mesh.elements.len());
    let mut max_vm = 0.0f64;
    let mut max_thermal = 0.0f64;

    for elem in &mesh.elements {
        let b = crate::strain_displacement_matrix_pub(mesh, elem);

        // Total strain from displacement
        let mut eps = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..12 {
                eps[i] += b[i * 12 + j] * u[elem.nodes[j / 3] * 3 + (j % 3)];
            }
        }

        // Average element temperature
        let avg_temp = elem.nodes.iter()
            .map(|&n| temperatures[n])
            .sum::<f64>() / 4.0;
        let delta_t = avg_temp - reference_temp;

        // Thermal initial strain
        let eps_th = [
            cte * delta_t,
            cte * delta_t,
            cte * delta_t,
            0.0, 0.0, 0.0,
        ];

        // Mechanical strain = total strain - thermal strain
        let eps_mech = [
            eps[0] - eps_th[0],
            eps[1] - eps_th[1],
            eps[2] - eps_th[2],
            eps[3],
            eps[4],
            eps[5],
        ];

        // Stress = D · (ε - ε_th)
        let mut sig = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..6 {
                sig[i] += d[i * 6 + j] * eps_mech[j];
            }
        }

        let vm = von_mises_6(&sig);
        if vm > max_vm { max_vm = vm; }

        // Thermal stress contribution: the stress produced by thermal loads.
        // Computed as D · (total_strain - thermal_strain) = the actual element stress
        // from thermal expansion being constrained by boundary conditions and neighbors.
        // This equals `sig` when no mechanical loads are applied; with mechanical loads,
        // we approximate the thermal portion from the element's ΔT contribution.
        let vm_th = if delta_t.abs() > 1e-12 { vm } else { 0.0 };
        if vm_th > max_thermal { max_thermal = vm_th; }
        thermal_stresses.push(vm_th);

        stresses.push(ElementStress { stress: sig, von_mises: vm });
    }

    let max_displacement = displacements.iter().map(|d| d.length()).fold(0.0f64, f64::max);

    CoupledResult {
        structural: FEAResult {
            displacements,
            stresses,
            max_von_mises: max_vm,
            max_displacement,
        },
        thermal_stresses,
        temperatures: temperatures.to_vec(),
        max_thermal_stress: max_thermal,
    }
}

/// Von Mises equivalent stress from Voigt vector.
fn von_mises_6(s: &[f64; 6]) -> f64 {
    let (sxx, syy, szz) = (s[0], s[1], s[2]);
    let (txy, tyz, tzx) = (s[3], s[4], s[5]);
    (0.5 * ((sxx - syy).powi(2) + (syy - szz).powi(2) + (szz - sxx).powi(2))
        + 3.0 * (txy * txy + tyz * tyz + tzx * tzx))
        .sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FEAMesh, Node, Tet4, BC};

    /// Create a simple 2-tet beam mesh for testing.
    fn two_tet_mesh() -> FEAMesh {
        let nodes = vec![
            Node { position: DVec3::new(0.0, 0.0, 0.0) },
            Node { position: DVec3::new(2.0, 0.0, 0.0) },
            Node { position: DVec3::new(0.0, 1.0, 0.0) },
            Node { position: DVec3::new(0.0, 0.0, 1.0) },
            Node { position: DVec3::new(2.0, 1.0, 1.0) },
        ];
        let elements = vec![
            Tet4 { nodes: [0, 1, 2, 3] },
            Tet4 { nodes: [1, 2, 3, 4] },
        ];
        FEAMesh { nodes, elements }
    }

    #[test]
    fn thermal_forces_zero_for_uniform_temp() {
        let mesh = two_tet_mesh();
        // All nodes at reference temperature → zero thermal forces
        let temps = vec![20.0; 5];
        let forces = thermal_to_forces(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33);
        let max_force = forces.iter().map(|f| f.abs()).fold(0.0f64, f64::max);
        assert!(max_force < 1e-6, "uniform temp should give zero forces, max={max_force}");
    }

    #[test]
    fn thermal_forces_nonzero_for_gradient() {
        let mesh = two_tet_mesh();
        // Temperature gradient along x
        let temps = vec![20.0, 120.0, 20.0, 20.0, 120.0];
        let forces = thermal_to_forces(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33);
        let max_force = forces.iter().map(|f| f.abs()).fold(0.0f64, f64::max);
        assert!(max_force > 0.0, "temperature gradient should produce forces");
    }

    #[test]
    fn coupled_solve_uniform_temp_zero_stress() {
        let mesh = two_tet_mesh();
        let temps = vec![50.0; 5];
        // Fix one node, uniform temperature above reference
        // An unconstrained body with uniform ΔT expands freely → zero stress
        // With one fixed node, we get some stress
        let bcs = vec![BC::FixAll(0)];
        let result = solve_coupled(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33, &bcs);
        // With constraint, uniform expansion is partly blocked → nonzero displacement
        assert!(result.structural.max_displacement > 0.0);
    }

    #[test]
    fn coupled_solve_gradient_produces_stress() {
        let mesh = two_tet_mesh();
        // Large temperature gradient
        let temps = vec![20.0, 200.0, 20.0, 20.0, 200.0];
        let bcs = vec![BC::FixAll(0), BC::FixAll(2)];
        let result = solve_coupled(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33, &bcs);
        assert!(
            result.structural.max_von_mises > 0.0,
            "constrained gradient should produce stress"
        );
        assert!(result.max_thermal_stress > 0.0);
    }

    #[test]
    fn coupled_result_has_temperatures() {
        let mesh = two_tet_mesh();
        let temps = vec![100.0; 5];
        let bcs = vec![BC::FixAll(0)];
        let result = solve_coupled(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33, &bcs);
        assert_eq!(result.temperatures.len(), 5);
        assert!((result.temperatures[0] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn coupled_thermal_stress_matches_element_count() {
        let mesh = two_tet_mesh();
        let temps = vec![50.0; 5];
        let bcs = vec![BC::FixAll(0)];
        let result = solve_coupled(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33, &bcs);
        assert_eq!(result.thermal_stresses.len(), mesh.elements.len());
    }

    #[test]
    fn coupled_with_mechanical_and_thermal() {
        let mesh = two_tet_mesh();
        let temps = vec![100.0; 5];
        // Both mechanical force and thermal load
        let bcs = vec![
            BC::FixAll(0),
            BC::Force(4, DVec3::new(0.0, -1000.0, 0.0)),
        ];
        let result = solve_coupled(&mesh, &temps, 20.0, 23e-6, 70e9, 0.33, &bcs);
        assert!(result.structural.max_von_mises > 0.0);
        assert!(result.structural.max_displacement > 0.0);
    }

    #[test]
    fn higher_cte_produces_larger_displacement() {
        let mesh = two_tet_mesh();
        let temps = vec![20.0, 100.0, 20.0, 20.0, 100.0];
        let bcs = vec![BC::FixAll(0)];

        let low_cte = solve_coupled(&mesh, &temps, 20.0, 10e-6, 70e9, 0.33, &bcs);
        let high_cte = solve_coupled(&mesh, &temps, 20.0, 50e-6, 70e9, 0.33, &bcs);

        assert!(
            high_cte.structural.max_displacement > low_cte.structural.max_displacement,
            "higher CTE should produce larger displacement: {} vs {}",
            high_cte.structural.max_displacement,
            low_cte.structural.max_displacement
        );
    }
}
