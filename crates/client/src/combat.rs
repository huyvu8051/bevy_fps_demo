use avian3d::prelude::*;
use bevy::prelude::*;

use crate::dissolve::begin_dissolve;
use crate::map::*;
use crate::sim::*;
use shared::{
    resolve_bone_world_positions, zombie_hit_damage_ray, zombie_hit_nearest_region, BoneWorldPositions, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity,
    Player, PlayerDied, RequestRestart, SetDamageMultiplier,
    ShotConfirmed, ShotFx, Zombie, ZombieHitboxBones, KNOCKBACK_FORCE,
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
    mut zombies: Query<(Entity, &Transform, &ZombieHitboxBones, &mut Health, &mut KnockbackVelocity), With<Zombie>>,
    mut respawns: ResMut<PendingRespawns>,
    mut threats: Query<&mut PlayerThreat>,
    damage_multiplier: Res<DamageMultiplierServer>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    let wall_distance = spatial_query
        .cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &SpatialQueryFilter::default())
        .map(|hit| hit.distance);

    let cylinder = Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT);

    let mut candidates: Vec<(f32, Entity, BoneWorldPositions)> = zombies
        .iter()
        .filter_map(|(entity, transform, bones, _, _)| {
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

        let Some((hit_point, normal, damage)) = zombie_hit_damage_ray(shot.origin, direction, MAX_SHOT_DISTANCE, bones) else {
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
        let damage = damage * damage_multiplier.0;

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
        if let Ok((_, zombie_transform, _, mut health, mut knockback)) = zombies.get_mut(entity) {
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                let start_y = zombie_transform.translation.y;
                begin_dissolve(&mut commands, entity, start_y, &children, &mesh_materials, &mut materials);
                respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
            } else {
                let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                knockback.0 += push;
            }

            commands.trigger(ShotConfirmed { point: hit_point, normal, damage, killed });
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

