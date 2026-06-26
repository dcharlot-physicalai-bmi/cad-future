//! `physical-gen-bridge` — CAD to visual conditioning bridge for generative AI.
//!
//! Forward direction: takes CAD geometry and produces camera orbits, depth/normal/edge
//! conditioning maps, and natural-language descriptions suitable for driving
//! Flux, Trellis, Wan 2.1, and similar generative models.

use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Camera orbit generation
// ---------------------------------------------------------------------------

/// Configuration for a camera orbit around a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraOrbit {
    /// Center point of the orbit.
    pub center: [f32; 3],
    /// Distance from center to camera.
    pub radius: f32,
    /// Output image resolution (width, height).
    pub resolution: [u32; 2],
    /// Vertical field-of-view in degrees.
    pub fov_deg: f32,
}

/// A single view configuration (camera position + orientation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    /// Camera position in world space.
    pub eye: [f32; 3],
    /// Look-at target.
    pub target: [f32; 3],
    /// Up vector.
    pub up: [f32; 3],
    /// Field-of-view in degrees (0 means orthographic).
    pub fov_deg: f32,
    /// Resolution (width, height).
    pub resolution: [u32; 2],
    /// Human-readable label for the view.
    pub label: String,
}

impl CameraOrbit {
    /// Auto-frame a camera orbit from an axis-aligned bounding box.
    pub fn from_bounds(min: [f32; 3], max: [f32; 3]) -> Self {
        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let extent = Vec3::new(
            max[0] - min[0],
            max[1] - min[1],
            max[2] - min[2],
        );
        // Place camera far enough to see the full diagonal.
        let radius = extent.length() * 1.2;
        Self {
            center,
            radius,
            resolution: [512, 512],
            fov_deg: 50.0,
        }
    }

    /// Generate N views rotating around the model at a fixed elevation (turntable).
    pub fn turntable(&self, n_views: u32, elevation_deg: f32) -> Vec<ViewConfig> {
        let elev = elevation_deg.to_radians();
        let mut views = Vec::with_capacity(n_views as usize);
        for i in 0..n_views {
            let azimuth = 2.0 * std::f32::consts::PI * (i as f32) / (n_views as f32);
            let x = self.center[0] + self.radius * elev.cos() * azimuth.cos();
            let y = self.center[1] + self.radius * elev.sin();
            let z = self.center[2] + self.radius * elev.cos() * azimuth.sin();
            views.push(ViewConfig {
                eye: [x, y, z],
                target: self.center,
                up: [0.0, 1.0, 0.0],
                fov_deg: self.fov_deg,
                resolution: self.resolution,
                label: format!("turntable_{i}"),
            });
        }
        views
    }

    /// Fibonacci hemisphere sampling for uniform view coverage.
    pub fn hemisphere(&self, n_views: u32) -> Vec<ViewConfig> {
        let golden_ratio = (1.0 + 5.0_f32.sqrt()) / 2.0;
        let mut views = Vec::with_capacity(n_views as usize);
        for i in 0..n_views {
            // Fibonacci sphere — upper hemisphere only (theta in 0..PI/2).
            let theta = (1.0 - (i as f32) / (n_views as f32)).acos() * 0.5;
            let phi = 2.0 * std::f32::consts::PI * (i as f32) / golden_ratio;
            let x = self.center[0] + self.radius * theta.sin() * phi.cos();
            let y = self.center[1] + self.radius * theta.cos();
            let z = self.center[2] + self.radius * theta.sin() * phi.sin();
            views.push(ViewConfig {
                eye: [x, y, z],
                target: self.center,
                up: [0.0, 1.0, 0.0],
                fov_deg: self.fov_deg,
                resolution: self.resolution,
                label: format!("hemisphere_{i}"),
            });
        }
        views
    }

