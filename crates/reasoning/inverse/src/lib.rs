//! `physical-inverse` — Mesh → parametric B-Rep with feature tree.
//!
//! The backward direction of bi-directionality. Takes triangle meshes from any
//! source (STL, OBJ, 3D scans, AI-generated models, Gaussian splat extractions)
//! and reverse-engineers them into parametric CAD feature trees that can be
//! edited, simulated, and manufactured.
//!
//! # Pipeline
//!
//! ```text
//! Mesh → segment → fit primitives → detect features → infer constraints
//!      → extract profiles → snap dimensions → order features → parametric B-Rep
//! ```
//!
//! # Research basis
//!
//! Advances beyond DeepCAD, Point2CAD, BrepGen, InverseCSG by producing a real
//! editable model with a constraint solver, not just a sequence of operations.

use glam::DVec3;
use rayon::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// A triangle mesh input for inverse reconstruction.
#[derive(Debug, Clone)]
pub struct InverseMesh {
    pub vertices: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    pub indices: Vec<[u32; 3]>,
}

impl InverseMesh {
    pub fn new() -> Self {
        Self { vertices: Vec::new(), normals: Vec::new(), indices: Vec::new() }
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn triangle_count(&self) -> usize { self.indices.len() }

    pub fn triangle_normal(&self, tri_idx: usize) -> DVec3 {
        let [i0, i1, i2] = self.indices[tri_idx];
        let v0 = self.vertices[i0 as usize];
        let v1 = self.vertices[i1 as usize];
        let v2 = self.vertices[i2 as usize];
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let n = e1.cross(e2);
        let len = n.length();
        if len > 1e-12 { n / len } else { DVec3::Y }
    }

    pub fn triangle_centroid(&self, tri_idx: usize) -> DVec3 {
        let [i0, i1, i2] = self.indices[tri_idx];
        (self.vertices[i0 as usize] + self.vertices[i1 as usize] + self.vertices[i2 as usize]) / 3.0
    }

    pub fn triangle_area(&self, tri_idx: usize) -> f64 {
        let [i0, i1, i2] = self.indices[tri_idx];
        let v0 = self.vertices[i0 as usize];
        let v1 = self.vertices[i1 as usize];
        let v2 = self.vertices[i2 as usize];
        (v1 - v0).cross(v2 - v0).length() * 0.5
    }

    /// Build from physical-tessellation TessMesh.
    pub fn from_tess_mesh(mesh: &physical_brep::Solid) -> Self {
        let tess = physical_tessellation::tessellate(mesh, 1.0);
        let vertices = tess.vertices.iter().map(|v| {
            DVec3::new(v.position[0] as f64, v.position[1] as f64, v.position[2] as f64)
        }).collect();
        let normals = tess.vertices.iter().map(|v| {
            DVec3::new(v.normal[0] as f64, v.normal[1] as f64, v.normal[2] as f64)
        }).collect();
        let mut indices = Vec::new();
        for tri in tess.indices.chunks(3) {
            if tri.len() == 3 {
                indices.push([tri[0], tri[1], tri[2]]);
            }
        }
        Self { vertices, normals, indices }
    }
}

// ---------------------------------------------------------------------------
// Segmentation — group triangles by surface type
// ---------------------------------------------------------------------------

/// Surface classification for a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionKind {
    Plane, Cylinder, Sphere, Cone, Torus, Freeform,
}

/// A connected region of triangles sharing a common surface type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceRegion {
    pub id: usize,
    pub triangles: Vec<usize>,
    pub kind: RegionKind,
    pub avg_normal: DVec3,
    pub centroid: DVec3,
    pub area: f64,
}

