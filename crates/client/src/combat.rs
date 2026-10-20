use avian3d::prelude::*;
use bevy::prelude::*;

use crate::dismember::LimbDamage;
use crate::dissolve::begin_death;
use crate::ragdoll::DEATH_PUSH_SPEED;
use crate::map::*;
use crate::sim::*;
use shared::{
    resolve_bone_world_positions, zombie_hit_damage_ray, zombie_hit_nearest_region, BoneWorldPositions, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity,
    Player, PlayerDied, RequestRestart, SetDamageMultiplier,
    ShotConfirmed, ShotFx, Zombie, ZombieAnim, ZombieAnimationPlayer, ZombieHitboxBones, ZombieKind,
    KNOCKBACK_FORCE,
    MAX_SHOT_DISTANCE, Scenario,
    ZOMBIE_REST_HEIGHT, PLAYER_THREAT_PER_SHOT, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS, ZOMBIE_RESPAWN_DELAY,
};

#[derive(Resource)]
pub(crate) struct DamageMultiplierServer(f32);
impl Default for DamageMultiplierServer {
    fn default() -> Self {
        Self(1.0)
    }
}

pub(crate) fn on_set_damage_multiplier(trigger: On<SetDamageMultiplier>, mut multiplier: ResMut<DamageMultiplierServer>) {
    multiplier.0 = trigger.event().0.max(0.0);
}

pub(crate) fn handle_fire_shot(
    trigger: On<FireShot>,
    mut commands: Commands,
    spatial_query: SpatialQuery,
    bone_globals: Query<&GlobalTransform>,
    mut zombies: Query<
        (
            Entity,
            &Transform,
            &ZombieHitboxBones,
            &mut Health,
            &mut KnockbackVelocity,
            Option<&ZombieKind>,
            Option<&ZombieAnimationPlayer>,
        ),
        With<Zombie>,
    >,
    anim: Res<ZombieAnim>,
    mut anim_players: Query<&mut AnimationPlayer>,
    mut respawns: ResMut<PendingRespawns>,
    mut threats: Query<&mut PlayerThreat>,
    damage_multiplier: Res<DamageMultiplierServer>,
    mut limb_damage: ResMut<LimbDamage>,
) {
    for mut threat in &mut threats {
        threat.0 += PLAYER_THREAT_PER_SHOT;
    }

    let shot = *trigger.event();
    let Ok(direction) = Dir3::new(shot.direction) else {
        return;
    };
    info!("[shot] fired: origin={:.2?} direction={:.2?}", shot.origin, *direction);

    let filter = SpatialQueryFilter::default();
    let path = trace_shot_path(&spatial_query, shot.origin, direction, &filter);
    let wall_distance = path.blocking_distance();
    for wall in &path.through {
        commands.trigger(ShotFx::Penetrated { point: wall.entry, normal: wall.normal });
        commands.trigger(ShotFx::Penetrated { point: wall.exit, normal: -wall.normal });
    }

    let cylinder = Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT);

    let mut candidates: Vec<(f32, Entity, BoneWorldPositions)> = zombies
        .iter()
        .filter_map(|(entity, transform, bones, _, _, _, _)| {
            let (distance, _outer_normal) =
                cylinder.cast_ray(transform.translation, transform.rotation, shot.origin, *direction, MAX_SHOT_DISTANCE, true)?;
            let resolved = resolve_bone_world_positions(bones, |bone| bone_globals.get(bone).ok().map(|g| g.translation()));
            Some((distance, entity, resolved))
        })
        .collect();
    candidates.sort_by(|(a, ..), (b, ..)| a.total_cmp(b));

    for (distance, entity, bones) in &candidates {
        let entity = *entity;
        info!("[shot] outer capsule hit: zombie={entity} distance={distance:.2}");

        let Some(hit) = zombie_hit_damage_ray(shot.origin, direction, MAX_SHOT_DISTANCE, bones) else {
            let outer_hit_point = shot.origin + *direction * distance;
            if let Some((region, nearest_distance, radius)) = zombie_hit_nearest_region(outer_hit_point, bones) {
                info!(
                    "[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} — nearest was {region}, \
                     distance={nearest_distance:.3} radius={radius:.3} (missed by {:.3}) — trying the next zombie behind it, if any",
                    nearest_distance - radius
                );
            } else {
                info!("[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} (no bones resolved yet) — trying the next zombie behind it, if any");
            }
            continue;
        };
        let (hit_point, normal, damage) = (hit.point, hit.normal, hit.damage);
        if let Some((limb, segment)) = hit.limb {
            limb_damage.record(entity, limb, segment, damage);
        }
        let damage = damage * damage_multiplier.0
            * path.damage_scale_at(shot.origin, shot.origin.distance(hit_point));

        let hit_distance = shot.origin.distance(hit_point);
        if wall_distance.is_some_and(|w| w < hit_distance - 0.1) {
            info!(
                "[shot] blocked by level geometry before zombie={entity} (wall at {:.2} < zombie at {:.2})",
                wall_distance.unwrap(),
                hit_distance
            );
            continue;
        }
        info!("[shot] inner bone capsule hit: zombie={entity} damage={damage:.1}");
        if let Ok((_, _, _, mut health, mut knockback, kind, link)) = zombies.get_mut(entity) {
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                let severed = limb_damage.take(entity);
                begin_death(
                    &mut commands,
                    entity,
                    kind.copied().unwrap_or_default(),
                    &anim,
                    link,
                    &mut anim_players,
                    Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero()
                        * DEATH_PUSH_SPEED,
                    severed,
                );
                respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
            } else {
                let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                knockback.0 += push;
            }

            commands.trigger(ShotConfirmed { origin: shot.origin, point: hit_point, normal, damage, killed });
        }
        return;
    }

    let Some(hit) = spatial_query.cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &filter) else {
        info!("[shot] missed everything (no zombie, no surface)");
        return;
    };
    if zombies.contains(hit.entity) {
        info!("[shot] outer capsule hit but missed every inner bone capsule — treating as a genuine miss, no VFX");
        return;
    }

    let surface_point = shot.origin + *direction * hit.distance;
    info!("[shot] hit a surface, no zombie: point={surface_point:.2?}");
    commands.trigger(ShotFx::SurfaceHit { origin: shot.origin, point: surface_point, normal: hit.normal });
}

