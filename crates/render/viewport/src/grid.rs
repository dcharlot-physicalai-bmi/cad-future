//! Infinite ground grid rendered via a fullscreen quad shader.
//!
//! The grid is a visual reference plane that fades with distance.
//! Uses the camera's view-projection matrix to project grid lines in the fragment shader.

use bytemuck::cast_slice;
use wgpu::*;

const GRID_WGSL: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

// Fullscreen triangle positions
const POSITIONS = array<vec2<f32>, 6>(
    vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0),
);

fn unproject(pos: vec2<f32>, z: f32) -> vec3<f32> {
    let clip = vec4(pos, z, 1.0);
    let world = uniforms.inv_view_proj * clip;
    return world.xyz / world.w;
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let pos = POSITIONS[idx];
    out.clip_position = vec4(pos, 0.0, 1.0);
    out.near_point = unproject(pos, 0.0);
    out.far_point = unproject(pos, 1.0);
    return out;
}

fn grid(world_pos: vec3<f32>, scale: f32) -> vec4<f32> {
    let coord = world_pos.xz / scale;
    let derivative = fwidth(coord);
    let grid_val = abs(fract(coord - 0.5) - 0.5) / derivative;
    let line = min(grid_val.x, grid_val.y);
    let color = 0.3 - min(line, 1.0) * 0.3;

    // Axis highlighting
    var axis_color = vec3(color);
    if abs(world_pos.x) < derivative.x * scale * 0.5 {
        axis_color = vec3(0.0, 0.0, color + 0.4); // Z axis = blue
    }
    if abs(world_pos.z) < derivative.y * scale * 0.5 {
        axis_color = vec3(color + 0.4, 0.0, 0.0); // X axis = red
    }

    return vec4(axis_color, color);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = -in.near_point.y / (in.far_point.y - in.near_point.y);
    let world_pos = in.near_point + t * (in.far_point - in.near_point);

    // Only render if the ray hits the ground plane (y=0)
    if t < 0.0 || t > 1.0 {
        discard;
    }

    // Distance fade
    let dist = length(world_pos.xz - uniforms.camera_pos.xz);
    let fade = 1.0 - smoothstep(50.0, 200.0, dist);

    let g = grid(world_pos, 1.0);
    return vec4(g.rgb, g.a * fade);
}
"#;

/// Uniform buffer for the grid shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GridUniforms {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    pub _pad: f32,
}

/// Renders an infinite ground-plane grid.
pub struct GridRenderer {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    uniform_buffer: Buffer,
}

impl GridRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("grid-shader"),
            source: ShaderSource::Wgsl(GRID_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("grid-bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(std::mem::size_of::<GridUniforms>() as u64),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("grid-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("grid-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
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
            label: Some("grid-uniform-buffer"),
            size: std::mem::size_of::<GridUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { pipeline, bind_group_layout, uniform_buffer }
    }

    /// Render the grid. Call after clearing the framebuffer, before UI.
    pub fn render(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        uniforms: &GridUniforms,
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[*uniforms]));

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("grid-bg"),
            layout: &self.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("grid-pass"),
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
        pass.draw(0..6, 0..1);
    }
}
