//! Variable layer height — adapts layer thickness to geometry curvature.
//!
//! Thin layers where the surface is nearly horizontal (high curvature),
//! thick layers where the surface is near-vertical (low visual impact).

use serde::{Deserialize, Serialize};

/// Configuration for adaptive layer heights.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveLayerConfig {
    /// Minimum layer height (mm).
    pub min_height: f64,
    /// Maximum layer height (mm).
    pub max_height: f64,
    /// Quality factor (0.0 = fastest, 1.0 = highest quality).
    pub quality: f64,
    /// Surface angle threshold (degrees): below this → min layer.
    pub threshold_angle: f64,
}

impl Default for AdaptiveLayerConfig {
    fn default() -> Self {
        Self {
            min_height: 0.08,
            max_height: 0.32,
            quality: 0.5,
            threshold_angle: 30.0,
        }
    }
}

/// A computed layer with variable height.
#[derive(Clone, Debug)]
pub struct AdaptiveLayer {
    /// Z position of the top of this layer.
    pub z: f64,
    /// Layer thickness.
    pub height: f64,
    /// Average surface normal angle from vertical (degrees).
    pub avg_surface_angle: f64,
}

/// Compute adaptive layer heights for a mesh.
///
/// Analyzes triangle normals at each Z level to determine optimal layer
/// thickness. Nearly-horizontal surfaces get thin layers for quality,
/// while vertical walls get thick layers for speed.
pub fn compute_adaptive_layers(
    total_height: f64,
    surface_angles: &[(f64, f64)], // (z, angle_degrees) samples
    config: &AdaptiveLayerConfig,
) -> Vec<AdaptiveLayer> {
    let mut layers = Vec::new();
    let mut z = 0.0;

    while z < total_height {
        // Find surface angle at current z
        let angle = interpolate_angle(z, surface_angles);

        // Map angle to layer height
        // angle=0 (horizontal) → min height (need precision)
        // angle=90 (vertical) → max height (stair-stepping is invisible)
        let t = (angle / 90.0).clamp(0.0, 1.0);
        let quality_factor = 1.0 - config.quality;
        let height = config.min_height
            + (config.max_height - config.min_height) * t * (0.5 + quality_factor * 0.5);

        // Clamp to valid range
        let height = height.clamp(config.min_height, config.max_height);

        // Don't overshoot total height
        let height = height.min(total_height - z);
        if height < 0.001 { break; }

        z += height;
        layers.push(AdaptiveLayer {
            z,
            height,
            avg_surface_angle: angle,
        });
    }

    layers
}

/// Interpolate surface angle at a given Z height.
fn interpolate_angle(z: f64, samples: &[(f64, f64)]) -> f64 {
    if samples.is_empty() { return 90.0; } // default to vertical

    // Find bracketing samples
    let mut below = None;
    let mut above = None;
    for &(sz, angle) in samples {
        if sz <= z { below = Some((sz, angle)); }
        if sz >= z && above.is_none() { above = Some((sz, angle)); }
    }

    match (below, above) {
        (Some((_, a)), None) => a,
        (None, Some((_, a))) => a,
        (Some((z0, a0)), Some((z1, a1))) => {
            if (z1 - z0).abs() < 1e-10 { return a0; }
            let t = (z - z0) / (z1 - z0);
            a0 + t * (a1 - a0)
        }
        (None, None) => 90.0,
    }
}

/// Estimate layer count savings vs uniform layers.
pub fn layer_count_savings(adaptive: &[AdaptiveLayer], uniform_height: f64) -> (usize, usize) {
    let total_z = adaptive.last().map(|l| l.z).unwrap_or(0.0);
    let uniform_count = (total_z / uniform_height).ceil() as usize;
    let adaptive_count = adaptive.len();
    (uniform_count, adaptive_count)
}

