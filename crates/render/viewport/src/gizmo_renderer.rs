//! GPU renderer for the transform gizmo.
//!
//! Takes generated gizmo geometry and renders it as unlit colored mesh
//! on top of the scene (depth biased to always appear in front).

use bytemuck::{cast_slice, Pod, Zeroable};
use wgpu::*;

use crate::gizmo::{Gizmo, GizmoAxis};

const GIZMO_WGSL: &str = include_str!("shaders/gizmo.wgsl");

/// Vertex format for gizmo: position + color (no normals/UVs needed).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GizmoVertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl GizmoVertex {
    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<GizmoVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GizmoUniforms {
    view_proj: [[f32; 4]; 4],
}

const MAX_GIZMO_VERTS: usize = 4096;
const MAX_GIZMO_INDICES: usize = 8192;

/// Renders gizmo geometry on top of the scene.
pub struct GizmoRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl GizmoRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("gizmo-shader"),
            source: ShaderSource::Wgsl(GIZMO_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("gizmo-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(std::mem::size_of::<GizmoUniforms>() as u64),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("gizmo-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("gizmo-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[GizmoVertex::layout()],
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
                cull_mode: None, // No culling for gizmo — visible from all angles
                ..Default::default()
            },
            depth_stencil: None, // No depth test — always on top
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gizmo-ub"),
            size: std::mem::size_of::<GizmoUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gizmo-vb"),
            size: (MAX_GIZMO_VERTS * std::mem::size_of::<GizmoVertex>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gizmo-ib"),
            size: (MAX_GIZMO_INDICES * std::mem::size_of::<u32>()) as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
        }
    }

    /// Render the gizmo for the current frame.
    pub fn render(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        view_proj: glam::Mat4,
        gizmo: &Gizmo,
    ) {
        if !gizmo.visible {
            return;
        }

        // Generate geometry
        let (src_verts, src_indices) = gizmo.translate_geometry();
        if src_verts.is_empty() {
            return;
        }

        // Assign colors per-axis to the vertices
        let axis_colors = [
            gizmo_axis_color(GizmoAxis::X, gizmo),
            gizmo_axis_color(GizmoAxis::Y, gizmo),
            gizmo_axis_color(GizmoAxis::Z, gizmo),
        ];

        let segments = 8u32;
        let shaft_tris = segments * 6; // quads = segments * 2 triangles * 3 indices? No, segments * 6 indices
        let head_tris = segments * 3;
        let _indices_per_axis = (shaft_tris + head_tris) as usize;
        let verts_per_axis_shaft = (segments * 4) as usize;
        let verts_per_axis_head = (segments * 3) as usize;
        let verts_per_axis = verts_per_axis_shaft + verts_per_axis_head;

        let mut gpu_verts: Vec<GizmoVertex> = Vec::with_capacity(src_verts.len());
        for (i, v) in src_verts.iter().enumerate() {
            let axis_idx = i / verts_per_axis;
            let color = if axis_idx < 3 { axis_colors[axis_idx] } else { [0.5, 0.5, 0.5, 1.0] };
            gpu_verts.push(GizmoVertex {
                position: v.position,
                color,
            });
        }

        let num_verts = gpu_verts.len().min(MAX_GIZMO_VERTS);
        let num_indices = src_indices.len().min(MAX_GIZMO_INDICES);

        // Upload
        let uniforms = GizmoUniforms {
            view_proj: view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));
        queue.write_buffer(&self.vertex_buffer, 0, cast_slice(&gpu_verts[..num_verts]));
        queue.write_buffer(&self.index_buffer, 0, cast_slice(&src_indices[..num_indices]));

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("gizmo-bg"),
            layout: &self.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("gizmo-pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..num_indices as u32, 0, 0..1);
    }
}

fn gizmo_axis_color(axis: GizmoAxis, gizmo: &Gizmo) -> [f32; 4] {
    let hover = [1.0, 0.9, 0.3, 1.0];
    if gizmo.hovered_axis == axis || gizmo.dragging_axis == axis {
        return hover;
    }
    match axis {
        GizmoAxis::X => [0.9, 0.2, 0.2, 1.0],
        GizmoAxis::Y => [0.2, 0.8, 0.2, 1.0],
        GizmoAxis::Z => [0.3, 0.4, 0.95, 1.0],
        GizmoAxis::None => [0.5, 0.5, 0.5, 1.0],
    }
}
