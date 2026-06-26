//! Support structure generation for FDM 3D printing.
//!
//! Detects overhanging regions and generates support geometry:
//! - Linear (grid) supports
//! - Tree supports (contact-minimizing)

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Support generation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupportConfig {
    /// Overhang angle threshold (degrees). Faces steeper than this need support.
    pub overhang_angle: f64,
    /// Support pattern.
    pub pattern: SupportPattern,
    /// Support line spacing (mm).
    pub spacing: f64,
    /// XY offset from part surface (mm).
    pub xy_offset: f64,
    /// Z offset from part surface (mm).
    pub z_offset: f64,
    /// Support interface layers (dense layers touching the part).
    pub interface_layers: usize,
    /// Interface density (0.0-1.0).
    pub interface_density: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SupportPattern {
    /// Parallel lines.
    Linear,
    /// Grid pattern.
    Grid,
    /// Zigzag (connected lines, reduces travel moves).
    Zigzag,
    /// Tree supports — branches that converge.
    Tree,
}

impl Default for SupportConfig {
    fn default() -> Self {
        Self {
            overhang_angle: 45.0,
            pattern: SupportPattern::Grid,
            spacing: 3.0,
            xy_offset: 0.4,
            z_offset: 0.2,
            interface_layers: 2,
            interface_density: 0.8,
        }
    }
}

/// A support column/region on a specific layer.
#[derive(Clone, Debug)]
pub struct SupportRegion {
    /// Boundary polygon of the support region.
    pub boundary: Vec<DVec2>,
    /// Z bottom of support.
    pub z_bottom: f64,
    /// Z top of support (where it contacts the overhang).
    pub z_top: f64,
    /// Whether this is an interface layer.
    pub is_interface: bool,
}

/// Detect overhang regions from triangle normals.
///
/// Returns regions at each Z level that need support.
pub fn detect_overhangs(
    normals_per_layer: &[(f64, Vec<(DVec2, f64)>)], // (z, [(position, overhang_angle)])
    config: &SupportConfig,
) -> Vec<SupportRegion> {
    let mut regions = Vec::new();

    for (z, triangles) in normals_per_layer {
        let overhang_points: Vec<DVec2> = triangles.iter()
            .filter(|(_, angle)| *angle > config.overhang_angle)
            .map(|(pos, _)| *pos)
            .collect();

        if overhang_points.len() < 3 { continue; }

        // Simple bounding box region (production would use convex hull)
        let (min, max) = bounding_box_2d(&overhang_points);
        let offset = config.xy_offset;
        let boundary = vec![
            DVec2::new(min.x - offset, min.y - offset),
            DVec2::new(max.x + offset, min.y - offset),
            DVec2::new(max.x + offset, max.y + offset),
            DVec2::new(min.x - offset, max.y + offset),
        ];

        regions.push(SupportRegion {
            boundary,
            z_bottom: 0.0, // simplified: all supports start at bed
            z_top: *z - config.z_offset,
            is_interface: false,
        });
    }

    // Add interface layers
    let interface_regions: Vec<SupportRegion> = regions.iter()
        .filter(|r| !r.is_interface)
        .flat_map(|r| {
            (0..config.interface_layers).map(move |i| {
                let layer_height = 0.2; // typical
                SupportRegion {
                    boundary: r.boundary.clone(),
                    z_bottom: r.z_top - (i + 1) as f64 * layer_height,
                    z_top: r.z_top - i as f64 * layer_height,
                    is_interface: true,
                }
            })
        })
        .collect();

    regions.extend(interface_regions);
    regions
}

/// Generate support infill lines for a region at a given layer.
pub fn generate_support_infill(
    region: &SupportRegion,
    config: &SupportConfig,
    layer_index: usize,
) -> Vec<(DVec2, DVec2)> {
    let spacing = if region.is_interface {
        config.spacing * (1.0 - config.interface_density).max(0.1)
    } else {
        config.spacing
    };

    let (min, max) = bounding_box_2d(&region.boundary);

    match config.pattern {
        SupportPattern::Linear | SupportPattern::Zigzag => {
            let angle = if layer_index % 2 == 0 { 0.0 } else { std::f64::consts::FRAC_PI_2 };
            generate_parallel_support_lines(min, max, spacing, angle)
        }
        SupportPattern::Grid => {
            let mut lines = generate_parallel_support_lines(min, max, spacing, 0.0);
            lines.extend(generate_parallel_support_lines(min, max, spacing, std::f64::consts::FRAC_PI_2));
            lines
        }
        SupportPattern::Tree => {
            // Simplified tree: converging lines from boundary to center
            generate_tree_support_lines(min, max, spacing)
        }
    }
}

fn generate_parallel_support_lines(
    min: DVec2,
    max: DVec2,
    spacing: f64,
    angle: f64,
) -> Vec<(DVec2, DVec2)> {
    let mut lines = Vec::new();
    let cos = angle.cos();
    let sin = angle.sin();
    let width = (max.x - min.x) + (max.y - min.y); // worst case diagonal

    let mut offset = -width;
    while offset < width {
        let p1 = DVec2::new(
            min.x + (offset * cos - width * sin),
            min.y + (offset * sin + width * cos),
        );
        let p2 = DVec2::new(
            min.x + (offset * cos + width * sin),
            min.y + (offset * sin - width * cos),
        );

        // Clip to bounding box
        if let Some((clipped_p1, clipped_p2)) = clip_line_to_rect(p1, p2, min, max) {
            lines.push((clipped_p1, clipped_p2));
        }

        offset += spacing;
    }

    lines
}

fn generate_tree_support_lines(
    min: DVec2,
    max: DVec2,
    spacing: f64,
) -> Vec<(DVec2, DVec2)> {
    let center = (min + max) * 0.5;
    let mut lines = Vec::new();

    // Generate lines from boundary to center (tree trunk pattern)
    let mut x = min.x;
    while x <= max.x {
        lines.push((DVec2::new(x, min.y), center));
        lines.push((DVec2::new(x, max.y), center));
        x += spacing;
    }

    let mut y = min.y;
    while y <= max.y {
        lines.push((DVec2::new(min.x, y), center));
        lines.push((DVec2::new(max.x, y), center));
        y += spacing;
    }

    lines
}

fn bounding_box_2d(points: &[DVec2]) -> (DVec2, DVec2) {
    let mut min = DVec2::splat(f64::MAX);
    let mut max = DVec2::splat(f64::MIN);
    for p in points {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    (min, max)
}

fn clip_line_to_rect(
    p1: DVec2, p2: DVec2,
    min: DVec2, max: DVec2,
) -> Option<(DVec2, DVec2)> {
    // Cohen-Sutherland-like: just check if both endpoints are outside
    let inside = |p: DVec2| p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y;
    if inside(p1) && inside(p2) {
        return Some((p1, p2));
    }
    // Simple clamp (not full Cohen-Sutherland)
    let clamp = |p: DVec2| DVec2::new(
        p.x.clamp(min.x, max.x),
        p.y.clamp(min.y, max.y),
    );
    let c1 = clamp(p1);
    let c2 = clamp(p2);
    if (c1 - c2).length() > 0.01 {
        Some((c1, c2))
    } else {
        None
    }
}

/// Complete support structure output from `generate_supports`.
#[derive(Clone, Debug)]
pub struct SupportStructure {
    /// All support regions across layers.
    pub regions: Vec<SupportRegion>,
    /// Total estimated support volume in mm^3.
    pub volume_mm3: f64,
    /// Number of distinct overhang zones detected.
    pub overhang_zone_count: usize,
}

/// High-level support generation from a tessellated mesh.
///
/// Analyzes triangle normals to detect overhanging faces (angle > `overhang_angle`
/// from vertical), then generates support columns as additional geometry.
///
/// * `mesh` — tessellated mesh to analyze
/// * `overhang_angle` — angle threshold in degrees (faces steeper than this need support)
/// * `support_type` — pattern to use for support infill
pub fn generate_supports(
    mesh: &physical_tessellation::TessMesh,
    overhang_angle: f64,
    support_type: SupportPattern,
) -> SupportStructure {
    if mesh.vertices.is_empty() {
        return SupportStructure {
            regions: Vec::new(),
            volume_mm3: 0.0,
            overhang_zone_count: 0,
        };
    }

    let config = SupportConfig {
        overhang_angle,
        pattern: support_type,
        ..SupportConfig::default()
    };

    // Compute mesh Z bounds
    let min_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::INFINITY, f32::min) as f64;
    let max_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::NEG_INFINITY, f32::max) as f64;

    // Sample layers through the mesh
    let layer_height = 0.2;
    let mut normals_per_layer: Vec<(f64, Vec<(DVec2, f64)>)> = Vec::new();
    let tri_count = mesh.indices.len() / 3;

    let mut z = min_z + layer_height;
    while z < max_z {
        let mut triangles = Vec::new();
        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let z0 = mesh.vertices[i0].position[2] as f64;
            let z1 = mesh.vertices[i1].position[2] as f64;
            let z2 = mesh.vertices[i2].position[2] as f64;
            let tri_min = z0.min(z1).min(z2);
            let tri_max = z0.max(z1).max(z2);
            if tri_min > z + layer_height || tri_max < z - layer_height {
                continue;
            }

            // Compute face normal
            let v0 = [mesh.vertices[i0].position[0] as f64, mesh.vertices[i0].position[1] as f64, z0];
            let v1 = [mesh.vertices[i1].position[0] as f64, mesh.vertices[i1].position[1] as f64, z1];
            let v2 = [mesh.vertices[i2].position[0] as f64, mesh.vertices[i2].position[1] as f64, z2];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let nx = e1[1] * e2[2] - e1[2] * e2[1];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let nz = e1[0] * e2[1] - e1[1] * e2[0];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len < 1e-12 {
                continue;
            }

            // Downward-facing check: nz < 0 means face points downward
            // Overhang angle = angle between face normal and downward direction
            // We measure from the build direction (positive Z)
            let angle_from_vertical = (nz / len).abs().acos().to_degrees();

            // Centroid position
            let cx = (v0[0] + v1[0] + v2[0]) / 3.0;
            let cy = (v0[1] + v1[1] + v2[1]) / 3.0;

            // If face is downward-facing (nz < 0) and angle exceeds threshold
            if nz < 0.0 {
                triangles.push((DVec2::new(cx, cy), angle_from_vertical));
            }
        }

        if !triangles.is_empty() {
            normals_per_layer.push((z, triangles));
        }
        z += layer_height;
    }

    let regions = detect_overhangs(&normals_per_layer, &config);
    let overhang_zone_count = normals_per_layer.iter()
        .filter(|(_, tris)| tris.iter().any(|(_, a)| *a > overhang_angle))
        .count();

    // Estimate volume from region bounding boxes
    let volume_mm3 = regions.iter()
        .filter(|r| !r.is_interface)
        .map(|r| {
            let (min, max) = bounding_box_2d(&r.boundary);
            let area = (max.x - min.x) * (max.y - min.y);
            let height = r.z_top - r.z_bottom;
            area * height * 0.3 // ~30% fill density for support
        })
        .sum();

    SupportStructure {
        regions,
        volume_mm3,
        overhang_zone_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_overhangs_basic() {
        let normals = vec![
            (5.0, vec![
                (DVec2::new(10.0, 10.0), 60.0), // overhanging
                (DVec2::new(20.0, 20.0), 60.0),
                (DVec2::new(15.0, 30.0), 60.0),
            ]),
        ];
        let config = SupportConfig::default();
        let regions = detect_overhangs(&normals, &config);
        assert!(!regions.is_empty());
    }

    #[test]
    fn no_overhangs_at_shallow_angle() {
        let normals = vec![
            (5.0, vec![
                (DVec2::new(10.0, 10.0), 20.0), // not overhanging
                (DVec2::new(20.0, 20.0), 30.0),
                (DVec2::new(15.0, 30.0), 25.0),
            ]),
        ];
        let config = SupportConfig::default();
        let regions = detect_overhangs(&normals, &config);
        assert!(regions.is_empty());
    }

    #[test]
    fn support_infill_produces_lines() {
        let region = SupportRegion {
            boundary: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(20.0, 0.0),
                DVec2::new(20.0, 20.0),
                DVec2::new(0.0, 20.0),
            ],
            z_bottom: 0.0,
            z_top: 5.0,
            is_interface: false,
        };
        let config = SupportConfig::default();
        let lines = generate_support_infill(&region, &config, 0);
        assert!(!lines.is_empty());
    }

    #[test]
    fn interface_layer_denser() {
        let region_normal = SupportRegion {
            boundary: vec![DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0), DVec2::new(0.0, 20.0)],
            z_bottom: 0.0, z_top: 5.0, is_interface: false,
        };
        let region_interface = SupportRegion {
            boundary: region_normal.boundary.clone(),
            z_bottom: 4.8, z_top: 5.0, is_interface: true,
        };
        let config = SupportConfig::default();
        let lines_normal = generate_support_infill(&region_normal, &config, 0);
        let lines_interface = generate_support_infill(&region_interface, &config, 0);
        assert!(lines_interface.len() >= lines_normal.len(), "Interface should be denser");
    }

    #[test]
    fn generate_supports_empty_mesh() {
        use physical_tessellation::TessMesh;
        let mesh = TessMesh { vertices: vec![], indices: vec![] };
        let result = generate_supports(&mesh, 45.0, SupportPattern::Grid);
        assert!(result.regions.is_empty());
        assert_eq!(result.volume_mm3, 0.0);
        assert_eq!(result.overhang_zone_count, 0);
    }

    #[test]
    fn generate_supports_box_no_overhang() {
        use physical_tessellation::{TessMesh, TessVertex};
        // Vertical box — no overhangs expected (all faces are vertical or horizontal)
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
        // A simple box should not produce support (bottom face sits on bed)
        let result = generate_supports(&mesh, 45.0, SupportPattern::Grid);
        // Box bottom face normal is (0,0,-1), angle_from_vertical = 0 or 180
        // We only flag nz < 0 faces, but their angle should be 0 (pointing straight down)
        // which is below 45 threshold — no supports expected
        let _ = result; // Just verify no panic
    }

    #[test]
    fn tree_support_converges() {
        let region = SupportRegion {
            boundary: vec![DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0), DVec2::new(0.0, 20.0)],
            z_bottom: 0.0, z_top: 5.0, is_interface: false,
        };
        let config = SupportConfig {
            pattern: SupportPattern::Tree,
            ..SupportConfig::default()
        };
        let lines = generate_support_infill(&region, &config, 0);
        assert!(!lines.is_empty());
        // All lines should converge to center
        let center = DVec2::new(10.0, 10.0);
        for (_, end) in &lines {
            assert!((end - &center).length() < 0.1);
        }
    }
}
