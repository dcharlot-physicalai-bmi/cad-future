// Selection outline — silhouette pass.
// Renders selected objects slightly inflated along normals with front-face culling,
// so only the back-face "halo" is visible around edges. Depth tested against
// the main scene depth so it doesn't show through occluding geometry.

struct OutlineUniforms {
    view_proj:   mat4x4<f32>,
    outline_color: vec4<f32>,
    thickness:     f32,
    _pad0:         f32,
    _pad1:         f32,
    _pad2:         f32,
};

@group(0) @binding(0) var<uniform> outline: OutlineUniforms;

struct ObjectData {
    model:      mat4x4<f32>,
    normal_mat: mat4x4<f32>,
};

@group(1) @binding(0) var<uniform> obj: ObjectData;

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
    // Inflate vertex along its normal in model space
    let inflated = in.position + normalize(in.normal) * outline.thickness;
    let world = obj.model * vec4<f32>(inflated, 1.0);

    var out: VertexOutput;
    out.clip_pos = outline.view_proj * world;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return outline.outline_color;
}
