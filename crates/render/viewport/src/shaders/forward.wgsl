// Forward renderer for CAD: Blinn-Phong with single directional light.
// No shadows — clean engineering viewport.

// ── Bind group 0: Per-frame uniforms ─────────────────────────────────────

struct FrameUniforms {
    view:        mat4x4<f32>,
    proj:        mat4x4<f32>,
    camera_pos:  vec3<f32>,
    _pad0:       f32,
    sun_dir:     vec3<f32>,
    sun_intensity: f32,
    sun_color:   vec3<f32>,
    ambient:     f32,
    // Clip plane: vec4(normal.xyz, distance). w=0 means disabled.
    clip_plane:  vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

// ── Bind group 1: Per-object uniforms (dynamic offset) ───────────────────

struct ObjectUniforms {
    model:       mat4x4<f32>,
    normal_mat:  mat4x4<f32>,
    albedo:      vec4<f32>,
    roughness:   f32,
    metallic:    f32,
    selected:    f32,    // 1.0 if selected, 0.0 otherwise
    _obj_pad1:   f32,
};

@group(1) @binding(0) var<uniform> obj: ObjectUniforms;

// ── Vertex / Fragment ────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos:  vec3<f32>,
    @location(1) world_norm: vec3<f32>,
    @location(2) uv:         vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = obj.model * vec4<f32>(in.position, 1.0);
    out.world_pos = world.xyz;
    out.world_norm = normalize((obj.normal_mat * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.clip_pos = frame.proj * frame.view * world;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Cross-section clip plane: discard if on the negative side
    let clip_active = abs(frame.clip_plane.x) + abs(frame.clip_plane.y) + abs(frame.clip_plane.z);
    if clip_active > 0.001 {
        let dist = dot(frame.clip_plane.xyz, in.world_pos) + frame.clip_plane.w;
        if dist < 0.0 {
            discard;
        }
    }

    let N = normalize(in.world_norm);
    let V = normalize(frame.camera_pos - in.world_pos);
    let albedo = obj.albedo.rgb;

    // Ambient
    var color = albedo * frame.ambient;

    // Directional (sun) light — Blinn-Phong
    let L = normalize(-frame.sun_dir);
    let H = normalize(L + V);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let spec_power = mix(16.0, 128.0, 1.0 - obj.roughness);
    let specular = pow(NdotH, spec_power) * (1.0 - obj.roughness);

    // Metallic blend: metals tint specular with albedo
    let diffuse = albedo * NdotL * (1.0 - obj.metallic);
    let spec_color = mix(vec3<f32>(0.04), albedo, obj.metallic);
    let spec_final = spec_color * specular;

    color += (diffuse + spec_final) * frame.sun_color * frame.sun_intensity;

    // Rim light for depth perception
    let rim = 1.0 - max(dot(N, V), 0.0);
    let rim_factor = pow(rim, 3.0) * 0.15;
    color += vec3<f32>(rim_factor);

    // Selection highlight: bright Fresnel outline + color tint
    if obj.selected > 0.5 {
        let sel_color = vec3<f32>(0.3, 0.65, 1.0);     // bright blue
        let outline_color = vec3<f32>(0.4, 0.8, 1.0);   // cyan-white edge

        // Strong Fresnel edge glow — the key visual indicator
        let edge_glow = pow(rim, 1.5) * 1.2;
        let inner_tint = 0.12; // subtle body tint

        // Composite: bright edge + subtle inner tint
        let sel_factor = clamp(edge_glow + inner_tint, 0.0, 1.0);
        color = mix(color, mix(sel_color, outline_color, rim), sel_factor);

        // Boost overall brightness slightly for selected objects
        color = color * 1.08;
    }

    return vec4<f32>(color, obj.albedo.a);
}
