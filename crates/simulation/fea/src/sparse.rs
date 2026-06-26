//! Sparse matrix solver — CSR storage and Preconditioned Conjugate Gradient.
//!
//! Replaces dense O(n³) Gaussian elimination for large FEA problems with
//! iterative PCG that scales as O(nnz × iterations). Stiffness matrices from
//! FEA are symmetric positive definite (SPD) after boundary conditions are
//! applied, making CG the natural choice.

use glam::DVec3;
use rayon::prelude::*;
use crate::{FEAMesh, BC, FEAResult, ElementStress};

// ---------------------------------------------------------------------------
// CSR (Compressed Sparse Row) matrix
// ---------------------------------------------------------------------------

/// Sparse matrix in Compressed Sparse Row format.
///
/// For an n×n matrix with `nnz` non-zeros:
/// - `row_ptr[i]` is the index into `col_idx`/`values` where row `i` starts.
/// - `col_idx[k]` is the column index of the k-th stored entry.
/// - `values[k]` is the value of the k-th stored entry.
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    pub n: usize,
    pub row_ptr: Vec<usize>,
    pub col_idx: Vec<usize>,
    pub values: Vec<f64>,
}

/// A (row, col, value) triplet for assembling sparse matrices.
#[derive(Debug, Clone, Copy)]
struct Triplet {
    row: usize,
    col: usize,
    val: f64,
}

impl SparseMatrix {
    /// Build a CSR matrix from coordinate (triplet) format.
    ///
    /// Duplicate entries at the same (row, col) are **summed** — exactly the
    /// semantics needed for FEA assembly.
    pub fn from_triplets(
        rows: &[usize],
        cols: &[usize],
        values: &[f64],
        n: usize,
    ) -> Self {
        assert_eq!(rows.len(), cols.len());
        assert_eq!(rows.len(), values.len());

        // Sort triplets by (row, col)
        let mut triplets: Vec<Triplet> = rows
            .iter()
            .zip(cols.iter())
            .zip(values.iter())
            .map(|((&r, &c), &v)| Triplet { row: r, col: c, val: v })
            .collect();
        triplets.sort_by(|a, b| a.row.cmp(&b.row).then(a.col.cmp(&b.col)));

        // Merge duplicates and build CSR arrays
        let mut csr_row_ptr = vec![0usize; n + 1];
        let mut csr_col_idx = Vec::with_capacity(triplets.len());
        let mut csr_values = Vec::with_capacity(triplets.len());

        let mut i = 0;
        while i < triplets.len() {
            let r = triplets[i].row;
            let c = triplets[i].col;
            let mut sum = 0.0;
            while i < triplets.len() && triplets[i].row == r && triplets[i].col == c {
                sum += triplets[i].val;
                i += 1;
            }
            if sum != 0.0 {
                csr_col_idx.push(c);
                csr_values.push(sum);
                csr_row_ptr[r + 1] += 1;
            }
        }

        // Prefix sum for row_ptr
        for i in 0..n {
            csr_row_ptr[i + 1] += csr_row_ptr[i];
        }

        Self {
            n,
            row_ptr: csr_row_ptr,
            col_idx: csr_col_idx,
            values: csr_values,
        }
    }

    /// Number of stored non-zeros.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Look up a single entry A[i][j]. Returns 0 if not stored. O(log nnz_row).
    pub fn get(&self, i: usize, j: usize) -> f64 {
        let start = self.row_ptr[i];
        let end = self.row_ptr[i + 1];
        let slice = &self.col_idx[start..end];
        match slice.binary_search(&j) {
            Ok(pos) => self.values[start + pos],
            Err(_) => 0.0,
        }
    }

    /// Sparse matrix-vector product y = A · x.
    pub fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(x.len(), self.n);
        let mut y = vec![0.0f64; self.n];
        for i in 0..self.n {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            let mut sum = 0.0;
            for k in start..end {
                sum += self.values[k] * x[self.col_idx[k]];
            }
            y[i] = sum;
        }
        y
    }

    /// Extract the diagonal as a vector.
    pub fn diagonal(&self) -> Vec<f64> {
        let mut diag = vec![0.0f64; self.n];
        for i in 0..self.n {
            diag[i] = self.get(i, i);
        }
        diag
    }
}

// ---------------------------------------------------------------------------
// Preconditioners
// ---------------------------------------------------------------------------

