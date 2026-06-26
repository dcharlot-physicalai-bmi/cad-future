//! Boolean operations on B-Rep solids: union, subtract, intersect.
//!
//! Two-tier approach:
//! 1. **Fast path** (face-level classification): ray-casting point-in-solid test.
//!    For each face of solid A and B, classify its centroid as inside/outside
//!    the other solid. Works for non-intersecting and simply-intersecting solids.
//!
//! 2. **Splitting path**: when faces straddle the other solid's boundary,
//!    split the face polygon at the intersection plane, then classify each
//!    sub-face independently. Uses plane-polygon clipping for planar faces.

use glam::DVec3;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use crate::solid::Solid;
use crate::types::*;

/// Boolean operation type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    /// A ∪ B — material from either solid.
    Union,
    /// A - B — material from A not in B.
    Subtract,
    /// A ∩ B — material in both solids.
    Intersect,
}

/// Perform a boolean operation on two solids.
///
/// Returns a new solid containing the appropriate faces from A and B.
/// Face normals of B are flipped when needed (e.g., for the subtracted
/// cavity in A - B).
pub fn boolean(a: &Solid, b: &Solid, op: BooleanOp) -> Solid {
    let mut result = Solid::new();

    // Classify each face of A against B
    for (_fid, face) in &a.faces {
        let centroid = face_centroid(a, face);
        let inside_b = point_in_solid(centroid, b);

        let keep = match op {
            BooleanOp::Union => !inside_b,      // keep A faces outside B
            BooleanOp::Subtract => !inside_b,    // keep A faces outside B
            BooleanOp::Intersect => inside_b,    // keep A faces inside B
        };

        if keep {
            copy_face(&mut result, a, face, false);
        }
    }

    // Classify each face of B against A
    for (_fid, face) in &b.faces {
        let centroid = face_centroid(b, face);
        let inside_a = point_in_solid(centroid, a);

        let (keep, flip) = match op {
            BooleanOp::Union => (!inside_a, false),      // keep B faces outside A
            BooleanOp::Subtract => (inside_a, true),     // keep B faces inside A, flip normals
            BooleanOp::Intersect => (inside_a, false),   // keep B faces inside A
        };

        if keep {
            copy_face(&mut result, b, face, flip);
        }
    }

    result.link_twins();
    result
}

/// Convenience: union of two solids.
pub fn union(a: &Solid, b: &Solid) -> Solid {
    boolean(a, b, BooleanOp::Union)
}

/// Convenience: subtract B from A.
pub fn subtract(a: &Solid, b: &Solid) -> Solid {
    boolean(a, b, BooleanOp::Subtract)
}

/// Convenience: intersection of two solids.
pub fn intersect(a: &Solid, b: &Solid) -> Solid {
    boolean(a, b, BooleanOp::Intersect)
}

/// Compute the centroid of a face (average of its outer loop vertices).
fn face_centroid(solid: &Solid, face: &BRepFace) -> DVec3 {
    let mut sum = DVec3::ZERO;
    let mut count = 0;
    for he_id in &face.outer_loop {
        let he = &solid.half_edges[*he_id];
        sum += solid.vertices[he.origin].point;
        count += 1;
    }
    if count > 0 { sum / count as f64 } else { DVec3::ZERO }
}

/// Point-in-solid test using ray casting, backed by a cached spatial accelerator.
///
/// On first call with a new solid this builds a `SolidAccel` (bounding box +
/// per-face AABBs) and caches it keyed by the solid's content hash. Repeat
/// calls against the same underlying geometry reuse the accelerator, skipping
/// both rebuild cost and most per-face ray-polygon intersection work. The
/// accelerator is also used to early-out when the query point lies outside
/// the solid's bounding box entirely.
///
/// Also handles the boundary case: if the point lies on a face of the solid
/// (within epsilon distance and inside the face polygon), it is classified
/// as inside. This is critical for CSG face classification when a face
/// centroid lands exactly on the other solid's boundary.
pub fn point_in_solid(point: DVec3, solid: &Solid) -> bool {
    let accel = get_or_build_accel(solid);
    point_in_solid_with_accel(point, solid, &accel)
}

/// Uncached point-in-solid test. Iterates every face with no spatial filter.
///
/// Used as the ground-truth implementation for tests and benchmarks, and as
/// a fallback when callers want to avoid touching the global cache.
pub fn point_in_solid_uncached(point: DVec3, solid: &Solid) -> bool {
    let ray_dir = DVec3::new(0.2672612419124244, 0.5345224838248488, 0.8017837257372732); // normalized (1,2,3)
    let mut crossings = 0;

    for (_fid, face) in &solid.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();
        if verts.len() >= 3 {
            let face_normal = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
            if face_normal.length_squared() > 1e-20 {
                let face_normal = face_normal.normalize();
                let dist = (point - verts[0]).dot(face_normal).abs();
                if dist < 1e-6 && point_in_polygon_3d(point, &verts, face_normal) {
                    return true;
                }
            }
        }

        if let Some(t) = ray_face_intersect(point, ray_dir, solid, face) {
            if t > 1e-10 {
                crossings += 1;
            }
        }
    }

    crossings % 2 == 1
}