    /// Six standard orthographic views: front, back, left, right, top, bottom.
    pub fn ortho6(&self) -> Vec<ViewConfig> {
        let c = self.center;
        let r = self.radius;
        let dirs: &[([f32; 3], [f32; 3], &str)] = &[
            ([c[0], c[1], c[2] + r], [0.0, 1.0, 0.0], "front"),
            ([c[0], c[1], c[2] - r], [0.0, 1.0, 0.0], "back"),
            ([c[0] - r, c[1], c[2]], [0.0, 1.0, 0.0], "left"),
            ([c[0] + r, c[1], c[2]], [0.0, 1.0, 0.0], "right"),
            ([c[0], c[1] + r, c[2]], [0.0, 0.0, -1.0], "top"),
            ([c[0], c[1] - r, c[2]], [0.0, 0.0, 1.0], "bottom"),
        ];
        dirs.iter()
            .map(|(eye, up, label)| ViewConfig {
                eye: *eye,
                target: c,
                up: *up,
                fov_deg: 0.0, // orthographic
                resolution: self.resolution,
                label: label.to_string(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Conditioning map generation
// ---------------------------------------------------------------------------

/// Depth buffer produced by software rasterisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthBuffer {
    pub width: u32,
    pub height: u32,
    /// Row-major depth values; `f32::INFINITY` means no hit.
    pub data: Vec<f32>,
}

/// Normal buffer (3-channel, row-major).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalBuffer {
    pub width: u32,
    pub height: u32,
    /// Packed [nx, ny, nz] per pixel.
    pub data: Vec<f32>,
}

/// Binary edge buffer derived from depth discontinuities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeBuffer {
    pub width: u32,
    pub height: u32,
    /// 1.0 = edge, 0.0 = no edge.
    pub data: Vec<f32>,
}

impl DepthBuffer {
    /// Normalise raw depth values to 0..1 range (near=0, far=1).
    /// Infinite (no-hit) pixels map to 1.0.
    pub fn normalized(&self) -> Vec<f32> {
        let finite_vals: Vec<f32> = self.data.iter().copied().filter(|d| d.is_finite()).collect();
        if finite_vals.is_empty() {
            return vec![1.0; self.data.len()];
        }
        let min = finite_vals.iter().copied().fold(f32::INFINITY, f32::min);
        let max = finite_vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(1e-8);
        self.data
            .iter()
            .map(|&d| if d.is_finite() { (d - min) / range } else { 1.0 })
            .collect()
    }
}

impl EdgeBuffer {
    /// Sobel-like edge detection from depth discontinuities.
    pub fn from_depth(depth: &DepthBuffer, threshold: f32) -> Self {
        let w = depth.width as usize;
        let h = depth.height as usize;
        let mut data = vec![0.0_f32; w * h];
        let norm = depth.normalized();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                // Sobel gradient magnitude.
                let idx = |dx: usize, dy: usize| norm[dy * w + dx];
                let gx = -idx(x - 1, y - 1) - 2.0 * idx(x - 1, y) - idx(x - 1, y + 1)
                    + idx(x + 1, y - 1)
                    + 2.0 * idx(x + 1, y)
                    + idx(x + 1, y + 1);
                let gy = -idx(x - 1, y - 1) - 2.0 * idx(x, y - 1) - idx(x + 1, y - 1)
                    + idx(x - 1, y + 1)
                    + 2.0 * idx(x, y + 1)
                    + idx(x + 1, y + 1);
                let mag = (gx * gx + gy * gy).sqrt();
                data[y * w + x] = if mag > threshold { 1.0 } else { 0.0 };
            }
        }
        Self {
            width: depth.width,
            height: depth.height,
            data,
        }
    }
}

/// Simple software Z-buffer rasteriser (triangle soup), backed by a
/// content-addressed LUT cache.
///
/// The global `DepthCache` keyed on (mesh hash, view hash) stores previously
/// rasterised depth buffers. Repeat renders of the same (mesh, view) pair —
/// common when driving generative conditioning across many diffusion steps —
/// hit the cache and skip rasterisation entirely.
pub fn render_depth(
    vertices: &[[f32; 3]],
    indices: &[[u32; 3]],
    view: &ViewConfig,
) -> DepthBuffer {
    let arc = global_depth_cache()
        .lock()
        .unwrap()
        .get_or_render(vertices, indices, view);
    (*arc).clone()
}