#[derive(Component)]
pub(crate) struct Dead;

pub(crate) fn on_player_died(
    _trigger: On<PlayerDied>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,

    players: Query<Entity, With<Player>>,
    dead: Query<(), With<Dead>>,
) {
    let Ok(entity) = players.single() else {
        return;
    };
    commands.entity(entity).insert(Dead);

    if game_over.0 {
        return;
    }
    let all_dead = players.iter().all(|e| e == entity || dead.contains(e));
    if all_dead {
        game_over.0 = true;
        commands.trigger(GameStateEvent::GameOver);
    }
}

pub(crate) fn on_request_restart(
    _trigger: On<RequestRestart>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    mut respawns: ResMut<PendingRespawns>,
    scenario: Res<Scenario>,
    points: Res<ZombieSpawnPoints>,
    zombies: Query<Entity, With<Zombie>>,
    mut players: Query<(Entity, &mut Transform, &mut Player), With<Player>>,
) {
    if !game_over.0 {
        return;
    }
    game_over.0 = false;
    respawns.0.clear();

    for entity in &zombies {
        commands.entity(entity).despawn();
    }

    let spawns: Vec<(Vec3, f32)> = points
        .0
        .iter()
        .take(scenario.zombie_count())
        .map(|p| (*p + Vec3::Y * ZOMBIE_REST_HEIGHT, 0.0))
        .collect();
    spawn_zombies(&mut commands, &spawns, true);
    let player_spawn = scenario.player_spawn();

    for (entity, mut transform, mut player) in &mut players {
        commands.entity(entity).remove::<Dead>();
        transform.translation = player_spawn;
        player.vertical_velocity = 0.0;
        player.grounded = false;
    }

    commands.trigger(GameStateEvent::Restarted);
}


const MAX_PENETRATIONS: usize = 2;
const MAX_WALL_THICKNESS: f32 = 0.35;
const PENETRATION_DAMAGE_SCALE: f32 = 0.55;
const PENETRATION_STEP_OFF: f32 = 0.02;

pub(crate) struct Penetration {
    pub(crate) entry: Vec3,
    pub(crate) normal: Vec3,
    pub(crate) exit: Vec3,
}

pub(crate) struct ShotPath {
    pub(crate) through: Vec<Penetration>,
    pub(crate) blocked_at: Option<(f32, Vec3, Vec3)>,
}

impl ShotPath {
    pub(crate) fn blocking_distance(&self) -> Option<f32> {
        self.blocked_at.map(|(distance, _, _)| distance)
    }

    pub(crate) fn damage_scale_at(&self, origin: Vec3, distance: f32) -> f32 {
        let passed = self
            .through
            .iter()
            .filter(|wall| origin.distance(wall.entry) < distance)
            .count();
        PENETRATION_DAMAGE_SCALE.powi(passed as i32)
    }
}

/// Measures how thick the surface at `entry` is by probing back toward it from
/// beyond. Casting forward from inside the wall would need the ray to report
/// back faces; coming the other way, the first thing hit is the far side.
fn exit_point(
    spatial_query: &SpatialQuery,
    entry: Vec3,
    direction: Dir3,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    let beyond = entry + *direction * MAX_WALL_THICKNESS;
    let back = Dir3::new(-*direction).ok()?;
    let hit = spatial_query.cast_ray(beyond, back, MAX_WALL_THICKNESS, true, filter)?;
    let thickness = MAX_WALL_THICKNESS - hit.distance;
    (thickness > 0.0).then(|| entry + *direction * thickness)
}

/// Walks the shot forward through anything thin enough to punch through,
/// stopping at the first wall that is not.
pub(crate) fn trace_shot_path(
    spatial_query: &SpatialQuery,
    origin: Vec3,
    direction: Dir3,
    filter: &SpatialQueryFilter,
) -> ShotPath {
    let mut through = Vec::new();
    let mut from = origin;

    loop {
        let remaining = MAX_SHOT_DISTANCE - origin.distance(from);
        if remaining <= 0.0 {
            return ShotPath { through, blocked_at: None };
        }
        let Some(hit) = spatial_query.cast_ray(from, direction, remaining, true, filter) else {
            return ShotPath { through, blocked_at: None };
        };
        let entry = from + *direction * hit.distance;
        let entry_distance = origin.distance(entry);

        if through.len() >= MAX_PENETRATIONS {
            return ShotPath { through, blocked_at: Some((entry_distance, entry, hit.normal)) };
        }
        let Some(exit) = exit_point(spatial_query, entry, direction, filter) else {
            return ShotPath { through, blocked_at: Some((entry_distance, entry, hit.normal)) };
        };
        through.push(Penetration { entry, normal: hit.normal, exit });
        from = exit + *direction * PENETRATION_STEP_OFF;
    }
}