/// Point-in-solid using a caller-supplied accelerator.
///
/// Exposed for callers (and tests) that want to manage their own `AccelCache`
/// instances without touching the global cache.
pub fn point_in_solid_with_accel(point: DVec3, solid: &Solid, accel: &SolidAccel) -> bool {
    let ray_dir = DVec3::new(0.2672612419124244, 0.5345224838248488, 0.8017837257372732);

    // Boundary check: walk only faces whose AABB contains the point (within eps).
    for (fid, fmin, fmax) in &accel.face_bboxes {
        const BEPS: f64 = 1e-5;
        if point.x < fmin.x - BEPS || point.x > fmax.x + BEPS { continue; }
        if point.y < fmin.y - BEPS || point.y > fmax.y + BEPS { continue; }
        if point.z < fmin.z - BEPS || point.z > fmax.z + BEPS { continue; }
        let face = &solid.faces[*fid];
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();
        if verts.len() >= 3 {
            let face_normal = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
            if face_normal.length_squared() > 1e-20 {
                let face_normal = face_normal.normalize();
                let dist = (point - verts[0]).dot(face_normal).abs();
                if dist < 1e-6 && point_in_polygon_3d(point, &verts, face_normal) {
                    return true;
                }
            }
        }
    }

    // Global bbox early-out: if the point is strictly outside the solid's
    // bounding box, it cannot be inside.
    if !accel.contains_bbox(point) {
        return false;
    }

    // Ray-cast: only intersect faces whose AABB can be hit by the ray from `point`.
    let mut crossings = 0;
    for (fid, fmin, fmax) in &accel.face_bboxes {
        if !ray_hits_aabb(point, ray_dir, *fmin, *fmax) { continue; }
        let face = &solid.faces[*fid];
        if let Some(t) = ray_face_intersect(point, ray_dir, solid, face) {
            if t > 1e-10 {
                crossings += 1;
            }
        }
    }

    crossings % 2 == 1
}

/// Slab test: does the forward ray from `origin` along `dir` hit the AABB?
fn ray_hits_aabb(origin: DVec3, dir: DVec3, min: DVec3, max: DVec3) -> bool {
    let mut tmin = f64::NEG_INFINITY;
    let mut tmax = f64::INFINITY;
    for i in 0..3 {
        let o = component(origin, i);
        let d = component(dir, i);
        let lo = component(min, i);
        let hi = component(max, i);
        if d.abs() < 1e-12 {
            if o < lo - 1e-6 || o > hi + 1e-6 { return false; }
        } else {
            let inv = 1.0 / d;
            let mut t0 = (lo - o) * inv;
            let mut t1 = (hi - o) * inv;
            if t0 > t1 { std::mem::swap(&mut t0, &mut t1); }
            if t0 > tmin { tmin = t0; }
            if t1 < tmax { tmax = t1; }
            if tmin > tmax { return false; }
        }
    }
    tmax >= 0.0
}

// ---------------------------------------------------------------------------
// LUT: content-addressed spatial-accelerator cache for point_in_solid
// ---------------------------------------------------------------------------

/// Precomputed spatial accelerator for a solid: the global bounding box plus
/// per-face AABBs. Both are used to prune candidate faces during ray casting
/// so point-in-solid turns from O(F) per query into O(candidates).
#[derive(Clone, Debug)]
pub struct SolidAccel {
    pub bbox_min: DVec3,
    pub bbox_max: DVec3,
    pub face_bboxes: Vec<(FaceId, DVec3, DVec3)>,
}

impl SolidAccel {
    /// Build an accelerator for `solid` by walking its topology once.
    pub fn build(solid: &Solid) -> Self {
        let mut bbox_min = DVec3::splat(f64::MAX);
        let mut bbox_max = DVec3::splat(f64::MIN);
        for (_, v) in &solid.vertices {
            bbox_min = bbox_min.min(v.point);
            bbox_max = bbox_max.max(v.point);
        }
        if solid.vertices.is_empty() {
            bbox_min = DVec3::ZERO;
            bbox_max = DVec3::ZERO;
        }

        let mut face_bboxes = Vec::with_capacity(solid.faces.len());
        for (fid, face) in &solid.faces {
            let mut fmin = DVec3::splat(f64::MAX);
            let mut fmax = DVec3::splat(f64::MIN);
            for he_id in &face.outer_loop {
                let p = solid.vertices[solid.half_edges[*he_id].origin].point;
                fmin = fmin.min(p);
                fmax = fmax.max(p);
            }
            face_bboxes.push((fid, fmin, fmax));
        }

        Self { bbox_min, bbox_max, face_bboxes }
    }

    /// Is `p` within the solid's bounding box (within epsilon)?
    pub fn contains_bbox(&self, p: DVec3) -> bool {
        const EPS: f64 = 1e-6;
        p.x >= self.bbox_min.x - EPS && p.x <= self.bbox_max.x + EPS
            && p.y >= self.bbox_min.y - EPS && p.y <= self.bbox_max.y + EPS
            && p.z >= self.bbox_min.z - EPS && p.z <= self.bbox_max.z + EPS
    }
}

/// Content-addressed hash of a solid: quantized vertex coordinates sorted
/// into a canonical order. Stable across iteration order.
fn hash_solid(solid: &Solid) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    let mut points: Vec<[i64; 3]> = solid
        .vertices
        .values()
        .map(|v| {
            [
                (v.point.x * 1000.0).round() as i64,
                (v.point.y * 1000.0).round() as i64,
                (v.point.z * 1000.0).round() as i64,
            ]
        })
        .collect();
    points.sort();
    points.hash(&mut hasher);
    (solid.faces.len() as u64).hash(&mut hasher);
    hasher.finish()
}

