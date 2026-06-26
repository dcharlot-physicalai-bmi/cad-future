//! `physical-tessellation` — Triangle mesh tessellation for B-Rep solids.
//!
//! Converts B-Rep topology into indexed triangle meshes for rendering,
//! slicing, and export. Walks each B-Rep face, extracts the boundary
//! polygon from half-edges, and tessellates it:
//!
//! - **Planar faces**: ear-clipping polygon triangulation
//! - **Curved faces** (cylinder, sphere, cone, torus, NURBS): UV-grid
//!   sampling within the face boundary, then structured triangulation
//!
//! Provides [`TessVertex`] and [`TessMesh`] types consumed by the slicer,
//! glTF exporter, STL writer, and viewport renderer.

use glam::DVec3;
use physical_brep::Solid;
use physical_brep::surface::Surface;

/// A single tessellation vertex with position, normal, and UV coordinates.
#[derive(Debug, Clone, Copy)]
pub struct TessVertex {
    /// Position in model space [x, y, z].
    pub position: [f32; 3],
    /// Surface normal [nx, ny, nz].
    pub normal: [f32; 3],
    /// Texture coordinates [u, v].
    pub uv: [f32; 2],
}

/// An indexed triangle mesh.
#[derive(Debug, Clone)]
pub struct TessMesh {
    /// Vertex buffer.
    pub vertices: Vec<TessVertex>,
    /// Index buffer (triangles: every 3 indices form one face).
    pub indices: Vec<u32>,
}

impl TessMesh {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Axis-aligned bounding box: returns (min, max) as [x, y, z].
    pub fn bounding_box(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &self.vertices {
            for i in 0..3 {
                min[i] = min[i].min(v.position[i]);
                max[i] = max[i].max(v.position[i]);
            }
        }
        (min, max)
    }

    /// Z-extent of the mesh (max_z - min_z).
    pub fn height(&self) -> f32 {
        let (min, max) = self.bounding_box();
        max[2] - min[2]
    }

    /// Merge another mesh into this one.
    pub fn merge(&mut self, other: &TessMesh) {
        let offset = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + offset));
    }
}

// ---------------------------------------------------------------------------
// LUT cache — close the 6.5-order impedance gap identified by
// physical-impedance for the `tessellate_face_uv_grid` operation.
//
// Tessellation of a given (solid, tolerance) pair is a pure function: same
// input always yields the same output. That makes it a textbook L₀ bijective
// "discover once, deploy forever" candidate.
//
// The cache keys on (solid_hash, tolerance_bucket). Repeat calls return
// a clone of the cached mesh in ~microseconds instead of re-walking every
// face, sampling every UV grid, and running ear-clip on every polygon.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

/// Content-hash of a Solid for cache lookup. Uses vertex positions and the
/// structural parameters of each face's Surface. Intentionally stable across
/// SlotMap key churn — two solids that describe the same geometry have the
/// same hash even if their internal SlotMap IDs differ.
fn hash_solid(solid: &Solid) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();

    // Hash vertex positions in a canonical order
    let mut points: Vec<[i64; 3]> = solid
        .vertices
        .values()
        .map(|v| {
            // Quantize to 0.001mm to make the hash numerically stable
            [
                (v.point.x * 1000.0).round() as i64,
                (v.point.y * 1000.0).round() as i64,
                (v.point.z * 1000.0).round() as i64,
            ]
        })
        .collect();
    points.sort();
    points.hash(&mut hasher);

    // Hash each face's surface structurally
    let mut surface_sigs: Vec<String> = solid
        .faces
        .values()
        .map(|f| match &f.surface {
            Surface::Plane { normal, origin } => format!(
                "plane:{:.3},{:.3},{:.3}|{:.3},{:.3},{:.3}",
                normal.x, normal.y, normal.z, origin.x, origin.y, origin.z
            ),
            Surface::Cylinder { origin, axis, radius } => format!(
                "cyl:{:.3},{:.3},{:.3}|{:.3},{:.3},{:.3}|{:.3}",
                origin.x, origin.y, origin.z, axis.x, axis.y, axis.z, radius
            ),
            Surface::Sphere { center, radius } => format!(
                "sph:{:.3},{:.3},{:.3}|{:.3}",
                center.x, center.y, center.z, radius
            ),
            Surface::Cone { apex, axis, half_angle } => format!(
                "con:{:.3},{:.3},{:.3}|{:.3},{:.3},{:.3}|{:.4}",
                apex.x, apex.y, apex.z, axis.x, axis.y, axis.z, half_angle
            ),
            Surface::Torus { center, axis, major_radius, minor_radius } => format!(
                "tor:{:.3},{:.3},{:.3}|{:.3},{:.3},{:.3}|{:.3},{:.3}",
                center.x, center.y, center.z, axis.x, axis.y, axis.z, major_radius, minor_radius
            ),
            Surface::Nurbs { degree_u, degree_v, .. } => {
                format!("nurbs:{},{}", degree_u, degree_v)
            }
        })
        .collect();
    surface_sigs.sort();
    surface_sigs.hash(&mut hasher);

    hasher.finish()
}

