struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv:  vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0, -3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 2.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[i], 0.0, 1.0);
    out.uv = uv[i];
    return out;
}

@group(0) @binding(0) var xfb_tex: texture_2d<f32>;
@group(0) @binding(1) var xfb_sam: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(xfb_tex, xfb_sam, in.uv);
}