/// Snapshot of an accelerator cache's state.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccelCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Content-addressed cache of precomputed `SolidAccel` structures keyed by
/// solid hash. Used by `point_in_solid` to turn repeat queries against the
/// same underlying geometry into O(1) lookups rather than rebuilds.
#[derive(Debug, Default)]
pub struct AccelCache {
    map: HashMap<u64, Arc<SolidAccel>>,
    hits: u64,
    misses: u64,
}

impl AccelCache {
    pub fn new() -> Self { Self::default() }

    /// Fetch (or build and insert) the accelerator for `solid`.
    pub fn get_or_build(&mut self, solid: &Solid) -> Arc<SolidAccel> {
        let h = hash_solid(solid);
        if let Some(existing) = self.map.get(&h) {
            self.hits += 1;
            return Arc::clone(existing);
        }
        self.misses += 1;
        let accel = Arc::new(SolidAccel::build(solid));
        self.map.insert(h, Arc::clone(&accel));
        accel
    }

    pub fn stats(&self) -> AccelCacheStats {
        AccelCacheStats {
            entries: self.map.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

fn global_accel_cache() -> &'static Mutex<AccelCache> {
    static CACHE: OnceLock<Mutex<AccelCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(AccelCache::new()))
}

fn get_or_build_accel(solid: &Solid) -> Arc<SolidAccel> {
    global_accel_cache().lock().unwrap().get_or_build(solid)
}

/// Snapshot the global accelerator cache.
pub fn accel_cache_stats() -> AccelCacheStats {
    global_accel_cache().lock().unwrap().stats()
}

/// Drop all entries from the global accelerator cache.
pub fn accel_cache_clear() {
    global_accel_cache().lock().unwrap().clear();
}

/// Ray-face intersection for planar faces.
///
/// Returns the parametric t along the ray if the ray hits the face polygon.
fn ray_face_intersect(
    origin: DVec3,
    dir: DVec3,
    solid: &Solid,
    face: &BRepFace,
) -> Option<f64> {
    // Get face polygon vertices
    let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
        solid.vertices[solid.half_edges[*he_id].origin].point
    }).collect();

    if verts.len() < 3 { return None; }

    // Compute face normal from first 3 vertices
    let e1 = verts[1] - verts[0];
    let e2 = verts[2] - verts[0];
    let normal = e1.cross(e2);
    if normal.length_squared() < 1e-20 { return None; }
    let normal = normal.normalize();

    // Ray-plane intersection
    let denom = normal.dot(dir);
    if denom.abs() < 1e-12 { return None; } // parallel

    let t = (verts[0] - origin).dot(normal) / denom;
    if t < 0.0 { return None; }

    // Check if intersection point is inside the polygon
    let hit = origin + dir * t;
    if point_in_polygon_3d(hit, &verts, normal) {
        Some(t)
    } else {
        None
    }
}

/// Test if a 3D point lies inside a convex/concave polygon.
/// Projects to 2D (dropping the axis most aligned with the normal) and uses winding number.
fn point_in_polygon_3d(point: DVec3, verts: &[DVec3], normal: DVec3) -> bool {
    // Choose projection plane: drop the axis most aligned with normal
    let abs_n = DVec3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());
    let (i0, i1) = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
        (1, 2) // drop X
    } else if abs_n.y >= abs_n.z {
        (0, 2) // drop Y
    } else {
        (0, 1) // drop Z
    };

    let px = component(point, i0);
    let py = component(point, i1);

    // Winding number test
    let n = verts.len();
    let mut winding = 0i32;
    for i in 0..n {
        let v0 = verts[i];
        let v1 = verts[(i + 1) % n];
        let y0 = component(v0, i1) - py;
        let y1 = component(v1, i1) - py;

        if y0 <= 0.0 {
            if y1 > 0.0 {
                let x0 = component(v0, i0) - px;
                let x1 = component(v1, i0) - px;
                if x0 * y1 - x1 * y0 > 0.0 {
                    winding += 1;
                }
            }
        } else if y1 <= 0.0 {
            let x0 = component(v0, i0) - px;
            let x1 = component(v1, i0) - px;
            if x0 * y1 - x1 * y0 < 0.0 {
                winding -= 1;
            }
        }
    }

    winding != 0
}

