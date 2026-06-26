//! Vertex types for 3D mesh rendering.

use bytemuck::{Pod, Zeroable};

/// Standard vertex for 3D meshes: position + normal + uv = 32 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn vertex_size_is_32_bytes() {
        assert_eq!(mem::size_of::<Vertex>(), 32);
    }

    #[test]
    fn vertex_alignment_is_4() {
        assert_eq!(mem::align_of::<Vertex>(), 4);
    }

    #[test]
    fn vertex_bytemuck_roundtrip() {
        let v = Vertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.5, 0.75],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 32);
        let v2: &Vertex = bytemuck::from_bytes(bytes);
        assert_eq!(v2.position, v.position);
        assert_eq!(v2.normal, v.normal);
        assert_eq!(v2.uv, v.uv);
    }

    #[test]
    fn vertex_zeroable() {
        let v = Vertex::zeroed();
        assert_eq!(v.position, [0.0; 3]);
        assert_eq!(v.normal, [0.0; 3]);
        assert_eq!(v.uv, [0.0; 2]);
    }

    #[test]
    fn vertex_attribute_offsets_are_correct() {
        let v = Vertex::zeroed();
        let base = &v as *const Vertex as usize;
        let pos_offset = &v.position as *const [f32; 3] as usize - base;
        assert_eq!(pos_offset, 0);
        let normal_offset = &v.normal as *const [f32; 3] as usize - base;
        assert_eq!(normal_offset, 12);
        let uv_offset = &v.uv as *const [f32; 2] as usize - base;
        assert_eq!(uv_offset, 24);
    }
}