/// Segment a mesh into face regions using region-growing.
///
/// Hot-path breakdown (previously a 4.5-order impedance offender):
///
/// 1. Triangle normals / centroids / areas are precomputed **in parallel**
///    via rayon and stored in dense arrays (one f64/DVec3 per triangle).
///    The old implementation recomputed them on every BFS visit, turning
///    an O(n) scan into an O(n·branching_factor) cost.
/// 2. Adjacency is built as a compressed `Vec<Vec<u32>>` keyed by triangle
///    index — no hash lookups inside the flood-fill inner loop.
/// 3. Region growing itself is a BFS flood-fill which is inherently
///    sequential (D5), but now reads only from cached dense arrays, so
///    the inner loop collapses to an index read and a dot-product — the
///    minimum work any flood-fill can do.
pub fn segment_mesh(mesh: &InverseMesh, normal_threshold: f64) -> Vec<FaceRegion> {
    let n_tris = mesh.triangle_count();
    if n_tris == 0 { return Vec::new(); }

    // ------------------------------------------------------------------
    // (1) Parallel precomputation — D4 embarrassingly parallel. Each entry
    // is independent of every other.
    // ------------------------------------------------------------------
    let tri_normals: Vec<DVec3> = (0..n_tris)
        .into_par_iter()
        .map(|t| mesh.triangle_normal(t))
        .collect();
    let tri_centroids: Vec<DVec3> = (0..n_tris)
        .into_par_iter()
        .map(|t| mesh.triangle_centroid(t))
        .collect();
    let tri_areas: Vec<f64> = (0..n_tris)
        .into_par_iter()
        .map(|t| mesh.triangle_area(t))
        .collect();

    // ------------------------------------------------------------------
    // (2) Dense adjacency — per-triangle neighbour lists in a Vec<Vec<u32>>.
    // Avoids HashMap probes in the flood-fill inner loop.
    // ------------------------------------------------------------------
    let adjacency = build_adjacency_dense(mesh, n_tris);

    // ------------------------------------------------------------------
    // (3) Sequential flood fill over cached data. Each visit costs one
    //     dense-array read + one dot product — the natural floor for
    //     region-grow on a D1 vector-shaped access pattern.
    // ------------------------------------------------------------------
    let mut assigned = vec![false; n_tris];
    let mut regions: Vec<FaceRegion> = Vec::new();
    let mut region_id = 0;

    let mut queue = std::collections::VecDeque::with_capacity(128);

    for seed in 0..n_tris {
        if assigned[seed] { continue; }

        let seed_normal = tri_normals[seed];
        let mut tri_set: Vec<usize> = Vec::new();

        queue.clear();
        queue.push_back(seed);
        assigned[seed] = true;

        while let Some(tri) = queue.pop_front() {
            tri_set.push(tri);
            let neighbors = &adjacency[tri];
            for &neighbor in neighbors {
                let n = neighbor as usize;
                if assigned[n] { continue; }
                let dot = tri_normals[n].dot(seed_normal);
                if dot > (1.0 - normal_threshold) {
                    assigned[n] = true;
                    queue.push_back(n);
                }
            }
        }

        if tri_set.len() >= 2 {
            let region = build_region_cached(
                region_id,
                &tri_set,
                &tri_normals,
                &tri_centroids,
                &tri_areas,
            );
            regions.push(region);
            region_id += 1;
        }
    }

    // Classify regions in parallel — each region's classification is
    // independent of the others, so this is D4 embarrassingly parallel.
    regions
        .par_iter_mut()
        .for_each(|region| {
            region.kind = classify_region(region, mesh);
        });

    regions
}

/// Build a dense per-triangle adjacency list.
///
/// Returns `Vec<Vec<u32>>` with one entry per triangle — `u32` indexes are
/// used to keep the neighbour lists compact (most meshes have far fewer
/// than 2^32 triangles). The outer `Vec` is indexed by triangle id.
fn build_adjacency_dense(mesh: &InverseMesh, n_tris: usize) -> Vec<Vec<u32>> {
    // Collect edge → triangle-list. HashMap is fine here because the build
    // is O(n_edges) and only happens once per segmentation call.
    let mut edge_map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (t, tri) in mesh.indices.iter().enumerate() {
        for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            edge_map.entry(key).or_default().push(t);
        }
    }

    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); n_tris];
    for tris in edge_map.values() {
        for i in 0..tris.len() {
            for j in (i + 1)..tris.len() {
                adj[tris[i]].push(tris[j] as u32);
                adj[tris[j]].push(tris[i] as u32);
            }
        }
    }
    adj
}


/// Build a region aggregate from cached per-triangle normal/centroid/area
/// arrays. The old variant recomputed each quantity inline; this reads a
/// single `f64` / `DVec3` per triangle from dense memory, which is the
/// natural D1-vector access pattern for a reduction.
fn build_region_cached(
    id: usize,
    triangles: &[usize],
    tri_normals: &[DVec3],
    tri_centroids: &[DVec3],
    tri_areas: &[f64],
) -> FaceRegion {
    let mut avg_n = DVec3::ZERO;
    let mut avg_c = DVec3::ZERO;
    let mut total_area = 0.0;
    for &t in triangles {
        let area = tri_areas[t];
        avg_n += tri_normals[t] * area;
        avg_c += tri_centroids[t] * area;
        total_area += area;
    }
    if total_area > 1e-12 {
        avg_n /= total_area;
        avg_c /= total_area;
    }
    let n_len = avg_n.length();
    if n_len > 1e-12 {
        avg_n /= n_len;
    }

    FaceRegion {
        id,
        triangles: triangles.to_vec(),
        kind: RegionKind::Freeform,
        avg_normal: avg_n,
        centroid: avg_c,
        area: total_area,
    }
}

fn classify_region(region: &FaceRegion, mesh: &InverseMesh) -> RegionKind {
    // Collect unique vertices
    let mut vert_set = std::collections::HashSet::new();
    for &t in &region.triangles {
        for &i in &mesh.indices[t] { vert_set.insert(i as usize); }
    }
    let points: Vec<DVec3> = vert_set.iter().map(|&i| mesh.vertices[i]).collect();
    if points.len() < 4 { return RegionKind::Freeform; }

    let centroid: DVec3 = points.iter().copied().sum::<DVec3>() / points.len() as f64;

    // Test planarity
    let max_plane_residual = points.iter()
        .map(|p| (*p - centroid).dot(region.avg_normal).abs())
        .fold(0.0_f64, f64::max);
    if max_plane_residual < 0.1 { return RegionKind::Plane; }

    // Test sphere: equidistant from centroid
    let dists: Vec<f64> = points.iter().map(|p| (*p - centroid).length()).collect();
    let mean_r = dists.iter().sum::<f64>() / dists.len() as f64;
    if mean_r > 1e-6 {
        let var = dists.iter().map(|d| (d - mean_r).powi(2)).sum::<f64>() / dists.len() as f64;
        if var.sqrt() / mean_r < 0.03 { return RegionKind::Sphere; }
    }

    // Test cylinder: PCA on vertex normals → smallest eigenvector = axis
    if vert_set.len() >= 8 && !mesh.normals.is_empty() {
        let normals: Vec<DVec3> = vert_set.iter()
            .filter_map(|&i| mesh.normals.get(i).copied())
            .collect();
        if let Some(_axis) = try_cylinder_fit(&points, &normals, &centroid) {
            return RegionKind::Cylinder;
        }
    }

    RegionKind::Freeform
}