/// Uncached software Z-buffer rasteriser. Used as the ground truth and by
/// callers that want to avoid the global depth cache.
pub fn render_depth_uncached(
    vertices: &[[f32; 3]],
    indices: &[[u32; 3]],
    view: &ViewConfig,
) -> DepthBuffer {
    let w = view.resolution[0] as usize;
    let h = view.resolution[1] as usize;
    let mut zbuf = vec![f32::INFINITY; w * h];

    let eye = Vec3::from(view.eye);
    let target = Vec3::from(view.target);
    let up = Vec3::from(view.up);

    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let cam_up = right.cross(forward).normalize();

    let ortho = view.fov_deg == 0.0;
    let half_fov_tan = if ortho {
        1.0
    } else {
        (view.fov_deg.to_radians() * 0.5).tan()
    };

    // Project a world-space point to screen [col, row, depth].
    let project = |p: Vec3| -> Option<(f32, f32, f32)> {
        let v = p - eye;
        let z = v.dot(forward);
        if !ortho && z <= 0.0 {
            return None;
        }
        let (sx, sy) = if ortho {
            (v.dot(right), v.dot(cam_up))
        } else {
            (v.dot(right) / (z * half_fov_tan), v.dot(cam_up) / (z * half_fov_tan))
        };
        // Map from [-1,1] to [0, w/h).
        let col = (sx * 0.5 + 0.5) * w as f32;
        let row = (0.5 - sy * 0.5) * h as f32;
        Some((col, row, z))
    };

    for tri in indices {
        let Some(a) = project(Vec3::from(vertices[tri[0] as usize])) else {
            continue;
        };
        let Some(b) = project(Vec3::from(vertices[tri[1] as usize])) else {
            continue;
        };
        let Some(c) = project(Vec3::from(vertices[tri[2] as usize])) else {
            continue;
        };
        // Bounding box.
        let min_x = a.0.min(b.0).min(c.0).max(0.0) as usize;
        let max_x = (a.0.max(b.0).max(c.0) as usize).min(w.saturating_sub(1));
        let min_y = a.1.min(b.1).min(c.1).max(0.0) as usize;
        let max_y = (a.1.max(b.1).max(c.1) as usize).min(h.saturating_sub(1));

        let denom = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
        if denom.abs() < 1e-10 {
            continue;
        }
        let inv = 1.0 / denom;
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let w0 = ((b.1 - c.1) * (fx - c.0) + (c.0 - b.0) * (fy - c.1)) * inv;
                let w1 = ((c.1 - a.1) * (fx - c.0) + (a.0 - c.0) * (fy - c.1)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                    let idx = py * w + px;
                    if z < zbuf[idx] {
                        zbuf[idx] = z;
                    }
                }
            }
        }
    }

    DepthBuffer {
        width: view.resolution[0],
        height: view.resolution[1],
        data: zbuf,
    }
}

// ---------------------------------------------------------------------------
// LUT: content-addressed depth buffer cache
// ---------------------------------------------------------------------------

/// Snapshot of a depth cache's hit/miss counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct DepthCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Content-addressed cache from (mesh hash, view hash) to a precomputed
/// depth buffer. Turns repeat `render_depth` calls on identical inputs into
/// O(1) clones rather than full rasterisation passes.
#[derive(Debug, Default)]
pub struct DepthCache {
    map: HashMap<(u64, u64), Arc<DepthBuffer>>,
    hits: u64,
    misses: u64,
}

impl DepthCache {
    pub fn new() -> Self { Self::default() }

