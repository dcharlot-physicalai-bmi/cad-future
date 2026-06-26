//! `physical-topology` — Topology optimization via SIMP method.
//!
//! Solid Isotropic Material with Penalization (SIMP) for structural
//! topology optimization. Given loads, constraints, and a volume fraction
//! target, computes the optimal material distribution to minimize compliance
//! (maximize stiffness).

use glam::DVec3;

// ---------------------------------------------------------------------------
// Problem Definition
// ---------------------------------------------------------------------------

/// A node in the optimization grid.
#[derive(Debug, Clone, Copy)]
pub struct GridNode {
    pub position: DVec3,
}

/// Load applied to a node.
#[derive(Debug, Clone, Copy)]
pub struct Load {
    pub node: usize,
    pub force: DVec3,
}

/// Boundary condition: fixed node.
#[derive(Debug, Clone, Copy)]
pub struct Support {
    pub node: usize,
    /// Fixed in x.
    pub fix_x: bool,
    /// Fixed in y.
    pub fix_y: bool,
    /// Fixed in z.
    pub fix_z: bool,
}

/// Topology optimization problem definition.
#[derive(Debug, Clone)]
pub struct TopologyProblem {
    /// Grid dimensions (elements per axis).
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// Element size (uniform cubic).
    pub element_size: f64,
    /// Young's modulus of solid material.
    pub e_modulus: f64,
    /// Poisson's ratio.
    pub poisson: f64,
    /// Target volume fraction (0.0 - 1.0).
    pub volume_fraction: f64,
    /// SIMP penalization power (typically 3.0).
    pub penalization: f64,
    /// Applied loads.
    pub loads: Vec<Load>,
    /// Boundary conditions.
    pub supports: Vec<Support>,
    /// Minimum density (avoid singularity).
    pub rho_min: f64,
}

impl TopologyProblem {
    /// Create a 2D topology optimization problem (nz=1).
    pub fn new_2d(nx: usize, ny: usize, element_size: f64) -> Self {
        Self {
            nx, ny, nz: 1,
            element_size,
            e_modulus: 1.0, // normalized
            poisson: 0.3,
            volume_fraction: 0.5,
            penalization: 3.0,
            loads: Vec::new(),
            supports: Vec::new(),
            rho_min: 0.001,
        }
    }

    /// Create a 3D topology optimization problem.
    pub fn new_3d(nx: usize, ny: usize, nz: usize, element_size: f64) -> Self {
        Self {
            nx, ny, nz,
            element_size,
            e_modulus: 1.0,
            poisson: 0.3,
            volume_fraction: 0.5,
            penalization: 3.0,
            loads: Vec::new(),
            supports: Vec::new(),
            rho_min: 0.001,
        }
    }

    /// Number of elements.
    pub fn num_elements(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Number of nodes ((nx+1) × (ny+1) × (nz+1)).
    pub fn num_nodes(&self) -> usize {
        (self.nx + 1) * (self.ny + 1) * (self.nz + 1)
    }

    /// Node index from grid coordinates.
    pub fn node_index(&self, ix: usize, iy: usize, iz: usize) -> usize {
        iz * (self.nx + 1) * (self.ny + 1) + iy * (self.nx + 1) + ix
    }

    /// Element index from grid coordinates.
    pub fn element_index(&self, ix: usize, iy: usize, iz: usize) -> usize {
        iz * self.nx * self.ny + iy * self.nx + ix
    }
}

// ---------------------------------------------------------------------------
// Optimization Result
// ---------------------------------------------------------------------------

/// Result of topology optimization.
#[derive(Debug, Clone)]
pub struct TopologyResult {
    /// Element density field (0.0 = void, 1.0 = solid).
    pub densities: Vec<f64>,
    /// Compliance (strain energy) at each iteration.
    pub compliance_history: Vec<f64>,
    /// Final compliance value.
    pub final_compliance: f64,
    /// Number of iterations to converge.
    pub iterations: usize,
    /// Whether the optimization converged.
    pub converged: bool,
}

impl TopologyResult {
    /// Get elements above a density threshold (the "solid" region).
    pub fn solid_elements(&self, threshold: f64) -> Vec<usize> {
        self.densities.iter().enumerate()
            .filter(|(_, d)| **d >= threshold)
            .map(|(i, _)| i)
            .collect()
    }