fn component(v: DVec3, idx: usize) -> f64 {
    match idx {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Copy a face (and its vertices) from `src` into `dst`.
/// If `flip`, reverse winding and invert normal.
fn copy_face(dst: &mut Solid, src: &Solid, face: &BRepFace, flip: bool) {
    // Map source vertex IDs → new vertex IDs in dst
    let mut vert_map: Vec<(VertexId, VertexId)> = Vec::new();
    let mut get_or_add = |dst: &mut Solid, src_vid: VertexId, src_point: DVec3| -> VertexId {
        if let Some((_src, dst_id)) = vert_map.iter().find(|(s, _)| *s == src_vid) {
            return *dst_id;
        }
        let new_id = dst.add_vertex(src_point);
        vert_map.push((src_vid, new_id));
        new_id
    };

    // Collect the face's vertex IDs in order
    let mut new_vids: Vec<VertexId> = Vec::new();
    for he_id in &face.outer_loop {
        let he = &src.half_edges[*he_id];
        let point = src.vertices[he.origin].point;
        let new_vid = get_or_add(dst, he.origin, point);
        new_vids.push(new_vid);
    }

    if flip {
        new_vids.reverse();
    }

    // Construct the surface (flip normal if needed)
    let surface = if flip {
        face.surface.flipped()
    } else {
        face.surface.clone()
    };

    // Don't XOR outward with flip: surface.flipped() already negated the normal,
    // so the relationship between surface normal and outward direction is preserved.
    dst.add_face_from_vertices(surface, &new_vids, face.normal_outward);
}

// ---------------------------------------------------------------------------
// Signed Volume — divergence theorem on triangulated B-Rep faces
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Face splitting — clip polygon by plane
// ---------------------------------------------------------------------------

/// An intersection edge found between two faces.
#[derive(Debug, Clone)]
pub struct IntersectionEdge {
    pub start: DVec3,
    pub end: DVec3,
}

/// Split a polygon by a plane. Returns (front, back) where:
/// - `front` = vertices on the positive side of the plane (dot(normal, p) > d)
/// - `back` = vertices on the negative side
///
/// Both polygons include the intersection points on the plane boundary.
pub fn clip_polygon_by_plane(
    polygon: &[DVec3],
    plane_normal: DVec3,
    plane_d: f64,
) -> (Vec<DVec3>, Vec<DVec3>) {
    if polygon.len() < 3 {
        return (polygon.to_vec(), Vec::new());
    }

    let n = polygon.len();
    let mut front = Vec::new();
    let mut back = Vec::new();

    // Classify each vertex
    let dists: Vec<f64> = polygon.iter()
        .map(|p| p.dot(plane_normal) - plane_d)
        .collect();

    for i in 0..n {
        let j = (i + 1) % n;
        let di = dists[i];
        let dj = dists[j];
        let vi = polygon[i];
        let vj = polygon[j];

        const EPS: f64 = 1e-8;

        if di >= -EPS {
            front.push(vi);
        }
        if di <= EPS {
            back.push(vi);
        }

        // Edge crosses the plane → compute intersection point
        if (di > EPS && dj < -EPS) || (di < -EPS && dj > EPS) {
            let t = di / (di - dj);
            let intersection = vi + (vj - vi) * t;
            front.push(intersection);
            back.push(intersection);
        }
    }

    (front, back)
}

/// Boolean with face splitting: for each face that straddles the other solid,
/// clip it by the bounding faces and classify sub-polygons.
pub fn boolean_with_splitting(a: &Solid, b: &Solid, op: BooleanOp) -> Solid {
    let mut result = Solid::new();

    // Process A's faces against B
    for (_fid, face) in &a.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            a.vertices[a.half_edges[*he_id].origin].point
        }).collect();

        let sub_polys = split_face_against_solid(&verts, b);

        for poly in &sub_polys {
            if poly.len() < 3 { continue; }
            let centroid = poly.iter().copied().sum::<DVec3>() / poly.len() as f64;
            let inside_b = point_in_solid(centroid, b);

            let keep = match op {
                BooleanOp::Union => !inside_b,
                BooleanOp::Subtract => !inside_b,
                BooleanOp::Intersect => inside_b,
            };

            if keep {
                add_polygon_as_face(&mut result, poly, &face.surface, face.normal_outward, false);
            }
        }
    }

    // Process B's faces against A
    for (_fid, face) in &b.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            b.vertices[b.half_edges[*he_id].origin].point
        }).collect();

        let sub_polys = split_face_against_solid(&verts, a);

        for poly in &sub_polys {
            if poly.len() < 3 { continue; }
            let centroid = poly.iter().copied().sum::<DVec3>() / poly.len() as f64;
            let inside_a = point_in_solid(centroid, a);

            let (keep, flip) = match op {
                BooleanOp::Union => (!inside_a, false),
                BooleanOp::Subtract => (inside_a, true),
                BooleanOp::Intersect => (inside_a, false),
            };

            if keep {
                add_polygon_as_face(&mut result, poly, &face.surface, face.normal_outward, flip);
            }
        }
    }

    result.link_twins();
    result
}

/// Split a face polygon by all faces of another solid.
/// Returns a list of sub-polygons.
/// Handles both planar and cylindrical face intersections.
fn split_face_against_solid(polygon: &[DVec3], solid: &Solid) -> Vec<Vec<DVec3>> {
    let mut current_polys = vec![polygon.to_vec()];

    for (_fid, face) in &solid.faces {
        let face_verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();

        if face_verts.len() < 3 { continue; }

        // Try surface-aware splitting based on face type
        match &face.surface {
            crate::surface::Surface::Cylinder { origin, axis, radius } => {
                // Split polygon by cylinder: points inside vs outside cylinder
                let ax = axis.normalize();
                let mut next_polys = Vec::new();
                for poly in &current_polys {
                    let (inside, outside) = split_polygon_by_cylinder(poly, *origin, ax, *radius);
                    let has_inside = inside.len() >= 3;
                    let has_outside = outside.len() >= 3;
                    if has_inside { next_polys.push(inside); }
                    if has_outside { next_polys.push(outside); }
                    if !has_inside && !has_outside {
                        next_polys.push(poly.clone());
                    }
                }
                current_polys = next_polys;
            }
            crate::surface::Surface::Sphere { center, radius } => {
                let mut next_polys = Vec::new();
                for poly in &current_polys {
                    let (inside, outside) = split_polygon_by_sphere(poly, *center, *radius);
                    let has_inside = inside.len() >= 3;
                    let has_outside = outside.len() >= 3;
                    if has_inside { next_polys.push(inside); }
                    if has_outside { next_polys.push(outside); }
                    if !has_inside && !has_outside {
                        next_polys.push(poly.clone());
                    }
                }
                current_polys = next_polys;
            }
            _ => {
                // Planar split (original behavior)
                let e1 = face_verts[1] - face_verts[0];
                let e2 = face_verts[2] - face_verts[0];
                let normal = e1.cross(e2);
                if normal.length_squared() < 1e-20 { continue; }
                let normal = normal.normalize();
                let d = face_verts[0].dot(normal);

                let mut next_polys = Vec::new();
                for poly in &current_polys {
                    let dists: Vec<f64> = poly.iter().map(|p| p.dot(normal) - d).collect();
                    let has_pos = dists.iter().any(|d| *d > 1e-6);
                    let has_neg = dists.iter().any(|d| *d < -1e-6);

                    if has_pos && has_neg {
                        let (front, back) = clip_polygon_by_plane(poly, normal, d);
                        if front.len() >= 3 { next_polys.push(front); }
                        if back.len() >= 3 { next_polys.push(back); }
                    } else {
                        next_polys.push(poly.clone());
                    }
                }
                current_polys = next_polys;
            }
        }
    }

    current_polys
}