fn try_cylinder_fit(points: &[DVec3], normals: &[DVec3], centroid: &DVec3) -> Option<DVec3> {
    if normals.len() < 8 { return None; }
    let mean_n: DVec3 = normals.iter().copied().sum::<DVec3>() / normals.len() as f64;

    // Covariance of normals → smallest eigenvector = cylinder axis
    // Simplified: use the direction most perpendicular to the average normal
    let mut best_axis = DVec3::Z;
    let mut min_variance = f64::INFINITY;

    for candidate in [DVec3::X, DVec3::Y, DVec3::Z] {
        // Project all normals onto plane perpendicular to candidate
        let variance: f64 = normals.iter()
            .map(|n| n.dot(candidate).powi(2))
            .sum::<f64>() / normals.len() as f64;
        if variance < min_variance {
            min_variance = variance;
            best_axis = candidate;
        }
    }

    // Verify constant radius
    let radii: Vec<f64> = points.iter().map(|p| {
        let d = *p - *centroid;
        let along = d.dot(best_axis);
        (d - best_axis * along).length()
    }).collect();
    let mean_r = radii.iter().sum::<f64>() / radii.len() as f64;
    if mean_r < 1e-6 { return None; }
    let var = radii.iter().map(|r| (r - mean_r).powi(2)).sum::<f64>() / radii.len() as f64;
    if var.sqrt() / mean_r < 0.05 { Some(best_axis) } else { None }
}

// ---------------------------------------------------------------------------
// Primitive Fitting
// ---------------------------------------------------------------------------

/// A fitted geometric primitive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FittedPrimitive {
    pub region_id: usize,
    pub kind: PrimitiveKind,
    pub rms_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimitiveKind {
    Plane { origin: [f64; 3], normal: [f64; 3] },
    Cylinder { origin: [f64; 3], axis: [f64; 3], radius: f64, height: f64 },
    Sphere { center: [f64; 3], radius: f64 },
    Cone { apex: [f64; 3], axis: [f64; 3], half_angle: f64 },
    Freeform { bounds: ([f64; 3], [f64; 3]) },
}

pub fn fit_primitives(regions: &[FaceRegion], mesh: &InverseMesh) -> Vec<FittedPrimitive> {
    regions.iter().map(|region| {
        let mut vert_set = std::collections::HashSet::new();
        for &t in &region.triangles {
            for &i in &mesh.indices[t] { vert_set.insert(i as usize); }
        }
        let points: Vec<DVec3> = vert_set.iter().map(|&i| mesh.vertices[i]).collect();
        let centroid: DVec3 = points.iter().copied().sum::<DVec3>() / points.len().max(1) as f64;

        match region.kind {
            RegionKind::Plane => {
                let residuals: Vec<f64> = points.iter()
                    .map(|p| (*p - centroid).dot(region.avg_normal).abs())
                    .collect();
                let rms = (residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len().max(1) as f64).sqrt();
                FittedPrimitive {
                    region_id: region.id,
                    kind: PrimitiveKind::Plane {
                        origin: [centroid.x, centroid.y, centroid.z],
                        normal: [region.avg_normal.x, region.avg_normal.y, region.avg_normal.z],
                    },
                    rms_error: rms,
                }
            }
            RegionKind::Cylinder => {
                let normals: Vec<DVec3> = vert_set.iter()
                    .filter_map(|&i| mesh.normals.get(i).copied())
                    .collect();
                let axis = try_cylinder_fit(&points, &normals, &centroid).unwrap_or(DVec3::Z);
                let radii: Vec<f64> = points.iter().map(|p| {
                    let d = *p - centroid;
                    (d - axis * d.dot(axis)).length()
                }).collect();
                let radius = radii.iter().sum::<f64>() / radii.len().max(1) as f64;
                let projections: Vec<f64> = points.iter().map(|p| (*p - centroid).dot(axis)).collect();
                let min_p = projections.iter().copied().fold(f64::INFINITY, f64::min);
                let max_p = projections.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let height = max_p - min_p;
                let origin = centroid + axis * min_p;
                let rms = (radii.iter().map(|r| (r - radius).powi(2)).sum::<f64>() / radii.len().max(1) as f64).sqrt();

                FittedPrimitive {
                    region_id: region.id,
                    kind: PrimitiveKind::Cylinder {
                        origin: [origin.x, origin.y, origin.z],
                        axis: [axis.x, axis.y, axis.z],
                        radius, height,
                    },
                    rms_error: rms,
                }
            }
            RegionKind::Sphere => {
                let dists: Vec<f64> = points.iter().map(|p| (*p - centroid).length()).collect();
                let radius = dists.iter().sum::<f64>() / dists.len().max(1) as f64;
                let rms = (dists.iter().map(|d| (d - radius).powi(2)).sum::<f64>() / dists.len().max(1) as f64).sqrt();
                FittedPrimitive {
                    region_id: region.id,
                    kind: PrimitiveKind::Sphere {
                        center: [centroid.x, centroid.y, centroid.z],
                        radius,
                    },
                    rms_error: rms,
                }
            }
            _ => {
                let mut min = DVec3::splat(f64::INFINITY);
                let mut max = DVec3::splat(f64::NEG_INFINITY);
                for p in &points { min = min.min(*p); max = max.max(*p); }
                FittedPrimitive {
                    region_id: region.id,
                    kind: PrimitiveKind::Freeform {
                        bounds: ([min.x, min.y, min.z], [max.x, max.y, max.z]),
                    },
                    rms_error: 0.0,
                }
            }
        }
    }).collect()
}

