// Wireframe overlay — renders mesh edges as colored lines.
// Shares the same bind groups as the forward shader.

struct FrameUniforms {
    view:        mat4x4<f32>,
    proj:        mat4x4<f32>,
    camera_pos:  vec3<f32>,
    _pad0:       f32,
    sun_dir:     vec3<f32>,
    sun_intensity: f32,
    sun_color:   vec3<f32>,
    ambient:     f32,
};

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct ObjectUniforms {
    model:       mat4x4<f32>,
    normal_mat:  mat4x4<f32>,
    albedo:      vec4<f32>,
    roughness:   f32,
    metallic:    f32,
    selected:    f32,
    _obj_pad1:   f32,
};

@group(1) @binding(0) var<uniform> obj: ObjectUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = obj.model * vec4<f32>(in.position, 1.0);
    out.clip_pos = frame.proj * frame.view * world;
    // Slight depth bias to draw lines on top of filled faces
    out.clip_pos.z = out.clip_pos.z * 0.9999;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Dark lines for wireframe overlay, brighter if selected
    if obj.selected > 0.5 {
        return vec4<f32>(0.3, 0.7, 1.0, 1.0);
    }
    return vec4<f32>(0.1, 0.1, 0.1, 0.8);
}
