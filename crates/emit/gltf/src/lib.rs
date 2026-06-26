//! `physical-emit-gltf` — glTF 2.0 binary (.glb) export and import.
//!
//! Converts [`TessMesh`] into valid GLB files and parses them back.
//! Implements the format directly without external glTF libraries.

use physical_tessellation::{TessMesh, TessVertex};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// GLB constants
// ---------------------------------------------------------------------------

const GLB_MAGIC: u32 = 0x46546C67; // "glTF"
const GLB_VERSION: u32 = 2;
const GLB_HEADER_SIZE: u32 = 12;
const CHUNK_HEADER_SIZE: u32 = 8;
const CHUNK_TYPE_JSON: u32 = 0x4E4F534A;
const CHUNK_TYPE_BIN: u32 = 0x004E4942;

const COMPONENT_FLOAT: u32 = 5126;
const COMPONENT_UNSIGNED_INT: u32 = 5125;
const BUFFER_VIEW_TARGET_ARRAY: u32 = 34962;
const BUFFER_VIEW_TARGET_ELEMENT_ARRAY: u32 = 34963;

// ---------------------------------------------------------------------------
// Public material type
// ---------------------------------------------------------------------------

/// PBR metallic-roughness material for glTF export.
#[derive(Debug, Clone, Copy)]
pub struct GltfMaterial {
    /// Base color RGBA, linear. Default [1, 1, 1, 1].
    pub base_color_factor: [f32; 4],
    /// Metallic factor 0..1.
    pub metallic_factor: f32,
    /// Roughness factor 0..1.
    pub roughness_factor: f32,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 0.0,
            roughness_factor: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// JSON schema types (serde)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct GltfRoot {
    asset: GltfAsset,
    scene: u32,
    scenes: Vec<GltfScene>,
    nodes: Vec<GltfNode>,
    meshes: Vec<GltfMeshDef>,
    accessors: Vec<GltfAccessor>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<GltfBufferView>,
    buffers: Vec<GltfBuffer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    materials: Option<Vec<GltfMaterialDef>>,
}

#[derive(Serialize, Deserialize)]
struct GltfAsset {
    version: String,
    generator: String,
}

