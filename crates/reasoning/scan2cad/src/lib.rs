//! `physical-scan2cad` — Point cloud processing, RANSAC primitive detection,
//! and B-Rep solid reconstruction.
//!
//! Converts raw 3D scan data (point clouds) into parametric B-Rep geometry
//! suitable for downstream CAD operations.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use glam::DVec3;
use physical_brep::{Solid, Surface, make_box};
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Internal LCG random number generator (no external crate)
// ---------------------------------------------------------------------------

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    fn next_u64(&mut self) -> u64 {
        // Numerical Recipes LCG
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }

    /// Uniform f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Random index in [0, n).
    fn next_usize(&mut self, n: usize) -> usize {
        (self.next_f64() * n as f64) as usize % n
    }
}

// ---------------------------------------------------------------------------
// Point cloud
// ---------------------------------------------------------------------------

/// A 3D point cloud with optional per-point normals and colors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointCloud {
    pub points: Vec<DVec3>,
    pub normals: Option<Vec<DVec3>>,
    pub colors: Option<Vec<[u8; 3]>>,
}

impl PointCloud {
    /// Create a point cloud from a slice of points.
    pub fn from_points(pts: &[DVec3]) -> Self {
        Self {
            points: pts.to_vec(),
            normals: None,
            colors: None,
        }
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the cloud is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Normal estimation (PCA on k-nearest neighbors)
// ---------------------------------------------------------------------------

/// Estimate normals using PCA on covariance of k nearest neighbors.
///
/// Uses a brute-force kNN search (acceptable for moderate point counts).
pub fn estimate_normals(cloud: &mut PointCloud, k: usize) {
    let n = cloud.points.len();
    if n == 0 {
        cloud.normals = Some(Vec::new());
        return;
    }
    let k = k.min(n);
    let mut normals = Vec::with_capacity(n);

    for i in 0..n {
        let p = cloud.points[i];

        // Find k nearest neighbors by brute force.
        let mut dists: Vec<(usize, f64)> = cloud
            .points
            .iter()
            .enumerate()
            .map(|(j, q)| (j, p.distance_squared(*q)))
            .collect();
        dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Compute centroid of neighbors.
        let neighbors: Vec<DVec3> = dists[..k].iter().map(|&(j, _)| cloud.points[j]).collect();
        let centroid = neighbors.iter().copied().sum::<DVec3>() / k as f64;

        // Build 3x3 covariance matrix (symmetric).
        let mut cov = [[0.0f64; 3]; 3];
        for q in &neighbors {
            let d = *q - centroid;
            let da = [d.x, d.y, d.z];
            for r in 0..3 {
                for c in 0..3 {
                    cov[r][c] += da[r] * da[c];
                }
            }
        }

        // Power iteration to find the eigenvector with the *smallest* eigenvalue.
        // We do power iteration on (lambda_max * I - Cov) to get the min eigenvector.
        // First, find max eigenvalue via power iteration.
        let max_eval = power_iteration_eigenvalue(&cov, 50);

        // Shifted matrix: S = maxEval * I - Cov
        let mut shifted = [[0.0f64; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                shifted[r][c] = -cov[r][c];
            }
            shifted[r][r] += max_eval + 1e-10; // small shift for stability
        }

        let normal = power_iteration_vector(&shifted, 50);
        normals.push(normal);
    }

    cloud.normals = Some(normals);
}

fn mat_vec_mul(m: &[[f64; 3]; 3], v: DVec3) -> DVec3 {
    DVec3::new(
        m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
        m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
        m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
    )
}

fn power_iteration_eigenvalue(m: &[[f64; 3]; 3], iters: usize) -> f64 {
    let mut v = DVec3::new(1.0, 1.0, 1.0).normalize();
    for _ in 0..iters {
        let mv = mat_vec_mul(m, v);
        let len = mv.length();
        if len < 1e-15 {
            return 0.0;
        }
        v = mv / len;
    }
    let mv = mat_vec_mul(m, v);
    v.dot(mv)
}

fn power_iteration_vector(m: &[[f64; 3]; 3], iters: usize) -> DVec3 {
    let mut v = DVec3::new(1.0, 0.4, 0.7).normalize();
    for _ in 0..iters {
        let mv = mat_vec_mul(m, v);
        let len = mv.length();
        if len < 1e-15 {
            return DVec3::Y;
        }
        v = mv / len;
    }
    v
}

// ---------------------------------------------------------------------------
// Voxel-grid downsampling
// ---------------------------------------------------------------------------

/// Downsample a point cloud using a voxel grid.
///
/// Each occupied voxel contributes its centroid to the output.
pub fn downsample_voxel(cloud: &PointCloud, voxel_size: f64) -> PointCloud {
    if cloud.is_empty() || voxel_size <= 0.0 {
        return PointCloud::from_points(&[]);
    }

    let inv = 1.0 / voxel_size;
    let mut buckets: HashMap<(i64, i64, i64), (DVec3, usize)> = HashMap::new();

    for &p in &cloud.points {
        let key = (
            (p.x * inv).floor() as i64,
            (p.y * inv).floor() as i64,
            (p.z * inv).floor() as i64,
        );
        let entry = buckets.entry(key).or_insert((DVec3::ZERO, 0));
        entry.0 += p;
        entry.1 += 1;
    }

    let pts: Vec<DVec3> = buckets
        .values()
        .map(|&(sum, count)| sum / count as f64)
        .collect();

    PointCloud::from_points(&pts)
}

// ---------------------------------------------------------------------------
// Statistical outlier removal
// ---------------------------------------------------------------------------

/// Remove statistical outliers based on mean distance to k nearest neighbors.
///
/// Points whose mean neighbor distance exceeds `mean + std_ratio * std_dev`
/// are discarded.
pub fn remove_outliers(cloud: &PointCloud, k: usize, std_ratio: f64) -> PointCloud {
    let n = cloud.points.len();
    if n <= 1 {
        return cloud.clone();
    }
    let k = k.min(n - 1);

    // Compute mean distance to k-nearest neighbors for each point.
    let mut mean_dists = Vec::with_capacity(n);
    for i in 0..n {
        let p = cloud.points[i];
        let mut dists: Vec<f64> = cloud
            .points
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, q)| p.distance(*q))
            .collect();
        dists.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let md: f64 = dists[..k].iter().sum::<f64>() / k as f64;
        mean_dists.push(md);
    }

