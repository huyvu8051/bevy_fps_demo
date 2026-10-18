use bevy::prelude::*;

use crate::*;
use shared::{Chase, ZOMBIE_LIE_STILL_SECONDS};

const DISSOLVE_DURATION: f32 = 1.1;

#[derive(Component)]
pub(crate) struct PlayingDeath {
    remaining: f32,
}

pub(crate) fn begin_death(
    commands: &mut Commands,
    entity: Entity,
    kind: ZombieKind,
    anim: &ZombieAnim,
    link: Option<&ZombieAnimationPlayer>,
    players: &mut Query<&mut AnimationPlayer>,
) {
    let (clip, clip_seconds) = anim.slot(kind).death(entity.to_bits() as usize);
    if let Some(link) = link {
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            player.play(clip);
        }
    }
    commands
        .entity(entity)
        .remove::<Zombie>()
        .remove::<Chase>()
        .insert(PlayingDeath {
            remaining: clip_seconds + ZOMBIE_LIE_STILL_SECONDS,
        });
}

pub(crate) fn update_death(
    time: Res<Time>,
    mut commands: Commands,
    mut dying: Query<(Entity, &mut PlayingDeath, &Transform)>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut death, transform) in &mut dying {
        death.remaining -= time.delta_secs();
        if death.remaining > 0.0 {
            continue;
        }
        let start_y = transform.translation.y;
        commands.entity(entity).remove::<PlayingDeath>();
        begin_dissolve(&mut commands, entity, start_y, &children, &mesh_materials, &mut materials);
    }
}
const DISSOLVE_SINK_DISTANCE: f32 = 0.55;
const DISSOLVE_HOLD_FRACTION: f32 = 0.25;

#[derive(Component)]
pub(crate) struct Dissolving {
    remaining: f32,
    start_y: f32,
}

pub(crate) fn begin_dissolve(
    commands: &mut Commands,
    entity: Entity,
    start_y: f32,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
) {
    for descendant in children.iter_descendants(entity) {
        let Ok(handle) = mesh_materials.get(descendant) else {
            continue;
        };
        let Some(source) = materials.get(&handle.0) else {
            continue;
        };
        let mut owned = source.clone();
        owned.alpha_mode = AlphaMode::Blend;
        commands.entity(descendant).insert(MeshMaterial3d(materials.add(owned)));
    }

    commands
        .entity(entity)
        .remove::<Zombie>()
        .remove::<Chase>()
        .insert(Dissolving { remaining: DISSOLVE_DURATION, start_y });
}

pub(crate) fn update_dissolve(
    time: Res<Time>,
    mut commands: Commands,
    mut dying: Query<(Entity, &mut Dissolving, &mut Transform)>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut dissolving, mut transform) in &mut dying {
        dissolving.remaining -= time.delta_secs();
        if dissolving.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        let elapsed = 1.0 - (dissolving.remaining / DISSOLVE_DURATION).clamp(0.0, 1.0);
        let progress = ((elapsed - DISSOLVE_HOLD_FRACTION) / (1.0 - DISSOLVE_HOLD_FRACTION)).clamp(0.0, 1.0);

        transform.translation.y = dissolving.start_y - progress * progress * DISSOLVE_SINK_DISTANCE;
        let alpha = 1.0 - progress * progress;

        for descendant in children.iter_descendants(entity) {
            let Ok(handle) = mesh_materials.get(descendant) else {
                continue;
            };
            let Some(mut material) = materials.get_mut(&handle.0) else {
                continue;
            };
            material.base_color.set_alpha(alpha);
        }
    }
}