#[derive(Serialize, Deserialize)]
struct GltfScene {
    nodes: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
struct GltfNode {
    mesh: u32,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct GltfMeshDef {
    name: String,
    primitives: Vec<GltfPrimitive>,
}

#[derive(Serialize, Deserialize)]
struct GltfPrimitive {
    attributes: GltfAttributes,
    indices: u32,
    mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct GltfAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
}

#[derive(Serialize, Deserialize)]
struct GltfAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: u32,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: u32,
    #[serde(rename = "type")]
    accessor_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<Vec<f32>>,
}

#[derive(Serialize, Deserialize)]
struct GltfBufferView {
    buffer: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: u32,
    #[serde(rename = "byteLength")]
    byte_length: u32,
    #[serde(rename = "byteStride", skip_serializing_if = "Option::is_none")]
    byte_stride: Option<u32>,
    target: u32,
}

#[derive(Serialize, Deserialize)]
struct GltfBuffer {
    #[serde(rename = "byteLength")]
    byte_length: u32,
}

#[derive(Serialize, Deserialize)]
struct GltfMaterialDef {
    name: String,
    #[serde(rename = "pbrMetallicRoughness")]
    pbr: GltfPbr,
}

#[derive(Serialize, Deserialize)]
struct GltfPbr {
    #[serde(rename = "baseColorFactor")]
    base_color_factor: [f32; 4],
    #[serde(rename = "metallicFactor")]
    metallic_factor: f32,
    #[serde(rename = "roughnessFactor")]
    roughness_factor: f32,
}

// ---------------------------------------------------------------------------
// Write helpers
// ---------------------------------------------------------------------------

fn build_bin_buffer(mesh: &TessMesh) -> Vec<u8> {
    let vertex_stride = 6; // 3 position + 3 normal (f32 each)
    let vertex_byte_len = mesh.vertices.len() * vertex_stride * 4;
    let index_byte_len = mesh.indices.len() * 4;
    let total = vertex_byte_len + index_byte_len;

    let mut buf = Vec::with_capacity(total);
    for v in &mesh.vertices {
        for &c in &v.position {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        for &c in &v.normal {
            buf.extend_from_slice(&c.to_le_bytes());
        }
    }
    for &idx in &mesh.indices {
        buf.extend_from_slice(&idx.to_le_bytes());
    }
    buf
}

fn build_json(mesh: &TessMesh, name: &str, material: Option<GltfMaterial>) -> String {
    let vertex_count = mesh.vertices.len() as u32;
    let index_count = mesh.indices.len() as u32;
    let vertex_stride: u32 = 24; // 6 * f32
    let vertex_byte_len = vertex_count * vertex_stride;
    let index_byte_len = index_count * 4;
    let total_bin = vertex_byte_len + index_byte_len;

    // Compute position min/max
    let (pos_min, pos_max) = if mesh.vertices.is_empty() {
        ([0.0_f32; 3], [0.0_f32; 3])
    } else {
        mesh.bounding_box()
    };

    let materials_def = material.map(|m| {
        vec![GltfMaterialDef {
            name: format!("{name}_material"),
            pbr: GltfPbr {
                base_color_factor: m.base_color_factor,
                metallic_factor: m.metallic_factor,
                roughness_factor: m.roughness_factor,
            },
        }]
    });

    let root = GltfRoot {
        asset: GltfAsset {
            version: "2.0".into(),
            generator: "physical-emit-gltf".into(),
        },
        scene: 0,
        scenes: vec![GltfScene { nodes: vec![0] }],
        nodes: vec![GltfNode {
            mesh: 0,
            name: name.into(),
        }],
        meshes: vec![GltfMeshDef {
            name: name.into(),
            primitives: vec![GltfPrimitive {
                attributes: GltfAttributes {
                    position: 0,
                    normal: 1,
                },
                indices: 2,
                mode: 4, // TRIANGLES
                material: if materials_def.is_some() {
                    Some(0)
                } else {
                    None
                },
            }],
        }],
        accessors: vec![
            // 0: position
            GltfAccessor {
                buffer_view: 0,
                byte_offset: 0,
                component_type: COMPONENT_FLOAT,
                count: vertex_count,
                accessor_type: "VEC3".into(),
                min: Some(pos_min.to_vec()),
                max: Some(pos_max.to_vec()),
            },
            // 1: normal
            GltfAccessor {
                buffer_view: 0,
                byte_offset: 12, // after 3 floats of position
                component_type: COMPONENT_FLOAT,
                count: vertex_count,
                accessor_type: "VEC3".into(),
                min: None,
                max: None,
            },
            // 2: indices
            GltfAccessor {
                buffer_view: 1,
                byte_offset: 0,
                component_type: COMPONENT_UNSIGNED_INT,
                count: index_count,
                accessor_type: "SCALAR".into(),
                min: None,
                max: None,
            },
        ],
        buffer_views: vec![
            // 0: interleaved vertex data
            GltfBufferView {
                buffer: 0,
                byte_offset: 0,
                byte_length: vertex_byte_len,
                byte_stride: Some(vertex_stride),
                target: BUFFER_VIEW_TARGET_ARRAY,
            },
            // 1: index data
            GltfBufferView {
                buffer: 0,
                byte_offset: vertex_byte_len,
                byte_length: index_byte_len,
                target: BUFFER_VIEW_TARGET_ELEMENT_ARRAY,
                byte_stride: None,
            },
        ],
        buffers: vec![GltfBuffer {
            byte_length: total_bin,
        }],
        materials: materials_def,
    };

    serde_json::to_string(&root).expect("JSON serialization failed")
}

/// Pad `len` up to the next multiple of 4.
fn align4(len: u32) -> u32 {
    (len + 3) & !3
}

fn assemble_glb(json_bytes: &[u8], bin_bytes: &[u8]) -> Vec<u8> {
    let json_padded_len = align4(json_bytes.len() as u32);
    let bin_padded_len = align4(bin_bytes.len() as u32);
    let total_len =
        GLB_HEADER_SIZE + CHUNK_HEADER_SIZE + json_padded_len + CHUNK_HEADER_SIZE + bin_padded_len;

    let mut out = Vec::with_capacity(total_len as usize);

    // GLB header
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&total_len.to_le_bytes());

    // JSON chunk
    out.extend_from_slice(&json_padded_len.to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
    out.extend_from_slice(json_bytes);
    // Pad with spaces (0x20) per spec
    for _ in 0..(json_padded_len as usize - json_bytes.len()) {
        out.push(0x20);
    }

    // BIN chunk
    out.extend_from_slice(&bin_padded_len.to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
    out.extend_from_slice(bin_bytes);
    // Pad with zeros per spec
    for _ in 0..(bin_padded_len as usize - bin_bytes.len()) {
        out.push(0x00);
    }

    out
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Write a complete GLB binary from a tessellated mesh.
pub fn write_glb(mesh: &TessMesh, name: &str) -> Vec<u8> {
    let json_str = build_json(mesh, name, None);
    let bin = build_bin_buffer(mesh);
    assemble_glb(json_str.as_bytes(), &bin)
}

/// Write a GLB binary with PBR metallic-roughness material.
pub fn write_glb_with_material(mesh: &TessMesh, name: &str, material: GltfMaterial) -> Vec<u8> {
    let json_str = build_json(mesh, name, Some(material));
    let bin = build_bin_buffer(mesh);
    assemble_glb(json_str.as_bytes(), &bin)
}

/// Parse a GLB binary back into a [`TessMesh`].
///
/// Returns `None` if the data is invalid or cannot be parsed.
pub fn read_glb(data: &[u8]) -> Option<TessMesh> {
    if data.len() < GLB_HEADER_SIZE as usize {
        return None;
    }

    let magic = u32::from_le_bytes(data[0..4].try_into().ok()?);
    let version = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let total_len = u32::from_le_bytes(data[8..12].try_into().ok()?);

    if magic != GLB_MAGIC || version != GLB_VERSION || (total_len as usize) > data.len() {
        return None;
    }

    // Parse JSON chunk
    let mut offset = GLB_HEADER_SIZE as usize;
    if offset + 8 > data.len() {
        return None;
    }
    let json_len = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
    let json_type = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
    if json_type != CHUNK_TYPE_JSON {
        return None;
    }
    offset += 8;
    if offset + json_len > data.len() {
        return None;
    }
    let json_bytes = &data[offset..offset + json_len];
    offset += json_len;

    // Parse BIN chunk
    if offset + 8 > data.len() {
        return None;
    }
    let bin_len = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
    let bin_type = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
    if bin_type != CHUNK_TYPE_BIN {
        return None;
    }
    offset += 8;
    if offset + bin_len > data.len() {
        return None;
    }
    let bin_data = &data[offset..offset + bin_len];

    // Parse JSON
    let root: GltfRoot = serde_json::from_slice(json_bytes).ok()?;

    // Get the first primitive
    let primitive = root.meshes.first()?.primitives.first()?;
    let pos_accessor_idx = primitive.attributes.position as usize;
    let norm_accessor_idx = primitive.attributes.normal as usize;
    let idx_accessor_idx = primitive.indices as usize;

    let pos_accessor = root.accessors.get(pos_accessor_idx)?;
    let norm_accessor = root.accessors.get(norm_accessor_idx)?;
    let idx_accessor = root.accessors.get(idx_accessor_idx)?;

    let pos_bv = root.buffer_views.get(pos_accessor.buffer_view as usize)?;
    let idx_bv = root.buffer_views.get(idx_accessor.buffer_view as usize)?;

    let vertex_count = pos_accessor.count as usize;
    let index_count = idx_accessor.count as usize;
    let stride = pos_bv.byte_stride.unwrap_or(24) as usize;

    // Read vertices
    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let base = pos_bv.byte_offset as usize + i * stride + pos_accessor.byte_offset as usize;
        let norm_base =
            pos_bv.byte_offset as usize + i * stride + norm_accessor.byte_offset as usize;

        if base + 12 > bin_data.len() || norm_base + 12 > bin_data.len() {
            return None;
        }

        let px = f32::from_le_bytes(bin_data[base..base + 4].try_into().ok()?);
        let py = f32::from_le_bytes(bin_data[base + 4..base + 8].try_into().ok()?);
        let pz = f32::from_le_bytes(bin_data[base + 8..base + 12].try_into().ok()?);

        let nx = f32::from_le_bytes(bin_data[norm_base..norm_base + 4].try_into().ok()?);
        let ny = f32::from_le_bytes(bin_data[norm_base + 4..norm_base + 8].try_into().ok()?);
        let nz = f32::from_le_bytes(bin_data[norm_base + 8..norm_base + 12].try_into().ok()?);

        vertices.push(TessVertex {
            position: [px, py, pz],
            normal: [nx, ny, nz],
            uv: [0.0, 0.0], // UV not stored in our format
        });
    }

    // Read indices
    let mut indices = Vec::with_capacity(index_count);
    let idx_base = idx_bv.byte_offset as usize + idx_accessor.byte_offset as usize;
    for i in 0..index_count {
        let off = idx_base + i * 4;
        if off + 4 > bin_data.len() {
            return None;
        }
        indices.push(u32::from_le_bytes(bin_data[off..off + 4].try_into().ok()?));
    }

    Some(TessMesh { vertices, indices })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mesh() -> TessMesh {
        TessMesh {
            vertices: vec![
                TessVertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                },
                TessVertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                },
                TessVertex {
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn roundtrip_preserves_counts() {
        let mesh = sample_mesh();
        let glb = write_glb(&mesh, "triangle");
        let parsed = read_glb(&glb).expect("should parse");
        assert_eq!(parsed.vertices.len(), mesh.vertices.len());
        assert_eq!(parsed.indices.len(), mesh.indices.len());
    }

    #[test]
    fn roundtrip_preserves_data() {
        let mesh = sample_mesh();
        let glb = write_glb(&mesh, "triangle");
        let parsed = read_glb(&glb).unwrap();
        for (orig, rt) in mesh.vertices.iter().zip(parsed.vertices.iter()) {
            for i in 0..3 {
                assert!(
                    (orig.position[i] - rt.position[i]).abs() < f32::EPSILON,
                    "position mismatch"
                );
                assert!(
                    (orig.normal[i] - rt.normal[i]).abs() < f32::EPSILON,
                    "normal mismatch"
                );
            }
        }
        assert_eq!(mesh.indices, parsed.indices);
    }

    #[test]
    fn glb_header_validation() {
        let mesh = sample_mesh();
        let glb = write_glb(&mesh, "test");

        // Check magic
        let magic = u32::from_le_bytes(glb[0..4].try_into().unwrap());
        assert_eq!(magic, GLB_MAGIC);

        // Check version
        let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
        assert_eq!(version, 2);

        // Check total length matches actual data
        let total_len = u32::from_le_bytes(glb[8..12].try_into().unwrap());
        assert_eq!(total_len as usize, glb.len());
    }

    #[test]
    fn material_export_produces_valid_json() {
        let mesh = sample_mesh();
        let mat = GltfMaterial {
            base_color_factor: [0.8, 0.2, 0.1, 1.0],
            metallic_factor: 0.9,
            roughness_factor: 0.3,
        };
        let glb = write_glb_with_material(&mesh, "colored", mat);

        // Extract JSON chunk
        let json_len =
            u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json_str =
            std::str::from_utf8(&glb[20..20 + json_len]).unwrap().trim();
        let root: serde_json::Value = serde_json::from_str(json_str).unwrap();

        let materials = root["materials"].as_array().expect("materials array");
        assert_eq!(materials.len(), 1);

        let pbr = &materials[0]["pbrMetallicRoughness"];
        let bcf = pbr["baseColorFactor"].as_array().unwrap();
        assert!((bcf[0].as_f64().unwrap() - 0.8).abs() < 1e-6);
        assert!((pbr["metallicFactor"].as_f64().unwrap() - 0.9).abs() < 1e-6);
        assert!((pbr["roughnessFactor"].as_f64().unwrap() - 0.3).abs() < 1e-6);
    }

    #[test]
    fn empty_mesh_handling() {
        let mesh = TessMesh {
            vertices: vec![],
            indices: vec![],
        };
        let glb = write_glb(&mesh, "empty");

        // Should still produce a valid GLB
        let magic = u32::from_le_bytes(glb[0..4].try_into().unwrap());
        assert_eq!(magic, GLB_MAGIC);

        // Roundtrip
        let parsed = read_glb(&glb).expect("should parse empty mesh");
        assert_eq!(parsed.vertices.len(), 0);
        assert_eq!(parsed.indices.len(), 0);
    }

    #[test]
    fn invalid_data_returns_none() {
        assert!(read_glb(&[]).is_none());
        assert!(read_glb(&[0u8; 12]).is_none());
        assert!(read_glb(b"not a glb file at all").is_none());
    }

    #[test]
    fn box_mesh_via_tessellation() {
        let solid = physical_brep::make_box(10.0, 20.0, 30.0);
        let mesh = physical_tessellation::tessellate(&solid, 1.0);
        // Real tessellator: 6 planar faces with ear-clip triangulation
        assert!(mesh.triangle_count() >= 12, "box needs >= 12 tris, got {}", mesh.triangle_count());

        let glb = write_glb(&mesh, "box");
        let parsed = read_glb(&glb).expect("should parse box");
        assert!(parsed.vertices.len() >= 8, "need >= 8 vertices");
        assert!(parsed.triangle_count() >= 12, "need >= 12 triangles");
    }

    #[test]
    fn material_roundtrip_still_parses() {
        let mesh = sample_mesh();
        let mat = GltfMaterial::default();
        let glb = write_glb_with_material(&mesh, "mat_test", mat);
        let parsed = read_glb(&glb).expect("material GLB should parse");
        assert_eq!(parsed.vertices.len(), 3);
    }

    #[test]
    fn json_chunk_padded_to_4_bytes() {
        let mesh = sample_mesh();
        let glb = write_glb(&mesh, "a"); // short name to test padding

        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap());
        assert_eq!(json_len % 4, 0, "JSON chunk length must be 4-byte aligned");
    }
}
