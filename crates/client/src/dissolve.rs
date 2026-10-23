use bevy::prelude::*;

use crate::*;
use shared::{Chase, ZOMBIE_LIE_STILL_SECONDS};

const DISSOLVE_DURATION: f32 = 1.1;

/// Share of the death clip that plays before physics takes the body. The last
/// stretch is ragdoll, so the fall lands on what is actually there and the
/// handoff is hidden inside a movement the clip had already started.
const CLIP_SHARE_BEFORE_RAGDOLL: f32 = 0.7;

/// How long physics keeps the body after the clip would have ended.
const RAGDOLL_AFTER_CLIP_SECONDS: f32 = 5.0;

#[derive(Component)]
pub(crate) struct PlayingDeath {
    until_handoff: f32,
    total: f32,
}

#[derive(Component)]
pub(crate) struct LyingStill {
    remaining: f32,
}

pub(crate) fn begin_death(
    commands: &mut Commands,
    entity: Entity,
    kind: ZombieKind,
    anim: Option<&ZombieAnim>,
    link: Option<&ZombieAnimationPlayer>,
    players: &mut Query<&mut AnimationPlayer>,
    push: Vec3,
) {
    // Without the graphs the body still has to die, or a kill that lands
    // before they are built leaves a corpse walking and shootable.
    let clip_seconds = match anim {
        Some(anim) => {
            let (clip, seconds) = anim.slot(kind).death(entity.to_bits() as usize);
            if let Some(link) = link {
                if let Ok(mut player) = players.get_mut(link.0) {
                    player.stop_all();
                    player.play(clip);
                }
            }
            seconds
        }
        None => ZOMBIE_LIE_STILL_SECONDS,
    };
    commands
        .entity(entity)
        .remove::<Zombie>()
        .remove::<Chase>()
        .insert((
            PlayingDeath {
                until_handoff: clip_seconds * CLIP_SHARE_BEFORE_RAGDOLL,
                total: clip_seconds * CLIP_SHARE_BEFORE_RAGDOLL,
            },
            DeathClipPlaying,
            DeathImpulse { push },
        ));
}

/// The clip gets the first moments — the stagger, the knee buckle — then
/// physics takes the fall, so the body lands on whatever is actually there
/// instead of clipping through a stair or a wall.
pub(crate) fn death_logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("DEATH_LOG").is_ok())
}

pub(crate) fn update_death(
    time: Res<Time>,
    mut commands: Commands,
    mut dying: Query<(Entity, &mut PlayingDeath, Option<&ZombieAnimationPlayer>, Option<&ZombieHitboxBones>, &Transform)>,
    players: Query<&AnimationPlayer>,
    globals: Query<&GlobalTransform>,
) {
    for (entity, mut death, link, bones, root) in &mut dying {
        death.until_handoff -= time.delta_secs();
        if death_logging() {
            let state = link
                .and_then(|l| players.get(l.0).ok())
                .map(|p| {
                    let active: Vec<String> = p
                        .playing_animations()
                        .map(|(node, a)| format!("node{node:?} seek={:.3} done={}", a.seek_time(), a.is_finished()))
                        .collect();
                    if active.is_empty() { "NOTHING PLAYING".to_string() } else { active.join(", ") }
                })
                .unwrap_or_else(|| "no player link".to_string());
            let chest = bones
                .and_then(|b| b.chest)
                .and_then(|e| globals.get(e).ok())
                .map(|g| g.translation().y - root.translation.y)
                .unwrap_or(f32::NAN);
            info!("[death] id={:?} t={:.2}s chest={chest:.3} {state}", entity, death.total - death.until_handoff);
        }
        if death.until_handoff > 0.0 {
            continue;
        }
        // total is the share of the clip that played, so the rest of it is what
        // physics owes before the extra run starts.
        let rest_of_clip =
            death.total * (1.0 - CLIP_SHARE_BEFORE_RAGDOLL) / CLIP_SHARE_BEFORE_RAGDOLL;
        commands
            .entity(entity)
            .remove::<PlayingDeath>()
            .insert(ReadyToRagdoll(rest_of_clip + RAGDOLL_AFTER_CLIP_SECONDS));
    }
}

pub(crate) fn settle_corpse(
    time: Res<Time>,
    mut commands: Commands,
    newly_settled: Query<Entity, (With<CorpseSettled>, Without<LyingStill>)>,
    mut lying: Query<(Entity, &mut LyingStill, &Transform)>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for entity in &newly_settled {
        commands.entity(entity).insert(LyingStill { remaining: ZOMBIE_LIE_STILL_SECONDS });
    }
    for (entity, mut lying, transform) in &mut lying {
        lying.remaining -= time.delta_secs();
        if lying.remaining > 0.0 {
            continue;
        }
        let start_y = transform.translation.y;
        commands.entity(entity).remove::<LyingStill>().remove::<CorpseSettled>();
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