/// Trait for a preconditioner: applies M⁻¹ to a vector.
pub trait Preconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]);
}

/// Jacobi (diagonal) preconditioner: M⁻¹ = diag(A)⁻¹.
pub struct JacobiPreconditioner {
    inv_diag: Vec<f64>,
}

impl JacobiPreconditioner {
    pub fn new(a: &SparseMatrix) -> Self {
        let diag = a.diagonal();
        let inv_diag: Vec<f64> = diag
            .iter()
            .map(|&d| if d.abs() > 1e-30 { 1.0 / d } else { 1.0 })
            .collect();
        Self { inv_diag }
    }
}

impl Preconditioner for JacobiPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for i in 0..r.len() {
            z[i] = self.inv_diag[i] * r[i];
        }
    }
}

/// Incomplete Cholesky IC(0) preconditioner for SPD matrices.
///
/// Computes L such that A ≈ L·Lᵀ, keeping only the sparsity pattern of the
/// lower triangle of A (zero-fill). Applying the preconditioner solves
/// L·Lᵀ·z = r via forward/back substitution.
pub struct IncompleteCholeskyPreconditioner {
    n: usize,
    /// Lower triangular factor in CSR-like storage.
    l_row_ptr: Vec<usize>,
    l_col_idx: Vec<usize>,
    l_values: Vec<f64>,
    /// Diagonal of L (stored separately for fast access).
    l_diag: Vec<f64>,
}

impl IncompleteCholeskyPreconditioner {
    pub fn new(a: &SparseMatrix) -> Self {
        let n = a.n;

        // Extract lower triangle entries (including diagonal) in row-major order.
        let mut l_row_ptr = vec![0usize; n + 1];
        let mut l_col_idx = Vec::new();
        let mut l_values = Vec::new();

        for i in 0..n {
            let start = a.row_ptr[i];
            let end = a.row_ptr[i + 1];
            for k in start..end {
                let j = a.col_idx[k];
                if j <= i {
                    l_col_idx.push(j);
                    l_values.push(a.values[k]);
                    l_row_ptr[i + 1] += 1;
                }
            }
        }
        for i in 0..n {
            l_row_ptr[i + 1] += l_row_ptr[i];
        }

        let mut l_diag = vec![0.0f64; n];

        // IC(0) factorization: modify values in place
        for i in 0..n {
            let row_start = l_row_ptr[i];
            let row_end = l_row_ptr[i + 1];

            // For each entry L[i][j] with j < i
            for idx in row_start..row_end {
                let j = l_col_idx[idx];
                if j == i {
                    // Diagonal: L[i][i] = sqrt(A[i][i] - sum_k L[i][k]²)
                    let mut sum = 0.0;
                    for prev in row_start..idx {
                        sum += l_values[prev] * l_values[prev];
                    }
                    let val = l_values[idx] - sum;
                    let val = if val > 1e-30 { val.sqrt() } else { 1e-15 };
                    l_values[idx] = val;
                    l_diag[i] = val;
                } else {
                    // Off-diagonal: L[i][j] = (A[i][j] - sum_k L[i][k]*L[j][k]) / L[j][j]
                    let mut sum = 0.0;
                    // Walk entries of row i with col < j, and match with row j
                    let j_start = l_row_ptr[j];
                    let j_end = l_row_ptr[j + 1];
                    for ki in row_start..idx {
                        let ci = l_col_idx[ki];
                        // Find same column in row j
                        for kj in j_start..j_end {
                            if l_col_idx[kj] == ci {
                                sum += l_values[ki] * l_values[kj];
                                break;
                            }
                            if l_col_idx[kj] > ci {
                                break;
                            }
                        }
                    }
                    let diag_j = l_diag[j];
                    if diag_j.abs() > 1e-30 {
                        l_values[idx] = (l_values[idx] - sum) / diag_j;
                    } else {
                        l_values[idx] = 0.0;
                    }
                }
            }
        }

        Self {
            n,
            l_row_ptr,
            l_col_idx,
            l_values,
            l_diag,
        }
    }
}

impl Preconditioner for IncompleteCholeskyPreconditioner {
    /// Solve L·Lᵀ·z = r.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let n = self.n;

