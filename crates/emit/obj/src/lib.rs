//! `physical-emit-obj` -- Wavefront OBJ format writer and reader.
//!
//! Converts between [`TessMesh`] and the ASCII Wavefront OBJ format,
//! including basic MTL material support.

use std::fmt::Write as _;

use physical_tessellation::{TessMesh, TessVertex};

// ---------------------------------------------------------------------------
// OBJ writing
// ---------------------------------------------------------------------------

/// Write a full OBJ string with object name, positions, normals, UVs, and faces.
pub fn write_obj(mesh: &TessMesh, name: &str) -> String {
    let mut out = String::new();
    writeln!(out, "# physical-emit-obj").unwrap();
    writeln!(out, "o {name}").unwrap();

    // Vertices
    for v in &mesh.vertices {
        let [x, y, z] = v.position;
        writeln!(out, "v {x} {y} {z}").unwrap();
    }

    // Normals
    for v in &mesh.vertices {
        let [nx, ny, nz] = v.normal;
        writeln!(out, "vn {nx} {ny} {nz}").unwrap();
    }

    // Texture coordinates
    for v in &mesh.vertices {
        let [u, v] = v.uv;
        writeln!(out, "vt {u} {v}").unwrap();
    }

    // Faces (1-based indices)
    for tri in mesh.indices.chunks(3) {
        if tri.len() == 3 {
            let (a, b, c) = (tri[0] + 1, tri[1] + 1, tri[2] + 1);
            writeln!(out, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}").unwrap();
        }
    }

    out
}

/// Write a minimal OBJ string with only vertex positions and faces (no normals/UVs).
pub fn write_obj_simple(mesh: &TessMesh) -> String {
    let mut out = String::new();
    writeln!(out, "# physical-emit-obj").unwrap();

    for v in &mesh.vertices {
        let [x, y, z] = v.position;
        writeln!(out, "v {x} {y} {z}").unwrap();
    }

    for tri in mesh.indices.chunks(3) {
        if tri.len() == 3 {
            let (a, b, c) = (tri[0] + 1, tri[1] + 1, tri[2] + 1);
            writeln!(out, "f {a} {b} {c}").unwrap();
        }
    }

    out
}

// ---------------------------------------------------------------------------
// OBJ reading
// ---------------------------------------------------------------------------

/// Parse a Wavefront OBJ string back into a [`TessMesh`].
///
/// Handles face formats: `v//vn`, `v/vt/vn`, `v/vt`, and `v`.
/// Supports negative (relative) indices. Returns `None` on structural errors.
pub fn read_obj(text: &str) -> Option<TessMesh> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();

    // Each unique combo of (pos_idx, norm_idx, uv_idx) maps to an output vertex.
    let mut vertex_map: Vec<(usize, Option<usize>, Option<usize>)> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("mtllib") || line.starts_with("usemtl") || line.starts_with('o') || line.starts_with('g') || line.starts_with('s') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "v" if parts.len() >= 4 => {
                let x: f32 = parts[1].parse().ok()?;
                let y: f32 = parts[2].parse().ok()?;
                let z: f32 = parts[3].parse().ok()?;
                positions.push([x, y, z]);
            }
            "vn" if parts.len() >= 4 => {
                let nx: f32 = parts[1].parse().ok()?;
                let ny: f32 = parts[2].parse().ok()?;
                let nz: f32 = parts[3].parse().ok()?;
                normals.push([nx, ny, nz]);
            }
            "vt" if parts.len() >= 3 => {
                let u: f32 = parts[1].parse().ok()?;
                let v: f32 = parts[2].parse().ok()?;
                uvs.push([u, v]);
            }
            "f" if parts.len() >= 4 => {
                let mut face_indices: Vec<u32> = Vec::new();
                for &token in &parts[1..] {
                    let idx = parse_face_vertex(token, positions.len(), normals.len(), uvs.len())?;
                    // Look up or insert the unique vertex combo
                    let vi = if let Some(pos) = vertex_map.iter().position(|v| *v == idx) {
                        pos as u32
                    } else {
                        let pos = vertex_map.len() as u32;
                        vertex_map.push(idx);
                        pos
                    };
                    face_indices.push(vi);
                }
                // Triangulate (fan from first vertex)
                for i in 1..face_indices.len() - 1 {
                    indices.push(face_indices[0]);
                    indices.push(face_indices[i]);
                    indices.push(face_indices[i + 1]);
                }
            }
            // Skip unknown directives gracefully
            _ => {}
        }
    }

    // Build final vertex buffer
    let vertices: Vec<TessVertex> = vertex_map
        .iter()
        .map(|&(pi, ni, ti)| {
            let position = positions[pi];
            let normal = ni.map(|i| normals[i]).unwrap_or([0.0, 0.0, 0.0]);
            let uv = ti.map(|i| uvs[i]).unwrap_or([0.0, 0.0]);
            TessVertex { position, normal, uv }
        })
        .collect();

    Some(TessMesh { vertices, indices })
}