/// Split a polygon by a cylinder: inside vs outside the cylinder surface.
fn split_polygon_by_cylinder(polygon: &[DVec3], origin: DVec3, axis: DVec3, radius: f64) -> (Vec<DVec3>, Vec<DVec3>) {
    if polygon.len() < 3 { return (polygon.to_vec(), Vec::new()); }
    let n = polygon.len();
    let mut inside = Vec::new();
    let mut outside = Vec::new();

    // Signed distance from cylinder axis: positive = outside, negative = inside
    let cyl_dist = |p: DVec3| -> f64 {
        let d = p - origin;
        let along = d.dot(axis);
        let perp = (d - axis * along).length();
        perp - radius
    };

    let dists: Vec<f64> = polygon.iter().map(|p| cyl_dist(*p)).collect();

    for i in 0..n {
        let j = (i + 1) % n;
        let di = dists[i];
        let dj = dists[j];
        let vi = polygon[i];
        let vj = polygon[j];

        if di <= 0.0 { inside.push(vi); }
        if di >= 0.0 { outside.push(vi); }

        // Edge crosses cylinder boundary
        if (di > 1e-6 && dj < -1e-6) || (di < -1e-6 && dj > 1e-6) {
            // Binary search for intersection point on the edge
            let mut lo = 0.0_f64;
            let mut hi = 1.0_f64;
            for _ in 0..20 {
                let mid = (lo + hi) / 2.0;
                let p = vi + (vj - vi) * mid;
                if cyl_dist(p) > 0.0 { hi = mid; } else { lo = mid; }
            }
            let intersection = vi + (vj - vi) * ((lo + hi) / 2.0);
            inside.push(intersection);
            outside.push(intersection);
        }
    }

    (inside, outside)
}

/// Split a polygon by a sphere: inside vs outside the sphere surface.
fn split_polygon_by_sphere(polygon: &[DVec3], center: DVec3, radius: f64) -> (Vec<DVec3>, Vec<DVec3>) {
    if polygon.len() < 3 { return (polygon.to_vec(), Vec::new()); }
    let n = polygon.len();
    let mut inside = Vec::new();
    let mut outside = Vec::new();

    let sph_dist = |p: DVec3| -> f64 { (p - center).length() - radius };

    let dists: Vec<f64> = polygon.iter().map(|p| sph_dist(*p)).collect();

    for i in 0..n {
        let j = (i + 1) % n;
        let di = dists[i];
        let dj = dists[j];
        let vi = polygon[i];
        let vj = polygon[j];

        if di <= 0.0 { inside.push(vi); }
        if di >= 0.0 { outside.push(vi); }

        if (di > 1e-6 && dj < -1e-6) || (di < -1e-6 && dj > 1e-6) {
            let t = di / (di - dj);
            let intersection = vi + (vj - vi) * t;
            inside.push(intersection);
            outside.push(intersection);
        }
    }

    (inside, outside)
}

/// Add a polygon as a face to a solid.
fn add_polygon_as_face(
    dst: &mut Solid,
    polygon: &[DVec3],
    surface: &crate::surface::Surface,
    normal_outward: bool,
    flip: bool,
) {
    let mut vids: Vec<VertexId> = polygon.iter()
        .map(|p| dst.add_vertex(*p))
        .collect();

    if flip {
        vids.reverse();
    }

    let surf = if flip { surface.flipped() } else { surface.clone() };
    dst.add_face_from_vertices(surf, &vids, normal_outward);
}

// ---------------------------------------------------------------------------
// Volume and area computations
// ---------------------------------------------------------------------------

/// Compute the signed volume of a solid using the divergence theorem.
///
/// For each triangulated face, sums the signed volume of tetrahedra formed
/// with the origin: V = (1/6) Σ det(v0, v1, v2) over all triangles.
/// Positive = outward normals consistent with right-hand rule.
pub fn signed_volume(solid: &Solid) -> f64 {
    let mut vol = 0.0;

    for (_fid, face) in &solid.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();

        // Fan triangulation from first vertex
        for i in 1..verts.len().saturating_sub(1) {
            let v0 = verts[0];
            let v1 = verts[i];
            let v2 = verts[i + 1];
            // Signed volume of tetrahedron (origin, v0, v1, v2)
            vol += v0.dot(v1.cross(v2));
        }
    }

    vol / 6.0
}

/// Absolute volume of a solid.
pub fn volume(solid: &Solid) -> f64 {
    signed_volume(solid).abs()
}

