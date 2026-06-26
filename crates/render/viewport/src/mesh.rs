//! Mesh primitive generators for CAD — cube, sphere, cylinder, torus, icosphere.

use std::collections::HashMap;
use std::f32::consts::PI;

use crate::vertex::Vertex;

pub fn cube(size: f32) -> (Vec<Vertex>, Vec<u32>) {
    let h = size * 0.5;

    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, 1.0], [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]]),
        ([0.0, 0.0, -1.0], [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]]),
        ([1.0, 0.0, 0.0], [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]]),
        ([-1.0, 0.0, 0.0], [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]]),
        ([0.0, 1.0, 0.0], [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]]),
        ([0.0, -1.0, 0.0], [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]]),
    ];

    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    for (normal, positions) in &faces {
        let base = vertices.len() as u32;
        for (i, pos) in positions.iter().enumerate() {
            vertices.push(Vertex {
                position: *pos,
                normal: *normal,
                uv: uvs[i],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    (vertices, indices)
}

pub fn sphere(radius: f32, segments: u32, rings: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(((rings + 1) * (segments + 1)) as usize);
    let mut indices = Vec::with_capacity((rings * segments * 6) as usize);

    for r in 0..=rings {
        let v = r as f32 / rings as f32;
        let theta = v * PI;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for s in 0..=segments {
            let u = s as f32 / segments as f32;
            let phi = u * 2.0 * PI;

            let nx = sin_theta * phi.cos();
            let ny = cos_theta;
            let nz = sin_theta * phi.sin();

            vertices.push(Vertex {
                position: [radius * nx, radius * ny, radius * nz],
                normal: [nx, ny, nz],
                uv: [u, v],
            });
        }
    }

    let stride = segments + 1;
    for r in 0..rings {
        for s in 0..segments {
            let tl = r * stride + s;
            let tr = tl + 1;
            let bl = tl + stride;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    (vertices, indices)
}

pub fn cylinder(radius: f32, height: f32, segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let half_h = height * 0.5;

    // Side wall
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let angle = u * 2.0 * PI;
        let nx = angle.cos();
        let nz = angle.sin();

        vertices.push(Vertex {
            position: [radius * nx, -half_h, radius * nz],
            normal: [nx, 0.0, nz],
            uv: [u, 1.0],
        });
        vertices.push(Vertex {
            position: [radius * nx, half_h, radius * nz],
            normal: [nx, 0.0, nz],
            uv: [u, 0.0],
        });
    }

    for i in 0..segments {
        let base = i * 2;
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }

    // Caps
    for &(y, ny, flip) in &[(half_h, 1.0_f32, false), (-half_h, -1.0_f32, true)] {
        let center = vertices.len() as u32;
        vertices.push(Vertex {
            position: [0.0, y, 0.0],
            normal: [0.0, ny, 0.0],
            uv: [0.5, 0.5],
        });

        for i in 0..=segments {
            let angle = i as f32 / segments as f32 * 2.0 * PI;
            let cx = angle.cos();
            let cz = angle.sin();
            vertices.push(Vertex {
                position: [radius * cx, y, radius * cz],
                normal: [0.0, ny, 0.0],
                uv: [0.5 + cx * 0.5, 0.5 + cz * 0.5],
            });
        }

        for i in 0..segments {
            let a = center + 1 + i;
            let b = center + 2 + i;
            if flip {
                indices.extend_from_slice(&[center, b, a]);
            } else {
                indices.extend_from_slice(&[center, a, b]);
            }
        }
    }

    (vertices, indices)
}

pub fn torus(
    major_radius: f32,
    minor_radius: f32,
    major_segments: u32,
    minor_segments: u32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices =
        Vec::with_capacity(((major_segments + 1) * (minor_segments + 1)) as usize);
    let mut indices = Vec::with_capacity((major_segments * minor_segments * 6) as usize);

    for i in 0..=major_segments {
        let u = i as f32 / major_segments as f32;
        let theta = u * 2.0 * PI;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        for j in 0..=minor_segments {
            let v = j as f32 / minor_segments as f32;
            let phi = v * 2.0 * PI;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            let x = (major_radius + minor_radius * cos_phi) * cos_theta;
            let y = minor_radius * sin_phi;
            let z = (major_radius + minor_radius * cos_phi) * sin_theta;

            let nx = cos_phi * cos_theta;
            let ny = sin_phi;
            let nz = cos_phi * sin_theta;

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx, ny, nz],
                uv: [u, v],
            });
        }
    }

    let stride = minor_segments + 1;
    for i in 0..major_segments {
        for j in 0..minor_segments {
            let tl = i * stride + j;
            let tr = tl + 1;
            let bl = tl + stride;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    (vertices, indices)
}

pub fn icosphere(radius: f32, subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;

    let mut positions: Vec<[f32; 3]> = vec![
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ];

    for p in &mut positions {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        p[0] /= len;
        p[1] /= len;
        p[2] /= len;
    }

    let mut triangles: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();

    for _ in 0..subdivisions {
        let mut new_triangles = Vec::with_capacity(triangles.len() * 4);
        midpoint_cache.clear();

        for tri in &triangles {
            let a = get_midpoint(tri[0], tri[1], &mut positions, &mut midpoint_cache);
            let b = get_midpoint(tri[1], tri[2], &mut positions, &mut midpoint_cache);
            let c = get_midpoint(tri[2], tri[0], &mut positions, &mut midpoint_cache);

            new_triangles.push([tri[0], a, c]);
            new_triangles.push([tri[1], b, a]);
            new_triangles.push([tri[2], c, b]);
            new_triangles.push([a, b, c]);
        }

        triangles = new_triangles;
    }

    let vertices: Vec<Vertex> = positions
        .iter()
        .map(|p| {
            let n = *p;
            let u = 0.5 + p[2].atan2(p[0]) / (2.0 * PI);
            let v = 0.5 - p[1].asin() / PI;
            Vertex {
                position: [p[0] * radius, p[1] * radius, p[2] * radius],
                normal: n,
                uv: [u, v],
            }
        })
        .collect();

    let indices: Vec<u32> = triangles.iter().flat_map(|t| t.iter().copied()).collect();

    (vertices, indices)
}

fn get_midpoint(
    a: u32,
    b: u32,
    positions: &mut Vec<[f32; 3]>,
    cache: &mut HashMap<(u32, u32), u32>,
) -> u32 {
    let key = if a < b { (a, b) } else { (b, a) };

    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    let pa = positions[a as usize];
    let pb = positions[b as usize];
    let mut mid = [
        (pa[0] + pb[0]) * 0.5,
        (pa[1] + pb[1]) * 0.5,
        (pa[2] + pb[2]) * 0.5,
    ];

    let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    mid[0] /= len;
    mid[1] /= len;
    mid[2] /= len;

    let idx = positions.len() as u32;
    positions.push(mid);
    cache.insert(key, idx);
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal_length(n: &[f32; 3]) -> f32 {
        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
    }

    #[test]
    fn cube_vertex_and_index_counts() {
        let (verts, indices) = cube(1.0);
        assert_eq!(verts.len(), 24);
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn cube_normals_are_unit_length() {
        let (verts, _) = cube(2.0);
        for v in &verts {
            let len = normal_length(&v.normal);
            assert!((len - 1.0).abs() < 1e-5, "cube normal not unit: len = {len}");
        }
    }

    #[test]
    fn sphere_vertex_and_index_counts() {
        let segments = 32u32;
        let rings = 16u32;
        let (verts, indices) = sphere(0.5, segments, rings);
        assert_eq!(verts.len(), ((rings + 1) * (segments + 1)) as usize);
        assert_eq!(indices.len(), (rings * segments * 6) as usize);
    }

    #[test]
    fn sphere_positions_bounded_by_radius() {
        let radius = 2.0;
        let (verts, _) = sphere(radius, 32, 16);
        for v in &verts {
            let dist = (v.position[0] * v.position[0]
                + v.position[1] * v.position[1]
                + v.position[2] * v.position[2])
                .sqrt();
            assert!((dist - radius).abs() < 1e-4, "vertex at dist {dist}, expected {radius}");
        }
    }

    #[test]
    fn cylinder_has_vertices_and_indices() {
        let (verts, indices) = cylinder(0.5, 1.0, 16);
        assert!(!verts.is_empty());
        assert!(!indices.is_empty());
        assert_eq!(indices.len() % 3, 0);
    }

    #[test]
    fn torus_vertex_and_index_counts() {
        let (verts, indices) = torus(1.0, 0.3, 24, 12);
        assert_eq!(verts.len(), ((24 + 1) * (12 + 1)) as usize);
        assert_eq!(indices.len(), (24 * 12 * 6) as usize);
    }

    #[test]
    fn icosphere_base_has_12_verts_20_faces() {
        let (verts, indices) = icosphere(1.0, 0);
        assert_eq!(verts.len(), 12);
        assert_eq!(indices.len(), 60);
    }

    #[test]
    fn all_meshes_have_triangulated_indices() {
        let meshes: Vec<(Vec<Vertex>, Vec<u32>)> = vec![
            cube(1.0),
            sphere(1.0, 16, 8),
            cylinder(1.0, 1.0, 16),
            torus(1.0, 0.3, 16, 8),
            icosphere(1.0, 1),
        ];
        for (i, (_, indices)) in meshes.iter().enumerate() {
            assert_eq!(indices.len() % 3, 0, "mesh {i}: not triangulated");
        }
    }

    #[test]
    fn all_meshes_indices_in_bounds() {
        let meshes: Vec<(Vec<Vertex>, Vec<u32>)> = vec![
            cube(1.0),
            sphere(1.0, 16, 8),
            cylinder(0.5, 1.0, 16),
            torus(1.0, 0.3, 16, 8),
            icosphere(1.0, 2),
        ];
        for (i, (verts, indices)) in meshes.iter().enumerate() {
            let max_idx = verts.len() as u32;
            for &idx in indices {
                assert!(idx < max_idx, "mesh {i}: index {idx} >= {max_idx}");
            }
        }
    }
}