/// Parse a single face vertex token like `1/2/3`, `1//3`, `1/2`, or `1`.
/// Returns `(position_index, normal_index, uv_index)` as 0-based.
fn parse_face_vertex(
    token: &str,
    num_pos: usize,
    num_norm: usize,
    num_uv: usize,
) -> Option<(usize, Option<usize>, Option<usize>)> {
    let parts: Vec<&str> = token.split('/').collect();
    match parts.len() {
        1 => {
            let pi = resolve_index(parts[0].parse::<i64>().ok()?, num_pos)?;
            Some((pi, None, None))
        }
        2 => {
            let pi = resolve_index(parts[0].parse::<i64>().ok()?, num_pos)?;
            let ti = resolve_index(parts[1].parse::<i64>().ok()?, num_uv)?;
            Some((pi, None, Some(ti)))
        }
        3 => {
            let pi = resolve_index(parts[0].parse::<i64>().ok()?, num_pos)?;
            let ni = if parts[1].is_empty() {
                // v//vn format
                None
            } else {
                Some(resolve_index(parts[1].parse::<i64>().ok()?, num_uv)?)
            };
            let norm_i = resolve_index(parts[2].parse::<i64>().ok()?, num_norm)?;
            Some((pi, Some(norm_i), ni))
        }
        _ => None,
    }
}

/// Convert a 1-based (possibly negative) OBJ index to a 0-based index.
fn resolve_index(idx: i64, count: usize) -> Option<usize> {
    if idx > 0 {
        let i = (idx - 1) as usize;
        if i < count { Some(i) } else { None }
    } else if idx < 0 {
        let i = count as i64 + idx;
        if i >= 0 && (i as usize) < count { Some(i as usize) } else { None }
    } else {
        None // 0 is invalid in OBJ
    }
}

// ---------------------------------------------------------------------------
// MTL writing / reading
// ---------------------------------------------------------------------------

/// Write a basic MTL material file string with the given diffuse color.
pub fn write_mtl(name: &str, r: f64, g: f64, b: f64) -> String {
    let mut out = String::new();
    writeln!(out, "# physical-emit-obj material").unwrap();
    writeln!(out, "newmtl {name}").unwrap();
    writeln!(out, "Ka 0.1 0.1 0.1").unwrap();
    writeln!(out, "Kd {r} {g} {b}").unwrap();
    writeln!(out, "Ks 0.0 0.0 0.0").unwrap();
    writeln!(out, "d 1.0").unwrap();
    writeln!(out, "illum 1").unwrap();
    out
}

