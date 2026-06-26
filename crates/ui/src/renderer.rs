//! GPU rendering backend for the immediate-mode UI.

use bytemuck::cast_slice;
use wgpu::*;

use crate::draw::{DrawList, UiVertex};

const UI_WGSL: &str = include_str!("shaders/ui.wgsl");

/// Uniform buffer layout: just screen size (vec2<f32>, padded to 16 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Renders UI draw lists to the screen via wgpu.
pub struct UiRenderer {
    pipeline: RenderPipeline,
    bgl: BindGroupLayout,
    uniform_buffer: Buffer,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    white_texture: Texture,
    white_view: TextureView,
    sampler: Sampler,
}

impl UiRenderer {
    /// Maximum initial vertex buffer size in bytes.
    const INIT_VERTEX_BYTES: u64 = 64 * 1024;
    /// Maximum initial index buffer size in bytes.
    const INIT_INDEX_BYTES: u64 = 64 * 1024;

    /// Create a new UI renderer.
    pub fn new(device: &Device, queue: &Queue, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("ui-shader"),
            source: ShaderSource::Wgsl(UI_WGSL.into()),
        });

        let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("ui-bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(
                            std::mem::size_of::<ScreenUniforms>() as u64,
                        ),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("ui-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("ui-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
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
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("ui-uniform-buffer"),
            size: std::mem::size_of::<ScreenUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("ui-vertex-buffer"),
            size: Self::INIT_VERTEX_BYTES,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("ui-index-buffer"),
            size: Self::INIT_INDEX_BYTES,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 1x1 white texture for "no texture" mode.
        let white_texture = device.create_texture(&TextureDescriptor {
            label: Some("ui-white-texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_texture.create_view(&TextureViewDescriptor::default());

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("ui-sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bgl,
            uniform_buffer,
            vertex_buffer,
            index_buffer,
            vertex_capacity: Self::INIT_VERTEX_BYTES,
            index_capacity: Self::INIT_INDEX_BYTES,
            white_texture,
            white_view,
            sampler,
        }
    }

    /// Render all draw lists to the given output view.
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        screen_size: [f32; 2],
        draw_lists: &[DrawList],
    ) {
        if draw_lists.is_empty() {
            return;
        }

        let screen_width = screen_size[0];
        let screen_height = screen_size[1];

        let uniforms = ScreenUniforms {
            screen_size,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));

        let total_verts: usize = draw_lists.iter().map(|dl| dl.vertices.len()).sum();
        let total_indices: usize = draw_lists.iter().map(|dl| dl.indices.len()).sum();

        if total_verts == 0 || total_indices == 0 {
            return;
        }

        let vert_bytes = (total_verts * std::mem::size_of::<UiVertex>()) as u64;
        let idx_bytes = (total_indices * std::mem::size_of::<u32>()) as u64;

        if vert_bytes > self.vertex_capacity {
            let new_cap = vert_bytes.next_power_of_two();
            self.vertex_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("ui-vertex-buffer"),
                size: new_cap,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_cap;
        }
        if idx_bytes > self.index_capacity {
            let new_cap = idx_bytes.next_power_of_two();
            self.index_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("ui-index-buffer"),
                size: new_cap,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity = new_cap;
        }

        let mut all_verts = Vec::with_capacity(total_verts);
        let mut all_indices = Vec::with_capacity(total_indices);
        let mut batches: Vec<BatchInfo> = Vec::with_capacity(draw_lists.len());

        for dl in draw_lists {
            let base_vertex = all_verts.len() as u32;
            let first_index = all_indices.len() as u32;
            all_verts.extend_from_slice(&dl.vertices);
            for &idx in &dl.indices {
                all_indices.push(idx + base_vertex);
            }
            batches.push(BatchInfo {
                first_index,
                index_count: dl.indices.len() as u32,
                clip_rect: dl.clip_rect,
            });
        }

        queue.write_buffer(&self.vertex_buffer, 0, cast_slice(&all_verts));
        queue.write_buffer(&self.index_buffer, 0, cast_slice(&all_indices));

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("ui-bg"),
            layout: &self.bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.white_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("ui-pass"),
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

        let sw = screen_width as u32;
        let sh = screen_height as u32;

        for batch in &batches {
            if let Some(clip) = batch.clip_rect {
                let cx = clip[0].max(0.0) as u32;
                let cy = clip[1].max(0.0) as u32;
                let cw = (clip[2] as u32).min(sw.saturating_sub(cx));
                let ch = (clip[3] as u32).min(sh.saturating_sub(cy));
                if cw == 0 || ch == 0 {
                    continue;
                }
                pass.set_scissor_rect(cx, cy, cw, ch);
            } else {
                pass.set_scissor_rect(0, 0, sw, sh);
            }
            pass.draw_indexed(
                batch.first_index..batch.first_index + batch.index_count,
                0,
                0..1,
            );
        }
    }

    /// Returns a reference to the 1x1 white texture.
    pub fn white_texture(&self) -> &Texture {
        &self.white_texture
    }
}

struct BatchInfo {
    first_index: u32,
    index_count: u32,
    clip_rect: Option<[f32; 4]>,
}
