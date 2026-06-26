//! `physical-fea` — Finite Element Analysis simulation crate.
//!
//! Provides a complete on-device FEA pipeline for tetrahedral solid meshes:
//!
//! - **Static structural** solve: linear elastic Tet4 elements, stiffness assembly,
//!   Gaussian elimination, von Mises stress post-processing.
//! - **Modal analysis** ([`modal`]): natural frequency extraction via inverse power
//!   iteration on the generalized eigenvalue problem K·φ = ω²·M·φ.
//! - **Thermal analysis** ([`thermal`]): steady-state heat conduction with fixed-
//!   temperature, heat-flux, and convection boundary conditions.
//! - **Fatigue life** ([`fatigue`]): S-N curve analysis with Goodman/Soderberg/Gerber
//!   mean-stress correction and Miner's rule damage accumulation.
//! - **Buckling** ([`buckling`]): linear eigenvalue buckling via geometric stiffness
//!   and the Euler closed-form column formula.
//!
//! All solvers are pure Rust with no external linear-algebra dependencies; the only
//! runtime dependency is `glam` for 3-D vector math.

use glam::DVec3;

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

pub mod modal;
pub mod thermal;
pub mod fatigue;
pub mod buckling;
pub mod sparse;
pub mod coupled;

// ---------------------------------------------------------------------------
// Re-exports — key types surfaced at the crate root
// ---------------------------------------------------------------------------

// Modal
pub use modal::{modal_analysis, Mode, ModalResult};

// Thermal
pub use thermal::{solve_thermal, ThermalBC, ThermalResult};

// Fatigue
pub use fatigue::{
    miners_rule_damage, marin_surface_factor, marin_size_factor,
    MeanStressCorrection, SnCurve, SurfaceFinish,
};

// Buckling
pub use buckling::{
    buckling_analysis, euler_column_buckling,
    BucklingResult, EndCondition,
};

// Coupled thermal-structural
pub use coupled::{solve_coupled, thermal_to_forces, CoupledResult};

// Sparse
pub use sparse::{
    solve_sparse, SparseMatrix, pcg_solve,
    JacobiPreconditioner, IncompleteCholeskyPreconditioner,
    SPARSE_DOF_THRESHOLD,
};

// ---------------------------------------------------------------------------
// Core mesh types (shared by all sub-modules via `crate::`)
// ---------------------------------------------------------------------------

/// A single mesh node with a 3-D position.
#[derive(Debug, Clone)]
pub struct Node {
    pub position: DVec3,
}

/// A linear tetrahedral element with four corner node indices.
#[derive(Debug, Clone)]
pub struct Tet4 {
    pub nodes: [usize; 4],
}

/// FEA mesh: nodes and Tet4 elements.
#[derive(Debug, Clone)]
pub struct FEAMesh {
    pub nodes: Vec<Node>,
    pub elements: Vec<Tet4>,
}

// ---------------------------------------------------------------------------
// Boundary condition types
// ---------------------------------------------------------------------------

/// Mechanical boundary condition.
#[derive(Debug, Clone, Copy)]
pub enum BC {
    /// Fix all three DOFs of a node.
    FixAll(usize),
    /// Fix a single DOF of a node (0=x, 1=y, 2=z).
    Fix(usize, usize),
    /// Apply a concentrated force at a node.
    Force(usize, DVec3),
}

// ---------------------------------------------------------------------------
// Static structural solve
// ---------------------------------------------------------------------------

/// Per-element stress result.
#[derive(Debug, Clone)]
pub struct ElementStress {
    /// Voigt stress vector [σxx, σyy, σzz, τxy, τyz, τzx] in load units / area.
    pub stress: [f64; 6],
    /// von Mises equivalent stress.
    pub von_mises: f64,
}

/// Result of a linear-static FEA solve.
#[derive(Debug, Clone)]
pub struct FEAResult {
    /// Displacement vector per node.
    pub displacements: Vec<DVec3>,
    /// Per-element stresses.
    pub stresses: Vec<ElementStress>,
    /// Maximum von Mises stress across all elements.
    pub max_von_mises: f64,
    /// Maximum displacement magnitude across all nodes.
    pub max_displacement: f64,
}