/// Compute adaptive layer heights directly from a tessellated mesh.
///
/// Analyzes surface normals at each Z level to determine optimal layer
/// thickness. Returns a `Vec<f64>` of Z positions for each layer top.
///
/// * `mesh` — tessellated mesh to analyze
/// * `min_layer` — minimum layer height in mm
/// * `max_layer` — maximum layer height in mm
/// * `angle_threshold` — surface angle (degrees) below which thin layers are used
pub fn adaptive_layer_heights(
    mesh: &physical_tessellation::TessMesh,
    min_layer: f64,
    max_layer: f64,
    angle_threshold: f64,
) -> Vec<f64> {
    if mesh.vertices.is_empty() {
        return Vec::new();
    }

    // Compute mesh Z bounds
    let min_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::INFINITY, f32::min) as f64;
    let max_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::NEG_INFINITY, f32::max) as f64;
    let total_height = max_z - min_z;
    if total_height < 1e-6 {
        return Vec::new();
    }

    // Sample surface angles from triangle normals at various Z levels
    let n_samples = ((total_height / min_layer) as usize).min(500).max(10);
    let mut surface_angles: Vec<(f64, f64)> = Vec::with_capacity(n_samples);

    let tri_count = mesh.indices.len() / 3;
    for s in 0..n_samples {
        let z = min_z + (s as f64 + 0.5) / n_samples as f64 * total_height;
        let mut angle_sum = 0.0;
        let mut count = 0u32;

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;
            let z0 = mesh.vertices[i0].position[2] as f64;
            let z1 = mesh.vertices[i1].position[2] as f64;
            let z2 = mesh.vertices[i2].position[2] as f64;
            let tri_min = z0.min(z1).min(z2);
            let tri_max = z0.max(z1).max(z2);
            if tri_min > z + max_layer || tri_max < z - max_layer {
                continue;
            }
            // Normal from vertex data (average of triangle vertex normals)
            let nx = (mesh.vertices[i0].normal[0] + mesh.vertices[i1].normal[0] + mesh.vertices[i2].normal[0]) / 3.0;
            let ny = (mesh.vertices[i0].normal[1] + mesh.vertices[i1].normal[1] + mesh.vertices[i2].normal[1]) / 3.0;
            let nz = (mesh.vertices[i0].normal[2] + mesh.vertices[i1].normal[2] + mesh.vertices[i2].normal[2]) / 3.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len < 1e-12 {
                continue;
            }
            // Angle from vertical (Z axis): acos(|nz|/len) in degrees
            let angle_from_vertical = (nz.abs() as f64 / len as f64).acos().to_degrees();
            angle_sum += angle_from_vertical;
            count += 1;
        }

        let avg_angle = if count > 0 { angle_sum / count as f64 } else { 90.0 };
        surface_angles.push((z - min_z, avg_angle));
    }

    let config = AdaptiveLayerConfig {
        min_height: min_layer,
        max_height: max_layer,
        quality: 0.5,
        threshold_angle: angle_threshold,
    };

    let layers = compute_adaptive_layers(total_height, &surface_angles, &config);
    layers.iter().map(|l| min_z + l.z).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_layers_cover_height() {
        let config = AdaptiveLayerConfig::default();
        let samples = vec![(0.0, 90.0), (5.0, 45.0), (10.0, 10.0)];
        let layers = compute_adaptive_layers(10.0, &samples, &config);

        assert!(!layers.is_empty());
        let top_z = layers.last().unwrap().z;
        assert!((top_z - 10.0).abs() < config.max_height, "Should cover total height");
    }

    #[test]
    fn adaptive_thin_at_horizontal() {
        let config = AdaptiveLayerConfig {
            min_height: 0.1,
            max_height: 0.3,
            quality: 0.8,
            threshold_angle: 30.0,
        };
        // All horizontal surfaces → should use thin layers
        let samples = vec![(0.0, 5.0), (5.0, 5.0), (10.0, 5.0)];
        let layers = compute_adaptive_layers(10.0, &samples, &config);

        for layer in &layers {
            assert!(layer.height < 0.2, "Horizontal surface should get thin layers, got {}", layer.height);
        }
    }

    #[test]
    fn adaptive_thick_at_vertical() {
        let config = AdaptiveLayerConfig::default();
        // All vertical surfaces → should use thick layers
        let samples = vec![(0.0, 89.0), (5.0, 89.0), (10.0, 89.0)];
        let layers = compute_adaptive_layers(10.0, &samples, &config);

        for layer in &layers {
            assert!(layer.height > 0.15, "Vertical surface should get thick layers, got {}", layer.height);
        }
    }

    #[test]
    fn layer_count_savings_positive() {
        let config = AdaptiveLayerConfig::default();
        let samples = vec![(0.0, 80.0), (5.0, 80.0), (10.0, 80.0)]; // mostly vertical
        let layers = compute_adaptive_layers(10.0, &samples, &config);
        let (uniform, adaptive) = layer_count_savings(&layers, 0.2);
        assert!(adaptive <= uniform, "Adaptive should use fewer or equal layers");
    }

    #[test]
    fn adaptive_layer_heights_from_mesh() {
        use physical_tessellation::{TessMesh, TessVertex};
        // Simple 10mm tall box
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
        #[rustfmt::skip]
        let indices = vec![
            0, 2, 1, 0, 3, 2,
            4, 5, 6, 4, 6, 7,
            0, 1, 5, 0, 5, 4,
            2, 3, 7, 2, 7, 6,
            0, 4, 7, 0, 7, 3,
            1, 2, 6, 1, 6, 5,
        ];
        let mesh = TessMesh { vertices, indices };
        let heights = adaptive_layer_heights(&mesh, 0.1, 0.3, 30.0);
        assert!(!heights.is_empty(), "Should produce layers");
        // All heights should be between mesh min_z and max_z
        for &h in &heights {
            assert!(h >= 0.0 && h <= 10.5, "Height {h} out of range");
        }
        // Heights should be monotonically increasing
        for i in 1..heights.len() {
            assert!(heights[i] > heights[i - 1], "Heights must be monotonically increasing");
        }
    }

    #[test]
    fn adaptive_layer_heights_empty_mesh() {
        use physical_tessellation::TessMesh;
        let mesh = TessMesh { vertices: vec![], indices: vec![] };
        let heights = adaptive_layer_heights(&mesh, 0.1, 0.3, 30.0);
        assert!(heights.is_empty());
    }
}