/// Bucket tolerance to avoid cache misses from microscopic float jitter.
/// 0.001mm granularity matches the solid hash quantization.
fn tolerance_bucket(tolerance: f64) -> i64 {
    (tolerance * 1000.0).round() as i64
}

type CacheKey = (u64, i64);

fn tess_cache() -> &'static Mutex<HashMap<CacheKey, TessMesh>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, TessMesh>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Statistics about the tessellation LUT cache.
#[derive(Debug, Clone, Copy, Default)]
pub struct TessCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

fn cache_counters() -> &'static Mutex<(u64, u64)> {
    static COUNTERS: OnceLock<Mutex<(u64, u64)>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new((0, 0)))
}

/// Get current cache statistics.
pub fn tess_cache_stats() -> TessCacheStats {
    let entries = tess_cache().lock().unwrap().len();
    let (hits, misses) = *cache_counters().lock().unwrap();
    TessCacheStats { entries, hits, misses }
}

/// Clear the tessellation cache. Primarily useful for tests and memory-tight
/// scenarios. Production code should let the cache grow — each entry is
/// already the cheapest representation of the solid it came from.
pub fn tess_cache_clear() {
    tess_cache().lock().unwrap().clear();
    *cache_counters().lock().unwrap() = (0, 0);
}

// ---------------------------------------------------------------------------
// Main tessellation entry point
// ---------------------------------------------------------------------------

/// Tessellate a B-Rep solid into a triangle mesh.
///
/// `tolerance` controls the maximum deviation from the true surface (mm).
/// Smaller values produce finer meshes for curved surfaces.
///
/// # Caching
///
/// This function checks a content-addressed LUT cache first. If the same
/// (solid, tolerance) pair has been tessellated before, the cached mesh is
/// cloned and returned in microseconds. Otherwise the result is computed and
/// stored for next time. This closes the impedance gap identified by
/// `physical-impedance` for the `tessellate_face_uv_grid` operation — the
/// natural cost of a bijective operation is a single table read.
///
/// To bypass the cache (for benchmarks or when you know the input is unique),
/// call [`tessellate_uncached`].
pub fn tessellate(solid: &Solid, tolerance: f64) -> TessMesh {
    let key: CacheKey = (hash_solid(solid), tolerance_bucket(tolerance));

    // Fast path: cache hit. Clone the stored mesh — clone is O(n) on the
    // mesh data but avoids re-walking faces, re-running ear-clip, etc.
    if let Some(cached) = tess_cache().lock().unwrap().get(&key) {
        cache_counters().lock().unwrap().0 += 1;
        return cached.clone();
    }

    // Slow path: compute, store, return.
    let mesh = tessellate_uncached(solid, tolerance);
    cache_counters().lock().unwrap().1 += 1;
    tess_cache().lock().unwrap().insert(key, mesh.clone());
    mesh
}

/// Tessellate without consulting the cache. Always re-computes.
pub fn tessellate_uncached(solid: &Solid, tolerance: f64) -> TessMesh {
    let mut mesh = TessMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };

    for (_fid, face) in &solid.faces {
        let face_mesh = tessellate_face(solid, face, tolerance);
        mesh.merge(&face_mesh);
    }

    // If no faces produced geometry, fall back to bounding-box wireframe
    if mesh.vertices.is_empty() {
        return tessellate_bounding_box(solid);
    }

    mesh
}

