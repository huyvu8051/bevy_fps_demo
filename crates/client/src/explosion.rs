use avian3d::prelude::*;
use bevy::prelude::*;

use crate::sim::{PendingRespawn, PendingRespawns};
use crate::*;
use shared::{Chase, ZOMBIE_RESPAWN_DELAY};

const BLAST_ORIGIN_DEPTH: f32 = 1.6;
const BLAST_MIN_FALLOFF: f32 = 0.15;
const BLAST_SPIN: f32 = 14.0;
const BLAST_LIMB_SPREAD: f32 = 0.45;

#[derive(Event, Clone, Copy)]
pub(crate) struct Explosion {
    pub(crate) at: Vec3,
    pub(crate) radius: f32,
    pub(crate) force: f32,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct PendingBlast {
    at: Vec3,
    radius: f32,
    force: f32,
}

/// Direction and strength of the blast at a point.
///
/// The origin is sunk below the real blast point on purpose. A charge sits on
/// the floor, so a purely radial push from it is almost horizontal and bodies
/// slide along the ground instead of leaving it. Pulling the origin under the
/// floor tilts every vector upward, which is what launches them.
fn blast_at(point: Vec3, blast: &PendingBlast) -> Option<Vec3> {
    let reach = point.distance(blast.at);
    if reach > blast.radius {
        return None;
    }
    let falloff = (1.0 - reach / blast.radius).max(BLAST_MIN_FALLOFF);
    let source = blast.at - Vec3::Y * BLAST_ORIGIN_DEPTH;
    let away = (point - source).try_normalize()?;
    Some(away * blast.force * falloff * falloff)
}

fn jitter(seed: f32) -> Vec3 {
    Vec3::new(ops::sin(seed * 1.7), ops::sin(seed * 2.3), ops::sin(seed * 3.1))
}

/// Everything caught in the blast goes limp on the spot. Unlike a shot, there
/// is no stagger to play first — being thrown is instantaneous.
pub(crate) fn on_explosion(
    trigger: On<Explosion>,
    mut commands: Commands,
    mut respawns: ResMut<PendingRespawns>,
    mut players: Query<&mut AnimationPlayer>,
    standing: Query<(Entity, &Transform, Option<&ZombieAnimationPlayer>), With<Zombie>>,
    falling: Query<Entity, Or<(With<DeathClipPlaying>, With<Ragdolling>, With<CorpseSettled>)>>,
) {
    let blast = trigger.event();
    let pending = PendingBlast { at: blast.at, radius: blast.radius, force: blast.force };

    for (entity, transform, link) in &standing {
        if transform.translation.distance(blast.at) > blast.radius {
            continue;
        }
        if let Some(link) = link {
            if let Ok(mut player) = players.get_mut(link.0) {
                player.stop_all();
            }
        }
        commands
            .entity(entity)
            .remove::<Zombie>()
            .remove::<Chase>()
            .insert((RagdollArming::empty(), pending));
        respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
    }

    for entity in &falling {
        commands.entity(entity).insert(pending);
    }
}

/// Applies the blast to each bone separately rather than to the body as a
/// whole. One shove on the torso throws a mannequin; letting the near limbs
/// take more than the far ones is what makes it tumble and come apart.
pub(crate) fn apply_pending_blast(
    mut commands: Commands,
    corpses: Query<(Entity, &Ragdolling, &PendingBlast)>,
    mut bodies: Query<(&Transform, &mut LinearVelocity, &mut AngularVelocity), With<RagdollBody>>,
) {
    for (entity, ragdoll, blast) in &corpses {
        for (index, &(_, body, _)) in ragdoll.bodies().iter().enumerate() {
            let Ok((transform, mut velocity, mut spin)) = bodies.get_mut(body) else { continue };
            let Some(push) = blast_at(transform.translation, blast) else { continue };
            let seed = body.to_bits() as f32 * 0.37 + index as f32;
            velocity.0 += push + jitter(seed) * push.length() * BLAST_LIMB_SPREAD;
            spin.0 += jitter(seed * 1.9) * BLAST_SPIN;
        }
        commands.entity(entity).remove::<PendingBlast>();
    }
}

const TEST_BLAST_AHEAD: f32 = 6.0;
const TEST_BLAST_RADIUS: f32 = 7.0;
const TEST_BLAST_FORCE: f32 = 13.0;

pub(crate) fn test_blast_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    let Ok(eye) = camera.single() else { return };
    let at = eye.translation() + eye.forward() * TEST_BLAST_AHEAD;
    commands.trigger(Explosion { at, radius: TEST_BLAST_RADIUS, force: TEST_BLAST_FORCE });
    info!("[blast] at {at:.2?} radius {TEST_BLAST_RADIUS} force {TEST_BLAST_FORCE}");
}