    let global_mean = mean_dists.iter().sum::<f64>() / n as f64;
    let variance = mean_dists.iter().map(|d| (d - global_mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    let threshold = global_mean + std_ratio * std_dev;

    let pts: Vec<DVec3> = cloud
        .points
        .iter()
        .zip(mean_dists.iter())
        .filter(|(_, d)| **d <= threshold)
        .map(|(p, _)| *p)
        .collect();

    PointCloud::from_points(&pts)
}

// ---------------------------------------------------------------------------
// Detected primitive
// ---------------------------------------------------------------------------

/// A geometric primitive detected from the point cloud.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Primitive {
    Plane {
        origin: DVec3,
        normal: DVec3,
        inliers: Vec<usize>,
    },
    Cylinder {
        origin: DVec3,
        axis: DVec3,
        radius: f64,
        inliers: Vec<usize>,
    },
    Sphere {
        center: DVec3,
        radius: f64,
        inliers: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// RANSAC plane detection
// ---------------------------------------------------------------------------

/// Detect a plane from the point cloud using RANSAC.
///
/// `threshold` — maximum distance from plane for a point to be an inlier.
/// `iterations` — number of RANSAC iterations.
pub fn detect_plane(cloud: &PointCloud, threshold: f64, iterations: usize) -> Option<Primitive> {
    let n = cloud.points.len();
    if n < 3 {
        return None;
    }

    let mut rng = Lcg::new(42);
    let mut best_inliers: Vec<usize> = Vec::new();
    let mut best_origin = DVec3::ZERO;
    let mut best_normal = DVec3::Y;

    for _ in 0..iterations {
        let i0 = rng.next_usize(n);
        let mut i1 = rng.next_usize(n);
        while i1 == i0 {
            i1 = rng.next_usize(n);
        }
        let mut i2 = rng.next_usize(n);
        while i2 == i0 || i2 == i1 {
            i2 = rng.next_usize(n);
        }

        let p0 = cloud.points[i0];
        let p1 = cloud.points[i1];
        let p2 = cloud.points[i2];

        let normal = (p1 - p0).cross(p2 - p0);
        let len = normal.length();
        if len < 1e-12 {
            continue;
        }
        let normal = normal / len;

        let inliers: Vec<usize> = (0..n)
            .filter(|&j| (cloud.points[j] - p0).dot(normal).abs() < threshold)
            .collect();

        if inliers.len() > best_inliers.len() {
            best_inliers = inliers;
            best_origin = p0;
            best_normal = normal;
        }
    }

    if best_inliers.len() < 3 {
        return None;
    }

    Some(Primitive::Plane {
        origin: best_origin,
        normal: best_normal,
        inliers: best_inliers,
    })
}

// ---------------------------------------------------------------------------
// RANSAC cylinder detection
// ---------------------------------------------------------------------------

/// Detect a cylinder from the point cloud using RANSAC.
///
/// Requires estimated normals on the cloud. Two points and their normals
/// define a candidate axis; the radius is the median distance to that axis.
pub fn detect_cylinder(cloud: &PointCloud, threshold: f64, iterations: usize) -> Option<Primitive> {
    let n = cloud.points.len();
    let normals = cloud.normals.as_ref()?;
    if n < 2 || normals.len() != n {
        return None;
    }

    let mut rng = Lcg::new(137);
    let mut best_inliers: Vec<usize> = Vec::new();
    let mut best_origin = DVec3::ZERO;
    let mut best_axis = DVec3::Y;
    let mut best_radius = 0.0;

    for _ in 0..iterations {
        let i0 = rng.next_usize(n);
        let mut i1 = rng.next_usize(n);
        while i1 == i0 {
            i1 = rng.next_usize(n);
        }

        // Candidate axis from cross product of two surface normals.
        let axis = normals[i0].cross(normals[i1]);
        let len = axis.length();
        if len < 1e-12 {
            continue;
        }
        let axis = axis / len;

        // Project points onto the plane perpendicular to the axis through `p0`.
        let p0 = cloud.points[i0];
        // Estimate radius as median distance of a sample to the axis line.
        let dist_to_axis = |p: DVec3| -> f64 {
            let v = p - p0;
            let proj = v.dot(axis) * axis;
            (v - proj).length()
        };

        let mut dists: Vec<f64> = cloud.points.iter().map(|&p| dist_to_axis(p)).collect();
        dists.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let radius = dists[dists.len() / 2]; // median

        if radius < 1e-12 {
            continue;
        }

        let inliers: Vec<usize> = (0..n)
            .filter(|&j| (dist_to_axis(cloud.points[j]) - radius).abs() < threshold)
            .collect();

        if inliers.len() > best_inliers.len() {
            best_inliers = inliers;
            best_origin = p0;
            best_axis = axis;
            best_radius = radius;
        }
    }

    if best_inliers.len() < 2 {
        return None;
    }

    Some(Primitive::Cylinder {
        origin: best_origin,
        axis: best_axis,
        radius: best_radius,
        inliers: best_inliers,
    })
}

// ---------------------------------------------------------------------------
// RANSAC sphere detection
// ---------------------------------------------------------------------------

/// Detect a sphere from the point cloud using RANSAC.
///
/// Four non-coplanar points define a candidate sphere.
pub fn detect_sphere(cloud: &PointCloud, threshold: f64, iterations: usize) -> Option<Primitive> {
    let n = cloud.points.len();
    if n < 4 {
        return None;
    }

    let mut rng = Lcg::new(271);
    let mut best_inliers: Vec<usize> = Vec::new();
    let mut best_center = DVec3::ZERO;
    let mut best_radius = 0.0;

    for _ in 0..iterations {
        // Pick 4 distinct random points.
        let mut idx = [0usize; 4];
        idx[0] = rng.next_usize(n);
        loop {
            idx[1] = rng.next_usize(n);
            if idx[1] != idx[0] {
                break;
            }
        }
        loop {
            idx[2] = rng.next_usize(n);
            if idx[2] != idx[0] && idx[2] != idx[1] {
                break;
            }
        }
        loop {
            idx[3] = rng.next_usize(n);
            if idx[3] != idx[0] && idx[3] != idx[1] && idx[3] != idx[2] {
                break;
            }
        }

        let pts: Vec<DVec3> = idx.iter().map(|&i| cloud.points[i]).collect();

        // Solve for sphere center from 4 points using the linear system approach.
        // (p_i - c) . (p_i - c) = r^2 for all i.
        // Subtracting pairs removes the quadratic terms.
        let (a1, b1, c1) = (
            pts[1].x - pts[0].x,
            pts[1].y - pts[0].y,
            pts[1].z - pts[0].z,
        );
        let d1 = 0.5 * (pts[1].length_squared() - pts[0].length_squared());

        let (a2, b2, c2) = (
            pts[2].x - pts[0].x,
            pts[2].y - pts[0].y,
            pts[2].z - pts[0].z,
        );
        let d2 = 0.5 * (pts[2].length_squared() - pts[0].length_squared());

        let (a3, b3, c3) = (
            pts[3].x - pts[0].x,
            pts[3].y - pts[0].y,
            pts[3].z - pts[0].z,
        );
        let d3 = 0.5 * (pts[3].length_squared() - pts[0].length_squared());

        // Cramer's rule
        let det = a1 * (b2 * c3 - b3 * c2) - b1 * (a2 * c3 - a3 * c2)
            + c1 * (a2 * b3 - a3 * b2);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv_det = 1.0 / det;

        let cx = (d1 * (b2 * c3 - b3 * c2) - b1 * (d2 * c3 - d3 * c2)
            + c1 * (d2 * b3 - d3 * b2))
            * inv_det;
        let cy = (a1 * (d2 * c3 - d3 * c2) - d1 * (a2 * c3 - a3 * c2)
            + c1 * (a2 * d3 - a3 * d2))
            * inv_det;
        let cz = (a1 * (b2 * d3 - b3 * d2) - b1 * (a2 * d3 - a3 * d2)
            + d1 * (a2 * b3 - a3 * b2))
            * inv_det;

        let center = DVec3::new(cx, cy, cz);
        let radius = center.distance(pts[0]);

        if radius < 1e-12 {
            continue;
        }

        let inliers: Vec<usize> = (0..n)
            .filter(|&j| (cloud.points[j].distance(center) - radius).abs() < threshold)
            .collect();

        if inliers.len() > best_inliers.len() {
            best_inliers = inliers;
            best_center = center;
            best_radius = radius;
        }
    }

    if best_inliers.len() < 4 {
        return None;
    }

    Some(Primitive::Sphere {
        center: best_center,
        radius: best_radius,
        inliers: best_inliers,
    })
}

// ---------------------------------------------------------------------------
// Multi-primitive detection
// ---------------------------------------------------------------------------

/// Detect multiple primitives from a point cloud by iteratively running
/// RANSAC and removing inliers.
///
/// Returns a vector of detected primitives (planes, cylinders, spheres).
/// `min_inliers` — minimum number of inliers for a valid primitive.
pub fn detect_primitives(
    cloud: &PointCloud,
    threshold: f64,
    iterations: usize,
    min_inliers: usize,
) -> Vec<Primitive> {
    let mut remaining: Vec<bool> = vec![true; cloud.points.len()];
    let mut primitives = Vec::new();

    for _ in 0..20 {
        // Build sub-cloud from remaining points.
        let idx_map: Vec<usize> = (0..cloud.points.len())
            .filter(|&i| remaining[i])
            .collect();
        if idx_map.len() < min_inliers {
            break;
        }

        let sub_pts: Vec<DVec3> = idx_map.iter().map(|&i| cloud.points[i]).collect();
        let sub_normals: Option<Vec<DVec3>> = cloud
            .normals
            .as_ref()
            .map(|ns| idx_map.iter().map(|&i| ns[i]).collect());
        let sub_cloud = PointCloud {
            points: sub_pts,
            normals: sub_normals,
            colors: None,
        };

        // Try plane first (most common), then sphere, then cylinder.
        let mut best: Option<(Primitive, Vec<usize>)> = None;

        if let Some(Primitive::Plane { origin, normal, inliers }) =
            detect_plane(&sub_cloud, threshold, iterations)
        {
            if inliers.len() >= min_inliers {
                let mapped: Vec<usize> = inliers.iter().map(|&i| idx_map[i]).collect();
                best = Some((
                    Primitive::Plane {
                        origin,
                        normal,
                        inliers: mapped.clone(),
                    },
                    mapped,
                ));
            }
        }

        if let Some(Primitive::Sphere { center, radius, inliers }) =
            detect_sphere(&sub_cloud, threshold, iterations)
        {
            let dominated = best.as_ref().map_or(false, |(_, b)| b.len() >= inliers.len());
            if inliers.len() >= min_inliers && !dominated {
                let mapped: Vec<usize> = inliers.iter().map(|&i| idx_map[i]).collect();
                best = Some((
                    Primitive::Sphere {
                        center,
                        radius,
                        inliers: mapped.clone(),
                    },
                    mapped,
                ));
            }
        }

        if let Some(Primitive::Cylinder { origin, axis, radius, inliers }) =
            detect_cylinder(&sub_cloud, threshold, iterations)
        {
            let dominated = best.as_ref().map_or(false, |(_, b)| b.len() >= inliers.len());
            if inliers.len() >= min_inliers && !dominated {
                let mapped: Vec<usize> = inliers.iter().map(|&i| idx_map[i]).collect();
                best = Some((
                    Primitive::Cylinder {
                        origin,
                        axis,
                        radius,
                        inliers: mapped.clone(),
                    },
                    mapped,
                ));
            }
        }

        match best {
            Some((prim, global_inliers)) => {
                for &idx in &global_inliers {
                    remaining[idx] = false;
                }
                primitives.push(prim);
            }
            None => break,
        }
    }

    primitives
}

// ---------------------------------------------------------------------------
// B-Rep reconstruction
// ---------------------------------------------------------------------------

/// Assemble detected primitives into a B-Rep [`Solid`].
///
/// Each primitive becomes a face on the solid. Planes become planar faces,
/// cylinders and spheres become their respective analytic surfaces. The
/// resulting solid is an approximation — further healing is typically needed.
pub fn reconstruct_solid(primitives: &[Primitive], cloud: &PointCloud) -> Solid {
    let mut solid = Solid::new();

    for prim in primitives {
        match prim {
            Primitive::Plane { origin, normal, inliers } => {
                // Compute the bounding quad of the inlier points projected onto the plane.
                if inliers.len() < 3 {
                    continue;
                }
                let n = *normal;
                let o = *origin;

                // Build a local 2D frame on the plane.
                let arbitrary = if n.x.abs() < 0.9 {
                    DVec3::X
                } else {
                    DVec3::Y
                };
                let u_axis = n.cross(arbitrary).normalize();
                let v_axis = n.cross(u_axis).normalize();

                let mut u_min = f64::MAX;
                let mut u_max = f64::MIN;
                let mut v_min = f64::MAX;
                let mut v_max = f64::MIN;
                for &idx in inliers {
                    let d = cloud.points[idx] - o;
                    let u = d.dot(u_axis);
                    let v = d.dot(v_axis);
                    u_min = u_min.min(u);
                    u_max = u_max.max(u);
                    v_min = v_min.min(v);
                    v_max = v_max.max(v);
                }

                let corners = [
                    o + u_axis * u_min + v_axis * v_min,
                    o + u_axis * u_max + v_axis * v_min,
                    o + u_axis * u_max + v_axis * v_max,
                    o + u_axis * u_min + v_axis * v_max,
                ];

                let vids: Vec<_> = corners.iter().map(|&c| solid.add_vertex(c)).collect();
                solid.add_face_from_vertices(Surface::plane(o, n), &vids, true);
            }
            Primitive::Cylinder {
                origin,
                axis,
                radius,
                inliers,
            } => {
                if inliers.len() < 2 {
                    continue;
                }
                // Approximate with a quad strip face.
                let a = *axis;
                let o = *origin;
                let r = *radius;

                let arbitrary = if a.x.abs() < 0.9 {
                    DVec3::X
                } else {
                    DVec3::Y
                };
                let u_axis = a.cross(arbitrary).normalize();
                let v_axis = a.cross(u_axis).normalize();

                // Find extent along axis.
                let mut t_min = f64::MAX;
                let mut t_max = f64::MIN;
                for &idx in inliers {
                    let t = (cloud.points[idx] - o).dot(a);
                    t_min = t_min.min(t);
                    t_max = t_max.max(t);
                }

                // Build 8-sided polygon approximation at each end.
                let segments = 8;
                let mut bottom_vids = Vec::new();
                let mut top_vids = Vec::new();
                for i in 0..segments {
                    let angle =
                        2.0 * std::f64::consts::PI * (i as f64) / (segments as f64);
                    let dir = u_axis * angle.cos() + v_axis * angle.sin();
                    let bp = o + a * t_min + dir * r;
                    let tp = o + a * t_max + dir * r;
                    bottom_vids.push(solid.add_vertex(bp));
                    top_vids.push(solid.add_vertex(tp));
                }

                // Side faces (quads).
                for i in 0..segments {
                    let j = (i + 1) % segments;
                    let vids = [bottom_vids[i], bottom_vids[j], top_vids[j], top_vids[i]];
                    solid.add_face_from_vertices(
                        Surface::cylinder(o, a, r),
                        &vids,
                        true,
                    );
                }
            }
            Primitive::Sphere {
                center,
                radius,
                inliers,
            } => {
                if inliers.len() < 4 {
                    continue;
                }
                // Approximate sphere with an octahedron.
                let c = *center;
                let r = *radius;

                let v_top = solid.add_vertex(c + DVec3::Y * r);
                let v_bot = solid.add_vertex(c - DVec3::Y * r);
                let v_right = solid.add_vertex(c + DVec3::X * r);
                let v_left = solid.add_vertex(c - DVec3::X * r);
                let v_front = solid.add_vertex(c + DVec3::Z * r);
                let v_back = solid.add_vertex(c - DVec3::Z * r);

                let surf = Surface::sphere(c, r);
                let faces = [
                    [v_top, v_front, v_right],
                    [v_top, v_right, v_back],
                    [v_top, v_back, v_left],
                    [v_top, v_left, v_front],
                    [v_bot, v_right, v_front],
                    [v_bot, v_back, v_right],
                    [v_bot, v_left, v_back],
                    [v_bot, v_front, v_left],
                ];
                for vids in &faces {
                    solid.add_face_from_vertices(surf.clone(), vids, true);
                }
            }
        }
    }

    solid.link_twins();
    solid
}

// ---------------------------------------------------------------------------
// Bounding box fitting
// ---------------------------------------------------------------------------

/// Compute an axis-aligned bounding box and return it as a B-Rep box solid.
pub fn fit_bounding_box(cloud: &PointCloud) -> Option<Solid> {
    if cloud.is_empty() {
        return None;
    }

    let mut min = cloud.points[0];
    let mut max = cloud.points[0];
    for &p in &cloud.points[1..] {
        min = DVec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = DVec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }

    let size = max - min;
    let center = (min + max) * 0.5;

    // Use make_box (centered at origin) then translate.
    let mut solid = make_box(size.x, size.y, size.z);

    // Translate every vertex by center offset.
    let vert_ids: Vec<_> = solid.vertices.keys().collect();
    for vid in vert_ids {
        solid.vertices[vid].point += center;
    }

    Some(solid)
}

// ---------------------------------------------------------------------------
// PLY I/O (ASCII)
// ---------------------------------------------------------------------------

/// Read a PLY file (ASCII format) from a reader.
pub fn read_ply<R: std::io::Read>(reader: R) -> Result<PointCloud, String> {
    let buf = BufReader::new(reader);
    let mut lines = buf.lines();

    // Parse header.
    let mut vertex_count = 0usize;
    let mut has_nx = false;
    let mut has_red = false;
    let mut prop_order: Vec<String> = Vec::new();

    let first_line = lines
        .next()
        .ok_or("empty file")?
        .map_err(|e| e.to_string())?;
    if first_line.trim() != "ply" {
        return Err("not a PLY file".into());
    }

    loop {
        let line = lines
            .next()
            .ok_or("unexpected end of header")?
            .map_err(|e| e.to_string())?;
        let line = line.trim();
        if line == "end_header" {
            break;
        }
        if line.starts_with("element vertex") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            vertex_count = parts
                .get(2)
                .ok_or("missing vertex count")?
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
        }
        if line.starts_with("property") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(&name) = parts.get(2) {
                prop_order.push(name.to_string());
                if name == "nx" {
                    has_nx = true;
                }
                if name == "red" {
                    has_red = true;
                }
            }
        }
    }

    // Build index map for properties.
    let idx_of = |name: &str| -> Option<usize> {
        prop_order.iter().position(|s| s == name)
    };
    let ix = idx_of("x").ok_or("missing x property")?;
    let iy = idx_of("y").ok_or("missing y property")?;
    let iz = idx_of("z").ok_or("missing z property")?;
    let inx = idx_of("nx");
    let iny = idx_of("ny");
    let inz = idx_of("nz");
    let ir = idx_of("red");
    let ig = idx_of("green");
    let ib = idx_of("blue");

    let mut points = Vec::with_capacity(vertex_count);
    let mut normals = if has_nx {
        Some(Vec::with_capacity(vertex_count))
    } else {
        None
    };
    let mut colors = if has_red {
        Some(Vec::with_capacity(vertex_count))
    } else {
        None
    };

    for _ in 0..vertex_count {
        let line = lines
            .next()
            .ok_or("unexpected end of data")?
            .map_err(|e| e.to_string())?;
        let vals: Vec<&str> = line.split_whitespace().collect();

        let parse_f = |i: usize| -> Result<f64, String> {
            vals.get(i)
                .ok_or(format!("missing field {i}"))?
                .parse()
                .map_err(|e: std::num::ParseFloatError| e.to_string())
        };

        points.push(DVec3::new(parse_f(ix)?, parse_f(iy)?, parse_f(iz)?));

        if let (Some(nx), Some(ny), Some(nz)) = (inx, iny, inz) {
            if let Some(ref mut ns) = normals {
                ns.push(DVec3::new(parse_f(nx)?, parse_f(ny)?, parse_f(nz)?));
            }
        }

        if let (Some(r), Some(g), Some(b)) = (ir, ig, ib) {
            if let Some(ref mut cs) = colors {
                let rv: u8 = vals
                    .get(r)
                    .ok_or("missing red")?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                let gv: u8 = vals
                    .get(g)
                    .ok_or("missing green")?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                let bv: u8 = vals
                    .get(b)
                    .ok_or("missing blue")?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                cs.push([rv, gv, bv]);
            }
        }
    }

    Ok(PointCloud {
        points,
        normals,
        colors,
    })
}

/// Write a PLY file (ASCII format) to a writer.
pub fn write_ply<W: std::io::Write>(writer: &mut W, cloud: &PointCloud) -> Result<(), String> {
    let has_normals = cloud.normals.is_some();
    let has_colors = cloud.colors.is_some();

    writeln!(writer, "ply").map_err(|e| e.to_string())?;
    writeln!(writer, "format ascii 1.0").map_err(|e| e.to_string())?;
    writeln!(writer, "element vertex {}", cloud.points.len()).map_err(|e| e.to_string())?;
    writeln!(writer, "property float x").map_err(|e| e.to_string())?;
    writeln!(writer, "property float y").map_err(|e| e.to_string())?;
    writeln!(writer, "property float z").map_err(|e| e.to_string())?;
    if has_normals {
        writeln!(writer, "property float nx").map_err(|e| e.to_string())?;
        writeln!(writer, "property float ny").map_err(|e| e.to_string())?;
        writeln!(writer, "property float nz").map_err(|e| e.to_string())?;
    }
    if has_colors {
        writeln!(writer, "property uchar red").map_err(|e| e.to_string())?;
        writeln!(writer, "property uchar green").map_err(|e| e.to_string())?;
        writeln!(writer, "property uchar blue").map_err(|e| e.to_string())?;
    }
    writeln!(writer, "end_header").map_err(|e| e.to_string())?;

    for (i, p) in cloud.points.iter().enumerate() {
        write!(writer, "{} {} {}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        if let Some(ref ns) = cloud.normals {
            let n = ns[i];
            write!(writer, " {} {} {}", n.x, n.y, n.z).map_err(|e| e.to_string())?;
        }
        if let Some(ref cs) = cloud.colors {
            let c = cs[i];
            write!(writer, " {} {} {}", c[0], c[1], c[2]).map_err(|e| e.to_string())?;
        }
        writeln!(writer).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Helper: generate points on a plane z = h with some spread in x, y.
    fn plane_points(h: f64, n: usize) -> Vec<DVec3> {
        let mut pts = Vec::new();
        let side = (n as f64).sqrt().ceil() as usize;
        for i in 0..side {
            for j in 0..side {
                if pts.len() >= n {
                    break;
                }
                pts.push(DVec3::new(i as f64 * 0.1, j as f64 * 0.1, h));
            }
        }
        pts
    }

    /// Helper: generate points on a cylinder surface (axis = Y, radius = r).
    fn cylinder_points(origin: DVec3, r: f64, height: f64, n: usize) -> Vec<DVec3> {
        let mut pts = Vec::new();
        let rings = (n as f64).sqrt().ceil() as usize;
        let per_ring = (n + rings - 1) / rings;
        for i in 0..rings {
            let t = (i as f64) / (rings as f64 - 1.0).max(1.0) * height;
            for j in 0..per_ring {
                if pts.len() >= n {
                    break;
                }
                let angle = 2.0 * PI * (j as f64) / per_ring as f64;
                pts.push(origin + DVec3::new(r * angle.cos(), t, r * angle.sin()));
            }
        }
        pts
    }

    /// Helper: generate points on a sphere surface.
    fn sphere_points(center: DVec3, r: f64, n: usize) -> Vec<DVec3> {
        let mut pts = Vec::new();
        // Fibonacci sphere.
        let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;
        for i in 0..n {
            let theta = 2.0 * PI * (i as f64) / golden_ratio;
            let phi = ((1.0 - 2.0 * (i as f64 + 0.5) / n as f64)).acos();
            pts.push(
                center
                    + DVec3::new(
                        r * phi.sin() * theta.cos(),
                        r * phi.sin() * theta.sin(),
                        r * phi.cos(),
                    ),
            );
        }
        pts
    }

    #[test]
    fn voxel_downsample_reduces_count() {
        let pts: Vec<DVec3> = (0..1000)
            .map(|i| {
                let f = i as f64 * 0.001;
                DVec3::new(f, f * 2.0, f * 0.5)
            })
            .collect();
        let cloud = PointCloud::from_points(&pts);
        let down = downsample_voxel(&cloud, 0.1);
        assert!(down.len() < cloud.len());
        assert!(down.len() > 0);
    }

    #[test]
    fn outlier_removal_basic() {
        let mut pts: Vec<DVec3> = (0..100)
            .map(|i| {
                let f = i as f64 * 0.01;
                DVec3::new(f, 0.0, 0.0)
            })
            .collect();
        // Add an outlier far away.
        pts.push(DVec3::new(100.0, 100.0, 100.0));
        let cloud = PointCloud::from_points(&pts);
        let clean = remove_outliers(&cloud, 5, 1.0);
        assert!(clean.len() <= 100);
        // The outlier should have been removed.
        assert!(clean.len() < cloud.len());
    }

    #[test]
    fn detect_plane_from_points() {
        let pts = plane_points(5.0, 200);
        let cloud = PointCloud::from_points(&pts);
        let prim = detect_plane(&cloud, 0.01, 500).unwrap();
        match prim {
            Primitive::Plane { normal, .. } => {
                // Normal should be approximately +/-Z.
                assert!(normal.z.abs() > 0.99, "normal.z = {}", normal.z);
            }
            _ => panic!("expected plane"),
        }
    }

    #[test]
    fn detect_cylinder_from_points() {
        let pts = cylinder_points(DVec3::ZERO, 2.0, 5.0, 500);
        let mut cloud = PointCloud::from_points(&pts);
        estimate_normals(&mut cloud, 15);
        let prim = detect_cylinder(&cloud, 0.8, 2000).unwrap();
        match prim {
            Primitive::Cylinder { radius, .. } => {
                assert!(
                    (radius - 2.0).abs() < 2.0,
                    "radius = {radius}, expected ~2.0"
                );
            }
            _ => panic!("expected cylinder"),
        }
    }

    #[test]
    fn detect_sphere_from_points() {
        let center = DVec3::new(1.0, 2.0, 3.0);
        let pts = sphere_points(center, 5.0, 200);
        let cloud = PointCloud::from_points(&pts);
        let prim = detect_sphere(&cloud, 0.5, 1000).unwrap();
        match prim {
            Primitive::Sphere {
                center: c,
                radius: r,
                ..
            } => {
                assert!(
                    c.distance(center) < 1.0,
                    "center distance = {}",
                    c.distance(center)
                );
                assert!((r - 5.0).abs() < 1.0, "radius = {r}, expected ~5.0");
            }
            _ => panic!("expected sphere"),
        }
    }

    #[test]
    fn fit_bounding_box_dimensions() {
        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(0.0, 5.0, 0.0),
            DVec3::new(0.0, 0.0, 3.0),
        ];
        let cloud = PointCloud::from_points(&pts);
        let solid = fit_bounding_box(&cloud).unwrap();
        // Should have 8 vertices (box).
        assert_eq!(solid.vertices.len(), 8);
        // Check extents.
        let xs: Vec<f64> = solid.vertices.values().map(|v| v.point.x).collect();
        let width = xs.iter().cloned().fold(f64::MIN, f64::max)
            - xs.iter().cloned().fold(f64::MAX, f64::min);
        assert!((width - 10.0).abs() < 1e-9, "width = {width}");
    }

    #[test]
    fn ply_roundtrip() {
        let pts = vec![
            DVec3::new(1.0, 2.0, 3.0),
            DVec3::new(4.0, 5.0, 6.0),
        ];
        let cloud = PointCloud::from_points(&pts);
        let mut buf = Vec::new();
        write_ply(&mut buf, &cloud).unwrap();
        let read_back = read_ply(buf.as_slice()).unwrap();
        assert_eq!(read_back.points.len(), 2);
        assert!((read_back.points[0] - pts[0]).length() < 1e-6);
        assert!((read_back.points[1] - pts[1]).length() < 1e-6);
    }

    #[test]
    fn ply_with_normals() {
        let cloud = PointCloud {
            points: vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0)],
            normals: Some(vec![DVec3::Y, DVec3::Z]),
            colors: None,
        };
        let mut buf = Vec::new();
        write_ply(&mut buf, &cloud).unwrap();
        let read_back = read_ply(buf.as_slice()).unwrap();
        assert!(read_back.normals.is_some());
        let ns = read_back.normals.unwrap();
        assert!((ns[0] - DVec3::Y).length() < 1e-6);
        assert!((ns[1] - DVec3::Z).length() < 1e-6);
    }

    #[test]
    fn normal_estimation_unit_vectors() {
        let pts = plane_points(0.0, 50);
        let mut cloud = PointCloud::from_points(&pts);
        estimate_normals(&mut cloud, 6);
        let normals = cloud.normals.unwrap();
        for n in &normals {
            let len = n.length();
            assert!(
                (len - 1.0).abs() < 0.1,
                "normal length = {len}, expected ~1.0"
            );
        }
    }

    #[test]
    fn empty_cloud_handling() {
        let cloud = PointCloud::from_points(&[]);
        assert!(fit_bounding_box(&cloud).is_none());
        assert!(detect_plane(&cloud, 0.1, 100).is_none());
        assert!(detect_sphere(&cloud, 0.1, 100).is_none());
        let down = downsample_voxel(&cloud, 1.0);
        assert!(down.is_empty());
        let cleaned = remove_outliers(&cloud, 5, 1.0);
        assert!(cleaned.is_empty());
    }

    #[test]
    fn reconstruct_box_from_6_planes() {
        // Generate 6 planes of a unit box.
        let mut all_pts = Vec::new();
        let mut prims = Vec::new();

        let faces: [(DVec3, DVec3); 6] = [
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, -1.0)),
            (DVec3::new(0.0, 0.0, 1.0), DVec3::Z),
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(-1.0, 0.0, 0.0)),
            (DVec3::new(1.0, 0.0, 0.0), DVec3::X),
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, -1.0, 0.0)),
            (DVec3::new(0.0, 1.0, 0.0), DVec3::Y),
        ];

        for (origin, normal) in &faces {
            let base = all_pts.len();
            // Lay out a grid of points on this plane face.
            let arbitrary = if normal.x.abs() < 0.9 {
                DVec3::X
            } else {
                DVec3::Y
            };
            let u = normal.cross(arbitrary).normalize();
            let v = normal.cross(u).normalize();
            let inliers: Vec<usize> = (0..25)
                .map(|i| {
                    let row = i / 5;
                    let col = i % 5;
                    let pt = *origin + u * (col as f64 * 0.2) + v * (row as f64 * 0.2);
                    all_pts.push(pt);
                    base + i
                })
                .collect();
            prims.push(Primitive::Plane {
                origin: *origin,
                normal: *normal,
                inliers,
            });
        }

        let cloud = PointCloud::from_points(&all_pts);
        let solid = reconstruct_solid(&prims, &cloud);
        // 6 planes => 6 faces.
        assert_eq!(solid.faces.len(), 6);
    }

    #[test]
    fn detect_primitives_mixed() {
        // Create a cloud with a clear plane and a clear sphere.
        let mut pts = plane_points(0.0, 200);
        let sphere_pts = sphere_points(DVec3::new(10.0, 10.0, 10.0), 2.0, 150);
        pts.extend(sphere_pts);
        let cloud = PointCloud::from_points(&pts);
        let prims = detect_primitives(&cloud, 0.1, 500, 20);
        // Should detect at least 2 primitives.
        assert!(
            prims.len() >= 2,
            "expected >=2 primitives, got {}",
            prims.len()
        );
    }
}