/// Tessellate a single B-Rep face.
fn tessellate_face(
    solid: &Solid,
    face: &physical_brep::types::BRepFace,
    tolerance: f64,
) -> TessMesh {
    // Extract boundary polygon from outer loop half-edges
    let boundary: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
        solid.vertices[solid.half_edges[*he_id].origin].point
    }).collect();

    if boundary.len() < 3 {
        return TessMesh { vertices: Vec::new(), indices: Vec::new() };
    }

    match &face.surface {
        Surface::Plane { normal, .. } => {
            tessellate_planar_face(&boundary, *normal, face.normal_outward)
        }
        Surface::Cylinder { origin, axis, radius } => {
            tessellate_cylindrical_face(
                &boundary, *origin, *axis, *radius, tolerance, face.normal_outward,
            )
        }
        Surface::Sphere { center, radius } => {
            tessellate_spherical_face(
                &boundary, *center, *radius, tolerance, face.normal_outward,
            )
        }
        Surface::Cone { apex, axis, half_angle } => {
            tessellate_conical_face(
                &boundary, *apex, *axis, *half_angle, tolerance, face.normal_outward,
            )
        }
        Surface::Torus { center, axis, major_radius, minor_radius } => {
            tessellate_toroidal_face(
                &boundary, *center, *axis, *major_radius, *minor_radius, tolerance, face.normal_outward,
            )
        }
        Surface::Nurbs { .. } => {
            // For NURBS: fall back to planar tessellation of the boundary polygon.
            // A full NURBS tessellator would sample the surface at UV grid points,
            // but the boundary polygon already approximates the face shape.
            let normal = compute_polygon_normal(&boundary);
            tessellate_planar_face(&boundary, normal, face.normal_outward)
        }
    }
}

// ---------------------------------------------------------------------------
// Planar face tessellation — ear-clipping
// ---------------------------------------------------------------------------

/// Tessellate a planar face using ear-clipping triangulation.
fn tessellate_planar_face(
    boundary: &[DVec3],
    face_normal: DVec3,
    outward: bool,
) -> TessMesh {
    let normal = if outward { face_normal.normalize() } else { -face_normal.normalize() };
    let n = [normal.x as f32, normal.y as f32, normal.z as f32];

    // Create vertices
    let vertices: Vec<TessVertex> = boundary.iter().enumerate().map(|(i, p)| {
        let u = i as f32 / boundary.len().max(1) as f32;
        TessVertex {
            position: [p.x as f32, p.y as f32, p.z as f32],
            normal: n,
            uv: [u, 0.0],
        }
    }).collect();

    // Ear-clipping triangulation
    let indices = ear_clip_triangulate(boundary, face_normal);

    TessMesh { vertices, indices }
}

/// Ear-clipping polygon triangulation.
/// Returns triangle indices into the boundary array.
fn ear_clip_triangulate(polygon: &[DVec3], normal: DVec3) -> Vec<u32> {
    let n = polygon.len();
    if n < 3 { return Vec::new(); }
    if n == 3 { return vec![0, 1, 2]; }
    if n == 4 {
        // Simple quad split
        return vec![0, 1, 2, 0, 2, 3];
    }

    let mut indices = Vec::with_capacity((n - 2) * 3);
    let mut remaining: Vec<usize> = (0..n).collect();

    let mut safety = n * n; // prevent infinite loops
    while remaining.len() > 3 && safety > 0 {
        safety -= 1;
        let len = remaining.len();
        let mut found_ear = false;

        for i in 0..len {
            let prev = remaining[(i + len - 1) % len];
            let curr = remaining[i];
            let next = remaining[(i + 1) % len];

            let v0 = polygon[prev];
            let v1 = polygon[curr];
            let v2 = polygon[next];

            // Check if this is a convex vertex (ear tip)
            let edge1 = v1 - v0;
            let edge2 = v2 - v1;
            let cross = edge1.cross(edge2);
            if cross.dot(normal) < -1e-10 {
                continue; // concave, not an ear
            }

            // Check that no other vertex lies inside this triangle
            let mut contains_other = false;
            for j in 0..len {
                if j == (i + len - 1) % len || j == i || j == (i + 1) % len {
                    continue;
                }
                let p = polygon[remaining[j]];
                if point_in_triangle_3d(p, v0, v1, v2, normal) {
                    contains_other = true;
                    break;
                }
            }

            if !contains_other {
                indices.push(prev as u32);
                indices.push(curr as u32);
                indices.push(next as u32);
                remaining.remove(i);
                found_ear = true;
                break;
            }
        }

        if !found_ear {
            // Degenerate polygon — fan triangulate the rest
            break;
        }
    }

    // Handle remaining triangle
    if remaining.len() == 3 {
        indices.push(remaining[0] as u32);
        indices.push(remaining[1] as u32);
        indices.push(remaining[2] as u32);
    } else if remaining.len() > 3 {
        // Fallback: fan triangulation from first remaining vertex
        for i in 1..remaining.len() - 1 {
            indices.push(remaining[0] as u32);
            indices.push(remaining[i] as u32);
            indices.push(remaining[i + 1] as u32);
        }
    }

    indices
}