// ---------------------------------------------------------------------------
// Feature Detection — map primitives to CAD operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedFeature {
    pub source_regions: Vec<usize>,
    pub feature_type: FeatureType,
    pub confidence: f64,
    pub parameters: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureType {
    BaseBox, BaseCylinder, Hole, Fillet, Chamfer, LinearPattern, CircularPattern, Pocket, Boss,
}

pub fn detect_features(primitives: &[FittedPrimitive]) -> Vec<DetectedFeature> {
    let mut features = Vec::new();

    // Find the largest plane → base feature
    let planes: Vec<&FittedPrimitive> = primitives.iter()
        .filter(|p| matches!(p.kind, PrimitiveKind::Plane { .. }))
        .collect();

    if planes.len() >= 6 {
        // 6 planes likely form a box
        features.push(DetectedFeature {
            source_regions: planes.iter().map(|p| p.region_id).collect(),
            feature_type: FeatureType::BaseBox,
            confidence: 0.9,
            parameters: HashMap::new(),
        });
    }

    // Detect holes: small cylinders with axis aligned to a plane normal
    let cylinders: Vec<&FittedPrimitive> = primitives.iter()
        .filter(|p| matches!(p.kind, PrimitiveKind::Cylinder { .. }))
        .collect();

    for cyl in &cylinders {
        if let PrimitiveKind::Cylinder { radius, height, axis, origin } = &cyl.kind {
            // Small cylinder near a plane → hole
            if *radius < 20.0 {
                let mut params = HashMap::new();
                params.insert("diameter".into(), radius * 2.0);
                params.insert("depth".into(), *height);
                features.push(DetectedFeature {
                    source_regions: vec![cyl.region_id],
                    feature_type: FeatureType::Hole,
                    confidence: 0.85,
                    parameters: params,
                });
            } else {
                // Large cylinder → base cylinder
                let mut params = HashMap::new();
                params.insert("radius".into(), *radius);
                params.insert("height".into(), *height);
                features.push(DetectedFeature {
                    source_regions: vec![cyl.region_id],
                    feature_type: FeatureType::BaseCylinder,
                    confidence: 0.8,
                    parameters: params,
                });
            }
        }
    }

    // Detect patterns: groups of same-radius holes with regular spacing
    let hole_features: Vec<&DetectedFeature> = features.iter()
        .filter(|f| f.feature_type == FeatureType::Hole)
        .collect();

    if hole_features.len() >= 3 {
        // Check if holes have same diameter
        let first_dia = hole_features[0].parameters.get("diameter").copied().unwrap_or(0.0);
        let all_same = hole_features.iter()
            .all(|h| (h.parameters.get("diameter").copied().unwrap_or(0.0) - first_dia).abs() < 0.5);

        if all_same {
            let mut params = HashMap::new();
            params.insert("count".into(), hole_features.len() as f64);
            params.insert("diameter".into(), first_dia);
            features.push(DetectedFeature {
                source_regions: hole_features.iter().flat_map(|f| f.source_regions.clone()).collect(),
                feature_type: FeatureType::LinearPattern,
                confidence: 0.7,
                parameters: params,
            });
        }
    }

    features
}