/// Run a linear-static structural solve on the mesh.
///
/// Dispatches automatically: small problems (≤ `SPARSE_DOF_THRESHOLD` DOFs)
/// use the dense Gaussian solver; larger problems switch to sparse CSR +
/// rayon-parallel assembly + PCG, which turns this from O(n³) dense into
/// O(nnz·iterations) sparse on a natural D3Graph concurrency topology.
pub fn solve(
    mesh: &FEAMesh,
    e_modulus: f64,
    poisson: f64,
    bcs: &[BC],
) -> FEAResult {
    let n_nodes = mesh.nodes.len();
    let n_dof = n_nodes * 3;

    // Route large problems through the sparse PCG path — closes the
    // 5-order impedance gap that the dense path pays.
    if n_dof > SPARSE_DOF_THRESHOLD {
        return solve_sparse(mesh, e_modulus, poisson, bcs);
    }

    let d_matrix = constitutive_matrix(e_modulus, poisson);

    // Assemble global stiffness
    let mut k_global = vec![0.0f64; n_dof * n_dof];
    for elem in &mesh.elements {
        let ke = element_stiffness(mesh, elem, &d_matrix);
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

    // Build load vector and apply BCs
    let mut f_global = vec![0.0f64; n_dof];
    let penalty = 1e20 * e_modulus;

    for bc in bcs {
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
    let u = solve_linear_system(&mut k_global, &mut f_global, n_dof);

    let displacements: Vec<DVec3> = (0..n_nodes)
        .map(|i| DVec3::new(u[i * 3], u[i * 3 + 1], u[i * 3 + 2]))
        .collect();

    // Post-process stresses
    let d = constitutive_matrix(e_modulus, poisson);
    let mut stresses = Vec::with_capacity(mesh.elements.len());
    let mut max_vm = 0.0f64;

    for elem in &mesh.elements {
        let b = strain_displacement_matrix(mesh, elem);

        // ε = B · u_e  (6 components)
        let mut eps = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..12 {
                eps[i] += b[i * 12 + j] * u[elem.nodes[j / 3] * 3 + (j % 3)];
            }
        }

        // σ = D · ε
        let mut sig = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..6 {
                sig[i] += d[i * 6 + j] * eps[j];
            }
        }

        let vm = von_mises(&sig);
        if vm > max_vm { max_vm = vm; }
        stresses.push(ElementStress { stress: sig, von_mises: vm });
    }

    let max_displacement = displacements.iter().map(|d| d.length()).fold(0.0f64, f64::max);

    FEAResult { displacements, stresses, max_von_mises: max_vm, max_displacement }
}

// ---------------------------------------------------------------------------
// Shared element routines (used by sub-modules via `crate::`)
// ---------------------------------------------------------------------------

/// Isotropic linear-elastic constitutive matrix D (6×6, Voigt notation).
pub(crate) fn constitutive_matrix(e: f64, nu: f64) -> [f64; 36] {
    let c = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
    let a = c * (1.0 - nu);
    let b = c * nu;
    let g = c * (0.5 - nu);
    [
        a, b, b, 0., 0., 0.,
        b, a, b, 0., 0., 0.,
        b, b, a, 0., 0., 0.,
        0., 0., 0., g, 0., 0.,
        0., 0., 0., 0., g, 0.,
        0., 0., 0., 0., 0., g,
    ]
}

/// Strain-displacement matrix B (6×12) for a Tet4 element (crate-public wrapper).
pub(crate) fn strain_displacement_matrix_pub(mesh: &FEAMesh, elem: &Tet4) -> [f64; 72] {
    strain_displacement_matrix(mesh, elem)
}