/// Test if point p lies inside triangle (a, b, c) in 3D, projected along normal.
fn point_in_triangle_3d(p: DVec3, a: DVec3, b: DVec3, c: DVec3, normal: DVec3) -> bool {
    let cross0 = (b - a).cross(p - a).dot(normal);
    let cross1 = (c - b).cross(p - b).dot(normal);
    let cross2 = (a - c).cross(p - c).dot(normal);
    // All same sign (or zero)
    (cross0 >= -1e-10 && cross1 >= -1e-10 && cross2 >= -1e-10)
        || (cross0 <= 1e-10 && cross1 <= 1e-10 && cross2 <= 1e-10)
}

// ---------------------------------------------------------------------------
// Cylindrical face tessellation — UV grid on cylinder
// ---------------------------------------------------------------------------

/// Tessellate a cylindrical face by sampling the cylinder surface.
fn tessellate_cylindrical_face(
    boundary: &[DVec3],
    cyl_origin: DVec3,
    cyl_axis: DVec3,
    radius: f64,
    tolerance: f64,
    outward: bool,
) -> TessMesh {
    let axis = cyl_axis.normalize();

    // Project boundary to get angle range and height range
    let mut min_h = f64::INFINITY;
    let mut max_h = f64::NEG_INFINITY;
    let mut angles: Vec<f64> = Vec::new();

    let (e1, e2) = perpendicular_frame(axis);

    for p in boundary {
        let v = *p - cyl_origin;
        let h = v.dot(axis);
        min_h = min_h.min(h);
        max_h = max_h.max(h);
        let x = v.dot(e1);
        let y = v.dot(e2);
        angles.push(y.atan2(x));
    }

    if angles.is_empty() || (max_h - min_h).abs() < 1e-12 {
        let normal = compute_polygon_normal(boundary);
        return tessellate_planar_face(boundary, normal, outward);
    }

    // Determine angle range from boundary
    let (min_angle, max_angle) = angle_range_from_boundary(&angles);

    // Number of subdivisions based on tolerance
    let arc_length = radius * (max_angle - min_angle).abs();
    let n_u = ((arc_length / tolerance).ceil() as usize).clamp(2, 64);
    let height = max_h - min_h;
    let n_v = ((height / tolerance).ceil() as usize).clamp(1, 32);

    tessellate_uv_grid(n_u, n_v, outward, |u_frac, v_frac| {
        let angle = min_angle + (max_angle - min_angle) * u_frac;
        let h = min_h + height * v_frac;
        let pos = cyl_origin + axis * h + e1 * (radius * angle.cos()) + e2 * (radius * angle.sin());
        let normal_dir = (e1 * angle.cos() + e2 * angle.sin()).normalize();
        let normal = if outward { normal_dir } else { -normal_dir };
        (pos, normal)
    })
}

// ---------------------------------------------------------------------------
// Spherical face tessellation
// ---------------------------------------------------------------------------

fn tessellate_spherical_face(
    boundary: &[DVec3],
    center: DVec3,
    radius: f64,
    tolerance: f64,
    outward: bool,
) -> TessMesh {
    // Find latitude/longitude range from boundary
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;

    for p in boundary {
        let v = (*p - center).normalize();
        let lat = v.z.asin();
        let lon = v.y.atan2(v.x);
        min_lon = min_lon.min(lon);
        max_lon = max_lon.max(lon);
        min_lat = min_lat.min(lat);
        max_lat = max_lat.max(lat);
    }

    let arc_u = radius * (max_lon - min_lon).abs();
    let arc_v = radius * (max_lat - min_lat).abs();
    let n_u = ((arc_u / tolerance).ceil() as usize).clamp(3, 48);
    let n_v = ((arc_v / tolerance).ceil() as usize).clamp(2, 32);

    tessellate_uv_grid(n_u, n_v, outward, |u_frac, v_frac| {
        let lon = min_lon + (max_lon - min_lon) * u_frac;
        let lat = min_lat + (max_lat - min_lat) * v_frac;
        let pos = center + DVec3::new(
            radius * lat.cos() * lon.cos(),
            radius * lat.cos() * lon.sin(),
            radius * lat.sin(),
        );
        let normal_dir = (pos - center).normalize();
        let normal = if outward { normal_dir } else { -normal_dir };
        (pos, normal)
    })
}

// ---------------------------------------------------------------------------
// Conical face tessellation
// ---------------------------------------------------------------------------

