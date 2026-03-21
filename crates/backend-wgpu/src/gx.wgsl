struct FrameUniforms {
    tev_color_reg0: vec4<f32>,
    tev_color_reg1: vec4<f32>,
    tev_color_reg2: vec4<f32>,
    tev_color_reg3: vec4<f32>,
    tev_konst_colors: array<vec4<f32>, 16>,
    tev_color_env: array<vec4<u32>, 4>,
    tev_alpha_env: array<vec4<u32>, 4>,
    num_tev_stages: u32,
    alpha_ref0: f32,
    alpha_ref1: f32,
    alpha_comp0: u32,
    alpha_comp1: u32,
    alpha_op: u32,
};

struct DrawUniforms {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniforms;

@group(0) @binding(1)
var<uniform> draw: DrawUniforms;

@group(0) @binding(2)
var tex: texture_2d<f32>;

@group(0) @binding(3)
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
    out.clip_pos = draw.mvp * vec4<f32>(in.position, 1.0);
    // Remap depth: GameCube/OpenGL uses [-1,1], wgpu uses [0,1]
    out.clip_pos.z = out.clip_pos.z * 0.5 + out.clip_pos.w * 0.5;
    out.color = in.color;
    out.tex0 = in.tex0;
    return out;
}

// Read a single u32 from a packed array<vec4<u32>, 4> by flat index (0-15)
fn read_packed(arr: array<vec4<u32>, 4>, idx: u32) -> u32 {
    let vi = idx / 4u;
    let ci = idx % 4u;
    return arr[vi][ci];
}

// Extract `width` bits starting at bit `lo` from `val`
fn extract_bits(val: u32, lo: u32, width: u32) -> u32 {
    return (val >> lo) & ((1u << width) - 1u);
}

// TEV color input selector (TevColorIn, 4-bit, 16 variants)
fn tev_color_in(sel: u32, tex_color: vec4<f32>, ras_color: vec4<f32>, regs: array<vec4<f32>, 4>, konst_color: vec4<f32>) -> vec3<f32> {
    switch sel {
        case 0u:  { return regs[0].rgb; }           // PrevColor
        case 1u:  { return vec3(regs[0].a); }       // PrevAlpha
        case 2u:  { return regs[1].rgb; }           // Reg0Color
        case 3u:  { return vec3(regs[1].a); }       // Reg0Alpha
        case 4u:  { return regs[2].rgb; }           // Reg1Color
        case 5u:  { return vec3(regs[2].a); }       // Reg1Alpha
        case 6u:  { return regs[3].rgb; }           // Reg2Color
        case 7u:  { return vec3(regs[3].a); }       // Reg2Alpha
        case 8u:  { return tex_color.rgb; }         // TexColor
        case 9u:  { return vec3(tex_color.a); }     // TexAlpha
        case 10u: { return ras_color.rgb; }         // RasColor
        case 11u: { return vec3(ras_color.a); }     // RasAlpha
        case 12u: { return vec3(1.0); }             // One
        case 13u: { return vec3(0.5); }             // Half
        case 14u: { return konst_color.rgb; }       // Konst
        case 15u: { return vec3(0.0); }             // Zero
        default:  { return vec3(0.0); }
    }
}

// TEV alpha input selector (TevAlphaIn, 3-bit, 8 variants)
fn tev_alpha_in(sel: u32, tex_color: vec4<f32>, ras_color: vec4<f32>, regs: array<vec4<f32>, 4>, konst_color: vec4<f32>) -> f32 {
    switch sel {
        case 0u: { return regs[0].a; }      // PrevAlpha
        case 1u: { return regs[1].a; }      // Reg0Alpha
        case 2u: { return regs[2].a; }      // Reg1Alpha
        case 3u: { return regs[3].a; }      // Reg2Alpha
        case 4u: { return tex_color.a; }    // TexAlpha
        case 5u: { return ras_color.a; }    // RasAlpha
        case 6u: { return konst_color.a; }  // Konst
        case 7u: { return 0.0; }            // Zero
        default: { return 0.0; }
    }
}

// TEV color combiner: result = (d (+/-) ((1-c)*a + c*b) + bias) * scale
fn tev_combine_color(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>,
                     bias: u32, sub: bool, scale: u32, do_clamp: bool) -> vec3<f32> {
    let lerp = a * (vec3(1.0) - c) + b * c;
    
    var result: vec3<f32>;
    if sub {
        result = d - lerp;
    } else {
        result = d + lerp;
    }

    // Bias
    switch bias {
        case 1u: { result += vec3(0.5); }   // AddHalf
        case 2u: { result -= vec3(0.5); }   // SubHalf
        default: {}                         // Zero
    }
    
    // Scale
    switch scale {
        case 1u: { result *= 2.0; }     // Scale2
        case 2u: { result *= 4.0; }     // Scale4
        case 3u: { result *= 0.5; }     // Divide2
        default: {}                     // Scale1
    }

    if do_clamp {
        result = clamp(result, vec3(0.0), vec3(1.0));
    }
    
    return result;
}

// TEV alpha combiner: same formula as color but per-channel
fn tev_combine_alpha(a: f32, b: f32, c: f32, d: f32, bias: u32, sub: bool, scale: u32, do_clamp: bool) -> f32 {
    let lerp = a * (1.0 - c) + b * c;
    
    var result: f32;
    if sub {
        result = d - lerp;
    } else {
        result = d + lerp;
    }
    
    switch bias {
        case 1u: { result += 0.5; }
        case 2u: { result -= 0.5; }
        default: {}
    }
    
    switch scale {
        case 1u: { result *= 2.0; }
        case 2u: { result *= 4.0; }
        case 3u: { result *= 0.5; }
        default: {}
    }
    
    if do_clamp {
        result = clamp(result, 0.0, 1.0);
    }
    
    return result;
}