    /// Volume fraction of the result.
    pub fn volume_fraction(&self) -> f64 {
        self.densities.iter().sum::<f64>() / self.densities.len() as f64
    }
}

// ---------------------------------------------------------------------------
// SIMP Solver (2D for now, extensible to 3D)
// ---------------------------------------------------------------------------

/// Run SIMP topology optimization.
///
/// Uses the optimality criteria (OC) method for density update.
/// Filter radius prevents checkerboard patterns.
pub fn optimize(problem: &TopologyProblem, max_iterations: usize, filter_radius: f64) -> TopologyResult {
    let n_elem = problem.num_elements();
    let n_nodes = problem.num_nodes();
    let n_dof = n_nodes * 3;

    // Initialize uniform density
    let mut rho = vec![problem.volume_fraction; n_elem];
    let mut compliance_history = Vec::new();

    // Precompute element stiffness for unit material (E=1)
    let ke_unit = unit_element_stiffness_2d(problem.element_size, problem.poisson);

    // Precompute filter weights
    let filter_weights = build_filter(problem, filter_radius);

    let mut converged = false;
    let mut iterations = 0;

    for iter in 0..max_iterations {
        // Assemble global stiffness
        let mut k_global = vec![0.0f64; n_dof * n_dof];
        assemble_stiffness_2d(problem, &rho, &ke_unit, &mut k_global);

        // Apply boundary conditions
        let mut f_global = vec![0.0f64; n_dof];
        let mut fixed = vec![false; n_dof];

        for load in &problem.loads {
            f_global[load.node * 3] += load.force.x;
            f_global[load.node * 3 + 1] += load.force.y;
            f_global[load.node * 3 + 2] += load.force.z;
        }

        for sup in &problem.supports {
            if sup.fix_x { fixed[sup.node * 3] = true; }
            if sup.fix_y { fixed[sup.node * 3 + 1] = true; }
            if sup.fix_z { fixed[sup.node * 3 + 2] = true; }
        }

        let penalty_val = 1e10 * problem.e_modulus.max(1.0);
        for i in 0..n_dof {
            if fixed[i] {
                k_global[i * n_dof + i] += penalty_val;
                f_global[i] = 0.0;
            }
        }

        // Solve K·u = f
        let u = gauss_solve(&mut k_global, &mut f_global, n_dof);

        // Compute element compliance and sensitivity
        let mut ce = vec![0.0f64; n_elem];
        let mut dc = vec![0.0f64; n_elem]; // sensitivity

        for iy in 0..problem.ny {
            for ix in 0..problem.nx {
                let e_idx = problem.element_index(ix, iy, 0);
                let nodes = element_nodes_2d(problem, ix, iy);
                let mut ue = [0.0f64; 24]; // 8 nodes × 3 DOF (but 2D uses 4 nodes × 3)
                for (i, &n) in nodes.iter().enumerate() {
                    ue[i * 3] = u[n * 3];
                    ue[i * 3 + 1] = u[n * 3 + 1];
                    ue[i * 3 + 2] = u[n * 3 + 2];
                }

                // ce = u_e^T · k0 · u_e
                let mut elem_c = 0.0;
                let ke_size = nodes.len() * 3;
                for i in 0..ke_size {
                    for j in 0..ke_size {
                        elem_c += ue[i] * ke_unit[i * ke_size + j] * ue[j];
                    }
                }

                let rho_e = rho[e_idx].max(problem.rho_min);
                let e_penalized = problem.e_modulus * rho_e.powf(problem.penalization);
                ce[e_idx] = e_penalized * elem_c;
                dc[e_idx] = -problem.penalization * rho_e.powf(problem.penalization - 1.0) * problem.e_modulus * elem_c;
            }
        }

        let compliance: f64 = ce.iter().sum();
        compliance_history.push(compliance);

        // Apply density filter to sensitivities
        let dc_filtered = apply_filter(&dc, &rho, &filter_weights, n_elem);

        // Optimality criteria (OC) update
        let new_rho = oc_update(problem, &rho, &dc_filtered);

        // Check convergence
        let max_change: f64 = rho.iter().zip(new_rho.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);

        rho = new_rho;
        iterations = iter + 1;

        if max_change < 0.01 && iter > 5 {
            converged = true;
            break;
        }
    }

    TopologyResult {
        densities: rho,
        final_compliance: compliance_history.last().copied().unwrap_or(0.0),
        compliance_history,
        iterations,
        converged,
    }
}

// ---------------------------------------------------------------------------
// Element Stiffness (2D plane stress, 4-node quad)
// ---------------------------------------------------------------------------

/// Unit element stiffness for a 2D quad (plane stress, 4 nodes × 3 DOF).
/// Returns 12×12 matrix (we use 3 DOF per node but z-DOF is zero for 2D).
fn unit_element_stiffness_2d(elem_size: f64, nu: f64) -> Vec<f64> {
    // Simplified 4-node plane stress element
    // Using analytical integration for rectangular element
    let k = 1.0 / (1.0 - nu * nu);
    let s = elem_size;

    // For a unit-thickness rectangular element under plane stress:
    // K = (t·E)/(12·(1-ν²)) × [...] but we use E=1, t=s, and return unnormalized
    let n_dof_per_elem = 12; // 4 nodes × 3 DOF
    let mut ke = vec![0.0f64; n_dof_per_elem * n_dof_per_elem];

    // Simple approximation: lumped stiffness based on element geometry
    // Each pair of adjacent nodes connected by spring of stiffness ∝ E·t/L
    let stiffness = k * s / 4.0;

    // Diagonal terms (self-stiffness)
    for i in 0..4 {
        ke[(i * 3) * n_dof_per_elem + (i * 3)] = 2.0 * stiffness;         // x
        ke[(i * 3 + 1) * n_dof_per_elem + (i * 3 + 1)] = 2.0 * stiffness; // y
        // z DOF stiffness (out of plane) — small for 2D
        ke[(i * 3 + 2) * n_dof_per_elem + (i * 3 + 2)] = 0.01 * stiffness;
    }

    // Off-diagonal: coupling between adjacent nodes
    let adjacency = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)];
    for &(a, b) in &adjacency {
        let coupling = if a + b == 2 || (a == 0 && b == 2) { -0.5 * stiffness } else { -stiffness / 3.0 };
        for d in 0..2 { // x and y only
            ke[(a * 3 + d) * n_dof_per_elem + (b * 3 + d)] = coupling;
            ke[(b * 3 + d) * n_dof_per_elem + (a * 3 + d)] = coupling;
        }
        // Poisson coupling
        ke[(a * 3) * n_dof_per_elem + (b * 3 + 1)] = coupling * nu;
        ke[(b * 3 + 1) * n_dof_per_elem + (a * 3)] = coupling * nu;
        ke[(a * 3 + 1) * n_dof_per_elem + (b * 3)] = coupling * nu;
        ke[(b * 3) * n_dof_per_elem + (a * 3 + 1)] = coupling * nu;
    }

    ke
}

