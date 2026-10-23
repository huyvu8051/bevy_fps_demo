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

/// Whether the eyepiece bends and fringes what it shows.
///
/// Off by default: the bulge pulls the picture away from the rim, so the glass
/// shows less than it could, and a sight is for reading what is in it. The
/// vignette and the rim fade stay either way -- they are what makes the disc
/// read as a circle of glass rather than a square of screen.
#[derive(Resource, Default)]
pub(crate) struct ScopeLensDistortion(pub(crate) bool);

pub(crate) fn apply_scope_lens_distortion(
    distortion: Res<ScopeLensDistortion>,
    mut materials: ResMut<Assets<ScopeLensMaterial>>,
) {
    if !distortion.is_changed() {
        return;
    }
    let (barrel, fringe) = if distortion.0 {
        (SCOPE_BARREL_DISTORTION, SCOPE_CHROMATIC_FRINGE)
    } else {
        (0.0, 0.0)
    };
    for (_, material) in materials.iter_mut() {
        material.settings.x = barrel;
        material.settings.z = fringe;
    }
}