    pub fn get(&mut self, key: &(u64, u64)) -> Option<Arc<DepthBuffer>> {
        if let Some(buf) = self.map.get(key) {
            self.hits += 1;
            Some(Arc::clone(buf))
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, key: (u64, u64), buf: Arc<DepthBuffer>) {
        self.map.insert(key, buf);
    }

    /// Render-or-fetch helper: looks up the (mesh, view) pair, running
    /// `render_depth_uncached` on a cache miss and storing the result.
    pub fn get_or_render(
        &mut self,
        vertices: &[[f32; 3]],
        indices: &[[u32; 3]],
        view: &ViewConfig,
    ) -> Arc<DepthBuffer> {
        let key = (hash_mesh(vertices, indices), hash_view(view));
        if let Some(buf) = self.map.get(&key) {
            self.hits += 1;
            return Arc::clone(buf);
        }
        self.misses += 1;
        let buf = Arc::new(render_depth_uncached(vertices, indices, view));
        self.map.insert(key, Arc::clone(&buf));
        buf
    }

    pub fn stats(&self) -> DepthCacheStats {
        DepthCacheStats {
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

fn global_depth_cache() -> &'static Mutex<DepthCache> {
    static CACHE: OnceLock<Mutex<DepthCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DepthCache::new()))
}

/// Snapshot the global depth cache state.
pub fn depth_cache_stats() -> DepthCacheStats {
    global_depth_cache().lock().unwrap().stats()
}

/// Clear every entry from the global depth cache.
pub fn depth_cache_clear() {
    global_depth_cache().lock().unwrap().clear();
}

/// Content-addressed hash of a triangle mesh: quantized vertex coords + indices.
fn hash_mesh(vertices: &[[f32; 3]], indices: &[[u32; 3]]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    for v in vertices {
        ((v[0] * 1000.0).round() as i64).hash(&mut hasher);
        ((v[1] * 1000.0).round() as i64).hash(&mut hasher);
        ((v[2] * 1000.0).round() as i64).hash(&mut hasher);
    }
    for tri in indices {
        tri[0].hash(&mut hasher);
        tri[1].hash(&mut hasher);
        tri[2].hash(&mut hasher);
    }
    hasher.finish()
}

/// Content-addressed hash of a view configuration.
fn hash_view(view: &ViewConfig) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    let quantize = |f: f32| (f * 1_000_000.0).round() as i64;
    for f in &view.eye { quantize(*f).hash(&mut hasher); }
    for f in &view.target { quantize(*f).hash(&mut hasher); }
    for f in &view.up { quantize(*f).hash(&mut hasher); }
    quantize(view.fov_deg).hash(&mut hasher);
    view.resolution[0].hash(&mut hasher);
    view.resolution[1].hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Feature tree -> natural language
// ---------------------------------------------------------------------------

/// Convert a list of feature descriptions into a single natural-language summary.
pub fn describe_features(features: &[String]) -> String {
    if features.is_empty() {
        return "An empty CAD model with no features.".to_string();
    }
    let body = features.join(", ");
    format!("A 3D mechanical part with {body}.")
}

/// Convert feature descriptions into a set of tags suitable for prompt conditioning.
pub fn describe_tags(features: &[String]) -> Vec<String> {
    let mut tags = vec!["3D CAD".to_string(), "mechanical part".to_string()];
    for f in features {
        let lower = f.to_lowercase();
        if lower.contains("hole") {
            tags.push("drilled holes".to_string());
        }
        if lower.contains("fillet") || lower.contains("round") {
            tags.push("filleted edges".to_string());
        }
        if lower.contains("chamfer") {
            tags.push("chamfered edges".to_string());
        }
        if lower.contains("thread") {
            tags.push("threaded features".to_string());
        }
        if lower.contains("slot") {
            tags.push("slotted features".to_string());
        }
        if lower.contains("pocket") {
            tags.push("pocketed features".to_string());
        }
        if lower.contains("pattern") || lower.contains("array") {
            tags.push("patterned features".to_string());
        }
        if lower.contains("extrude") || lower.contains("boss") {
            tags.push("extruded geometry".to_string());
        }
        if lower.contains("revolve") {
            tags.push("revolved geometry".to_string());
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

// ---------------------------------------------------------------------------
// API client types
// ---------------------------------------------------------------------------

/// Conditioning map attached to a generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditioningMap {
    Depth(Vec<f32>),
    Edge(Vec<f32>),
    Normal(Vec<f32>),
}

/// Image generation request (e.g. Flux, Stable Diffusion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenRequest {
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub conditioning: Option<ConditioningMap>,
}

/// 3D mesh generation request (e.g. Trellis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh3DRequest {
    pub image_url: String,
    pub output_format: String,
}

/// Configuration for a generative-AI API endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenApiConfig {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bounds_centers_correctly() {
        let orbit = CameraOrbit::from_bounds([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        assert_eq!(orbit.center, [5.0, 5.0, 5.0]);
        assert!(orbit.radius > 0.0);
    }

    #[test]
    fn turntable_view_count() {
        let orbit = CameraOrbit::from_bounds([0.0; 3], [1.0; 3]);
        let views = orbit.turntable(8, 30.0);
        assert_eq!(views.len(), 8);
        assert!(views.iter().all(|v| v.label.starts_with("turntable_")));
    }

    #[test]
    fn hemisphere_view_count() {
        let orbit = CameraOrbit::from_bounds([0.0; 3], [1.0; 3]);
        let views = orbit.hemisphere(12);
        assert_eq!(views.len(), 12);
    }

    #[test]
    fn ortho6_produces_six_views() {
        let orbit = CameraOrbit::from_bounds([-1.0; 3], [1.0; 3]);
        let views = orbit.ortho6();
        assert_eq!(views.len(), 6);
        let labels: Vec<&str> = views.iter().map(|v| v.label.as_str()).collect();
        assert!(labels.contains(&"front"));
        assert!(labels.contains(&"top"));
    }

    #[test]
    fn ortho6_views_are_orthographic() {
        let orbit = CameraOrbit::from_bounds([0.0; 3], [2.0; 3]);
        let views = orbit.ortho6();
        assert!(views.iter().all(|v| v.fov_deg == 0.0));
    }

    #[test]
    fn depth_buffer_normalized_range() {
        let buf = DepthBuffer {
            width: 2,
            height: 2,
            data: vec![1.0, 3.0, f32::INFINITY, 2.0],
        };
        let norm = buf.normalized();
        assert_eq!(norm.len(), 4);
        assert!((norm[0] - 0.0).abs() < 1e-6); // min -> 0
        assert!((norm[1] - 1.0).abs() < 1e-6); // max -> 1
        assert!((norm[2] - 1.0).abs() < 1e-6); // inf -> 1
        assert!((norm[3] - 0.5).abs() < 1e-6); // mid -> 0.5
    }

    #[test]
    fn edge_detection_from_depth() {
        let mut data = vec![0.5_f32; 16];
        // Create a sharp discontinuity in the middle column.
        for y in 0..4 {
            data[y * 4 + 2] = 5.0;
            data[y * 4 + 3] = 5.0;
        }
        let depth = DepthBuffer { width: 4, height: 4, data };
        let edges = EdgeBuffer::from_depth(&depth, 0.1);
        assert_eq!(edges.width, 4);
        assert_eq!(edges.data.len(), 16);
        // Inner pixels near the discontinuity should have edges.
        // At (1,1) the Sobel kernel straddles the step at column 2.
        assert!(edges.data[1 * 4 + 1] > 0.0 || edges.data[1 * 4 + 2] > 0.0);
    }

    #[test]
    fn render_depth_simple_triangle() {
        let verts = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "test".into(),
        };
        let buf = render_depth(&verts, &indices, &view);
        assert_eq!(buf.data.len(), 256);
        // At least some pixels should be hit (finite depth).
        let hits = buf.data.iter().filter(|d| d.is_finite()).count();
        assert!(hits > 0, "Expected some pixels to be rasterised");
    }

