use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const DAMAGE_OVERLAY_SHADER: &str = "shaders/damage_overlay.wesl";
const HIT_FLASH_DURATION: f32 = 0.35;
const HURT_EASE_RATE: f32 = 6.0;
const HURT_VISIBLE_BELOW_HEALTH: f32 = 20.0;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct DamageOverlayMaterial {
    #[uniform(0)]
    settings: Vec4,
}

impl UiMaterial for DamageOverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        DAMAGE_OVERLAY_SHADER.into()
    }
}

#[derive(Component)]
pub(crate) struct DamageOverlayNode;

#[derive(Resource, Default)]
pub(crate) struct HitFlash(pub(crate) f32);

pub(crate) fn spawn_damage_overlay(commands: &mut Commands, materials: &mut Assets<DamageOverlayMaterial>) {
    let material = materials.add(DamageOverlayMaterial { settings: Vec4::ZERO });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        MaterialNode(material),
        GlobalZIndex(STATIC_UI_Z_INDEX - 1),
        Pickable::IGNORE,
        DamageOverlayNode,
        NativeUiRoot,
    ));
}

pub(crate) fn update_damage_overlay(
    time: Res<Time>,
    mut flash: ResMut<HitFlash>,
    player: Query<&Health, With<Player>>,
    nodes: Query<&MaterialNode<DamageOverlayMaterial>, With<DamageOverlayNode>>,
    mut materials: ResMut<Assets<DamageOverlayMaterial>>,
    mut eased_hurt: Local<f32>,
) {
    let Ok(node) = nodes.single() else {
        return;
    };
    let Some(mut material) = materials.get_mut(&node.0) else {
        return;
    };

    let health = player.single().map(|h| h.0).unwrap_or(PLAYER_MAX_HEALTH);
    let hurt = ((HURT_VISIBLE_BELOW_HEALTH - health) / HURT_VISIBLE_BELOW_HEALTH).clamp(0.0, 1.0);
    let blend = (HURT_EASE_RATE * time.delta_secs()).min(1.0);
    *eased_hurt += (hurt - *eased_hurt) * blend;

    flash.0 = (flash.0 - time.delta_secs() / HIT_FLASH_DURATION).max(0.0);

    material.settings = Vec4::new(*eased_hurt, time.elapsed_secs(), flash.0, 0.0);
}