/// Strain-displacement matrix B (6×12) for a Tet4 element.
fn strain_displacement_matrix(mesh: &FEAMesh, elem: &Tet4) -> [f64; 72] {
    let p = [
        mesh.nodes[elem.nodes[0]].position,
        mesh.nodes[elem.nodes[1]].position,
        mesh.nodes[elem.nodes[2]].position,
        mesh.nodes[elem.nodes[3]].position,
    ];

    let vol6 = 6.0 * tet_volume_from_points(&p);
    let mut b = [0.0f64; 72];

    if vol6.abs() < 1e-30 {
        return b;
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
        let bv = p[l] - p[j];
        let n = a.cross(bv);
        let sign = if (p[i] - p[j]).dot(n) > 0.0 { 1.0 } else { -1.0 };
        grad[i][0] = sign * n.x / vol6;
        grad[i][1] = sign * n.y / vol6;
        grad[i][2] = sign * n.z / vol6;
    }

    // B matrix rows: εxx, εyy, εzz, γxy, γyz, γzx
    for i in 0..4 {
        let col = i * 3;
        b[0 * 12 + col]     = grad[i][0]; // dNi/dx
        b[1 * 12 + col + 1] = grad[i][1]; // dNi/dy
        b[2 * 12 + col + 2] = grad[i][2]; // dNi/dz
        b[3 * 12 + col]     = grad[i][1]; // γxy: dNi/dy
        b[3 * 12 + col + 1] = grad[i][0]; // γxy: dNi/dx
        b[4 * 12 + col + 1] = grad[i][2]; // γyz: dNi/dz
        b[4 * 12 + col + 2] = grad[i][1]; // γyz: dNi/dy
        b[5 * 12 + col]     = grad[i][2]; // γzx: dNi/dz
        b[5 * 12 + col + 2] = grad[i][0]; // γzx: dNi/dx
    }

    b
}

/// Element stiffness matrix Ke = ∫ B^T D B dV (12×12).
pub(crate) fn element_stiffness(mesh: &FEAMesh, elem: &Tet4, d: &[f64; 36]) -> [f64; 144] {
    let b = strain_displacement_matrix(mesh, elem);
    let vol = tet_volume(mesh, elem).abs();
    let mut ke = [0.0f64; 144];

    // Ke = V × B^T · D · B
    for i in 0..12 {
        for k in 0..6 {
            let bki = b[k * 12 + i];
            if bki == 0.0 { continue; }
            for j in 0..12 {
                let mut db = 0.0;
                for l in 0..6 {
                    db += d[k * 6 + l] * b[l * 12 + j];
                }
                ke[i * 12 + j] += vol * bki * db;
            }
        }
    }

    ke
}

/// Signed volume of a Tet4 element (uses node positions from mesh).
pub(crate) fn tet_volume(mesh: &FEAMesh, elem: &Tet4) -> f64 {
    let p = [
        mesh.nodes[elem.nodes[0]].position,
        mesh.nodes[elem.nodes[1]].position,
        mesh.nodes[elem.nodes[2]].position,
        mesh.nodes[elem.nodes[3]].position,
    ];
    tet_volume_from_points(&p)
}

/// Signed volume from four corner points.
pub(crate) fn tet_volume_from_points(p: &[DVec3; 4]) -> f64 {
    let a = p[1] - p[0];
    let b = p[2] - p[0];
    let c = p[3] - p[0];
    a.dot(b.cross(c)) / 6.0
}