    #[test]
    fn describe_features_empty() {
        assert_eq!(describe_features(&[]), "An empty CAD model with no features.");
    }

    #[test]
    fn describe_features_nonempty() {
        let feats = vec![
            "a rectangular base (100x50x20mm)".to_string(),
            "4 holes dia 8mm".to_string(),
        ];
        let desc = describe_features(&feats);
        assert!(desc.starts_with("A 3D mechanical part"));
        assert!(desc.contains("rectangular base"));
    }

    #[test]
    fn describe_tags_recognises_keywords() {
        let feats = vec!["4 drilled holes".to_string(), "fillet R2".to_string()];
        let tags = describe_tags(&feats);
        assert!(tags.contains(&"drilled holes".to_string()));
        assert!(tags.contains(&"filleted edges".to_string()));
        assert!(tags.contains(&"3D CAD".to_string()));
    }

    #[test]
    fn depth_cache_local_hit_rate() {
        let mut cache = DepthCache::new();
        let verts = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "test".into(),
        };
        let _ = cache.get_or_render(&verts, &indices, &view);
        let _ = cache.get_or_render(&verts, &indices, &view);
        let _ = cache.get_or_render(&verts, &indices, &view);
        let s = cache.stats();
        assert_eq!(s.misses, 1, "first call is a miss");
        assert_eq!(s.hits, 2, "subsequent calls are hits");
        assert_eq!(s.entries, 1);
    }

    #[test]
    fn depth_cache_local_distinct_keys_for_different_views() {
        let mut cache = DepthCache::new();
        let verts = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view_a = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "a".into(),
        };
        let mut view_b = view_a.clone();
        view_b.fov_deg = 60.0;
        let _ = cache.get_or_render(&verts, &indices, &view_a);
        let _ = cache.get_or_render(&verts, &indices, &view_b);
        assert_eq!(cache.stats().entries, 2);
    }

    #[test]
    fn depth_cache_local_distinct_keys_for_different_meshes() {
        let mut cache = DepthCache::new();
        let verts_a = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let verts_b = [[0.0, 0.6, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "v".into(),
        };
        let _ = cache.get_or_render(&verts_a, &indices, &view);
        let _ = cache.get_or_render(&verts_b, &indices, &view);
        assert_eq!(cache.stats().entries, 2);
    }

    #[test]
    fn render_depth_matches_uncached() {
        let verts = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "cmp".into(),
        };
        let cached = render_depth(&verts, &indices, &view);
        let uncached = render_depth_uncached(&verts, &indices, &view);
        assert_eq!(cached.width, uncached.width);
        assert_eq!(cached.height, uncached.height);
        assert_eq!(cached.data.len(), uncached.data.len());
        for (a, b) in cached.data.iter().zip(uncached.data.iter()) {
            if a.is_finite() && b.is_finite() {
                assert!((a - b).abs() < 1e-6);
            } else {
                assert_eq!(a.is_finite(), b.is_finite());
            }
        }
    }

    #[test]
    fn depth_cache_clear_resets_local_state() {
        let mut cache = DepthCache::new();
        let verts = [[0.0, 0.5, 2.0], [-0.5, -0.5, 2.0], [0.5, -0.5, 2.0]];
        let indices = [[0u32, 1, 2]];
        let view = ViewConfig {
            eye: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            resolution: [16, 16],
            label: "v".into(),
        };
        let _ = cache.get_or_render(&verts, &indices, &view);
        cache.clear();
        let s = cache.stats();
        assert_eq!(s.entries, 0);
        assert_eq!(s.hits, 0);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn api_types_serialise() {
        let req = ImageGenRequest {
            prompt: "test".into(),
            width: 512,
            height: 512,
            conditioning: Some(ConditioningMap::Depth(vec![0.5])),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test"));

        let mesh_req = Mesh3DRequest {
            image_url: "https://example.com/img.png".into(),
            output_format: "glb".into(),
        };
        let json2 = serde_json::to_string(&mesh_req).unwrap();
        assert!(json2.contains("glb"));
    }
}
