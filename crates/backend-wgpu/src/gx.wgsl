struct Uniforms {
    mvp: mat4x4<f32>,
    has_texture: u32,
    _pad: vec3<u32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var tex: texture_2d<f32>;

@group(0) @binding(2)
var tex_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex0: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex0: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = uniforms.mvp * vec4<f32>(in.position, 1.0);
    // Remap depth: GameCube/OpenGL uses [-1,1], wgpu uses [0,1]
    out.clip_pos.z = out.clip_pos.z * 0.5 + out.clip_pos.w * 0.5;
    out.color = in.color;
    out.tex0 = in.tex0;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // TODO: pretty sure this is wrong but it works for texturetest.dol
    if uniforms.has_texture == 1u {
        return textureSample(tex, tex_sampler, in.tex0);
    } else {
        return in.color;
    }
}
