//! Core slicing algorithm — intersect triangle mesh with horizontal planes.

use glam::DVec2;
use physical_mfg_toolpath::contour::{chain_segments, Contour};
use physical_tessellation::TessMesh;

/// Slice a triangle mesh at a given Z height, producing closed 2D contours.
///
/// For each triangle that straddles the plane z=Z, compute the two intersection
/// points on the triangle edges, yielding a line segment. Chain all segments
/// into closed contours.
pub fn slice_mesh_at_z(mesh: &TessMesh, z: f64) -> Vec<Contour> {
    let mut segments: Vec<(DVec2, DVec2)> = Vec::new();

    // Process each triangle
    let tri_count = mesh.indices.len() / 3;
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        let z0 = mesh.vertices[i0].position[2] as f64;
        let z1 = mesh.vertices[i1].position[2] as f64;
        let z2 = mesh.vertices[i2].position[2] as f64;

        // Quick rejection: triangle entirely above or below
        let min_z = z0.min(z1).min(z2);
        let max_z = z0.max(z1).max(z2);
        if max_z < z - 1e-8 || min_z > z + 1e-8 {
            continue;
        }

        // Find intersection points of triangle edges with plane z=Z
        let v0 = [
            mesh.vertices[i0].position[0] as f64,
            mesh.vertices[i0].position[1] as f64,
            z0,
        ];
        let v1 = [
            mesh.vertices[i1].position[0] as f64,
            mesh.vertices[i1].position[1] as f64,
            z1,
        ];
        let v2 = [
            mesh.vertices[i2].position[0] as f64,
            mesh.vertices[i2].position[1] as f64,
            z2,
        ];

        let edges = [(v0, v1), (v1, v2), (v2, v0)];
        let mut intersections = Vec::new();

        for (a, b) in &edges {
            if let Some(pt) = edge_plane_intersection(a, b, z) {
                // Deduplicate: don't add if very close to last point
                if intersections.last().map_or(true, |last: &DVec2| {
                    (*last - pt).length() > 1e-8
                }) {
                    intersections.push(pt);
                }
            }
        }

        // A plane-triangle intersection produces exactly 2 points (or edge-on which we skip)
        if intersections.len() == 2 {
            segments.push((intersections[0], intersections[1]));
        }
    }

    chain_segments(&segments)
}

/// Intersect a 3D edge with the plane z=Z.
/// Returns the 2D (x,y) intersection point if the edge crosses the plane.
fn edge_plane_intersection(a: &[f64; 3], b: &[f64; 3], z: f64) -> Option<DVec2> {
    let dz = b[2] - a[2];
    if dz.abs() < 1e-12 {
        // Edge is parallel to plane
        if (a[2] - z).abs() < 1e-8 {
            // Edge lies on plane — return midpoint
            return Some(DVec2::new((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0));
        }
        return None;
    }

    let t = (z - a[2]) / dz;
    if t < -1e-8 || t > 1.0 + 1e-8 {
        return None;
    }

    let t = t.clamp(0.0, 1.0);
    Some(DVec2::new(
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
    ))
}

/// Compute all slice heights for a mesh given layer height and first layer height.
pub fn compute_layer_heights(mesh: &TessMesh, layer_height: f64, first_layer_height: f64) -> Vec<f64> {
    if mesh.vertices.is_empty() {
        return Vec::new();
    }

    let min_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::INFINITY, f32::min) as f64;
    let max_z = mesh.vertices.iter().map(|v| v.position[2]).fold(f32::NEG_INFINITY, f32::max) as f64;

    let mut heights = Vec::new();
    let first_z = min_z + first_layer_height / 2.0; // Slice at mid-layer
    heights.push(first_z);

    let mut z = min_z + first_layer_height + layer_height / 2.0;
    while z < max_z {
        heights.push(z);
        z += layer_height;
    }

    heights
}

/// Slice a complete mesh into layers. Returns (z_height, contours) pairs.
pub fn slice_mesh(
    mesh: &TessMesh,
    layer_height: f64,
    first_layer_height: f64,
) -> Vec<(f64, Vec<Contour>)> {
    let heights = compute_layer_heights(mesh, layer_height, first_layer_height);
    heights
        .into_iter()
        .map(|z| {
            let contours = slice_mesh_at_z(mesh, z);
            (z, contours)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_tessellation::TessMesh;
    use physical_tessellation::TessVertex;

    fn simple_box_mesh() -> TessMesh {
        // A simple 10x10x10 box as triangles
        // Bottom face (z=0): 2 triangles
        // Top face (z=10): 2 triangles
        // 4 side faces: 8 triangles
        // Total: 12 triangles, 8 vertices
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
            // Bottom (z=0)
            0, 2, 1,  0, 3, 2,
            // Top (z=10)
            4, 5, 6,  4, 6, 7,
            // Front (y=0)
            0, 1, 5,  0, 5, 4,
            // Back (y=10)
            2, 3, 7,  2, 7, 6,
            // Left (x=0)
            0, 4, 7,  0, 7, 3,
            // Right (x=10)
            1, 2, 6,  1, 6, 5,
        ];
        TessMesh { vertices, indices }
    }

    #[test]
    fn slice_box_midplane() {
        let mesh = simple_box_mesh();
        let contours = slice_mesh_at_z(&mesh, 5.0);
        assert_eq!(contours.len(), 1, "Box cross-section should be 1 contour");
        assert!(contours[0].points.len() >= 4, "Box cross-section should have at least 4 vertices, got {}", contours[0].points.len());
    }

    #[test]
    fn slice_box_layers() {
        let mesh = simple_box_mesh();
        let layers = slice_mesh(&mesh, 1.0, 1.0);
        assert!(layers.len() >= 8, "10mm box at 1mm layers should have ~10 layers, got {}", layers.len());
        for (_z, contours) in &layers {
            assert!(!contours.is_empty(), "Each layer should have contours");
        }
    }

    #[test]
    fn no_contours_above_mesh() {
        let mesh = simple_box_mesh();
        let contours = slice_mesh_at_z(&mesh, 20.0);
        assert!(contours.is_empty());
    }

    #[test]
    fn no_contours_below_mesh() {
        let mesh = simple_box_mesh();
        let contours = slice_mesh_at_z(&mesh, -5.0);
        assert!(contours.is_empty());
    }
}
