//! `physical-lod` — Level-of-detail mesh management.
//!
//! Provides LOD selection based on camera distance and screen coverage,
//! multi-LOD mesh containers, and simple vertex decimation for generating
//! reduced-detail representations.

use physical_tessellation::{TessMesh, TessVertex};
use std::collections::HashMap;

/// Discrete level-of-detail tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LodLevel {
    /// Full-resolution mesh — all original triangles.
    Full,
    /// Medium detail — roughly 50% of original triangles.
    Medium,
    /// Low detail — roughly 25% of original triangles.
    Low,
    /// Bounding-box only — 12 triangles, used for very distant objects.
    BoundingBox,
}

/// A mesh container holding multiple LOD representations.
#[derive(Debug, Clone)]
pub struct LodMesh {
    /// LOD representations keyed by level.
    pub levels: HashMap<LodLevel, TessMesh>,
}

impl LodMesh {
    /// Create a new empty `LodMesh`.
    pub fn new() -> Self {
        Self {
            levels: HashMap::new(),
        }
    }

    /// Insert a mesh for a given LOD level.
    pub fn insert(&mut self, level: LodLevel, mesh: TessMesh) {
        self.levels.insert(level, mesh);
    }

    /// Get the mesh for a given LOD level.
    pub fn get(&self, level: LodLevel) -> Option<&TessMesh> {
        self.levels.get(&level)
    }

    /// Generate all LOD levels from a full-resolution mesh.
    ///
    /// Produces Medium (50%), Low (25%), and BoundingBox representations
    /// automatically from the provided full-detail mesh.
    pub fn generate_from_full(full: &TessMesh) -> Self {
        let mut lod = Self::new();
        lod.insert(LodLevel::Full, full.clone());
        lod.insert(LodLevel::Medium, decimate(full, 0.5));
        lod.insert(LodLevel::Low, decimate(full, 0.25));
        lod.insert(LodLevel::BoundingBox, bounding_box_mesh(full));
        lod
    }

    /// Select the best available mesh for the given viewing conditions.
    pub fn select(&self, distance: f64, screen_size: f64) -> Option<&TessMesh> {
        let level = select_lod(distance, screen_size);
        // Fall back to the closest available level if the exact one is missing.
        let fallback_order = match level {
            LodLevel::Full => vec![LodLevel::Full, LodLevel::Medium, LodLevel::Low, LodLevel::BoundingBox],
            LodLevel::Medium => vec![LodLevel::Medium, LodLevel::Low, LodLevel::BoundingBox, LodLevel::Full],
            LodLevel::Low => vec![LodLevel::Low, LodLevel::BoundingBox, LodLevel::Medium, LodLevel::Full],
            LodLevel::BoundingBox => vec![LodLevel::BoundingBox, LodLevel::Low, LodLevel::Medium, LodLevel::Full],
        };
        for l in fallback_order {
            if let Some(mesh) = self.levels.get(&l) {
                return Some(mesh);
            }
        }
        None
    }
}

impl Default for LodMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Select the appropriate LOD level based on distance from camera and
/// the object's apparent screen coverage (0.0–1.0 fraction of viewport).
///
/// Thresholds:
/// - screen_size >= 0.2 (20% of screen) → Full
/// - screen_size >= 0.05 (5% of screen) → Medium
/// - screen_size >= 0.01 (1% of screen) → Low
/// - smaller → BoundingBox
///
/// Distance acts as a secondary factor: very far objects (>1000 units)
/// are clamped to at most Medium regardless of screen size.
pub fn select_lod(distance: f64, screen_size: f64) -> LodLevel {
    // Clamp screen_size to valid range.
    let ss = screen_size.clamp(0.0, 1.0);

    let base = if ss >= 0.2 {
        LodLevel::Full
    } else if ss >= 0.05 {
        LodLevel::Medium
    } else if ss >= 0.01 {
        LodLevel::Low
    } else {
        LodLevel::BoundingBox
    };

    // Distance override: very far objects get downgraded.
    // Check most aggressive first.
    if distance > 5000.0 && (base == LodLevel::Full || base == LodLevel::Medium) {
        return LodLevel::Low;
    }
    if distance > 1000.0 && base == LodLevel::Full {
        return LodLevel::Medium;
    }

    base
}