/// Parse an MTL string and extract the first diffuse color `Kd r g b`.
pub fn read_mtl(text: &str) -> Option<(f64, f64, f64)> {
    for line in text.lines() {
        let line = line.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "Kd" {
            let r: f64 = parts[1].parse().ok()?;
            let g: f64 = parts[2].parse().ok()?;
            let b: f64 = parts[3].parse().ok()?;
            return Some((r, g, b));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a simple box mesh (8 verts, 12 triangles).
    fn make_box_mesh() -> TessMesh {
        let min = [0.0f32, 0.0, 0.0];
        let max = [1.0f32, 1.0, 1.0];
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
            0, 2, 1, 0, 3, 2,
            4, 5, 6, 4, 6, 7,
            0, 1, 5, 0, 5, 4,
            2, 3, 7, 2, 7, 6,
            0, 4, 7, 0, 7, 3,
            1, 2, 6, 1, 6, 5,
        ];
        TessMesh { vertices, indices }
    }

    #[test]
    fn write_read_roundtrip_box() {
        let mesh = make_box_mesh();
        let obj = write_obj(&mesh, "box");
        let parsed = read_obj(&obj).expect("should parse");
        assert_eq!(parsed.vertices.len(), mesh.vertices.len());
        assert_eq!(parsed.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn write_obj_simple_valid_format() {
        let mesh = make_box_mesh();
        let obj = write_obj_simple(&mesh);
        assert!(obj.contains("v "));
        assert!(obj.contains("f "));
        // Simple format should NOT contain vn or vt
        assert!(!obj.contains("vn "));
        assert!(!obj.contains("vt "));
    }

    #[test]
    fn face_indices_are_one_based() {
        let mesh = TessMesh {
            vertices: vec![
                TessVertex { position: [0.0, 0.0, 0.0], normal: [0.0; 3], uv: [0.0; 2] },
                TessVertex { position: [1.0, 0.0, 0.0], normal: [0.0; 3], uv: [0.0; 2] },
                TessVertex { position: [0.0, 1.0, 0.0], normal: [0.0; 3], uv: [0.0; 2] },
            ],
            indices: vec![0, 1, 2],
        };
        let obj = write_obj_simple(&mesh);
        assert!(obj.contains("f 1 2 3"));
    }

    #[test]
    fn read_obj_handles_comments() {
        let obj = "# this is a comment\nv 0 0 0\nv 1 0 0\nv 0 1 0\n# another\nf 1 2 3\n";
        let mesh = read_obj(obj).expect("should parse");
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertices.len(), 3);
    }

    #[test]
    fn read_obj_negative_indices() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n";
        let mesh = read_obj(obj).expect("should parse");
        assert_eq!(mesh.triangle_count(), 1);
        // First vertex should be position [0,0,0]
        assert_eq!(mesh.vertices[mesh.indices[0] as usize].position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn read_obj_with_normals_and_uvs() {
        let obj = "\
v 0 0 0\n\
v 1 0 0\n\
v 0 1 0\n\
vn 0 0 1\n\
vt 0.5 0.5\n\
f 1/1/1 2/1/1 3/1/1\n";
        let mesh = read_obj(obj).expect("should parse");
        assert_eq!(mesh.triangle_count(), 1);
        // Check that normal and uv were applied
        assert_eq!(mesh.vertices[0].normal, [0.0, 0.0, 1.0]);
        assert_eq!(mesh.vertices[0].uv, [0.5, 0.5]);
    }

    #[test]
    fn read_obj_returns_none_for_bad_vertex() {
        let obj = "v not_a_number 0 0\nf 1 2 3\n";
        assert!(read_obj(obj).is_none());
    }

    #[test]
    fn mtl_roundtrip() {
        let mtl = write_mtl("red_plastic", 0.8, 0.1, 0.1);
        let (r, g, b) = read_mtl(&mtl).expect("should parse");
        assert!((r - 0.8).abs() < 1e-9);
        assert!((g - 0.1).abs() < 1e-9);
        assert!((b - 0.1).abs() < 1e-9);
    }

    #[test]
    fn empty_mesh_produces_valid_output() {
        let mesh = TessMesh {
            vertices: vec![],
            indices: vec![],
        };
        let obj = write_obj(&mesh, "empty");
        assert!(obj.contains("o empty"));
        // Should parse back without error
        let parsed = read_obj(&obj).expect("should parse");
        assert_eq!(parsed.vertices.len(), 0);
        assert_eq!(parsed.triangle_count(), 0);
    }

    #[test]
    fn read_multiple_objects() {
        // OBJ with two `o` lines -- we skip them and merge all geometry
        let obj = "\
o first\n\
v 0 0 0\n\
v 1 0 0\n\
v 0 1 0\n\
f 1 2 3\n\
o second\n\
v 2 0 0\n\
v 3 0 0\n\
v 2 1 0\n\
f 4 5 6\n";
        let mesh = read_obj(obj).expect("should parse");
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn read_obj_v_vn_format() {
        // f v//vn format (no texture)
        let obj = "\
v 0 0 0\n\
v 1 0 0\n\
v 0 1 0\n\
vn 0 0 1\n\
f 1//1 2//1 3//1\n";
        let mesh = read_obj(obj).expect("should parse");
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertices[0].normal, [0.0, 0.0, 1.0]);
        // UV should be default zero
        assert_eq!(mesh.vertices[0].uv, [0.0, 0.0]);
    }
}