/// Gaussian elimination with partial pivoting. Mutates `a` and `b` in place.
pub(crate) fn solve_linear_system(a: &mut [f64], b: &mut [f64], n: usize) -> Vec<f64> {
    for col in 0..n {
        // Partial pivot
        let mut max_val = a[col * n + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let v = a[row * n + col].abs();
            if v > max_val { max_val = v; max_row = row; }
        }
        if max_row != col {
            for j in 0..n {
                let tmp = a[col * n + j];
                a[col * n + j] = a[max_row * n + j];
                a[max_row * n + j] = tmp;
            }
            b.swap(col, max_row);
        }

        let pivot = a[col * n + col];
        if pivot.abs() < 1e-30 { continue; }

        for row in (col + 1)..n {
            let factor = a[row * n + col] / pivot;
            for j in col..n {
                a[row * n + j] -= factor * a[col * n + j];
            }
            b[row] -= factor * b[col];
        }
    }

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

/// Von Mises equivalent stress from Voigt stress vector [σxx,σyy,σzz,τxy,τyz,τzx].
fn von_mises(s: &[f64; 6]) -> f64 {
    let (sxx, syy, szz) = (s[0], s[1], s[2]);
    let (txy, tyz, tzx) = (s[3], s[4], s[5]);
    (0.5 * ((sxx - syy).powi(2) + (syy - szz).powi(2) + (szz - sxx).powi(2))
        + 3.0 * (txy * txy + tyz * tyz + tzx * tzx))
        .sqrt()
}

// ---------------------------------------------------------------------------
// Mesh generation (simple tetrahedral subdivision of a B-rep box)
// ---------------------------------------------------------------------------

/// Tetrahedralize a `physical_brep` solid into an `FEAMesh`.
///
/// Uses a uniform grid subdivision and splits each hexahedral cell into 5 or 6
/// tetrahedra. Suitable for convex box-like geometries; complex B-rep shapes
/// should use an external mesher.
pub fn tetrahedralize(solid: &physical_brep::Solid) -> FEAMesh {
    tetrahedralize_with_density(solid, 5.0)
}

/// Tetrahedralize with a target element edge length (mm).
/// Smaller values produce finer meshes with better accuracy.
pub fn tetrahedralize_with_density(solid: &physical_brep::Solid, target_edge_mm: f64) -> FEAMesh {
    let (min, max) = solid.bounding_box();
    let size = max - min;

    // Aspect-ratio-aware subdivision: more divisions along longer axes
    // Target: each element edge ≈ target_edge_mm
    let target = target_edge_mm.max(0.5); // don't go below 0.5mm
    let divs = [
        ((size.x / target).ceil() as usize).max(2).min(40),
        ((size.y / target).ceil() as usize).max(2).min(40),
        ((size.z / target).ceil() as usize).max(2).min(40),
    ];

    let nx = divs[0] + 1;
    let ny = divs[1] + 1;
    let nz = divs[2] + 1;

    let mut nodes = Vec::with_capacity(nx * ny * nz);
    for iz in 0..nz {
        for iy in 0..ny {
            for ix in 0..nx {
                let pos = DVec3::new(
                    min.x + size.x * ix as f64 / divs[0] as f64,
                    min.y + size.y * iy as f64 / divs[1] as f64,
                    min.z + size.z * iz as f64 / divs[2] as f64,
                );
                nodes.push(Node { position: pos });
            }
        }
    }

    let idx = |ix: usize, iy: usize, iz: usize| iz * ny * nx + iy * nx + ix;

    let mut elements = Vec::new();
    for iz in 0..divs[2] {
        for iy in 0..divs[1] {
            for ix in 0..divs[0] {
                // 8 corners of the hex cell
                let n000 = idx(ix,     iy,     iz    );
                let n100 = idx(ix + 1, iy,     iz    );
                let n010 = idx(ix,     iy + 1, iz    );
                let n110 = idx(ix + 1, iy + 1, iz    );
                let n001 = idx(ix,     iy,     iz + 1);
                let n101 = idx(ix + 1, iy,     iz + 1);
                let n011 = idx(ix,     iy + 1, iz + 1);
                let n111 = idx(ix + 1, iy + 1, iz + 1);

                // Split hex into 6 tetrahedra (symmetric decomposition)
                // reduces directional bias and shear locking vs 5-tet split
                elements.push(Tet4 { nodes: [n000, n100, n110, n111] });
                elements.push(Tet4 { nodes: [n000, n110, n010, n111] });
                elements.push(Tet4 { nodes: [n000, n010, n011, n111] });
                elements.push(Tet4 { nodes: [n000, n011, n001, n111] });
                elements.push(Tet4 { nodes: [n000, n001, n101, n111] });
                elements.push(Tet4 { nodes: [n000, n101, n100, n111] });
            }
        }
    }

    FEAMesh { nodes, elements }
}