/// Get the 4 node indices for a 2D element (ix, iy).
fn element_nodes_2d(problem: &TopologyProblem, ix: usize, iy: usize) -> [usize; 4] {
    [
        problem.node_index(ix, iy, 0),
        problem.node_index(ix + 1, iy, 0),
        problem.node_index(ix + 1, iy + 1, 0),
        problem.node_index(ix, iy + 1, 0),
    ]
}

/// Assemble global stiffness matrix from element densities.
fn assemble_stiffness_2d(
    problem: &TopologyProblem,
    rho: &[f64],
    ke_unit: &[f64],
    k_global: &mut [f64],
) {
    let n_dof = problem.num_nodes() * 3;
    let ke_size = 12; // 4 nodes × 3 DOF

    for iy in 0..problem.ny {
        for ix in 0..problem.nx {
            let e_idx = problem.element_index(ix, iy, 0);
            let rho_e = rho[e_idx].max(problem.rho_min);
            let e_scale = problem.e_modulus * rho_e.powf(problem.penalization);

            let nodes = element_nodes_2d(problem, ix, iy);

            for i in 0..4 {
                for j in 0..4 {
                    for di in 0..3 {
                        for dj in 0..3 {
                            let gi = nodes[i] * 3 + di;
                            let gj = nodes[j] * 3 + dj;
                            if gi < n_dof && gj < n_dof {
                                k_global[gi * n_dof + gj] += e_scale * ke_unit[(i * 3 + di) * ke_size + (j * 3 + dj)];
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Density Filter (prevents checkerboard patterns)
// ---------------------------------------------------------------------------

/// Build filter weight matrix.
fn build_filter(problem: &TopologyProblem, radius: f64) -> Vec<Vec<(usize, f64)>> {
    let n_elem = problem.num_elements();
    let mut weights = vec![Vec::new(); n_elem];

    for iy in 0..problem.ny {
        for ix in 0..problem.nx {
            let e1 = problem.element_index(ix, iy, 0);
            let cx1 = (ix as f64 + 0.5) * problem.element_size;
            let cy1 = (iy as f64 + 0.5) * problem.element_size;

            for jy in 0..problem.ny {
                for jx in 0..problem.nx {
                    let e2 = problem.element_index(jx, jy, 0);
                    let cx2 = (jx as f64 + 0.5) * problem.element_size;
                    let cy2 = (jy as f64 + 0.5) * problem.element_size;

                    let dist = ((cx1 - cx2).powi(2) + (cy1 - cy2).powi(2)).sqrt();
                    if dist < radius {
                        weights[e1].push((e2, radius - dist));
                    }
                }
            }
        }
    }

    weights
}

/// Apply density filter to sensitivities.
fn apply_filter(dc: &[f64], rho: &[f64], weights: &[Vec<(usize, f64)>], n_elem: usize) -> Vec<f64> {
    let mut filtered = vec![0.0f64; n_elem];

    for e in 0..n_elem {
        let mut num = 0.0;
        let mut den = 0.0;
        for &(j, w) in &weights[e] {
            num += w * rho[j] * dc[j];
            den += w;
        }
        filtered[e] = if den > 0.0 { num / (rho[e].max(1e-10) * den) } else { dc[e] };
    }

    filtered
}

// ---------------------------------------------------------------------------
// Optimality Criteria Update
// ---------------------------------------------------------------------------

/// Update densities using the optimality criteria method.
fn oc_update(problem: &TopologyProblem, rho: &[f64], dc: &[f64]) -> Vec<f64> {
    let n_elem = problem.num_elements();
    let move_limit = 0.2;

    // Bisection to find Lagrange multiplier for volume constraint
    let mut l1 = 0.0f64;
    let mut l2 = 1e9f64;
    let target_volume = problem.volume_fraction * n_elem as f64;

    let mut new_rho = vec![0.0f64; n_elem];

    for _ in 0..100 {
        let lmid = (l1 + l2) / 2.0;

        for e in 0..n_elem {
            let be = if lmid > 0.0 { (-dc[e] / lmid).sqrt() } else { 1.0 };
            let candidate = rho[e] * be;
            new_rho[e] = candidate
                .max(problem.rho_min)
                .max(rho[e] - move_limit)
                .min(1.0)
                .min(rho[e] + move_limit);
        }

        let current_volume: f64 = new_rho.iter().sum();
        if current_volume > target_volume {
            l1 = lmid;
        } else {
            l2 = lmid;
        }

        if (l2 - l1) / (l1 + l2 + 1e-30) < 1e-6 { break; }
    }

    new_rho
}

// ---------------------------------------------------------------------------
// Linear Solver (Gaussian elimination, same as FEA)
// ---------------------------------------------------------------------------

fn gauss_solve(a: &mut [f64], b: &mut [f64], n: usize) -> Vec<f64> {
    for col in 0..n {
        let mut max_val = a[col * n + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = a[row * n + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
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

// ---------------------------------------------------------------------------
// LUT: Known Optimal Topologies
// ---------------------------------------------------------------------------

/// Pre-computed optimal topology for a common load case.
#[derive(Debug, Clone)]
pub struct KnownTopology {
    pub description: &'static str,
    pub load_case: &'static str,
    pub volume_fraction: f64,
    /// Threshold density pattern as run-length encoded string.
    pub pattern_description: &'static str,
}

pub static KNOWN_TOPOLOGIES: &[KnownTopology] = &[
    KnownTopology {
        description: "Cantilever beam, tip load",
        load_case: "Fixed left, point load down at right tip",
        volume_fraction: 0.5,
        pattern_description: "Warren truss-like topology",
    },
    KnownTopology {
        description: "Simply supported beam, center load",
        load_case: "Pinned left, roller right, point load at center",
        volume_fraction: 0.5,
        pattern_description: "I-beam cross section with diagonal bracing",
    },
    KnownTopology {
        description: "L-bracket",
        load_case: "Fixed top, load at right tip",
        volume_fraction: 0.4,
        pattern_description: "Diagonal strut with curved inner fillet",
    },
    KnownTopology {
        description: "MBB beam (half-symmetry)",
        load_case: "Roller left, pinned right, center top load",
        volume_fraction: 0.5,
        pattern_description: "Truss-like with triangulated members",
    },
];

// ---------------------------------------------------------------------------
// Iso-surface extraction — density field → mesh
// ---------------------------------------------------------------------------

/// Vertex of an extracted surface mesh.
#[derive(Debug, Clone, Copy)]
pub struct IsoVertex {
    pub position: DVec3,
    pub normal: DVec3,
}

/// Triangle mesh extracted from the density field.
#[derive(Debug, Clone)]
pub struct IsoMesh {
    pub vertices: Vec<IsoVertex>,
    pub indices: Vec<u32>,
}

/// Extract a surface mesh from the density field using marching squares (2D)
/// or a simplified marching cubes (3D). The `threshold` determines which cells
/// are considered solid (density >= threshold → solid).
pub fn extract_iso_surface(problem: &TopologyProblem, result: &TopologyResult, threshold: f64) -> IsoMesh {
    if problem.nz <= 1 {
        extract_2d(problem, result, threshold)
    } else {
        extract_3d(problem, result, threshold)
    }
}

/// 2D extraction: emit quads (as 2 triangles) for each solid cell, extruded
/// to unit thickness along z. Produces a watertight mesh.
fn extract_2d(problem: &TopologyProblem, result: &TopologyResult, threshold: f64) -> IsoMesh {
    let s = problem.element_size;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for iy in 0..problem.ny {
        for ix in 0..problem.nx {
            let e = problem.element_index(ix, iy, 0);
            if result.densities[e] < threshold {
                continue;
            }

            let x0 = ix as f64 * s;
            let y0 = iy as f64 * s;
            let x1 = x0 + s;
            let y1 = y0 + s;
            let z0 = 0.0;
            let z1 = s;

            // 8 corners of the extruded cell
            let corners = [
                DVec3::new(x0, y0, z0), // 0
                DVec3::new(x1, y0, z0), // 1
                DVec3::new(x1, y1, z0), // 2
                DVec3::new(x0, y1, z0), // 3
                DVec3::new(x0, y0, z1), // 4
                DVec3::new(x1, y0, z1), // 5
                DVec3::new(x1, y1, z1), // 6
                DVec3::new(x0, y1, z1), // 7
            ];

            // Check neighbors to decide which faces are exposed
            let base = vertices.len() as u32;
            for &c in &corners {
                vertices.push(IsoVertex { position: c, normal: DVec3::ZERO });
            }

            // 6 faces (normals assigned after)
            let faces: [(u32, u32, u32, u32, DVec3); 6] = [
                (0, 1, 2, 3, DVec3::new(0.0, 0.0, -1.0)), // bottom (-z)
                (4, 7, 6, 5, DVec3::new(0.0, 0.0, 1.0)),  // top (+z)
                (0, 4, 5, 1, DVec3::new(0.0, -1.0, 0.0)),  // front (-y)
                (2, 6, 7, 3, DVec3::new(0.0, 1.0, 0.0)),   // back (+y)
                (0, 3, 7, 4, DVec3::new(-1.0, 0.0, 0.0)),  // left (-x)
                (1, 5, 6, 2, DVec3::new(1.0, 0.0, 0.0)),   // right (+x)
            ];

            let neighbor_solid = |dx: i32, dy: i32| -> bool {
                let nx = ix as i32 + dx;
                let ny = iy as i32 + dy;
                if nx < 0 || ny < 0 || nx >= problem.nx as i32 || ny >= problem.ny as i32 {
                    return false;
                }
                let ne = problem.element_index(nx as usize, ny as usize, 0);
                result.densities[ne] >= threshold
            };

            // Only emit faces that are on the boundary
            let neighbor_checks: [(i32, i32); 6] = [
                (0, 0),   // bottom: always emit (z boundary)
                (0, 0),   // top: always emit (z boundary)
                (0, -1),  // front (-y)
                (0, 1),   // back (+y)
                (-1, 0),  // left (-x)
                (1, 0),   // right (+x)
            ];

            for (fi, &(a, b, c, d, normal)) in faces.iter().enumerate() {
                let emit = if fi < 2 {
                    true // always emit top/bottom for 2D extrusion
                } else {
                    !neighbor_solid(neighbor_checks[fi].0, neighbor_checks[fi].1)
                };
                if emit {
                    // Set normals for the face vertices
                    vertices[base as usize + a as usize].normal = normal;
                    vertices[base as usize + b as usize].normal = normal;
                    vertices[base as usize + c as usize].normal = normal;
                    vertices[base as usize + d as usize].normal = normal;
                    // Two triangles per quad
                    indices.extend_from_slice(&[base + a, base + b, base + c]);
                    indices.extend_from_slice(&[base + a, base + c, base + d]);
                }
            }
        }
    }

    IsoMesh { vertices, indices }
}

/// 3D extraction: emit exposed faces of solid voxels.
fn extract_3d(problem: &TopologyProblem, result: &TopologyResult, threshold: f64) -> IsoMesh {
    let s = problem.element_size;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let is_solid = |ix: i32, iy: i32, iz: i32| -> bool {
        if ix < 0 || iy < 0 || iz < 0
            || ix >= problem.nx as i32
            || iy >= problem.ny as i32
            || iz >= problem.nz as i32
        {
            return false;
        }
        let e = problem.element_index(ix as usize, iy as usize, iz as usize);
        result.densities[e] >= threshold
    };

    for iz in 0..problem.nz {
        for iy in 0..problem.ny {
            for ix in 0..problem.nx {
                let e = problem.element_index(ix, iy, iz);
                if result.densities[e] < threshold {
                    continue;
                }

                let x0 = ix as f64 * s;
                let y0 = iy as f64 * s;
                let z0 = iz as f64 * s;

                // 6 face directions: -x, +x, -y, +y, -z, +z
                let dirs: [(i32, i32, i32, DVec3); 6] = [
                    (-1, 0, 0, DVec3::new(-1.0, 0.0, 0.0)),
                    (1, 0, 0, DVec3::new(1.0, 0.0, 0.0)),
                    (0, -1, 0, DVec3::new(0.0, -1.0, 0.0)),
                    (0, 1, 0, DVec3::new(0.0, 1.0, 0.0)),
                    (0, 0, -1, DVec3::new(0.0, 0.0, -1.0)),
                    (0, 0, 1, DVec3::new(0.0, 0.0, 1.0)),
                ];

                for &(dx, dy, dz, normal) in &dirs {
                    if is_solid(ix as i32 + dx, iy as i32 + dy, iz as i32 + dz) {
                        continue; // interior face, skip
                    }

                    let base = vertices.len() as u32;
                    let (v0, v1, v2, v3) = face_quad(x0, y0, z0, s, dx, dy, dz);
                    for &pos in &[v0, v1, v2, v3] {
                        vertices.push(IsoVertex { position: pos, normal });
                    }
                    indices.extend_from_slice(&[base, base + 1, base + 2]);
                    indices.extend_from_slice(&[base, base + 2, base + 3]);
                }
            }
        }
    }

    IsoMesh { vertices, indices }
}

/// Compute the 4 corners of a face quad for a voxel at (x0, y0, z0) with size s,
/// face in direction (dx, dy, dz).
fn face_quad(x0: f64, y0: f64, z0: f64, s: f64, dx: i32, dy: i32, dz: i32) -> (DVec3, DVec3, DVec3, DVec3) {
    match (dx, dy, dz) {
        (-1, 0, 0) => (
            DVec3::new(x0, y0, z0),
            DVec3::new(x0, y0 + s, z0),
            DVec3::new(x0, y0 + s, z0 + s),
            DVec3::new(x0, y0, z0 + s),
        ),
        (1, 0, 0) => (
            DVec3::new(x0 + s, y0, z0),
            DVec3::new(x0 + s, y0, z0 + s),
            DVec3::new(x0 + s, y0 + s, z0 + s),
            DVec3::new(x0 + s, y0 + s, z0),
        ),
        (0, -1, 0) => (
            DVec3::new(x0, y0, z0),
            DVec3::new(x0, y0, z0 + s),
            DVec3::new(x0 + s, y0, z0 + s),
            DVec3::new(x0 + s, y0, z0),
        ),
        (0, 1, 0) => (
            DVec3::new(x0, y0 + s, z0),
            DVec3::new(x0 + s, y0 + s, z0),
            DVec3::new(x0 + s, y0 + s, z0 + s),
            DVec3::new(x0, y0 + s, z0 + s),
        ),
        (0, 0, -1) => (
            DVec3::new(x0, y0, z0),
            DVec3::new(x0 + s, y0, z0),
            DVec3::new(x0 + s, y0 + s, z0),
            DVec3::new(x0, y0 + s, z0),
        ),
        (0, 0, 1) => (
            DVec3::new(x0, y0, z0 + s),
            DVec3::new(x0, y0 + s, z0 + s),
            DVec3::new(x0 + s, y0 + s, z0 + s),
            DVec3::new(x0 + s, y0, z0 + s),
        ),
        _ => unreachable!(),
    }
}

/// Convenience: optimize and immediately extract a mesh.
pub fn optimize_and_extract(
    problem: &TopologyProblem,
    max_iterations: usize,
    filter_radius: f64,
    threshold: f64,
) -> (TopologyResult, IsoMesh) {
    let result = optimize(problem, max_iterations, filter_radius);
    let mesh = extract_iso_surface(problem, &result, threshold);
    (result, mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_dimensions() {
        let p = TopologyProblem::new_2d(10, 5, 1.0);
        assert_eq!(p.num_elements(), 50);
        assert_eq!(p.num_nodes(), 11 * 6 * 2); // (10+1)×(5+1)×(1+1)
    }

    #[test]
    fn node_index_unique() {
        let p = TopologyProblem::new_2d(5, 3, 1.0);
        let mut seen = std::collections::HashSet::new();
        for iz in 0..=p.nz {
            for iy in 0..=p.ny {
                for ix in 0..=p.nx {
                    let idx = p.node_index(ix, iy, iz);
                    assert!(seen.insert(idx), "duplicate node index {}", idx);
                }
            }
        }
    }

    #[test]
    fn cantilever_optimization() {
        // Small 6×3 cantilever problem
        let mut problem = TopologyProblem::new_2d(6, 3, 1.0);
        problem.volume_fraction = 0.5;
        problem.e_modulus = 1.0;

        // Fix left edge
        for iy in 0..=problem.ny {
            for iz in 0..=problem.nz {
                let n = problem.node_index(0, iy, iz);
                problem.supports.push(Support { node: n, fix_x: true, fix_y: true, fix_z: true });
            }
        }

        // Load at right-center
        let load_node = problem.node_index(problem.nx, problem.ny / 2, 0);
        problem.loads.push(Load { node: load_node, force: DVec3::new(0.0, -1.0, 0.0) });

        let result = optimize(&problem, 50, 1.5);

        assert!(result.densities.len() == problem.num_elements());
        assert!(result.compliance_history.len() > 0);
        // Volume fraction should be near target
        let vf = result.volume_fraction();
        assert!((vf - 0.5).abs() < 0.15, "vf={}", vf);
    }

    #[test]
    fn result_solid_elements() {
        let result = TopologyResult {
            densities: vec![0.1, 0.8, 0.95, 0.3, 0.99],
            compliance_history: vec![10.0],
            final_compliance: 10.0,
            iterations: 1,
            converged: true,
        };
        let solid = result.solid_elements(0.5);
        assert_eq!(solid, vec![1, 2, 4]);
    }

    #[test]
    fn known_topologies_exist() {
        assert!(KNOWN_TOPOLOGIES.len() >= 4);
    }

    #[test]
    fn extract_iso_surface_2d() {
        let problem = TopologyProblem::new_2d(3, 2, 1.0);
        // All solid
        let result = TopologyResult {
            densities: vec![1.0; 6],
            compliance_history: vec![1.0],
            final_compliance: 1.0,
            iterations: 1,
            converged: true,
        };
        let mesh = extract_iso_surface(&problem, &result, 0.5);
        assert!(!mesh.vertices.is_empty(), "should have vertices");
        assert!(!mesh.indices.is_empty(), "should have triangles");
        assert_eq!(mesh.indices.len() % 3, 0, "indices should be multiple of 3");
    }

    #[test]
    fn extract_iso_surface_partial() {
        let problem = TopologyProblem::new_2d(4, 2, 1.0);
        // Checkerboard pattern: some solid, some void
        let result = TopologyResult {
            densities: vec![1.0, 0.1, 1.0, 0.1, 0.1, 1.0, 0.1, 1.0],
            compliance_history: vec![1.0],
            final_compliance: 1.0,
            iterations: 1,
            converged: true,
        };
        let mesh = extract_iso_surface(&problem, &result, 0.5);
        // 4 solid cells — each should produce faces
        assert!(!mesh.vertices.is_empty());
        // Solid cells with no solid neighbors expose more faces
        assert!(mesh.indices.len() > 24, "isolated cells should have many faces");
    }

    #[test]
    fn extract_iso_surface_3d() {
        let mut problem = TopologyProblem::new_3d(2, 2, 2, 1.0);
        problem.volume_fraction = 1.0;
        // All 8 elements solid
        let result = TopologyResult {
            densities: vec![1.0; 8],
            compliance_history: vec![1.0],
            final_compliance: 1.0,
            iterations: 1,
            converged: true,
        };
        let mesh = extract_iso_surface(&problem, &result, 0.5);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        // A 2×2×2 solid block: only boundary faces are exposed
        // 6 outer faces of the block, each 2×2 = 4 quads = 8 triangles = 48 total
        assert_eq!(mesh.indices.len() / 3, 48, "2x2x2 block should have 48 triangles on surface");
    }

    #[test]
    fn extract_iso_surface_empty() {
        let problem = TopologyProblem::new_2d(3, 3, 1.0);
        let result = TopologyResult {
            densities: vec![0.01; 9],
            compliance_history: vec![],
            final_compliance: 0.0,
            iterations: 0,
            converged: true,
        };
        let mesh = extract_iso_surface(&problem, &result, 0.5);
        assert!(mesh.vertices.is_empty(), "no solid cells → no mesh");
        assert!(mesh.indices.is_empty());
    }

    #[test]
    fn optimize_and_extract_pipeline() {
        let mut problem = TopologyProblem::new_2d(6, 3, 1.0);
        problem.volume_fraction = 0.5;
        // Fix left edge
        for iy in 0..=problem.ny {
            for iz in 0..=problem.nz {
                let n = problem.node_index(0, iy, iz);
                problem.supports.push(Support { node: n, fix_x: true, fix_y: true, fix_z: true });
            }
        }
        let load_node = problem.node_index(problem.nx, problem.ny / 2, 0);
        problem.loads.push(Load { node: load_node, force: DVec3::new(0.0, -1.0, 0.0) });

        let (result, mesh) = optimize_and_extract(&problem, 30, 1.5, 0.3);
        assert!(result.densities.len() == 18);
        assert!(!mesh.vertices.is_empty(), "should produce a mesh");
        assert!(!mesh.indices.is_empty(), "should have triangles");
    }
}