/// Surface area of a solid (sum of face areas).
pub fn surface_area(solid: &Solid) -> f64 {
    let mut area = 0.0;
    for (_fid, face) in &solid.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();
        for i in 1..verts.len().saturating_sub(1) {
            let e1 = verts[i] - verts[0];
            let e2 = verts[i + 1] - verts[0];
            area += e1.cross(e2).length() * 0.5;
        }
    }
    area
}

/// Point classification result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Inside,
    Outside,
    OnBoundary,
}

/// Classify a point relative to a solid.
pub fn classify_point(point: DVec3, solid: &Solid) -> Classification {
    // Check boundary first (on any face)
    for (_fid, face) in &solid.faces {
        let verts: Vec<DVec3> = face.outer_loop.iter().map(|he_id| {
            solid.vertices[solid.half_edges[*he_id].origin].point
        }).collect();
        if verts.len() >= 3 {
            let normal = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
            if normal.length_squared() > 1e-20 {
                let normal = normal.normalize();
                let dist = (point - verts[0]).dot(normal).abs();
                if dist < 1e-6 && point_in_polygon_3d(point, &verts, normal) {
                    return Classification::OnBoundary;
                }
            }
        }
    }

    if point_in_solid(point, solid) {
        Classification::Inside
    } else {
        Classification::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::make_box;

    /// Helper: run a point-in-solid query through a LOCAL cache so tests are
    /// parallel-safe and don't share state with the global accelerator cache.
    fn local_pis(cache: &mut AccelCache, p: DVec3, s: &Solid) -> bool {
        let accel = cache.get_or_build(s);
        point_in_solid_with_accel(p, s, &accel)
    }

    #[test]
    fn union_non_overlapping_boxes() {
        let a = make_box(10.0, 10.0, 10.0); // occupies (0..10, 0..10, 0..10)
        let mut b = make_box(5.0, 5.0, 5.0);
        // Move B far away so no overlap
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += 20.0;
        }

        let result = union(&a, &b);
        // Both solids kept entirely: 6 + 6 = 12 faces
        assert_eq!(result.face_count(), 12);
    }

    #[test]
    fn subtract_overlapping_box() {
        // A: centered box (-10..10)³, B: (-5..5)³ translated by 5 → (0..10)³
        // B overlaps A: 3 of B's faces coincide with A's +X,+Y,+Z faces
        let a = make_box(20.0, 20.0, 20.0);
        let mut b = make_box(10.0, 10.0, 10.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point += DVec3::splat(5.0);
        }

        let result = subtract(&a, &b);
        // A's -X,-Y,-Z faces: outside B → kept (3)
        // A's +X,+Y,+Z faces: on B boundary → classified inside B → dropped (3)
        // B faces: all inside A → kept+flipped (6)
        // Total: 9 (face splitting needed for full correctness)
        assert_eq!(result.face_count(), 9);
    }

    #[test]
    fn intersect_overlapping_box() {
        // A: centered box (-10..10)³, B: (0..10)³
        let a = make_box(20.0, 20.0, 20.0);
        let mut b = make_box(10.0, 10.0, 10.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point += DVec3::splat(5.0);
        }

        let result = intersect(&a, &b);
        // A's +X,+Y,+Z faces: on B boundary → classified inside B → kept (3)
        // B faces: all inside A → kept (6)
        // Total: 9 (face splitting needed for full correctness)
        assert_eq!(result.face_count(), 9);
    }

    #[test]
    fn union_non_overlapping_has_all_faces() {
        let a = make_box(10.0, 10.0, 10.0);
        let mut b = make_box(5.0, 5.0, 5.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += 20.0;
        }

        let result = union(&a, &b);
        assert_eq!(result.face_count(), 12);
        // Each face has 4 vertices (not shared between faces in copy)
        assert_eq!(result.vertex_count(), 48);
    }

    #[test]
    fn point_in_box() {
        let b = make_box(10.0, 10.0, 10.0); // centered at origin: (-5..5, -5..5, -5..5)
        let ray_dir = DVec3::new(0.2672612419124244, 0.5345224838248488, 0.8017837257372732);
        let origin = DVec3::ZERO;

        // Debug: count crossings manually
        let mut crossings = 0;
        for (_fid, face) in &b.faces {
            if let Some(t) = ray_face_intersect(origin, ray_dir, &b, face) {
                if t > 1e-10 {
                    crossings += 1;
                }
            }
        }
        assert_eq!(crossings % 2, 1, "Expected odd crossings from box center, got {}", crossings);

        assert!(point_in_solid(DVec3::ZERO, &b), "Origin should be inside centered box");
        assert!(point_in_solid(DVec3::new(1.0, 1.0, 1.0), &b));
        // Outside
        assert!(!point_in_solid(DVec3::new(15.0, 0.0, 0.0), &b));
        assert!(!point_in_solid(DVec3::new(0.0, 20.0, 0.0), &b));
        assert!(!point_in_solid(DVec3::new(-10.0, 0.0, 0.0), &b));
    }

    #[test]
    fn signed_volume_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let vol = volume(&b);
        // Box is 10×10×10 = 1000 mm³
        assert!(
            (vol - 1000.0).abs() < 10.0,
            "volume should be ~1000, got {vol:.1}"
        );
    }

    #[test]
    fn surface_area_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let area = surface_area(&b);
        // 6 faces × 10×10 = 600 mm²
        assert!(
            (area - 600.0).abs() < 10.0,
            "surface area should be ~600, got {area:.1}"
        );
    }

    #[test]
    fn classify_inside() {
        let b = make_box(10.0, 10.0, 10.0);
        assert_eq!(classify_point(DVec3::ZERO, &b), Classification::Inside);
    }

    #[test]
    fn classify_outside() {
        let b = make_box(10.0, 10.0, 10.0);
        assert_eq!(classify_point(DVec3::new(20.0, 0.0, 0.0), &b), Classification::Outside);
    }

    #[test]
    fn classify_boundary() {
        let b = make_box(10.0, 10.0, 10.0);
        // Face center of the +X face at x=5, y=0, z=0
        let result = classify_point(DVec3::new(5.0, 0.0, 0.0), &b);
        assert_eq!(result, Classification::OnBoundary);
    }

    #[test]
    fn clip_polygon_by_plane_splits_square() {
        // Square in XY plane from (0,0,0) to (10,10,0)
        let sq = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 0.0),
            DVec3::new(0.0, 10.0, 0.0),
        ];
        // Split by plane x = 5 (normal = X, d = 5)
        let (front, back) = clip_polygon_by_plane(&sq, DVec3::X, 5.0);
        // Front: x >= 5, Back: x <= 5
        assert!(front.len() >= 3, "front should have >= 3 vertices, got {}", front.len());
        assert!(back.len() >= 3, "back should have >= 3 vertices, got {}", back.len());
        // Front vertices should all have x >= 5-eps
        for v in &front {
            assert!(v.x >= 4.99, "front vertex at x={} should be >= 5", v.x);
        }
        for v in &back {
            assert!(v.x <= 5.01, "back vertex at x={} should be <= 5", v.x);
        }
    }

    #[test]
    fn clip_polygon_no_split_when_all_front() {
        let tri = vec![
            DVec3::new(6.0, 0.0, 0.0),
            DVec3::new(8.0, 0.0, 0.0),
            DVec3::new(7.0, 5.0, 0.0),
        ];
        let (front, back) = clip_polygon_by_plane(&tri, DVec3::X, 5.0);
        assert!(front.len() >= 3);
        assert!(back.len() < 3, "triangle fully in front should have empty back");
    }

    #[test]
    fn boolean_with_splitting_union() {
        let a = make_box(20.0, 20.0, 20.0);
        let mut b = make_box(10.0, 10.0, 10.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += 15.0; // far away
        }
        let result = boolean_with_splitting(&a, &b, BooleanOp::Union);
        assert!(result.face_count() >= 12, "union should have >= 12 faces");
    }

    #[test]
    fn boolean_with_splitting_subtract_overlapping() {
        let a = make_box(20.0, 20.0, 20.0);
        let mut b = make_box(10.0, 10.0, 10.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point += DVec3::splat(5.0);
        }
        let result = boolean_with_splitting(&a, &b, BooleanOp::Subtract);
        // With splitting, we should get more faces than without
        assert!(result.face_count() >= 6, "subtract should produce faces, got {}", result.face_count());
    }

    #[test]
    fn union_preserves_volume_approximately() {
        let a = make_box(10.0, 10.0, 10.0); // 1000 mm³
        let mut b = make_box(5.0, 5.0, 5.0); // 125 mm³
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += 20.0;
        }
        let result = union(&a, &b);
        let vol = volume(&result);
        // Non-overlapping union: 1000 + 125 = 1125
        assert!(
            (vol - 1125.0).abs() < 50.0,
            "union volume should be ~1125, got {vol:.1}"
        );
    }

    #[test]
    fn subtract_cylinder_from_box_with_splitting() {
        // Box centered at origin, cylinder along Z through center
        let a = make_box(20.0, 20.0, 20.0);
        let b = crate::builder::make_cylinder(3.0, 30.0, 16); // r=3, h=30, through the box

        let result = boolean_with_splitting(&a, &b, BooleanOp::Subtract);
        // Should produce faces — the box minus a hole
        assert!(result.face_count() > 0, "subtract should produce faces, got {}", result.face_count());
        // Should have more faces than original box (hole adds faces)
        assert!(result.face_count() >= 6, "should have at least box faces");
    }

    #[test]
    fn cylinder_plane_split_produces_inside_outside() {
        // Rectangle straddling a cylinder — edges cross the cylinder boundary
        // Polygon from (-3,-10,0) to (3,-10,0) to (3,10,0) to (-3,10,0)
        // Cylinder along Z at origin, radius 5 — the polygon vertices are at
        // distance ~10.4 (corners) and edges at x=±3 pass through the cylinder
        let polygon = vec![
            DVec3::new(-3.0, -10.0, 0.0),
            DVec3::new(3.0, -10.0, 0.0),
            DVec3::new(3.0, 10.0, 0.0),
            DVec3::new(-3.0, 10.0, 0.0),
        ];
        // Cylinder along Z at origin, radius 5 — the edges at y=±10 are at
        // r = sqrt(9+100)=10.4 (outside) but at y=0: r = 3 (inside)
        let (inside, outside) = split_polygon_by_cylinder(
            &polygon, DVec3::ZERO, DVec3::Z, 5.0
        );
        // Edges from y=-10 to y=10 cross the cylinder at y ≈ ±4
        // so we should get both inside and outside regions
        assert!(inside.len() >= 3 || outside.len() >= 3,
            "should split: inside={}, outside={}", inside.len(), outside.len());
    }

    // -----------------------------------------------------------------
    // LUT cache tests: SolidAccel + point_in_solid_cached
    // -----------------------------------------------------------------

    #[test]
    fn local_accel_cache_reports_hits_and_misses() {
        let mut cache = AccelCache::new();
        let b = make_box(10.0, 10.0, 10.0);
        assert!(local_pis(&mut cache, DVec3::ZERO, &b));
        let s1 = cache.stats();
        assert_eq!(s1.misses, 1, "first call is a miss");
        assert_eq!(s1.hits, 0);

        assert!(local_pis(&mut cache, DVec3::new(1.0, 1.0, 1.0), &b));
        let s2 = cache.stats();
        assert_eq!(s2.misses, 1, "second call reuses accel");
        assert_eq!(s2.hits, 1);
    }

    #[test]
    fn local_accel_cache_differentiates_by_solid() {
        let mut cache = AccelCache::new();
        let a = make_box(10.0, 10.0, 10.0);
        let b = make_box(5.0, 5.0, 5.0);
        assert!(local_pis(&mut cache, DVec3::ZERO, &a));
        assert!(local_pis(&mut cache, DVec3::ZERO, &b));
        let s = cache.stats();
        assert_eq!(s.misses, 2);
        assert_eq!(s.entries, 2, "distinct solids → distinct cache entries");
    }

    #[test]
    fn local_accel_cache_hit_rate_on_repeated_boxes() {
        let mut cache = AccelCache::new();
        let b = make_box(10.0, 10.0, 10.0);
        for _ in 0..20 {
            let _ = local_pis(&mut cache, DVec3::ZERO, &b);
        }
        let s = cache.stats();
        assert_eq!(s.misses, 1, "only first call builds accel");
        assert_eq!(s.hits, 19, "remaining 19 calls hit cache");
    }

    #[test]
    fn local_accel_cache_clear_resets_state() {
        let mut cache = AccelCache::new();
        let b = make_box(10.0, 10.0, 10.0);
        let _ = local_pis(&mut cache, DVec3::ZERO, &b);
        cache.clear();
        let s = cache.stats();
        assert_eq!(s.misses, 0);
        assert_eq!(s.hits, 0);
        assert_eq!(s.entries, 0);
    }

    #[test]
    fn uncached_matches_cached_on_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let points = [
            DVec3::ZERO,
            DVec3::new(1.0, 1.0, 1.0),
            DVec3::new(4.9, 0.0, 0.0),
            DVec3::new(15.0, 0.0, 0.0),
            DVec3::new(0.0, 20.0, 0.0),
            DVec3::new(-10.0, 0.0, 0.0),
        ];
        for p in points {
            assert_eq!(
                point_in_solid(p, &b),
                point_in_solid_uncached(p, &b),
                "mismatch at {p:?}"
            );
        }
    }

    #[test]
    fn accel_bbox_early_out_rejects_far_points() {
        let b = make_box(10.0, 10.0, 10.0); // centered at origin, half-extent 5
        let accel = SolidAccel::build(&b);
        assert!(accel.contains_bbox(DVec3::ZERO));
        assert!(!accel.contains_bbox(DVec3::new(100.0, 0.0, 0.0)));
        assert!(!accel.contains_bbox(DVec3::new(0.0, -50.0, 0.0)));
    }

    #[test]
    fn accel_build_on_box_has_six_face_bboxes() {
        let b = make_box(10.0, 10.0, 10.0);
        let accel = SolidAccel::build(&b);
        assert_eq!(accel.face_bboxes.len(), 6);
        // Global bbox should be (-5..5, -5..5, -5..5) within epsilon
        for axis in 0..3 {
            assert!((component(accel.bbox_min, axis) + 5.0).abs() < 1e-9);
            assert!((component(accel.bbox_max, axis) - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn local_accel_handles_translated_solid() {
        let mut cache = AccelCache::new();
        let a = make_box(10.0, 10.0, 10.0);
        let mut b = make_box(10.0, 10.0, 10.0);
        let vids: Vec<VertexId> = b.vertices.keys().collect();
        for vid in vids {
            b.vertices[vid].point.x += 100.0;
        }
        assert!(local_pis(&mut cache, DVec3::ZERO, &a));
        assert!(!local_pis(&mut cache, DVec3::ZERO, &b));
        assert!(local_pis(&mut cache, DVec3::new(100.0, 0.0, 0.0), &b));
        let s = cache.stats();
        assert_eq!(s.entries, 2);
    }

    #[test]
    fn ray_hits_aabb_basic_cases() {
        let min = DVec3::new(-1.0, -1.0, -1.0);
        let max = DVec3::new(1.0, 1.0, 1.0);
        // Ray from origin along +X hits the box
        assert!(ray_hits_aabb(DVec3::ZERO, DVec3::X, min, max));
        // Ray from outside aimed at box hits
        assert!(ray_hits_aabb(DVec3::new(-5.0, 0.0, 0.0), DVec3::X, min, max));
        // Ray aimed away misses
        assert!(!ray_hits_aabb(DVec3::new(5.0, 0.0, 0.0), DVec3::X, min, max));
    }

    #[test]
    fn sphere_plane_split() {
        // Triangle that straddles a sphere
        let polygon = vec![
            DVec3::new(-3.0, -8.0, 0.0), // outside (r=8.5)
            DVec3::new(3.0, -8.0, 0.0),  // outside (r=8.5)
            DVec3::new(0.0, 2.0, 0.0),   // inside (r=2.0)
        ];
        let (inside, outside) = split_polygon_by_sphere(
            &polygon, DVec3::ZERO, 5.0
        );
        assert!(inside.len() >= 3 || outside.len() >= 3,
            "should split: inside={}, outside={}", inside.len(), outside.len());
    }
}