fn alpha_compare(a: f32, ref_val: f32, func: u32) -> bool {
    switch func {
        case 0u: { return false; }          // Never
        case 1u: { return a < ref_val; }    // Less
        case 2u: { return a == ref_val; }   // Equal
        case 3u: { return a <= ref_val; }   // LessEqual
        case 4u: { return a > ref_val; }    // Greater
        case 5u: { return a != ref_val; }   // NotEqual
        case 6u: { return a >= ref_val; }   // GreaterEqual
        case 7u: { return true; }           // Always
        default: { return true; }
    }
}

fn alpha_combine(a: bool, b: bool, op: u32) -> bool {
    switch op {
        case 0u: { return a && b; }     // AND
        case 1u: { return a || b; }     // OR
        case 2u: { return a != b; }     // XOR
        case 3u: { return a == b; }     // XNOR
        default: { return true; }
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Initialize TEV registers from uniforms
    var regs: array<vec4<f32>, 4>;
    regs[0] = frame.tev_color_reg0;
    regs[1] = frame.tev_color_reg1;
    regs[2] = frame.tev_color_reg2;
    regs[3] = frame.tev_color_reg3;

    // Inputs available to TEV stages
    let ras_color = in.color;
    let tex_color = textureSample(tex, tex_sampler, in.tex0);

    let num_stages = frame.num_tev_stages;

    for (var stage = 0u; stage < 16u; stage++) {
        if stage >= num_stages {
            break;
        }

        // Combine colors
        let cenv = read_packed(frame.tev_color_env, stage);

        // TevColorEnv bit layout (LSB0):
        //   [3:0]=d  [7:4]=c  [11:8]=b  [15:12]=a
        //   [17:16]=bias  [18]=sub  [19]=clamp  [21:20]=scale  [23:22]=dest
        let c_d = extract_bits(cenv, 0u, 4u);
        let c_c = extract_bits(cenv, 4u, 4u);
        let c_b = extract_bits(cenv, 8u, 4u);
        let c_a = extract_bits(cenv, 12u, 4u);
        let c_bias = extract_bits(cenv, 16u, 2u);
        let c_sub = extract_bits(cenv, 18u, 1u) != 0u;
        let c_clamp = extract_bits(cenv, 19u, 1u) != 0u;
        let c_scale = extract_bits(cenv, 20u, 2u);
        let c_dest = extract_bits(cenv, 22u, 2u);

        let konst_color = frame.tev_konst_colors[stage];

        let in_a = tev_color_in(c_a, tex_color, ras_color, regs, konst_color);
        let in_b = tev_color_in(c_b, tex_color, ras_color, regs, konst_color);
        let in_c = tev_color_in(c_c, tex_color, ras_color, regs, konst_color);
        let in_d = tev_color_in(c_d, tex_color, ras_color, regs, konst_color);

        let color_result = tev_combine_color(in_a, in_b, in_c, in_d, c_bias, c_sub, c_scale, c_clamp);
        // Write RGB to dest, preserve alpha
        regs[c_dest] = vec4(color_result, regs[c_dest].a);

        // Combine alpha
        let aenv = read_packed(frame.tev_alpha_env, stage);

        // TevAlphaEnv bit layout (LSB0):
        //   [6:4]=d  [9:7]=c  [12:10]=b  [15:13]=a
        //   [17:16]=bias  [18]=sub  [19]=clamp  [21:20]=scale  [23:22]=dest
        let a_d = extract_bits(aenv, 4u, 3u);
        let a_c = extract_bits(aenv, 7u, 3u);
        let a_b = extract_bits(aenv, 10u, 3u);
        let a_a = extract_bits(aenv, 13u, 3u);
        let a_bias = extract_bits(aenv, 16u, 2u);
        let a_sub = extract_bits(aenv, 18u, 1u) != 0u;
        let a_clamp = extract_bits(aenv, 19u, 1u) != 0u;
        let a_scale = extract_bits(aenv, 20u, 2u);
        let a_dest = extract_bits(aenv, 22u, 2u);

        let ain_a = tev_alpha_in(a_a, tex_color, ras_color, regs, konst_color);
        let ain_b = tev_alpha_in(a_b, tex_color, ras_color, regs, konst_color);
        let ain_c = tev_alpha_in(a_c, tex_color, ras_color, regs, konst_color);
        let ain_d = tev_alpha_in(a_d, tex_color, ras_color, regs, konst_color);

        let alpha_result = tev_combine_alpha(ain_a, ain_b, ain_c, ain_d, a_bias, a_sub, a_scale, a_clamp);

        // Write alpha to dest, preserve RGB
        regs[a_dest] = vec4(regs[a_dest].rgb, alpha_result);
    }

    // Output is always TEVPREV (regs[0])
    let color = regs[0];

    let c0 = alpha_compare(color.a, frame.alpha_ref0, frame.alpha_comp0);
    let c1 = alpha_compare(color.a, frame.alpha_ref1, frame.alpha_comp1);
    if !alpha_combine(c0, c1, frame.alpha_op) {
        discard;
    }

    return color;
}
