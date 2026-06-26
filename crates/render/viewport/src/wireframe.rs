//! Wireframe overlay renderer.
//!
//! Renders mesh edges as lines on top of the solid forward pass.
//! Uses `LineList` topology with edge indices extracted from triangle meshes.

use bytemuck::cast_slice;
use wgpu::*;

use crate::forward::ForwardFrameInput;
use crate::mesh_registry::MeshRegistry;
use crate::scene::RenderObject;
use crate::material::MaterialStore;
use crate::vertex::Vertex;

const WIREFRAME_WGSL: &str = include_str!("shaders/wireframe.wgsl");

/// Maximum objects per draw batch (matches forward renderer).
const MAX_OBJECTS: usize = 256;
const OBJECT_UNIFORM_ALIGNED: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _pad0: f32,
    sun_dir: [f32; 3],
    sun_intensity: f32,
    sun_color: [f32; 3],
    ambient: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniforms {
    model: [[f32; 4]; 4],
    normal_mat: [[f32; 4]; 4],
    albedo: [f32; 4],
    roughness: f32,
    metallic: f32,
    selected: f32,
    _pad1: f32,
}

/// Wireframe overlay renderer — draws edges as lines.
pub struct WireframeRenderer {
    pipeline: RenderPipeline,
    frame_ub: Buffer,
    frame_bg: BindGroup,
    object_ub: Buffer,
    object_bg: BindGroup,
}

impl WireframeRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("wireframe-shader"),
            source: ShaderSource::Wgsl(WIREFRAME_WGSL.into()),
        });

        let frame_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("wire-frame-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(
                        std::mem::size_of::<FrameUniforms>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let object_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("wire-object-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(
                        std::mem::size_of::<ObjectUniforms>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("wire-layout"),
            bind_group_layouts: &[Some(&frame_bgl), Some(&object_bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("wireframe-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false), // Don't write depth — overlay only
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let frame_ub = device.create_buffer(&BufferDescriptor {
            label: Some("wire-frame-ub"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("wire-frame-bg"),
            layout: &frame_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: frame_ub.as_entire_binding(),
            }],
        });

        let object_ub = device.create_buffer(&BufferDescriptor {
            label: Some("wire-object-ub"),
            size: (OBJECT_UNIFORM_ALIGNED * MAX_OBJECTS) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("wire-object-bg"),
            layout: &object_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &object_ub,
                    offset: 0,
                    size: BufferSize::new(std::mem::size_of::<ObjectUniforms>() as u64),
                }),
            }],
        });

        Self {
            pipeline,
            frame_ub,
            frame_bg,
            object_ub,
            object_bg,
        }
    }

    /// Render wireframe overlay for all objects.
    pub fn render(
        &self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        input: &ForwardFrameInput,
        objects: &[RenderObject],
        mesh_registry: &MeshRegistry,
        materials: &MaterialStore,
        selected_object_id: Option<u32>,
    ) {
        if objects.is_empty() {
            return;
        }

        // Upload frame uniforms
        let frame_uni = FrameUniforms {
            view: input.view.to_cols_array_2d(),
            proj: input.proj.to_cols_array_2d(),
            camera_pos: input.camera_pos.into(),
            _pad0: 0.0,
            sun_dir: input.sun_dir,
            sun_intensity: input.sun_intensity,
            sun_color: input.sun_color,
            ambient: input.ambient,
        };
        queue.write_buffer(&self.frame_ub, 0, cast_slice(&[frame_uni]));

        // Upload object uniforms
        let num_objects = objects.len().min(MAX_OBJECTS);
        let mut buf = vec![0u8; OBJECT_UNIFORM_ALIGNED * num_objects];

        for (i, obj) in objects[..num_objects].iter().enumerate() {
            let mat = materials
                .get(obj.material_id)
                .unwrap_or_else(|| materials.get(materials.default_id()).expect("default material"));

            let normal_mat = obj.transform.inverse().transpose();
            let is_selected = selected_object_id.map_or(0.0, |sid| if obj.object_id == sid { 1.0 } else { 0.0 });

            let uniforms = ObjectUniforms {
                model: obj.transform.to_cols_array_2d(),
                normal_mat: normal_mat.to_cols_array_2d(),
                albedo: mat.base_color,
                roughness: mat.roughness,
                metallic: mat.metallic,
                selected: is_selected,
                _pad1: 0.0,
            };

            let uni_arr = [uniforms];
            let bytes = cast_slice(&uni_arr);
            let offset = i * OBJECT_UNIFORM_ALIGNED;
            buf[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        queue.write_buffer(&self.object_ub, 0, &buf);

        // Render pass — line list, reading depth from forward pass
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("wireframe-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load, // Read existing depth
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame_bg, &[]);

            for (i, obj) in objects[..num_objects].iter().enumerate() {
                let dynamic_offset = (i * OBJECT_UNIFORM_ALIGNED) as u32;
                pass.set_bind_group(1, &self.object_bg, &[dynamic_offset]);

                if let Some((vb, edge_ib, edge_count)) = mesh_registry.get_edges(obj.mesh_id) {
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(edge_ib.slice(..), IndexFormat::Uint32);
                    pass.draw_indexed(0..edge_count, 0, 0..1);
                }
            }
        }
    }
}

/// Extract unique edges from a triangle index list.
/// Returns pairs of vertex indices suitable for `LineList` topology.
pub fn triangles_to_edges(indices: &[u32]) -> Vec<u32> {
    use std::collections::HashSet;

    let mut edge_set: HashSet<(u32, u32)> = HashSet::new();
    let mut edges = Vec::new();

    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        for &(u, v) in &[(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            if edge_set.insert(key) {
                edges.push(u);
                edges.push(v);
            }
        }
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_triangle_edges() {
        let indices = vec![0, 1, 2];
        let edges = triangles_to_edges(&indices);
        assert_eq!(edges.len(), 6); // 3 edges * 2 indices
    }

    #[test]
    fn shared_edge_deduplicated() {
        // Two triangles sharing edge 1-2
        let indices = vec![0, 1, 2, 2, 1, 3];
        let edges = triangles_to_edges(&indices);
        // 5 unique edges: 0-1, 1-2, 2-0, 2-3, 1-3
        assert_eq!(edges.len(), 10);
    }

    #[test]
    fn empty_indices() {
        let edges = triangles_to_edges(&[]);
        assert!(edges.is_empty());
    }
}
