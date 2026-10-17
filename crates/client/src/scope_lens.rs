use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;


const SCOPE_LENS_SHADER: &str = "shaders/scope_lens.wgsl";
const SCOPE_BARREL_DISTORTION: f32 = 0.16;
const SCOPE_VIGNETTE_START: f32 = 0.72;
const SCOPE_CHROMATIC_FRINGE: f32 = 0.004;
const SCOPE_EDGE_DARKNESS: f32 = 0.45;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct ScopeLensMaterial {
    #[uniform(0)]
    settings: Vec4,
    #[texture(1)]
    #[sampler(2)]
    view: Handle<Image>,
}

impl Material for ScopeLensMaterial {
    fn fragment_shader() -> ShaderRef {
        SCOPE_LENS_SHADER.into()
    }
}

pub(crate) fn scope_lens_material(view: Handle<Image>) -> ScopeLensMaterial {
    ScopeLensMaterial {
        settings: Vec4::new(
            SCOPE_BARREL_DISTORTION,
            SCOPE_VIGNETTE_START,
            SCOPE_CHROMATIC_FRINGE,
            SCOPE_EDGE_DARKNESS,
        ),
        view,
    }
}