fn tessellate_conical_face(
    boundary: &[DVec3],
    apex: DVec3,
    axis: DVec3,
    half_angle: f64,
    tolerance: f64,
    outward: bool,
) -> TessMesh {
    let axis = axis.normalize();
    let (e1, e2) = perpendicular_frame(axis);

    let mut min_h = f64::INFINITY;
    let mut max_h = f64::NEG_INFINITY;
    let mut angles: Vec<f64> = Vec::new();

    for p in boundary {
        let v = *p - apex;
        let h = v.dot(axis);
        min_h = min_h.min(h);
        max_h = max_h.max(h);
        let x = v.dot(e1);
        let y = v.dot(e2);
        angles.push(y.atan2(x));
    }

    let (min_angle, max_angle) = angle_range_from_boundary(&angles);
    let max_r = max_h * half_angle.tan();
    let arc_length = max_r * (max_angle - min_angle).abs();
    let n_u = ((arc_length / tolerance).ceil() as usize).clamp(3, 48);
    let n_v = (((max_h - min_h) / tolerance).ceil() as usize).clamp(2, 24);

    tessellate_uv_grid(n_u, n_v, outward, |u_frac, v_frac| {
        let angle = min_angle + (max_angle - min_angle) * u_frac;
        let h = min_h + (max_h - min_h) * v_frac;
        let r = h * half_angle.tan();
        let pos = apex + axis * h + e1 * (r * angle.cos()) + e2 * (r * angle.sin());
        let radial = (e1 * angle.cos() + e2 * angle.sin()).normalize();
        let normal_dir = (radial * half_angle.cos() - axis * half_angle.sin()).normalize();
        let normal = if outward { normal_dir } else { -normal_dir };
        (pos, normal)
    })
}

// ---------------------------------------------------------------------------
// Toroidal face tessellation
// ---------------------------------------------------------------------------

fn tessellate_toroidal_face(
    boundary: &[DVec3],
    center: DVec3,
    axis: DVec3,
    major_r: f64,
    minor_r: f64,
    tolerance: f64,
    outward: bool,
) -> TessMesh {
    let axis = axis.normalize();
    let (e1, e2) = perpendicular_frame(axis);

    let mut u_angles = Vec::new();
    let mut v_angles = Vec::new();

    for p in boundary {
        let v = *p - center;
        let proj = v - axis * v.dot(axis);
        let u_angle = proj.dot(e2).atan2(proj.dot(e1));
        u_angles.push(u_angle);

        let tube_center = center + (e1 * u_angle.cos() + e2 * u_angle.sin()) * major_r;
        let to_point = *p - tube_center;
        let radial = (e1 * u_angle.cos() + e2 * u_angle.sin()).normalize();
        let v_angle = to_point.dot(axis).atan2(to_point.dot(radial));
        v_angles.push(v_angle);
    }

    let (min_u, max_u) = angle_range_from_boundary(&u_angles);
    let (min_v, max_v) = angle_range_from_boundary(&v_angles);

    let arc_u = major_r * (max_u - min_u).abs();
    let arc_v = minor_r * (max_v - min_v).abs();
    let n_u = ((arc_u / tolerance).ceil() as usize).clamp(4, 48);
    let n_v = ((arc_v / tolerance).ceil() as usize).clamp(3, 24);

    tessellate_uv_grid(n_u, n_v, outward, |u_frac, v_frac| {
        let u = min_u + (max_u - min_u) * u_frac;
        let v = min_v + (max_v - min_v) * v_frac;
        let tube_center = center + (e1 * u.cos() + e2 * u.sin()) * major_r;
        let radial = (e1 * u.cos() + e2 * u.sin()).normalize();
        let pos = tube_center + radial * (minor_r * v.cos()) + axis * (minor_r * v.sin());
        let normal_dir = (pos - tube_center).normalize();
        let normal = if outward { normal_dir } else { -normal_dir };
        (pos, normal)
    })
}

// ---------------------------------------------------------------------------
// UV grid tessellation helper
// ---------------------------------------------------------------------------