        // Forward substitution: L · y = r
        let mut y = vec![0.0f64; n];
        for i in 0..n {
            let row_start = self.l_row_ptr[i];
            let row_end = self.l_row_ptr[i + 1];
            let mut sum = r[i];
            for k in row_start..row_end {
                let j = self.l_col_idx[k];
                if j < i {
                    sum -= self.l_values[k] * y[j];
                }
            }
            let diag = self.l_diag[i];
            y[i] = if diag.abs() > 1e-30 { sum / diag } else { 0.0 };
        }

        // Back substitution: Lᵀ · z = y
        // Lᵀ is upper triangular; we walk rows of L in reverse.
        for i in 0..n {
            z[i] = y[i];
        }
        for i in (0..n).rev() {
            let diag = self.l_diag[i];
            if diag.abs() > 1e-30 {
                z[i] /= diag;
            }
            let row_start = self.l_row_ptr[i];
            let row_end = self.l_row_ptr[i + 1];
            for k in row_start..row_end {
                let j = self.l_col_idx[k];
                if j < i {
                    z[j] -= self.l_values[k] * z[i];
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Preconditioned Conjugate Gradient
// ---------------------------------------------------------------------------

/// Result of a PCG solve.
pub struct PcgResult {
    pub solution: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
}

/// Preconditioned Conjugate Gradient solver for SPD system A·x = b.
///
/// Returns (solution, iterations, converged).
pub fn pcg_solve(
    a: &SparseMatrix,
    b: &[f64],
    preconditioner: &dyn Preconditioner,
    tol: f64,
    max_iter: usize,
) -> PcgResult {
    let n = a.n;
    let mut x = vec![0.0f64; n];
    let mut r: Vec<f64> = b.to_vec(); // r = b - A·x₀ (x₀ = 0)
    let mut z = vec![0.0f64; n];

    preconditioner.apply(&r, &mut z);
    let mut p = z.clone();

    let mut rz: f64 = r.iter().zip(z.iter()).map(|(ri, zi)| ri * zi).sum();

    let b_norm: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    let tol_abs = if b_norm > 1e-30 { tol * b_norm } else { tol };

    let mut iterations = 0;

    for iter in 0..max_iter {
        iterations = iter + 1;

        let ap = a.mul_vec(&p);
        let pap: f64 = p.iter().zip(ap.iter()).map(|(pi, api)| pi * api).sum();

        if pap.abs() < 1e-30 {
            break;
        }

        let alpha = rz / pap;

        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }

        let r_norm: f64 = r.iter().map(|v| v * v).sum::<f64>().sqrt();
        if r_norm < tol_abs {
            return PcgResult {
                solution: x,
                iterations,
                converged: true,
            };
        }

        preconditioner.apply(&r, &mut z);
        let rz_new: f64 = r.iter().zip(z.iter()).map(|(ri, zi)| ri * zi).sum();

        let beta = rz_new / rz;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        rz = rz_new;
    }

    PcgResult {
        solution: x,
        iterations,
        converged: false,
    }
}

// ---------------------------------------------------------------------------
// Sparse FEA assembly
// ---------------------------------------------------------------------------

/// Assemble global stiffness matrix in sparse (triplet) format directly from
/// element connectivity. Returns a CSR `SparseMatrix`.
///
/// Element stiffness computation is embarrassingly parallel — each Tet4 is
/// an independent 12×12 matrix product — so we compute them in parallel via
/// rayon and concatenate the triplet streams. The final CSR build still
/// runs serially (it needs a global sort+dedup), but at O(nnz log nnz) it's
/// negligible next to the per-element B^T·D·B work.
pub fn assemble_stiffness_sparse(
    mesh: &FEAMesh,
    d_matrix: &[f64; 36],
) -> SparseMatrix {
    let n_dof = mesh.nodes.len() * 3;

    // Each element emits its own (rows, cols, vals) triple, then we
    // concatenate. Per-element vectors avoid contention on a shared writer
    // and let rayon's work-stealing balance load across threads.
    let per_elem: Vec<(Vec<usize>, Vec<usize>, Vec<f64>)> = mesh
        .elements
        .par_iter()
        .map(|elem| {
            let ke = crate::element_stiffness(mesh, elem, d_matrix);
            let mut rows = Vec::with_capacity(144);
            let mut cols = Vec::with_capacity(144);
            let mut vals = Vec::with_capacity(144);
            for i in 0..4 {
                for j in 0..4 {
                    for di in 0..3 {
                        for dj in 0..3 {
                            let gi = elem.nodes[i] * 3 + di;
                            let gj = elem.nodes[j] * 3 + dj;
                            let val = ke[(i * 3 + di) * 12 + (j * 3 + dj)];
                            if val != 0.0 {
                                rows.push(gi);
                                cols.push(gj);
                                vals.push(val);
                            }
                        }
                    }
                }
            }
            (rows, cols, vals)
        })
        .collect();

    // Concatenate — pre-size to avoid realloc.
    let total: usize = per_elem.iter().map(|(r, _, _)| r.len()).sum();
    let mut rows = Vec::with_capacity(total);
    let mut cols = Vec::with_capacity(total);
    let mut vals = Vec::with_capacity(total);
    for (r, c, v) in per_elem {
        rows.extend(r);
        cols.extend(c);
        vals.extend(v);
    }

    SparseMatrix::from_triplets(&rows, &cols, &vals, n_dof)
}

/// Apply boundary conditions to a sparse stiffness matrix and load vector.
///
/// Returns a new `SparseMatrix` with penalty terms on constrained DOFs, plus
/// the modified load vector.
fn apply_bcs_sparse(
    k: &SparseMatrix,
    bcs: &[BC],
    _e_modulus: f64,
) -> (SparseMatrix, Vec<f64>) {
    let n_dof = k.n;
    // Use a moderate penalty relative to the max diagonal entry.
    // Dense solvers can tolerate 1e20 × E, but iterative solvers need a
    // smaller ratio to maintain reasonable condition numbers.
    let max_diag = k.diagonal().iter().fold(0.0f64, |a, &b| a.max(b.abs()));
    let penalty = 1e8 * max_diag.max(1.0);

    let mut f_global = vec![0.0f64; n_dof];

    // Collect penalty additions
    let mut penalty_rows = Vec::new();
    let mut penalty_cols = Vec::new();
    let mut penalty_vals = Vec::new();

    for bc in bcs {
        match *bc {
            BC::FixAll(node) => {
                for d in 0..3 {
                    let idx = node * 3 + d;
                    penalty_rows.push(idx);
                    penalty_cols.push(idx);
                    penalty_vals.push(penalty);
                }
            }
            BC::Fix(node, dof) => {
                let idx = node * 3 + dof;
                penalty_rows.push(idx);
                penalty_cols.push(idx);
                penalty_vals.push(penalty);
            }
            BC::Force(node, force) => {
                f_global[node * 3] += force.x;
                f_global[node * 3 + 1] += force.y;
                f_global[node * 3 + 2] += force.z;
            }
        }
    }

    // Merge original matrix entries with penalty terms
    let total = k.nnz() + penalty_vals.len();
    let mut rows = Vec::with_capacity(total);
    let mut cols = Vec::with_capacity(total);
    let mut vals = Vec::with_capacity(total);

    for i in 0..k.n {
        let start = k.row_ptr[i];
        let end = k.row_ptr[i + 1];
        for idx in start..end {
            rows.push(i);
            cols.push(k.col_idx[idx]);
            vals.push(k.values[idx]);
        }
    }

    rows.extend_from_slice(&penalty_rows);
    cols.extend_from_slice(&penalty_cols);
    vals.extend_from_slice(&penalty_vals);

    let k_bc = SparseMatrix::from_triplets(&rows, &cols, &vals, n_dof);
    (k_bc, f_global)
}

// ---------------------------------------------------------------------------
// Sparse static solver
// ---------------------------------------------------------------------------

/// Sparse DOF threshold: problems with more DOFs than this use the sparse solver.
pub const SPARSE_DOF_THRESHOLD: usize = 300;

/// Linear-static FEA solve using sparse PCG.
///
/// Equivalent to [`crate::solve`] but uses CSR storage and iterative PCG,
/// making it feasible for meshes with thousands of DOFs.
pub fn solve_sparse(
    mesh: &FEAMesh,
    e_modulus: f64,
    poisson: f64,
    bcs: &[BC],
) -> FEAResult {
    let n_nodes = mesh.nodes.len();
    let n_dof = n_nodes * 3;

    let d_matrix = crate::constitutive_matrix(e_modulus, poisson);

    // Assemble in sparse format
    let k_sparse = assemble_stiffness_sparse(mesh, &d_matrix);

    // Apply BCs
    let (k_bc, f_global) = apply_bcs_sparse(&k_sparse, bcs, e_modulus);

    // Solve with Jacobi-preconditioned CG.
    // Jacobi is preferred here because the penalty method for BCs creates
    // entries of order 1e20 on the diagonal, which breaks IC(0) factorization
    // but is handled naturally by diagonal scaling.
    let precond = JacobiPreconditioner::new(&k_bc);
    let result = pcg_solve(&k_bc, &f_global, &precond, 1e-10, n_dof.max(1000));

    let u = result.solution;

    // Post-process: displacements
    let displacements: Vec<DVec3> = (0..n_nodes)
        .map(|i| DVec3::new(u[i * 3], u[i * 3 + 1], u[i * 3 + 2]))
        .collect();

    // Post-process: stresses
    let d = crate::constitutive_matrix(e_modulus, poisson);
    let mut stresses = Vec::with_capacity(mesh.elements.len());
    let mut max_vm = 0.0f64;

    for elem in &mesh.elements {
        let b = crate::strain_displacement_matrix_pub(mesh, elem);

        let mut eps = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..12 {
                eps[i] += b[i * 12 + j] * u[elem.nodes[j / 3] * 3 + (j % 3)];
            }
        }

        let mut sig = [0.0f64; 6];
        for i in 0..6 {
            for j in 0..6 {
                sig[i] += d[i * 6 + j] * eps[j];
            }
        }

        let vm = von_mises_stress(&sig);
        if vm > max_vm {
            max_vm = vm;
        }
        stresses.push(ElementStress {
            stress: sig,
            von_mises: vm,
        });
    }

    let max_displacement = displacements
        .iter()
        .map(|d| d.length())
        .fold(0.0f64, f64::max);

    FEAResult {
        displacements,
        stresses,
        max_von_mises: max_vm,
        max_displacement,
    }
}

/// Von Mises equivalent stress (duplicated here to avoid visibility issues).
fn von_mises_stress(s: &[f64; 6]) -> f64 {
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
    use crate::{Node, FEAMesh, Tet4};

    /// 1D Laplacian as a known SPD test system: tridiagonal [-1, 2, -1].
    fn laplacian_1d(n: usize) -> SparseMatrix {
        let mut rows = Vec::new();
        let mut cols = Vec::new();
        let mut vals = Vec::new();

        for i in 0..n {
            rows.push(i);
            cols.push(i);
            vals.push(2.0);

            if i > 0 {
                rows.push(i);
                cols.push(i - 1);
                vals.push(-1.0);
            }
            if i + 1 < n {
                rows.push(i);
                cols.push(i + 1);
                vals.push(-1.0);
            }
        }

        SparseMatrix::from_triplets(&rows, &cols, &vals, n)
    }

    #[test]
    fn csr_construction_and_get() {
        let rows = vec![0, 0, 1, 1, 2, 2];
        let cols = vec![0, 1, 0, 1, 1, 2];
        let vals = vec![4.0, -1.0, -1.0, 4.0, -1.0, 4.0];
        let m = SparseMatrix::from_triplets(&rows, &cols, &vals, 3);

        assert_eq!(m.n, 3);
        assert_eq!(m.nnz(), 6);
        assert!((m.get(0, 0) - 4.0).abs() < 1e-12);
        assert!((m.get(0, 1) - (-1.0)).abs() < 1e-12);
        assert!((m.get(0, 2) - 0.0).abs() < 1e-12);
        assert!((m.get(1, 0) - (-1.0)).abs() < 1e-12);
        assert!((m.get(2, 2) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn csr_duplicate_summing() {
        // Two entries at (0,0): should be summed
        let rows = vec![0, 0, 1];
        let cols = vec![0, 0, 1];
        let vals = vec![3.0, 2.0, 5.0];
        let m = SparseMatrix::from_triplets(&rows, &cols, &vals, 2);

        assert!((m.get(0, 0) - 5.0).abs() < 1e-12);
        assert!((m.get(1, 1) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn csr_matvec() {
        // [ 2 -1  0 ]   [1]   [ 1]
        // [-1  2 -1 ] × [1] = [ 0]
        // [ 0 -1  2 ]   [1]   [ 1]
        let a = laplacian_1d(3);
        let x = vec![1.0, 1.0, 1.0];
        let y = a.mul_vec(&x);

        assert!((y[0] - 1.0).abs() < 1e-12);
        assert!((y[1] - 0.0).abs() < 1e-12);
        assert!((y[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn pcg_convergence_laplacian() {
        let n = 50;
        let a = laplacian_1d(n);

        // Known solution: x = [1, 2, 3, ..., n], compute b = A·x
        let x_exact: Vec<f64> = (1..=n).map(|i| i as f64).collect();
        let b = a.mul_vec(&x_exact);

        let precond = JacobiPreconditioner::new(&a);
        let result = pcg_solve(&a, &b, &precond, 1e-10, 200);

        assert!(result.converged, "PCG did not converge in {} iters", result.iterations);
        for i in 0..n {
            assert!(
                (result.solution[i] - x_exact[i]).abs() < 1e-6,
                "x[{}] = {}, expected {}",
                i,
                result.solution[i],
                x_exact[i]
            );
        }
    }

    #[test]
    fn pcg_convergence_ic0() {
        let n = 50;
        let a = laplacian_1d(n);

        let x_exact: Vec<f64> = (1..=n).map(|i| i as f64).collect();
        let b = a.mul_vec(&x_exact);

        let precond = IncompleteCholeskyPreconditioner::new(&a);
        let result = pcg_solve(&a, &b, &precond, 1e-10, 200);

        assert!(result.converged, "IC(0)-PCG did not converge in {} iters", result.iterations);
        for i in 0..n {
            assert!(
                (result.solution[i] - x_exact[i]).abs() < 1e-6,
                "x[{}] = {}, expected {}",
                i,
                result.solution[i],
                x_exact[i]
            );
        }
    }

    #[test]
    fn jacobi_preconditioner_correctness() {
        let a = laplacian_1d(5);
        let precond = JacobiPreconditioner::new(&a);

        // Diagonal of 1D Laplacian is all 2.0, so M⁻¹ = diag(0.5)
        let r = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut z = vec![0.0; 5];
        precond.apply(&r, &mut z);

        for i in 0..5 {
            assert!(
                (z[i] - r[i] * 0.5).abs() < 1e-12,
                "z[{}] = {}, expected {}",
                i,
                z[i],
                r[i] * 0.5
            );
        }
    }

    /// Helper: build a simple single-tet mesh for comparison tests.
    fn single_tet_mesh() -> FEAMesh {
        FEAMesh {
            nodes: vec![
                Node { position: DVec3::new(0.0, 0.0, 0.0) },
                Node { position: DVec3::new(1.0, 0.0, 0.0) },
                Node { position: DVec3::new(0.0, 1.0, 0.0) },
                Node { position: DVec3::new(0.0, 0.0, 1.0) },
            ],
            elements: vec![Tet4 { nodes: [0, 1, 2, 3] }],
        }
    }

    #[test]
    fn sparse_matches_dense_single_tet() {
        let mesh = single_tet_mesh();
        let e = 200_000.0;
        let nu = 0.3;

        let bcs = vec![
            BC::FixAll(0),
            BC::Fix(1, 1),
            BC::Fix(1, 2),
            BC::Fix(2, 2),
            BC::Force(3, DVec3::new(0.0, 0.0, -100.0)),
        ];

        let dense = crate::solve(&mesh, e, nu, &bcs);
        let sparse = solve_sparse(&mesh, e, nu, &bcs);

        // Displacements should match within tolerance.
        // The dense and sparse solvers use different penalty magnitudes for BCs,
        // so we compare with a relative tolerance on the free DOF displacements.
        let max_disp = dense.displacements.iter()
            .map(|d| d.length())
            .fold(0.0f64, f64::max);
        let tol = 1e-3 * max_disp.max(1e-15);

        for (i, (dd, ds)) in dense.displacements.iter().zip(sparse.displacements.iter()).enumerate() {
            let diff = (*dd - *ds).length();
            assert!(
                diff < tol,
                "node {} displacement mismatch: dense={:?}, sparse={:?}, tol={}",
                i, dd, ds, tol
            );
        }

        // Von Mises should match within tolerance
        assert!(
            (dense.max_von_mises - sparse.max_von_mises).abs()
                < 0.01 * dense.max_von_mises.max(1e-12),
            "max_vm mismatch: dense={}, sparse={}",
            dense.max_von_mises,
            sparse.max_von_mises
        );
    }

    #[test]
    fn sparse_matches_dense_box_mesh() {
        // Use a small well-conditioned box mesh (cantilever beam).
        use physical_brep::builder::make_box;
        let solid = make_box(20.0, 5.0, 5.0);
        let mesh = crate::tetrahedralize(&solid);

        let e = 200_000.0;
        let nu = 0.3;

        // Fix left face, load right face
        let mut bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.x < -8.0 {
                bcs.push(BC::FixAll(i));
            } else if node.position.x > 8.0 {
                bcs.push(BC::Force(i, DVec3::new(0.0, -10.0, 0.0)));
            }
        }

        let dense = crate::solve(&mesh, e, nu, &bcs);
        let sparse = solve_sparse(&mesh, e, nu, &bcs);

        // Compare displacements with tolerance relative to max displacement.
        let max_disp = dense.displacements.iter()
            .map(|d| d.length())
            .fold(0.0f64, f64::max);
        let tol = 0.01 * max_disp.max(1e-15); // 1% relative tolerance

        for (i, (dd, ds)) in dense.displacements.iter().zip(sparse.displacements.iter()).enumerate() {
            let diff = (*dd - *ds).length();
            assert!(
                diff < tol,
                "node {} displacement mismatch: dense={:?}, sparse={:?}, tol={}",
                i, dd, ds, tol
            );
        }

        // Von Mises should be in the same ballpark
        assert!(
            (dense.max_von_mises - sparse.max_von_mises).abs()
                < 0.05 * dense.max_von_mises.max(1e-12),
            "max_vm mismatch: dense={}, sparse={}",
            dense.max_von_mises,
            sparse.max_von_mises
        );
    }

    #[test]
    fn sparse_handles_large_problem() {
        // Generate a mesh with > 1000 DOFs (> 333 nodes).
        // Build a regular grid of tetrahedra.
        let nx = 8;
        let ny = 8;
        let nz = 8;
        let n_nodes_x = nx + 1;
        let n_nodes_y = ny + 1;
        let n_nodes_z = nz + 1;

        let mut nodes = Vec::new();
        for iz in 0..n_nodes_z {
            for iy in 0..n_nodes_y {
                for ix in 0..n_nodes_x {
                    nodes.push(Node {
                        position: DVec3::new(ix as f64, iy as f64, iz as f64),
                    });
                }
            }
        }

        let idx = |ix: usize, iy: usize, iz: usize| -> usize {
            iz * n_nodes_y * n_nodes_x + iy * n_nodes_x + ix
        };

        let mut elements = Vec::new();
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let n000 = idx(ix, iy, iz);
                    let n100 = idx(ix + 1, iy, iz);
                    let n010 = idx(ix, iy + 1, iz);
                    let n110 = idx(ix + 1, iy + 1, iz);
                    let n001 = idx(ix, iy, iz + 1);
                    let n101 = idx(ix + 1, iy, iz + 1);
                    let n011 = idx(ix, iy + 1, iz + 1);
                    let n111 = idx(ix + 1, iy + 1, iz + 1);

                    elements.push(Tet4 { nodes: [n000, n100, n010, n001] });
                    elements.push(Tet4 { nodes: [n100, n110, n010, n111] });
                    elements.push(Tet4 { nodes: [n001, n101, n111, n100] });
                    elements.push(Tet4 { nodes: [n001, n011, n111, n010] });
                    elements.push(Tet4 { nodes: [n001, n100, n111, n010] });
                }
            }
        }

        let mesh = FEAMesh { nodes, elements };
        let n_dof = mesh.nodes.len() * 3;
        assert!(n_dof > 1000, "n_dof = {} should be > 1000", n_dof);

        // Fix bottom face (z = 0)
        let mut bcs: Vec<BC> = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.z < 0.5 {
                bcs.push(BC::FixAll(i));
            }
        }
        // Apply force on top face
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.z > (nz as f64 - 0.5) {
                bcs.push(BC::Force(i, DVec3::new(0.0, 0.0, -1.0)));
            }
        }

        let result = solve_sparse(&mesh, 200_000.0, 0.3, &bcs);

        // Sanity checks
        assert_eq!(result.displacements.len(), mesh.nodes.len());
        assert_eq!(result.stresses.len(), mesh.elements.len());
        assert!(result.max_displacement > 0.0, "should have non-zero displacement");
        assert!(result.max_von_mises > 0.0, "should have non-zero stress");

        // Fixed nodes should have near-zero displacement
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.position.z < 0.5 {
                assert!(
                    result.displacements[i].length() < 1e-6,
                    "fixed node {} has displacement {:?}",
                    i,
                    result.displacements[i]
                );
            }
        }
    }
}
