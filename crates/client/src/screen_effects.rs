use bevy::core_pipeline::fullscreen_material::FullscreenMaterial;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use crate::*;

const SCREEN_EFFECTS_SHADER: &str = "shaders/screen_effects.wesl";
const GRAIN_STRENGTH: f32 = 0.035;
const DESATURATION_AT_DEATH: f32 = 0.7;
const DESATURATION_EASE_RATE: f32 = 3.0;

#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
pub(crate) struct ScreenEffects {
    grain: f32,
    seconds: f32,
    desaturation: f32,
    pad: f32,
}

impl FullscreenMaterial for ScreenEffects {
    fn fragment_shader() -> ShaderRef {
        SCREEN_EFFECTS_SHADER.into()
    }
}

pub(crate) fn update_screen_effects(
    time: Res<Time>,
    player: Query<&Health, With<Player>>,
    mut effects: Query<&mut ScreenEffects>,
    mut eased: Local<f32>,
) {
    let health = player.single().map(|h| h.0).unwrap_or(PLAYER_MAX_HEALTH);
    let hurt = 1.0 - (health / PLAYER_MAX_HEALTH).clamp(0.0, 1.0);
    let blend = (DESATURATION_EASE_RATE * time.delta_secs()).min(1.0);
    *eased += (hurt - *eased) * blend;

    for mut effect in &mut effects {
        effect.grain = GRAIN_STRENGTH;
        effect.seconds = time.elapsed_secs();
        effect.desaturation = *eased * DESATURATION_AT_DEATH;
    }
}