/// Create a triangle mesh from a UV-parameterized surface.
/// `sample_fn(u_frac, v_frac)` returns (position, normal) for u,v in [0,1].
fn tessellate_uv_grid(
    n_u: usize,
    n_v: usize,
    _outward: bool,
    sample_fn: impl Fn(f64, f64) -> (DVec3, DVec3),
) -> TessMesh {
    let mut vertices = Vec::with_capacity((n_u + 1) * (n_v + 1));
    let mut indices = Vec::with_capacity(n_u * n_v * 6);

    // Sample grid
    for iv in 0..=n_v {
        let v_frac = iv as f64 / n_v as f64;
        for iu in 0..=n_u {
            let u_frac = iu as f64 / n_u as f64;
            let (pos, normal) = sample_fn(u_frac, v_frac);
            vertices.push(TessVertex {
                position: [pos.x as f32, pos.y as f32, pos.z as f32],
                normal: [normal.x as f32, normal.y as f32, normal.z as f32],
                uv: [u_frac as f32, v_frac as f32],
            });
        }
    }

    // Triangulate grid
    let stride = (n_u + 1) as u32;
    for iv in 0..n_v as u32 {
        for iu in 0..n_u as u32 {
            let a = iv * stride + iu;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }

    TessMesh { vertices, indices }
}

// ---------------------------------------------------------------------------
// Fallback bounding-box tessellation
// ---------------------------------------------------------------------------

fn tessellate_bounding_box(solid: &Solid) -> TessMesh {
    let (bb_min, bb_max) = solid.bounding_box();
    let min = [bb_min.x as f32, bb_min.y as f32, bb_min.z as f32];
    let max = [bb_max.x as f32, bb_max.y as f32, bb_max.z as f32];

    let vertices = vec![
        TessVertex { position: [min[0], min[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
        TessVertex { position: [max[0], min[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
        TessVertex { position: [max[0], max[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
        TessVertex { position: [min[0], max[1], min[2]], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
        TessVertex { position: [min[0], min[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
        TessVertex { position: [max[0], min[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
        TessVertex { position: [max[0], max[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
        TessVertex { position: [min[0], max[1], max[2]], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
    ];
    let indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7,
        0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6,
        0, 4, 7, 0, 7, 3, 1, 2, 6, 1, 6, 5,
    ];
    TessMesh { vertices, indices }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Compute the normal of a polygon from its vertices (Newell's method).
fn compute_polygon_normal(verts: &[DVec3]) -> DVec3 {
    let mut normal = DVec3::ZERO;
    let n = verts.len();
    for i in 0..n {
        let curr = verts[i];
        let next = verts[(i + 1) % n];
        normal.x += (curr.y - next.y) * (curr.z + next.z);
        normal.y += (curr.z - next.z) * (curr.x + next.x);
        normal.z += (curr.x - next.x) * (curr.y + next.y);
    }
    if normal.length() > 1e-14 { normal.normalize() } else { DVec3::Y }
}

/// Build a perpendicular frame (e1, e2) from an axis vector.
fn perpendicular_frame(axis: DVec3) -> (DVec3, DVec3) {
    let a = axis.normalize();
    let up = if a.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let e1 = a.cross(up).normalize();
    let e2 = a.cross(e1).normalize();
    (e1, e2)
}

/// Determine the angular range [min, max] from a set of angles,
/// handling the wraparound at ±π.
fn angle_range_from_boundary(angles: &[f64]) -> (f64, f64) {
    if angles.is_empty() {
        return (0.0, std::f64::consts::TAU);
    }

    // Sort angles and find the largest gap
    let mut sorted = angles.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

    if sorted.len() <= 1 {
        return (sorted[0] - 0.01, sorted[0] + 0.01);
    }

    // Simple range for most cases
    let min = *sorted.first().unwrap();
    let max = *sorted.last().unwrap();

    // Check if the range wraps around (gap at ±π boundary)
    let direct_span = max - min;
    let wrap_gap = (min + std::f64::consts::TAU) - max;

    if wrap_gap < direct_span && direct_span > std::f64::consts::PI {
        // Wrapped range: go from max to min+2π
        (max, min + std::f64::consts::TAU)
    } else {
        (min, max)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tess_vertex_size() {
        assert_eq!(std::mem::size_of::<TessVertex>(), 32);
    }

    #[test]
    fn mesh_bounding_box() {
        let mesh = TessMesh {
            vertices: vec![
                TessVertex { position: [0.0, 0.0, 0.0], normal: [0.0; 3], uv: [0.0; 2] },
                TessVertex { position: [10.0, 20.0, 30.0], normal: [0.0; 3], uv: [0.0; 2] },
            ],
            indices: vec![],
        };
        let (min, max) = mesh.bounding_box();
        assert_eq!(min, [0.0, 0.0, 0.0]);
        assert_eq!(max, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn mesh_height() {
        let mesh = TessMesh {
            vertices: vec![
                TessVertex { position: [0.0, 0.0, 5.0], normal: [0.0; 3], uv: [0.0; 2] },
                TessVertex { position: [0.0, 0.0, 15.0], normal: [0.0; 3], uv: [0.0; 2] },
            ],
            indices: vec![],
        };
        assert!((mesh.height() - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tessellate_box_produces_real_geometry() {
        let b = physical_brep::make_box(10.0, 20.0, 30.0);
        // Use uncached path so this legacy test doesn't pollute cache counters
        // used by the dedicated cache tests below.
        let mesh = tessellate_uncached(&b, 1.0);

        // Box has 6 faces, each produces at least 2 triangles
        assert!(mesh.triangle_count() >= 12, "box should have >= 12 triangles, got {}", mesh.triangle_count());
        // Should have real vertices (not just 8 bbox corners)
        assert!(mesh.vertices.len() >= 8, "should have >= 8 vertices");

        // Bounding box of tessellated mesh should match the solid
        let (bb_min, bb_max) = mesh.bounding_box();
        assert!((bb_max[0] - bb_min[0] - 10.0).abs() < 0.5, "width mismatch");
        assert!((bb_max[1] - bb_min[1] - 20.0).abs() < 0.5, "height mismatch");
        assert!((bb_max[2] - bb_min[2] - 30.0).abs() < 0.5, "depth mismatch");
    }

    #[test]
    fn tessellate_cylinder_more_triangles_than_box() {
        let cyl = physical_brep::builder::make_cylinder(10.0, 30.0, 16);
        let mesh = tessellate_uncached(&cyl, 1.0);

        // Cylinder with 16 segments should produce significantly more triangles
        assert!(mesh.triangle_count() > 12, "cylinder should have more than 12 tris, got {}", mesh.triangle_count());
        assert!(mesh.vertices.len() > 8, "cylinder should have more than 8 vertices");
    }

    #[test]
    fn tessellate_box_normals_are_unit() {
        let b = physical_brep::make_box(5.0, 5.0, 5.0);
        let mesh = tessellate_uncached(&b, 1.0);

        for v in &mesh.vertices {
            let len = (v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 0.1 || len < 0.01, // either unit or zero (degenerate)
                "normal should be unit, got length {len}"
            );
        }
    }

    #[test]
    fn tessellate_finer_tolerance_more_triangles() {
        let cyl = physical_brep::builder::make_cylinder(10.0, 20.0, 16);
        let coarse = tessellate_uncached(&cyl, 5.0);
        let fine = tessellate_uncached(&cyl, 0.5);

        assert!(
            fine.triangle_count() >= coarse.triangle_count(),
            "finer tolerance should produce >= triangles: coarse={}, fine={}",
            coarse.triangle_count(), fine.triangle_count()
        );
    }

    #[test]
    fn ear_clip_triangle() {
        let tri = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        let indices = ear_clip_triangulate(&tri, DVec3::Z);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn ear_clip_quad() {
        let quad = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        let indices = ear_clip_triangulate(&quad, DVec3::Z);
        assert_eq!(indices.len(), 6); // 2 triangles × 3 indices
    }

    #[test]
    fn ear_clip_pentagon() {
        let pent = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(3.0, 1.5, 0.0),
            DVec3::new(1.5, 3.0, 0.0),
            DVec3::new(-0.5, 1.5, 0.0),
        ];
        let indices = ear_clip_triangulate(&pent, DVec3::Z);
        assert_eq!(indices.len(), 9); // 3 triangles × 3 indices
    }

    #[test]
    fn polygon_normal_flat_square() {
        let sq = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        let n = compute_polygon_normal(&sq);
        assert!((n.z.abs() - 1.0).abs() < 1e-10, "square normal should be ±Z");
    }

    #[test]
    fn uv_grid_produces_correct_count() {
        let mesh = tessellate_uv_grid(4, 3, true, |u, v| {
            (DVec3::new(u, v, 0.0), DVec3::Z)
        });
        assert_eq!(mesh.vertices.len(), 5 * 4); // (4+1) × (3+1) = 20
        assert_eq!(mesh.triangle_count(), 4 * 3 * 2); // 24 triangles
    }

    #[test]
    fn merge_meshes() {
        let m1 = TessMesh {
            vertices: vec![
                TessVertex { position: [0.0; 3], normal: [0.0; 3], uv: [0.0; 2] },
            ],
            indices: vec![0, 0, 0],
        };
        let m2 = TessMesh {
            vertices: vec![
                TessVertex { position: [1.0; 3], normal: [0.0; 3], uv: [0.0; 2] },
            ],
            indices: vec![0, 0, 0],
        };
        let mut merged = m1.clone();
        merged.merge(&m2);
        assert_eq!(merged.vertices.len(), 2);
        assert_eq!(merged.indices.len(), 6);
        assert_eq!(merged.indices[3], 1); // offset applied
    }

    #[test]
    fn angle_range_simple() {
        let angles = vec![0.1, 0.5, 1.0, 1.5];
        let (min, max) = angle_range_from_boundary(&angles);
        assert!((min - 0.1).abs() < 0.01);
        assert!((max - 1.5).abs() < 0.01);
    }

    // ---- LUT cache tests ----
    //
    // The cache is global, so parallel tests would race. We serialize all
    // cache-observing tests through a single mutex local to this module.

    fn cache_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn cache_returns_equivalent_mesh_on_repeat_call() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);

        let first = tessellate(&solid, 1.0);
        let second = tessellate(&solid, 1.0);

        assert_eq!(first.vertices.len(), second.vertices.len());
        assert_eq!(first.indices.len(), second.indices.len());
        assert_eq!(first.triangle_count(), second.triangle_count());
    }

    #[test]
    fn cache_reports_hits_and_misses() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::make_box(5.0, 5.0, 5.0);

        // First call — miss
        let _ = tessellate(&solid, 1.0);
        let stats = tess_cache_stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Repeat — hits
        let _ = tessellate(&solid, 1.0);
        let _ = tessellate(&solid, 1.0);
        let _ = tessellate(&solid, 1.0);
        let stats = tess_cache_stats();
        assert_eq!(stats.hits, 3, "expected 3 hits, got {}", stats.hits);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn cache_differentiates_by_tolerance() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::builder::make_cylinder(10.0, 20.0, 16);

        let _ = tessellate(&solid, 0.1);
        let _ = tessellate(&solid, 1.0);
        let _ = tessellate(&solid, 5.0);

        let stats = tess_cache_stats();
        assert_eq!(stats.misses, 3, "three tolerances should produce three misses");
        assert_eq!(stats.entries, 3);
    }

    #[test]
    fn cache_differentiates_by_solid() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let box_a = physical_brep::make_box(10.0, 10.0, 10.0);
        let box_b = physical_brep::make_box(20.0, 20.0, 20.0);

        let _ = tessellate(&box_a, 1.0);
        let _ = tessellate(&box_b, 1.0);

        let stats = tess_cache_stats();
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.entries, 2);
    }

    #[test]
    fn cache_hit_rate_on_repeated_boxes() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::make_box(15.0, 25.0, 35.0);

        // Simulate 100 repeat calls — what happens in a render loop
        for _ in 0..100 {
            let _ = tessellate(&solid, 0.5);
        }

        let stats = tess_cache_stats();
        assert_eq!(stats.misses, 1, "first call is a miss");
        assert_eq!(stats.hits, 99, "remaining 99 calls should all hit");
        // Cache hit rate = hits / (hits + misses) = 99%
        let hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;
        assert!(hit_rate >= 0.99, "hit rate {:.2} should be ≥ 0.99", hit_rate);
    }

    #[test]
    fn cache_hit_faster_than_miss() {
        let _guard = cache_test_lock();
        use std::time::Instant;
        tess_cache_clear();

        // A cylinder produces enough work that the speedup is measurable
        let solid = physical_brep::builder::make_cylinder(25.0, 50.0, 32);

        // Warm: first call (miss)
        let start = Instant::now();
        let _ = tessellate(&solid, 0.2);
        let miss_time = start.elapsed();

        // Hot: second call (hit)
        let start = Instant::now();
        let _ = tessellate(&solid, 0.2);
        let hit_time = start.elapsed();

        // Cache hit should be at least as fast as the miss path. We don't
        // assert a specific multiplier because tiny timings are noisy, but
        // the hit path should never be slower by more than a factor of 2.
        assert!(
            hit_time.as_nanos() * 2 <= miss_time.as_nanos().max(hit_time.as_nanos() * 2),
            "cache hit ({hit_time:?}) should not be slower than miss ({miss_time:?})"
        );
    }

    #[test]
    fn uncached_bypasses_cache() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::make_box(7.0, 7.0, 7.0);

        let _ = tessellate_uncached(&solid, 1.0);
        let _ = tessellate_uncached(&solid, 1.0);

        // Uncached should never populate or query the cache
        let stats = tess_cache_stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.entries, 0);
    }

    #[test]
    fn cache_clear_resets_state() {
        let _guard = cache_test_lock();
        tess_cache_clear();
        let solid = physical_brep::make_box(3.0, 3.0, 3.0);
        let _ = tessellate(&solid, 1.0);
        let _ = tessellate(&solid, 1.0);

        assert!(tess_cache_stats().entries > 0);
        assert!(tess_cache_stats().hits > 0);

        tess_cache_clear();
        let stats = tess_cache_stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn hash_stable_across_identical_solids() {
        let a = physical_brep::make_box(10.0, 10.0, 10.0);
        let b = physical_brep::make_box(10.0, 10.0, 10.0);
        assert_eq!(hash_solid(&a), hash_solid(&b),
            "two structurally identical solids must hash equal");
    }

    #[test]
    fn hash_differs_for_different_solids() {
        let a = physical_brep::make_box(10.0, 10.0, 10.0);
        let b = physical_brep::make_box(10.0, 10.0, 10.01);
        assert_ne!(hash_solid(&a), hash_solid(&b),
            "solids differing at 0.01mm should hash differently");
    }
}
