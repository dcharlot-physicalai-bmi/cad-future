//! Selection outline renderer — silhouette-based outlines for selected objects.
//!
//! Renders selected objects inflated along normals with front-face culling,
//! creating a visible outline "halo" around selection edges. Depth-tested
//! against the main scene to properly occlude behind other geometry.

use bytemuck::{cast_slice, Pod, Zeroable};
use glam::Mat4;
use wgpu::*;

use crate::mesh_registry::MeshRegistry;
use crate::scene::RenderObject;
use crate::vertex::Vertex;

const OUTLINE_WGSL: &str = include_str!("shaders/outline.wgsl");

/// 256-byte aligned stride for per-object dynamic offsets.
const OBJECT_ALIGNED: usize = 256;

const MAX_SELECTED: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OutlineUniforms {
    view_proj: [[f32; 4]; 4],
    outline_color: [f32; 4],
    thickness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectData {
    model: [[f32; 4]; 4],
    normal_mat: [[f32; 4]; 4],
}

/// Outline renderer for selected objects.
pub struct OutlineRenderer {
    pipeline: RenderPipeline,
    outline_ub: Buffer,
    outline_bg: BindGroup,
    object_ub: Buffer,
    object_bg: BindGroup,
}

impl OutlineRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("outline-shader"),
            source: ShaderSource::Wgsl(OUTLINE_WGSL.into()),
        });

        // Bind group 0: outline uniforms (view_proj, color, thickness)
        let outline_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("outline-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(
                        std::mem::size_of::<OutlineUniforms>() as u64,
                    ),
                },
                count: None,
            }],
        });

        // Bind group 1: per-object (dynamic offset)
        let object_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("outline-obj-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(
                        std::mem::size_of::<ObjectData>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("outline-pipeline-layout"),
            bind_group_layouts: &[Some(&outline_bgl), Some(&object_bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("outline-pipeline"),
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
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                // Key: cull FRONT faces so only the inflated back faces show as outline
                cull_mode: Some(Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                // Don't write depth — outline is cosmetic
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Buffers
        let outline_ub = device.create_buffer(&BufferDescriptor {
            label: Some("outline-ub"),
            size: std::mem::size_of::<OutlineUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let outline_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline-bg"),
            layout: &outline_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: outline_ub.as_entire_binding(),
            }],
        });

        let object_ub = device.create_buffer(&BufferDescriptor {
            label: Some("outline-obj-ub"),
            size: (OBJECT_ALIGNED * MAX_SELECTED) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline-obj-bg"),
            layout: &object_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &object_ub,
                    offset: 0,
                    size: BufferSize::new(std::mem::size_of::<ObjectData>() as u64),
                }),
            }],
        });

        Self {
            pipeline,
            outline_ub,
            outline_bg,
            object_ub,
            object_bg,
        }
    }

    /// Render outlines for selected objects.
    ///
    /// `selected_indices` — scene indices of selected objects.
    /// `objects` — full render object list (used to find selected by object_id).
    /// `depth_view` — the forward renderer's depth buffer for occlusion.
    pub fn render(
        &self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        objects: &[RenderObject],
        mesh_registry: &MeshRegistry,
        selected_ids: &[u32],
    ) {
        if selected_ids.is_empty() {
            return;
        }

        // Collect selected render objects
        let selected_objs: Vec<&RenderObject> = objects
            .iter()
            .filter(|o| selected_ids.contains(&o.object_id))
            .collect();

        if selected_objs.is_empty() {
            return;
        }

        let num = selected_objs.len().min(MAX_SELECTED);

        // Upload outline uniforms
        let uni = OutlineUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            outline_color: [0.25, 0.6, 1.0, 0.85], // bright selection blue
            thickness: 0.03,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        queue.write_buffer(&self.outline_ub, 0, cast_slice(&[uni]));

        // Upload per-object data
        let mut buf = vec![0u8; OBJECT_ALIGNED * num];
        for (i, obj) in selected_objs[..num].iter().enumerate() {
            let normal_mat = obj.transform.inverse().transpose();
            let data = ObjectData {
                model: obj.transform.to_cols_array_2d(),
                normal_mat: normal_mat.to_cols_array_2d(),
            };
            let data_arr = [data];
            let bytes = cast_slice(&data_arr);
            let offset = i * OBJECT_ALIGNED;
            buf[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        queue.write_buffer(&self.object_ub, 0, &buf);

        // Render pass
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("outline-pass"),
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
                        load: LoadOp::Load, // use existing depth
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.outline_bg, &[]);

            for (i, obj) in selected_objs[..num].iter().enumerate() {
                let dyn_offset = (i * OBJECT_ALIGNED) as u32;
                pass.set_bind_group(1, &self.object_bg, &[dyn_offset]);

                if let Some((vb, ib, ic)) = mesh_registry.get(obj.mesh_id) {
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), IndexFormat::Uint32);
                    pass.draw_indexed(0..ic, 0, 0..1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_uniforms_size() {
        assert_eq!(std::mem::size_of::<OutlineUniforms>(), 96);
    }

    #[test]
    fn object_data_fits_alignment() {
        assert!(std::mem::size_of::<ObjectData>() <= OBJECT_ALIGNED);
    }
}