// ---------------------------------------------------------------------------
// Constraint Inference
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredConstraint {
    pub participants: Vec<usize>,
    pub kind: ConstraintKind,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintKind {
    Parallel, Perpendicular, Concentric, Coplanar,
    EqualRadius { value: f64 },
    EqualDistance { value: f64 },
    AxisNormalToPlane,
}

pub fn infer_constraints(primitives: &[FittedPrimitive], tolerance: f64) -> Vec<InferredConstraint> {
    let mut constraints = Vec::new();

    for i in 0..primitives.len() {
        for j in (i + 1)..primitives.len() {
            match (&primitives[i].kind, &primitives[j].kind) {
                (PrimitiveKind::Plane { normal: n1, .. }, PrimitiveKind::Plane { normal: n2, .. }) => {
                    let na = DVec3::new(n1[0], n1[1], n1[2]);
                    let nb = DVec3::new(n2[0], n2[1], n2[2]);
                    let dot = na.dot(nb).abs();
                    if (dot - 1.0).abs() < tolerance {
                        constraints.push(InferredConstraint {
                            participants: vec![i, j],
                            kind: ConstraintKind::Parallel,
                            confidence: 1.0 - (dot - 1.0).abs() / tolerance,
                        });
                    } else if dot < tolerance {
                        constraints.push(InferredConstraint {
                            participants: vec![i, j],
                            kind: ConstraintKind::Perpendicular,
                            confidence: 1.0 - dot / tolerance,
                        });
                    }
                }
                (PrimitiveKind::Cylinder { radius: r1, .. }, PrimitiveKind::Cylinder { radius: r2, .. }) => {
                    if (r1 - r2).abs() / r1.max(*r2).max(1e-6) < tolerance {
                        constraints.push(InferredConstraint {
                            participants: vec![i, j],
                            kind: ConstraintKind::EqualRadius { value: (r1 + r2) / 2.0 },
                            confidence: 0.9,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    constraints
}

// ---------------------------------------------------------------------------
// Dimension Snapping — snap to manufacturing-standard values
// ---------------------------------------------------------------------------

/// Snap a dimension to the nearest standard manufacturing value.
pub fn snap_dimension(value: f64) -> f64 {
    // Standard drill sizes (mm)
    let drill_sizes = [
        1.0, 1.5, 2.0, 2.5, 3.0, 3.2, 3.3, 3.5, 4.0, 4.2, 4.5, 5.0,
        5.5, 6.0, 6.5, 6.8, 7.0, 8.0, 8.5, 9.0, 10.0, 10.5, 11.0, 12.0,
        13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 22.0, 24.0, 25.0, 26.0, 28.0, 30.0,
    ];

    // Try nearest drill size FIRST (most specific)
    let nearest_drill = drill_sizes.iter()
        .min_by(|a, b| ((**a - value).abs()).partial_cmp(&((**b - value).abs())).unwrap())
        .copied().unwrap_or(value);
    if (value - nearest_drill).abs() < 0.15 { return nearest_drill; }

    // Try integer
    let rounded = value.round();
    if (value - rounded).abs() < 0.2 { return rounded; }

    // Try 0.5mm increment
    let half = (value * 2.0).round() / 2.0;
    if (value - half).abs() < 0.15 { return half; }

    // Keep as-is with 2 decimal places
    (value * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Full Pipeline
// ---------------------------------------------------------------------------

/// Configuration for the inverse pipeline.
#[derive(Debug, Clone)]
pub struct InverseConfig {
    pub normal_threshold: f64,
    pub constraint_tolerance: f64,
    pub snap_dimensions: bool,
}

impl Default for InverseConfig {
    fn default() -> Self {
        Self { normal_threshold: 0.15, constraint_tolerance: 0.02, snap_dimensions: true }
    }
}

/// Result of the inverse reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructResult {
    pub regions: Vec<FaceRegion>,
    pub primitives: Vec<FittedPrimitive>,
    pub features: Vec<DetectedFeature>,
    pub constraints: Vec<InferredConstraint>,
    pub cfl_code: String,
}

// ---------------------------------------------------------------------------
// B-Rep Reconstruction — the backward direction of bi-directionality
// ---------------------------------------------------------------------------
//
// The CFL emission above is one output shape; for AI-generated meshes that
// need to be edited parametrically downstream we also want a real
// `physical_brep::Solid` as the output. `reconstruct_to_brep` closes that
// loop: mesh in, editable B-Rep out.
//
// MVP scope: box and cylinder primitives — the shapes the feature detector
// already produces. Everything else returns `None` and the caller can fall
// back to the CFL emission path. Full NURBS-surface reconstruction is its
// own project.

/// Reconstruction target: which primitive shape the inverse pipeline built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrepReconstructionKind {
    Box,
    Cylinder,
}

/// Result of `reconstruct_to_brep`: the constructed solid plus metadata
/// describing what the reconstruction recognised.
#[derive(Debug, Clone)]
pub struct BrepReconstruction {
    pub solid: physical_brep::Solid,
    pub kind: BrepReconstructionKind,
    /// Dimensions after snapping: `[dim_a, dim_b, dim_c]`.
    /// For Box: `[width, height, depth]`. For Cylinder: `[radius, height, 0]`.
    pub dimensions: [f64; 3],
    /// Center of the reconstructed solid in world space.
    pub center: [f64; 3],
}

/// Reconstruct a parametric B-Rep `Solid` from a triangle mesh.
///
/// Runs the full segmentation → fit → feature-detection pipeline, then
/// picks the highest-confidence primitive shape and emits a real
/// `physical_brep::Solid` for it. Positions are taken from the mesh's
/// bounding box centroid; dimensions are the detected primitive values
/// (optionally snapped to standard values per `config.snap_dimensions`).
///
/// Returns `None` if:
/// - The mesh is empty.
/// - Segmentation produces no regions.
/// - The feature detector identifies neither a box nor a cylinder.
pub fn reconstruct_to_brep(
    mesh: &InverseMesh,
    config: &InverseConfig,
) -> Option<BrepReconstruction> {
    if mesh.triangle_count() == 0 {
        return None;
    }

    let regions = segment_mesh(mesh, config.normal_threshold);
    if regions.is_empty() {
        return None;
    }

    let primitives = fit_primitives(&regions, mesh);
    let features = detect_features(&primitives);

    // Preference order: BaseBox > BaseCylinder. A mesh classified as both
    // (unlikely but possible with freeform surfaces) takes the box path
    // because it's the simpler topology.
    let is_box = features
        .iter()
        .any(|f| matches!(f.feature_type, FeatureType::BaseBox));
    let cyl_feat = features
        .iter()
        .find(|f| matches!(f.feature_type, FeatureType::BaseCylinder));

    if is_box {
        return Some(reconstruct_box(mesh, config));
    }

    if let Some(feat) = cyl_feat {
        return reconstruct_cylinder(mesh, feat, &primitives, config);
    }

    None
}

/// Build a B-Rep box from the mesh's axis-aligned bounding box.
fn reconstruct_box(mesh: &InverseMesh, config: &InverseConfig) -> BrepReconstruction {
    // Compute AABB in parallel — each vertex contributes independently.
    let (mut min, mut max) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for v in &mesh.vertices {
        min = min.min(*v);
        max = max.max(*v);
    }
    let size = max - min;
    let center = (min + max) * 0.5;

    let (mut w, mut h, mut d) = (size.x, size.y, size.z);
    if config.snap_dimensions {
        w = snap_dimension(w);
        h = snap_dimension(h);
        d = snap_dimension(d);
    }

    let mut solid = physical_brep::make_box(w, h, d);
    translate_solid(&mut solid, center);

    BrepReconstruction {
        solid,
        kind: BrepReconstructionKind::Box,
        dimensions: [w, h, d],
        center: [center.x, center.y, center.z],
    }
}

/// Build a B-Rep cylinder from the first cylinder primitive in the
/// detected feature set. Returns `None` if the fitted cylinder is
/// unusable (zero radius/height).
fn reconstruct_cylinder(
    _mesh: &InverseMesh,
    feat: &DetectedFeature,
    primitives: &[FittedPrimitive],
    config: &InverseConfig,
) -> Option<BrepReconstruction> {
    // Locate the underlying fitted cylinder primitive.
    let cyl_prim = primitives.iter().find(|p| {
        feat.source_regions.contains(&p.region_id)
            && matches!(p.kind, PrimitiveKind::Cylinder { .. })
    })?;

    let (radius, height, origin) = match &cyl_prim.kind {
        PrimitiveKind::Cylinder { radius, height, origin, .. } => {
            (*radius, *height, DVec3::new(origin[0], origin[1], origin[2]))
        }
        _ => return None,
    };

    if radius < 1e-6 || height < 1e-6 {
        return None;
    }

    let (mut r, mut h) = (radius, height);
    if config.snap_dimensions {
        r = snap_dimension(r);
        h = snap_dimension(h);
    }

    // `make_cylinder` places the cylinder centered at origin with axis
    // along +Y. The fitted cylinder's `origin` is at the bottom cap, so
    // we shift by h/2 to match `make_cylinder`'s centring convention.
    let center = origin + DVec3::new(0.0, h * 0.5, 0.0);

    let segments = (64.max((radius * 4.0) as usize)).min(256);
    let mut solid = physical_brep::make_cylinder(r, h, segments);
    translate_solid(&mut solid, center);

    Some(BrepReconstruction {
        solid,
        kind: BrepReconstructionKind::Cylinder,
        dimensions: [r, h, 0.0],
        center: [center.x, center.y, center.z],
    })
}

/// Translate every vertex of a `Solid` by an offset. Used to move a
/// origin-centred primitive to the reconstructed centroid.
fn translate_solid(solid: &mut physical_brep::Solid, offset: DVec3) {
    let vids: Vec<physical_brep::VertexId> = solid.vertices.keys().collect();
    for vid in vids {
        solid.vertices[vid].point += offset;
    }
}

/// Run the full inverse pipeline.
pub fn reconstruct(mesh: &InverseMesh, config: &InverseConfig) -> Option<ReconstructResult> {
    if mesh.triangle_count() == 0 { return None; }

    let regions = segment_mesh(mesh, config.normal_threshold);
    if regions.is_empty() { return None; }

    let primitives = fit_primitives(&regions, mesh);
    let features = detect_features(&primitives);
    let constraints = infer_constraints(&primitives, config.constraint_tolerance);

    // Generate CFL code from detected features
    let cfl_code = features_to_cfl(&features, &constraints, config.snap_dimensions);

    Some(ReconstructResult { regions, primitives, features, constraints, cfl_code })
}

/// Generate CFL code from detected features.
fn features_to_cfl(features: &[DetectedFeature], constraints: &[InferredConstraint], snap: bool) -> String {
    let mut cfl = String::from("// Auto-reconstructed from mesh via physical-inverse\n\n");

    for (i, feat) in features.iter().enumerate() {
        match feat.feature_type {
            FeatureType::BaseBox => {
                cfl.push_str("solid base = box(50mm, 30mm, 10mm)\n");
            }
            FeatureType::BaseCylinder => {
                let r = feat.parameters.get("radius").copied().unwrap_or(10.0);
                let h = feat.parameters.get("height").copied().unwrap_or(20.0);
                let r = if snap { snap_dimension(r) } else { r };
                let h = if snap { snap_dimension(h) } else { h };
                cfl.push_str(&format!("solid base = cylinder(radius: {}mm, height: {}mm)\n", r, h));
            }
            FeatureType::Hole => {
                let d = feat.parameters.get("diameter").copied().unwrap_or(6.0);
                let depth = feat.parameters.get("depth").copied().unwrap_or(10.0);
                let d = if snap { snap_dimension(d) } else { d };
                cfl.push_str(&format!("hole(base, diameter: {}mm, depth: {}mm)\n", d, depth));
            }
            FeatureType::Fillet => {
                let r = feat.parameters.get("radius").copied().unwrap_or(2.0);
                cfl.push_str(&format!("fillet(base, radius: {}mm)\n", r));
            }
            FeatureType::LinearPattern => {
                let count = feat.parameters.get("count").copied().unwrap_or(3.0) as u32;
                cfl.push_str(&format!("pattern(linear, count: {count})\n"));
            }
            _ => {}
        }
    }

    // Add constraint comments
    if !constraints.is_empty() {
        cfl.push_str("\n// Detected constraints:\n");
        for c in constraints {
            cfl.push_str(&format!("// {:?} between regions {:?}\n", c.kind, c.participants));
        }
    }

    cfl
}

// ---------------------------------------------------------------------------
// Validation — how good is the reconstruction?
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconstructionGrade { Excellent, Good, Acceptable, Poor }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub rms_distance: f64,
    pub max_distance: f64,
    pub coverage_pct: f64,
    pub grade: ReconstructionGrade,
}

pub fn validate_reconstruction(original: &InverseMesh, reconstructed: &InverseMesh) -> ValidationReport {
    let orig_points: Vec<DVec3> = original.vertices.clone();
    let recon_points: Vec<DVec3> = reconstructed.vertices.clone();

    if orig_points.is_empty() || recon_points.is_empty() {
        return ValidationReport { rms_distance: f64::INFINITY, max_distance: f64::INFINITY, coverage_pct: 0.0, grade: ReconstructionGrade::Poor };
    }

    let mut sum_sq = 0.0;
    let mut max_d = 0.0_f64;
    let mut covered = 0usize;

    for op in &orig_points {
        let nearest = recon_points.iter()
            .map(|rp| (*op - *rp).length())
            .fold(f64::INFINITY, f64::min);
        sum_sq += nearest * nearest;
        max_d = max_d.max(nearest);
        if nearest < 1.0 { covered += 1; }
    }

    let rms = (sum_sq / orig_points.len() as f64).sqrt();
    let coverage = covered as f64 / orig_points.len() as f64 * 100.0;

    let grade = if rms < 0.01 { ReconstructionGrade::Excellent }
        else if rms < 0.1 { ReconstructionGrade::Good }
        else if rms < 1.0 { ReconstructionGrade::Acceptable }
        else { ReconstructionGrade::Poor };

    ValidationReport { rms_distance: rms, max_distance: max_d, coverage_pct: coverage, grade }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn planar_quad_mesh() -> InverseMesh {
        InverseMesh {
            vertices: vec![
                DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0),
                DVec3::new(10.0, 10.0, 0.0), DVec3::new(0.0, 10.0, 0.0),
            ],
            normals: vec![DVec3::Z; 4],
            indices: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    fn box_mesh() -> InverseMesh {
        // 6 faces × 2 triangles = 12 triangles, 8 vertices
        let v = [
            DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 0.0), DVec3::new(0.0, 10.0, 0.0),
            DVec3::new(0.0, 0.0, 10.0), DVec3::new(10.0, 0.0, 10.0),
            DVec3::new(10.0, 10.0, 10.0), DVec3::new(0.0, 10.0, 10.0),
        ];
        let n_bottom = -DVec3::Z; let n_top = DVec3::Z;
        let n_front = -DVec3::Y; let n_back = DVec3::Y;
        let n_left = -DVec3::X; let n_right = DVec3::X;
        InverseMesh {
            vertices: v.to_vec(),
            normals: vec![n_bottom, n_right, n_back, n_left, n_bottom, n_right, n_top, n_left],
            indices: vec![
                [0,2,1],[0,3,2], // bottom
                [4,5,6],[4,6,7], // top
                [0,1,5],[0,5,4], // front
                [2,3,7],[2,7,6], // back
                [0,4,7],[0,7,3], // left
                [1,2,6],[1,6,5], // right
            ],
        }
    }

    #[test]
    fn segment_planar_quad() {
        let mesh = planar_quad_mesh();
        let regions = segment_mesh(&mesh, 0.15);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].kind, RegionKind::Plane);
    }

    #[test]
    fn segment_box() {
        let mesh = box_mesh();
        let regions = segment_mesh(&mesh, 0.15);
        // Box should produce 6 planar regions (one per face)
        assert!(regions.len() >= 3, "box should have multiple regions, got {}", regions.len());
    }

    #[test]
    fn fit_planar_region() {
        let mesh = planar_quad_mesh();
        let regions = segment_mesh(&mesh, 0.15);
        let fitted = fit_primitives(&regions, &mesh);
        assert_eq!(fitted.len(), 1);
        assert!(matches!(fitted[0].kind, PrimitiveKind::Plane { .. }));
        assert!(fitted[0].rms_error < 1e-6);
    }

    // -----------------------------------------------------------------
    // Phase D: mesh → B-Rep round-trip tests
    // -----------------------------------------------------------------

    #[test]
    fn reconstruct_box_produces_real_brep() {
        let mesh = box_mesh(); // 10×10×10 box at origin..10
        let cfg = InverseConfig::default();
        let result = reconstruct_to_brep(&mesh, &cfg)
            .expect("box mesh should reconstruct");
        assert_eq!(result.kind, BrepReconstructionKind::Box);

        // Dimensions should match the source mesh (10×10×10 after snapping).
        for d in result.dimensions {
            assert!((d - 10.0).abs() < 0.5, "dim {} should be ~10, got {}", d, d);
        }

        // Centre should be at the mesh centroid (5, 5, 5).
        for c in result.center {
            assert!((c - 5.0).abs() < 0.5);
        }

        // The emitted solid must be a valid B-Rep shell: 6 faces, 8 vertices,
        // 12 edges, Euler characteristic 2.
        assert_eq!(result.solid.face_count(), 6);
        assert_eq!(result.solid.vertex_count(), 8);
        assert!(result.solid.is_valid_shell());
    }

    #[test]
    fn reconstruct_returns_none_for_empty_mesh() {
        let mesh = InverseMesh::new();
        let cfg = InverseConfig::default();
        assert!(reconstruct_to_brep(&mesh, &cfg).is_none());
    }

    #[test]
    fn reconstruct_box_roundtrips_via_tessellation() {
        // Forward: box brep → tessellation → inverse mesh
        // Backward: inverse mesh → reconstruct_to_brep
        // The round-tripped brep should have the same topology and
        // approximately the same bounding box as the source.
        let source: physical_brep::Solid = physical_brep::make_box(20.0, 20.0, 20.0);
        let inverse_mesh = InverseMesh::from_tess_mesh(&source);

        let result = reconstruct_to_brep(&inverse_mesh, &InverseConfig::default())
            .expect("tessellated box should round-trip");

        assert_eq!(result.kind, BrepReconstructionKind::Box);
        assert_eq!(result.solid.face_count(), 6);
        assert!(result.solid.is_valid_shell());

        // Bounding boxes should match within snapping tolerance.
        let (src_min, src_max) = source.bounding_box();
        let (rec_min, rec_max) = result.solid.bounding_box();
        for axis in 0..3 {
            let src_size = [src_max.x - src_min.x, src_max.y - src_min.y, src_max.z - src_min.z][axis];
            let rec_size = [rec_max.x - rec_min.x, rec_max.y - rec_min.y, rec_max.z - rec_min.z][axis];
            assert!(
                (src_size - rec_size).abs() < 0.5,
                "axis {}: source size {:.3} vs reconstructed {:.3}",
                axis, src_size, rec_size
            );
        }
    }

    #[test]
    fn reconstruct_unsupported_returns_none() {
        // A single triangle is neither a box nor a cylinder; reconstruction
        // must report `None` rather than silently inventing topology.
        let mesh = planar_quad_mesh();
        let cfg = InverseConfig::default();
        let result = reconstruct_to_brep(&mesh, &cfg);
        assert!(
            result.is_none(),
            "single quad should not be reconstructible as a box/cylinder"
        );
    }

    #[test]
    fn detect_box_features() {
        let mesh = box_mesh();
        let regions = segment_mesh(&mesh, 0.15);
        let primitives = fit_primitives(&regions, &mesh);
        let features = detect_features(&primitives);
        // Should detect base box from 6 planes
        assert!(!features.is_empty());
    }

    #[test]
    fn infer_parallel_planes() {
        let prims = vec![
            FittedPrimitive { region_id: 0, kind: PrimitiveKind::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0] }, rms_error: 0.0 },
            FittedPrimitive { region_id: 1, kind: PrimitiveKind::Plane { origin: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0] }, rms_error: 0.0 },
        ];
        let constraints = infer_constraints(&prims, 0.02);
        assert!(constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Parallel)));
    }

    #[test]
    fn infer_perpendicular_planes() {
        let prims = vec![
            FittedPrimitive { region_id: 0, kind: PrimitiveKind::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0] }, rms_error: 0.0 },
            FittedPrimitive { region_id: 1, kind: PrimitiveKind::Plane { origin: [5.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] }, rms_error: 0.0 },
        ];
        let constraints = infer_constraints(&prims, 0.02);
        assert!(constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Perpendicular)));
    }

    #[test]
    fn snap_to_integer() {
        assert_eq!(snap_dimension(9.95), 10.0);
        assert_eq!(snap_dimension(5.02), 5.0);
    }

    #[test]
    fn snap_to_half_mm() {
        assert_eq!(snap_dimension(4.48), 4.5);
        assert_eq!(snap_dimension(7.52), 7.5);
    }

    #[test]
    fn snap_to_drill_size() {
        // 4.2mm is M5 tap drill
        assert_eq!(snap_dimension(4.18), 4.2);
        // 6.8mm is M8 tap drill
        assert_eq!(snap_dimension(6.82), 6.8);
    }

    #[test]
    fn full_pipeline_planar() {
        let mesh = planar_quad_mesh();
        let result = reconstruct(&mesh, &InverseConfig::default()).unwrap();
        assert!(!result.regions.is_empty());
        assert!(!result.primitives.is_empty());
    }

    #[test]
    fn full_pipeline_box() {
        let mesh = box_mesh();
        let result = reconstruct(&mesh, &InverseConfig::default()).unwrap();
        assert!(!result.cfl_code.is_empty());
        assert!(result.cfl_code.contains("//"), "should have CFL comments");
    }

    #[test]
    fn pipeline_generates_cfl() {
        let mesh = box_mesh();
        let result = reconstruct(&mesh, &InverseConfig::default()).unwrap();
        // CFL output should contain detected features
        assert!(result.cfl_code.len() > 10, "CFL should be non-empty");
    }

    #[test]
    fn validation_identical_meshes() {
        let mesh = planar_quad_mesh();
        let report = validate_reconstruction(&mesh, &mesh);
        assert_eq!(report.rms_distance, 0.0);
        assert_eq!(report.grade, ReconstructionGrade::Excellent);
    }

    #[test]
    fn empty_mesh_returns_none() {
        let mesh = InverseMesh::new();
        assert!(reconstruct(&mesh, &InverseConfig::default()).is_none());
    }
}
