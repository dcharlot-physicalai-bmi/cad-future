// Gizmo shader — unlit, always-on-top, per-vertex coloring.
// Used for translate arrows, rotate rings, scale handles.

struct FrameUniforms {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color:    vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = frame.view_proj * vec4<f32>(in.position, 1.0);
    // Push gizmo slightly forward in depth to render on top
    out.clip_pos.z = out.clip_pos.z * 0.5;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
