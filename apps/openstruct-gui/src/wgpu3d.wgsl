struct Uniforms {
    view_proj: mat4x4<f32>,
    clip: vec4<f32>,
    color: vec4<f32>,
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = u.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.world = in.position;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (dot(in.world, u.clip.xyz) + u.clip.w < 0.0) {
        discard;
    }
    let normal = normalize(in.normal);
    let light = normalize(u.light.xyz);
    let shade = 0.32 + 0.68 * abs(dot(normal, light));
    return vec4<f32>(u.color.rgb * shade, u.color.a);
}