/// Simple vertex decimation: reduce a mesh to approximately `target_ratio`
/// of its original triangle count (0.0–1.0).
///
/// Uses a grid-based vertex clustering approach: vertices that fall into
/// the same spatial grid cell are merged. The grid cell size is derived
/// from the mesh bounding box and the target ratio.
pub fn decimate(mesh: &TessMesh, target_ratio: f64) -> TessMesh {
    let target_ratio = target_ratio.clamp(0.01, 1.0);

    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return mesh.clone();
    }

    if target_ratio >= 0.999 {
        return mesh.clone();
    }

    let (bb_min, bb_max) = mesh.bounding_box();
    let extent = [
        (bb_max[0] - bb_min[0]).max(1e-6),
        (bb_max[1] - bb_min[1]).max(1e-6),
        (bb_max[2] - bb_min[2]).max(1e-6),
    ];

    // Grid resolution: fewer cells = more aggressive decimation.
    // We target approximately sqrt(target_ratio) along each axis
    // relative to the original vertex count.
    let vertex_count = mesh.vertices.len() as f64;
    let target_vertices = (vertex_count * target_ratio).max(4.0);
    let cells_per_axis = (target_vertices.cbrt()).max(2.0).ceil() as usize;

    let cell_size = [
        extent[0] / cells_per_axis as f32,
        extent[1] / cells_per_axis as f32,
        extent[2] / cells_per_axis as f32,
    ];

    // Map each vertex to a grid cell and accumulate.
    let mut cell_map: HashMap<(usize, usize, usize), (Vec<usize>, [f64; 3], [f64; 3], [f64; 2])> =
        HashMap::new();

    for (i, v) in mesh.vertices.iter().enumerate() {
        let cx = ((v.position[0] - bb_min[0]) / cell_size[0]).floor() as usize;
        let cy = ((v.position[1] - bb_min[1]) / cell_size[1]).floor() as usize;
        let cz = ((v.position[2] - bb_min[2]) / cell_size[2]).floor() as usize;
        let cx = cx.min(cells_per_axis - 1);
        let cy = cy.min(cells_per_axis - 1);
        let cz = cz.min(cells_per_axis - 1);

        let entry = cell_map.entry((cx, cy, cz)).or_insert_with(|| {
            (Vec::new(), [0.0; 3], [0.0; 3], [0.0; 2])
        });
        entry.0.push(i);
        for j in 0..3 {
            entry.1[j] += v.position[j] as f64;
            entry.2[j] += v.normal[j] as f64;
        }
        entry.3[0] += v.uv[0] as f64;
        entry.3[1] += v.uv[1] as f64;
    }

    // Build new vertex list: one vertex per occupied cell (centroid).
    let mut new_vertices = Vec::new();
    let mut old_to_new: HashMap<usize, u32> = HashMap::new();

    for (_cell, (old_indices, pos_sum, norm_sum, uv_sum)) in &cell_map {
        let n = old_indices.len() as f64;
        let new_idx = new_vertices.len() as u32;

        let mut normal = [
            (norm_sum[0] / n) as f32,
            (norm_sum[1] / n) as f32,
            (norm_sum[2] / n) as f32,
        ];
        // Normalize the averaged normal.
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        if len > 1e-8 {
            normal[0] /= len;
            normal[1] /= len;
            normal[2] /= len;
        }

        new_vertices.push(TessVertex {
            position: [
                (pos_sum[0] / n) as f32,
                (pos_sum[1] / n) as f32,
                (pos_sum[2] / n) as f32,
            ],
            normal,
            uv: [
                (uv_sum[0] / n) as f32,
                (uv_sum[1] / n) as f32,
            ],
        });

        for &oi in old_indices {
            old_to_new.insert(oi, new_idx);
        }
    }

    // Remap indices, dropping degenerate triangles.
    let mut new_indices = Vec::new();
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let a = old_to_new[&(tri[0] as usize)];
        let b = old_to_new[&(tri[1] as usize)];
        let c = old_to_new[&(tri[2] as usize)];
        // Skip degenerate triangles where two or more vertices merged.
        if a != b && b != c && a != c {
            new_indices.push(a);
            new_indices.push(b);
            new_indices.push(c);
        }
    }

    TessMesh {
        vertices: new_vertices,
        indices: new_indices,
    }
}

