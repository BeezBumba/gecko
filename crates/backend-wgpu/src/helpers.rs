use gekko::flipper::gx::regs::{BlendFactor, CompareFunc, MagFilter, MinFilter, WrapMode};

pub fn map_wrap_mode(wrap: WrapMode) -> wgpu::AddressMode {
    match wrap {
        WrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
    }
}

pub fn map_mag_filter(filter: MagFilter) -> wgpu::FilterMode {
    match filter {
        MagFilter::Nearest => wgpu::FilterMode::Nearest,
        MagFilter::Linear => wgpu::FilterMode::Linear,
    }
}

pub fn map_min_filter(filter: MinFilter) -> wgpu::FilterMode {
    match filter {
        MinFilter::Nearest | MinFilter::NearestMipmapNearest | MinFilter::NearestMipmapLinear => {
            wgpu::FilterMode::Nearest
        }
        MinFilter::Linear | MinFilter::LinearMipmapNearest | MinFilter::LinearMipmapLinear => wgpu::FilterMode::Linear,
    }
}

pub fn map_blend_factor(f: BlendFactor) -> wgpu::BlendFactor {
    match f {
        BlendFactor::Zero => wgpu::BlendFactor::Zero,
        BlendFactor::One => wgpu::BlendFactor::One,
        BlendFactor::SrcClr => wgpu::BlendFactor::Src,
        BlendFactor::InvSrcClr => wgpu::BlendFactor::OneMinusSrc,
        BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        BlendFactor::InvSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
        BlendFactor::InvDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
    }
}

pub fn map_compare_func(f: CompareFunc) -> wgpu::CompareFunction {
    match f {
        CompareFunc::Never => wgpu::CompareFunction::Never,
        CompareFunc::Less => wgpu::CompareFunction::Less,
        CompareFunc::Equal => wgpu::CompareFunction::Equal,
        CompareFunc::LessEqual => wgpu::CompareFunction::LessEqual,
        CompareFunc::Greater => wgpu::CompareFunction::Greater,
        CompareFunc::NotEqual => wgpu::CompareFunction::NotEqual,
        CompareFunc::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        CompareFunc::Always => wgpu::CompareFunction::Always,
    }
}
