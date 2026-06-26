//! Forward renderer for the CAD viewport.
//!
//! Single-pass Blinn-Phong with one directional light, depth buffer,
//! and per-object dynamic uniforms. No shadows, no post-processing.

use bytemuck::{cast_slice, Pod, Zeroable};
use wgpu::*;

use crate::material::MaterialStore;
use crate::mesh_registry::MeshRegistry;
use crate::scene::RenderObject;
use crate::vertex::Vertex;

const FORWARD_WGSL: &str = include_str!("shaders/forward.wgsl");

/// Maximum objects per draw batch.
const MAX_OBJECTS: usize = 256;

/// 256-byte aligned stride for dynamic uniform offsets.
const OBJECT_UNIFORM_ALIGNED: usize = 256;

// ── Uniform types ────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniforms {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _pad0: f32,
    sun_dir: [f32; 3],
    sun_intensity: f32,
    sun_color: [f32; 3],
    ambient: f32,
    clip_plane: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniforms {
    model: [[f32; 4]; 4],
    normal_mat: [[f32; 4]; 4],
    albedo: [f32; 4],
    roughness: f32,
    metallic: f32,
    selected: f32,
    _pad1: f32,
}

// ── Public configuration ─────────────────────────────────────────────────

/// Per-frame rendering input.
pub struct ForwardFrameInput {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub camera_pos: glam::Vec3,
    pub sun_dir: [f32; 3],
    pub sun_intensity: f32,
    pub sun_color: [f32; 3],
    pub ambient: f32,
    /// Clip plane equation [A, B, C, D]. All zeros = disabled.
    pub clip_plane: [f32; 4],
}

impl Default for ForwardFrameInput {
    fn default() -> Self {
        Self {
            view: glam::Mat4::IDENTITY,
            proj: glam::Mat4::IDENTITY,
            camera_pos: glam::Vec3::ZERO,
            sun_dir: [-0.3, -1.0, -0.5],
            sun_intensity: 1.2,
            sun_color: [1.0, 0.98, 0.95],
            ambient: 0.15,
            clip_plane: [0.0; 4],
        }
    }
}

// ── Forward Renderer ─────────────────────────────────────────────────────

/// Single-pass forward renderer for the CAD viewport.
pub struct ForwardRenderer {
    pipeline: RenderPipeline,
    frame_ub: Buffer,
    frame_bg: BindGroup,
    object_ub: Buffer,
    object_bg: BindGroup,
    depth_texture: Texture,
    depth_view: TextureView,
    width: u32,
    height: u32,
}

impl ForwardRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("forward-shader"),
            source: ShaderSource::Wgsl(FORWARD_WGSL.into()),
        });

        // ── Bind group layouts ───────────────────────────────────────────

        let frame_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("fwd-frame-bgl"),
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
            label: Some("fwd-object-bgl"),
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
            label: Some("fwd-pipeline-layout"),
            bind_group_layouts: &[Some(&frame_bgl), Some(&object_bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("forward-pipeline"),
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
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // ── Uniform buffers ──────────────────────────────────────────────

        let frame_ub = device.create_buffer(&BufferDescriptor {
            label: Some("fwd-frame-ub"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("fwd-frame-bg"),
            layout: &frame_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: frame_ub.as_entire_binding(),
            }],
        });

        let object_ub = device.create_buffer(&BufferDescriptor {
            label: Some("fwd-object-ub"),
            size: (OBJECT_UNIFORM_ALIGNED * MAX_OBJECTS) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("fwd-object-bg"),
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

        let depth_texture = Self::create_depth_target(device, width, height);
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

        Self {
            pipeline,
            frame_ub,
            frame_bg,
            object_ub,
            object_bg,
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    /// Resize depth buffer after a viewport resize.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.depth_texture = Self::create_depth_target(device, width, height);
        self.depth_view = self.depth_texture.create_view(&TextureViewDescriptor::default());
    }

    /// Render scene objects into the provided render pass.
    /// The caller is responsible for the clear pass and grid rendering.
    /// This renders meshes with depth testing on top of the existing framebuffer.
    pub fn render(
        &self,
        _device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        input: &ForwardFrameInput,
        objects: &[RenderObject],
        mesh_registry: &MeshRegistry,
        materials: &MaterialStore,
    ) {
        self.render_with_selection(_device, queue, encoder, output_view, input, objects, mesh_registry, materials, None);
    }

    /// Render scene objects with optional selection highlight.
    pub fn render_with_selection(
        &self,
        _device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
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
            clip_plane: input.clip_plane,
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

        // Render pass with depth
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("forward-color-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load, // preserve grid + clear
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
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

                if let Some((vb, ib, ic)) = mesh_registry.get(obj.mesh_id) {
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), IndexFormat::Uint32);
                    pass.draw_indexed(0..ic, 0, 0..1);
                }
            }
        }
    }

    /// The depth texture view for compositing.
    pub fn depth_view(&self) -> &TextureView {
        &self.depth_view
    }

    fn create_depth_target(device: &Device, width: u32, height: u32) -> Texture {
        device.create_texture(&TextureDescriptor {
            label: Some("fwd-depth"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }
}