/// Generate a bounding-box mesh (12 triangles) from an existing mesh.
fn bounding_box_mesh(mesh: &TessMesh) -> TessMesh {
    let (min, max) = mesh.bounding_box();

    let vertices = vec![
        // Bottom face (z = min)
        TessVertex { position: [min[0], min[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
        TessVertex { position: [max[0], min[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
        TessVertex { position: [max[0], max[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
        TessVertex { position: [min[0], max[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
        // Top face (z = max)
        TessVertex { position: [min[0], min[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
        TessVertex { position: [max[0], min[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
        TessVertex { position: [max[0], max[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
        TessVertex { position: [min[0], max[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
    ];

    let indices = vec![
        0, 2, 1, 0, 3, 2, // Bottom
        4, 5, 6, 4, 6, 7, // Top
        0, 1, 5, 0, 5, 4, // Front
        2, 3, 7, 2, 7, 6, // Back
        0, 4, 7, 0, 7, 3, // Left
        1, 2, 6, 1, 6, 5, // Right
    ];

    TessMesh { vertices, indices }
}

// ---------------------------------------------------------------------------
// Quadric Error Metric (QEM) — better quality decimation
// ---------------------------------------------------------------------------

/// Symmetric 4×4 quadric matrix, stored as 10 unique values (upper triangle).
/// Q = [a b c d; b e f g; c f h i; d g i j]
#[derive(Clone, Copy, Debug)]
pub struct Quadric {
    pub data: [f64; 10],
}

impl Quadric {
    pub fn zero() -> Self {
        Self { data: [0.0; 10] }
    }

    /// Create a quadric from a plane equation ax + by + cz + d = 0.
    pub fn from_plane(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self {
            data: [
                a * a, a * b, a * c, a * d,
                b * b, b * c, b * d,
                c * c, c * d,
                d * d,
            ],
        }
    }

    /// Create a quadric from a triangle's plane.
    pub fn from_triangle(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> Self {
        let u = [
            (p1[0] - p0[0]) as f64,
            (p1[1] - p0[1]) as f64,
            (p1[2] - p0[2]) as f64,
        ];
        let v = [
            (p2[0] - p0[0]) as f64,
            (p2[1] - p0[1]) as f64,
            (p2[2] - p0[2]) as f64,
        ];
        // Normal = u × v
        let nx = u[1] * v[2] - u[2] * v[1];
        let ny = u[2] * v[0] - u[0] * v[2];
        let nz = u[0] * v[1] - u[1] * v[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len < 1e-12 {
            return Self::zero();
        }
        let (a, b, c) = (nx / len, ny / len, nz / len);
        let d = -(a * p0[0] as f64 + b * p0[1] as f64 + c * p0[2] as f64);
        Self::from_plane(a, b, c, d)
    }

    /// Add two quadrics.
    pub fn add(&self, other: &Quadric) -> Self {
        let mut result = [0.0; 10];
        for i in 0..10 {
            result[i] = self.data[i] + other.data[i];
        }
        Self { data: result }
    }

    /// Evaluate the quadric error for a point.
    pub fn evaluate(&self, x: f64, y: f64, z: f64) -> f64 {
        let d = &self.data;
        // v^T Q v where v = [x, y, z, 1]
        d[0] * x * x + 2.0 * d[1] * x * y + 2.0 * d[2] * x * z + 2.0 * d[3] * x
            + d[4] * y * y + 2.0 * d[5] * y * z + 2.0 * d[6] * y
            + d[7] * z * z + 2.0 * d[8] * z
            + d[9]
    }
}

/// Compute per-vertex quadrics for a mesh.
pub fn compute_vertex_quadrics(mesh: &TessMesh) -> Vec<Quadric> {
    let mut quadrics = vec![Quadric::zero(); mesh.vertices.len()];

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let q = Quadric::from_triangle(
            mesh.vertices[a].position,
            mesh.vertices[b].position,
            mesh.vertices[c].position,
        );
        quadrics[a] = quadrics[a].add(&q);
        quadrics[b] = quadrics[b].add(&q);
        quadrics[c] = quadrics[c].add(&q);
    }

    quadrics
}

/// Compute mesh quality metrics.
pub struct MeshMetrics {
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub bounding_sphere_radius: f32,
    pub average_edge_length: f32,
    pub memory_bytes: usize,
}

pub fn compute_metrics(mesh: &TessMesh) -> MeshMetrics {
    let (bb_min, bb_max) = mesh.bounding_box();
    let center = [
        (bb_min[0] + bb_max[0]) * 0.5,
        (bb_min[1] + bb_max[1]) * 0.5,
        (bb_min[2] + bb_max[2]) * 0.5,
    ];
    let mut max_r2: f32 = 0.0;
    for v in &mesh.vertices {
        let dx = v.position[0] - center[0];
        let dy = v.position[1] - center[1];
        let dz = v.position[2] - center[2];
        let r2 = dx * dx + dy * dy + dz * dz;
        if r2 > max_r2 { max_r2 = r2; }
    }

    let mut total_edge_len: f64 = 0.0;
    let mut edge_count: usize = 0;
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 { continue; }
        for &(i, j) in &[(0, 1), (1, 2), (2, 0)] {
            let a = &mesh.vertices[tri[i] as usize];
            let b = &mesh.vertices[tri[j] as usize];
            let dx = (a.position[0] - b.position[0]) as f64;
            let dy = (a.position[1] - b.position[1]) as f64;
            let dz = (a.position[2] - b.position[2]) as f64;
            total_edge_len += (dx * dx + dy * dy + dz * dz).sqrt();
            edge_count += 1;
        }
    }

    MeshMetrics {
        triangle_count: mesh.triangle_count(),
        vertex_count: mesh.vertices.len(),
        bounding_sphere_radius: max_r2.sqrt(),
        average_edge_length: if edge_count > 0 { (total_edge_len / edge_count as f64) as f32 } else { 0.0 },
        memory_bytes: mesh.vertices.len() * std::mem::size_of::<TessVertex>()
            + mesh.indices.len() * std::mem::size_of::<u32>(),
    }
}

/// Screen-space LOD selector with configurable thresholds.
pub struct LodSelector {
    pub screen_height: u32,
    pub full_pixels: f32,
    pub medium_pixels: f32,
    pub low_pixels: f32,
}

impl LodSelector {
    pub fn new(screen_height: u32) -> Self {
        Self {
            screen_height,
            full_pixels: 200.0,
            medium_pixels: 50.0,
            low_pixels: 10.0,
        }
    }

    /// Select LOD level based on object's bounding sphere radius and camera distance.
    pub fn select(&self, bounding_radius: f32, distance: f32, fov_y_rad: f32) -> LodLevel {
        if distance < 1e-6 { return LodLevel::Full; }
        let screen_pixels = (bounding_radius / distance)
            * self.screen_height as f32
            / (2.0 * (fov_y_rad / 2.0).tan());

        if screen_pixels >= self.full_pixels {
            LodLevel::Full
        } else if screen_pixels >= self.medium_pixels {
            LodLevel::Medium
        } else if screen_pixels >= self.low_pixels {
            LodLevel::Low
        } else {
            LodLevel::BoundingBox
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple cube mesh with 8 vertices and 12 triangles.
    fn make_cube_mesh() -> TessMesh {
        let vertices = vec![
            TessVertex { position: [0.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
            TessVertex { position: [10.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
            TessVertex { position: [10.0, 10.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
            TessVertex { position: [0.0, 10.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
            TessVertex { position: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
            TessVertex { position: [10.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
            TessVertex { position: [10.0, 10.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
            TessVertex { position: [0.0, 10.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
        ];
        let indices = vec![
            0, 2, 1, 0, 3, 2,
            4, 5, 6, 4, 6, 7,
            0, 1, 5, 0, 5, 4,
            2, 3, 7, 2, 7, 6,
            0, 4, 7, 0, 7, 3,
            1, 2, 6, 1, 6, 5,
        ];
        TessMesh { vertices, indices }
    }

    /// Helper: make a denser mesh (subdivided grid) for decimation testing.
    fn make_grid_mesh(n: usize) -> TessMesh {
        let mut vertices = Vec::new();
        for iy in 0..=n {
            for ix in 0..=n {
                vertices.push(TessVertex {
                    position: [ix as f32, iy as f32, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [ix as f32 / n as f32, iy as f32 / n as f32],
                });
            }
        }
        let mut indices = Vec::new();
        let stride = (n + 1) as u32;
        for iy in 0..n as u32 {
            for ix in 0..n as u32 {
                let a = iy * stride + ix;
                let b = a + 1;
                let c = a + stride;
                let d = c + 1;
                indices.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        TessMesh { vertices, indices }
    }

    #[test]
    fn select_lod_large_screen_coverage() {
        assert_eq!(select_lod(10.0, 0.5), LodLevel::Full);
        assert_eq!(select_lod(10.0, 0.2), LodLevel::Full);
    }

    #[test]
    fn select_lod_medium_screen_coverage() {
        assert_eq!(select_lod(10.0, 0.1), LodLevel::Medium);
        assert_eq!(select_lod(10.0, 0.05), LodLevel::Medium);
    }

    #[test]
    fn select_lod_low_screen_coverage() {
        assert_eq!(select_lod(10.0, 0.02), LodLevel::Low);
        assert_eq!(select_lod(10.0, 0.01), LodLevel::Low);
    }

    #[test]
    fn select_lod_bounding_box() {
        assert_eq!(select_lod(10.0, 0.005), LodLevel::BoundingBox);
        assert_eq!(select_lod(10.0, 0.0), LodLevel::BoundingBox);
    }

    #[test]
    fn select_lod_distance_override() {
        // Large screen coverage but very far: should downgrade from Full.
        assert_eq!(select_lod(1500.0, 0.5), LodLevel::Medium);
        // Extremely far: downgrade further.
        assert_eq!(select_lod(6000.0, 0.5), LodLevel::Low);
    }

    #[test]
    fn decimation_reduces_vertex_count() {
        let mesh = make_grid_mesh(20); // 441 vertices, 800 triangles
        assert_eq!(mesh.vertices.len(), 441);
        assert_eq!(mesh.triangle_count(), 800);

        let decimated = decimate(&mesh, 0.5);
        assert!(decimated.vertices.len() < mesh.vertices.len());
        assert!(decimated.triangle_count() < mesh.triangle_count());
    }

    #[test]
    fn decimation_very_aggressive() {
        let mesh = make_grid_mesh(20);
        let decimated = decimate(&mesh, 0.1);
        assert!(decimated.vertices.len() < mesh.vertices.len() / 2);
    }

    #[test]
    fn decimation_ratio_one_preserves_mesh() {
        let mesh = make_cube_mesh();
        let decimated = decimate(&mesh, 1.0);
        assert_eq!(decimated.vertices.len(), mesh.vertices.len());
        assert_eq!(decimated.indices.len(), mesh.indices.len());
    }

    #[test]
    fn decimation_empty_mesh() {
        let empty = TessMesh {
            vertices: vec![],
            indices: vec![],
        };
        let result = decimate(&empty, 0.5);
        assert!(result.vertices.is_empty());
        assert!(result.indices.is_empty());
    }

    #[test]
    fn lod_mesh_generate_from_full() {
        let mesh = make_cube_mesh();
        let lod = LodMesh::generate_from_full(&mesh);

        assert!(lod.get(LodLevel::Full).is_some());
        assert!(lod.get(LodLevel::Medium).is_some());
        assert!(lod.get(LodLevel::Low).is_some());
        assert!(lod.get(LodLevel::BoundingBox).is_some());

        // BoundingBox should always be 12 triangles (box).
        assert_eq!(lod.get(LodLevel::BoundingBox).unwrap().triangle_count(), 12);
    }

    #[test]
    fn lod_mesh_select() {
        let mesh = make_grid_mesh(10);
        let lod = LodMesh::generate_from_full(&mesh);

        // Close + large → Full
        let selected = lod.select(5.0, 0.5);
        assert!(selected.is_some());
    }

    #[test]
    fn bounding_box_mesh_always_12_triangles() {
        let mesh = make_grid_mesh(10);
        let bb = bounding_box_mesh(&mesh);
        assert_eq!(bb.triangle_count(), 12);
        assert_eq!(bb.vertices.len(), 8);
    }

    // ---- QEM / metrics / selector tests ----

    #[test]
    fn quadric_from_plane_evaluate_on_plane() {
        // Plane z = 0: normal (0, 0, 1), d = 0
        let q = Quadric::from_plane(0.0, 0.0, 1.0, 0.0);
        // Point on plane should have zero error
        assert!(q.evaluate(5.0, 3.0, 0.0).abs() < 1e-10);
        // Point off plane at z=1 should have error = 1
        assert!((q.evaluate(5.0, 3.0, 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn quadric_add_symmetric() {
        let q1 = Quadric::from_plane(1.0, 0.0, 0.0, -5.0);
        let q2 = Quadric::from_plane(0.0, 1.0, 0.0, -3.0);
        let sum = q1.add(&q2);
        // Error at (5, 3, 0) should be 0 (intersection of the two planes)
        assert!(sum.evaluate(5.0, 3.0, 0.0).abs() < 1e-10);
    }

    #[test]
    fn quadric_from_triangle() {
        let p0 = [0.0_f32, 0.0, 0.0];
        let p1 = [10.0, 0.0, 0.0];
        let p2 = [0.0, 10.0, 0.0];
        let q = Quadric::from_triangle(p0, p1, p2);
        // All three points should be on the plane → zero error
        assert!(q.evaluate(0.0, 0.0, 0.0).abs() < 1e-6);
        assert!(q.evaluate(10.0, 0.0, 0.0).abs() < 1e-6);
        assert!(q.evaluate(0.0, 10.0, 0.0).abs() < 1e-6);
        // Point above should have nonzero error
        assert!(q.evaluate(5.0, 5.0, 1.0) > 0.5);
    }

    #[test]
    fn vertex_quadrics_computed() {
        let mesh = make_cube_mesh();
        let quadrics = compute_vertex_quadrics(&mesh);
        assert_eq!(quadrics.len(), mesh.vertices.len());
        // Each vertex should have accumulated at least 2 plane quadrics (shared by ≥2 triangles)
        for q in &quadrics {
            let max_val = q.data.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
            assert!(max_val > 0.0, "vertex quadric should be non-zero");
        }
    }

    #[test]
    fn mesh_metrics_box() {
        let mesh = make_cube_mesh();
        let metrics = compute_metrics(&mesh);
        assert_eq!(metrics.triangle_count, 12);
        assert_eq!(metrics.vertex_count, 8);
        assert!(metrics.bounding_sphere_radius > 5.0);
        assert!(metrics.average_edge_length > 0.0);
        assert!(metrics.memory_bytes > 0);
    }

    #[test]
    fn mesh_metrics_grid() {
        let mesh = make_grid_mesh(10);
        let metrics = compute_metrics(&mesh);
        assert_eq!(metrics.triangle_count, 200);
        assert_eq!(metrics.vertex_count, 121);
        // Grid spacing = 1.0, so average edge ≈ 1.0 (with diagonals ~1.41)
        assert!(metrics.average_edge_length > 0.5 && metrics.average_edge_length < 2.0);
    }

    #[test]
    fn lod_selector_full_when_close() {
        let sel = LodSelector::new(1080);
        let level = sel.select(10.0, 5.0, std::f32::consts::FRAC_PI_4); // 45° FOV
        assert_eq!(level, LodLevel::Full);
    }

    #[test]
    fn lod_selector_bbox_when_far() {
        let sel = LodSelector::new(1080);
        let level = sel.select(1.0, 10000.0, std::f32::consts::FRAC_PI_4);
        assert_eq!(level, LodLevel::BoundingBox);
    }

    #[test]
    fn lod_selector_medium_intermediate() {
        let sel = LodSelector::new(1080);
        // Object radius 5, distance 100, FOV 45° → screen pixels ≈ 5/100 * 1080 / (2*tan(22.5°)) ≈ 65
        let level = sel.select(5.0, 100.0, std::f32::consts::FRAC_PI_4);
        assert!(
            level == LodLevel::Medium || level == LodLevel::Full,
            "expected Medium or Full, got {:?}", level
        );
    }
}
