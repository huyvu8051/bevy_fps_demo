use avian3d::prelude::*;
use bevy_landmass::prelude::*;
use bevy::prelude::*;
use big_brain::prelude::*;
use shared::movement::{blocked_horizontally, resolve_slope_hit, try_step_up};
use shared::{
    entity_phase, resolve_bone_world_positions, zombie_hit_damage_ray, zombie_hit_nearest_region, BoneWorldPositions, Chase, ClearAllZombies, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity,
    Player, PlayerDied, RequestRestart,
    SetAutoMaintainZombies, SetDamageMultiplier, SetFreezeZombies, SetZombieCount,
    ShotConfirmed, ShotFx, SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, ZombiePose, zombie_speed_for, 
    GRAVITY, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE,
    MAX_SHOT_DISTANCE, Scenario,
    
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    PLAYER_THREAT_DECAY_RATE, PLAYER_THREAT_PER_SHOT, ZOMBIE_LOAD_BALANCE_PENALTY, zombie_kind_for_phase, ZOMBIE_KIND_WEIGHTS,
    ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS, ZOMBIE_RESPAWN_DELAY, ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS,
    ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_STOP_DISTANCE, ZOMBIE_THREAT_DISTANCE_BONUS,
    ZOMBIE_TURN_RATE_DEG,
};


pub(crate) struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .insert_resource(crate::current_scenario())
            .init_resource::<GameOver>()
            .init_resource::<FreezeZombiesServer>()
            .init_resource::<DamageMultiplierServer>()
            .init_resource::<PendingRespawns>()
            .init_resource::<ZombieTargetCount>()
            .init_resource::<AutoMaintainZombies>()
            .init_resource::<NextZombieSpawnIndex>()
            .init_resource::<ZombieSpawnPoints>()
            .init_resource::<WorldReady>()
            .add_systems(Startup, load_zombie_spawn_points)
            .add_systems(Update, (track_world_ready, spawn_initial_zombies.run_if(world_ready)).chain())
            .add_systems(
                PreUpdate,
                (
                    chase_action_system.in_set(BigBrainSet::Actions),
                    near_player_scorer.in_set(BigBrainSet::Scorers),
                ),
            )
            .add_systems(
                Update,
                (
                    move_zombies.after(LandmassSystems::Output).run_if(world_ready),
                    tick_respawns.run_if(world_ready),
                    decay_player_threat,
                ),
            )
            .add_observer(spawn_more_zombies)
            .add_observer(on_set_zombie_count)
            .add_observer(on_clear_all_zombies)
            .add_observer(on_set_auto_maintain_zombies)
            .add_observer(on_set_freeze_zombies)
            .add_observer(on_set_damage_multiplier)
            .add_observer(on_player_died)
            .add_observer(on_request_restart)
            .add_observer(handle_fire_shot);
    }
}

#[derive(Resource, Default)]
struct FreezeZombiesServer(bool);

fn on_set_freeze_zombies(trigger: On<SetFreezeZombies>, mut freeze: ResMut<FreezeZombiesServer>) {
    freeze.0 = trigger.event().0;
}

#[derive(Resource)]
struct DamageMultiplierServer(f32);
impl Default for DamageMultiplierServer {
    fn default() -> Self {
        Self(1.0)
    }
}

fn on_set_damage_multiplier(trigger: On<SetDamageMultiplier>, mut multiplier: ResMut<DamageMultiplierServer>) {
    multiplier.0 = trigger.event().0.max(0.0);
}

#[derive(Component, Default)]
pub(crate) struct PlayerThreat(f32);

fn decay_player_threat(time: Res<Time>, mut threats: Query<&mut PlayerThreat>) {
    let decay = (1.0 - PLAYER_THREAT_DECAY_RATE * time.delta_secs()).clamp(0.0, 1.0);
    for mut threat in &mut threats {
        threat.0 *= decay;
    }
}

#[derive(Resource)]
struct ZombieTargetCount(u32);

impl Default for ZombieTargetCount {
    fn default() -> Self {
        let count = std::env::var("ZOMBIE_COUNT").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        Self(count)
    }
}

#[derive(Resource)]
struct AutoMaintainZombies(bool);

impl Default for AutoMaintainZombies {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
struct NextZombieSpawnIndex(usize);

fn zombie_spawn_pos_for(scenario: Scenario, seed: usize, points: &[Vec3]) -> (Vec3, f32) {
    if points.is_empty() {
        return (scenario.player_spawn(), 0.0);
    }
    (points[seed % points.len()] + Vec3::Y * ZOMBIE_REST_HEIGHT, 0.0)
}

fn on_set_zombie_count(
    trigger: On<SetZombieCount>,
    mut commands: Commands,
    scenario: Res<Scenario>,
    mut target: ResMut<ZombieTargetCount>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    points: Res<ZombieSpawnPoints>,
    baseline_zombies: Query<Entity, With<Zombie>>,
) {
    let kind = *scenario;
    target.0 = trigger.event().0.min(shared::ZOMBIE_MAX_COUNT);
    let current = baseline_zombies.iter().count() as u32;
    if current < target.0 {
        let to_spawn: Vec<(Vec3, f32)> = (0..target.0 - current)
            .map(|_| {
                let spawn = zombie_spawn_pos_for(kind, next_spawn_index.0, &points.0);
                next_spawn_index.0 += 1;
                spawn
            })
            .collect();
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED);
    } else if current > target.0 {
        for entity in baseline_zombies.iter().take((current - target.0) as usize) {
            commands.entity(entity).try_despawn();
        }
    }
}

fn on_clear_all_zombies(
    _trigger: On<ClearAllZombies>,
    mut commands: Commands,
    all_zombies: Query<Entity, With<Zombie>>,
    mut respawns: ResMut<PendingRespawns>,
) {
    for entity in &all_zombies {
        commands.entity(entity).try_despawn();
    }
    respawns.0.clear();
}

fn on_set_auto_maintain_zombies(trigger: On<SetAutoMaintainZombies>, mut auto_maintain: ResMut<AutoMaintainZombies>) {
    auto_maintain.0 = trigger.event().0;
}

fn spawn_more_zombies(
    _trigger: On<SpawnMoreZombies>,
    points: Res<ZombieSpawnPoints>,
    mut commands: Commands,
) {
    let spawns: Vec<(Vec3, f32)> =
        points.0.iter().map(|p| (*p + Vec3::Y * ZOMBIE_REST_HEIGHT, 0.0)).collect();
    spawn_zombies(&mut commands, &spawns, ZOMBIE_AI_ENABLED);
}

fn spawn_zombie(commands: &mut Commands, position: Vec3, yaw: f32, ai_enabled: bool) {
    let mut zombie = commands.spawn_empty();
    let phase = entity_phase(zombie.id());
    zombie.insert((
        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw)),
        Zombie,

        zombie_kind_for_phase(phase, ZOMBIE_KIND_WEIGHTS),
        ZombieMotion {
            path_recalc_timer: phase * ZOMBIE_PATH_RECALC_INTERVAL,
            ..default()
        },
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        Health(ZOMBIE_MAX_HEALTH),

        ZombieHitboxBones::default(),

        crate::vfx::ZombieHeadLook::default(),

        ZombiePose::default(),

        Visibility::default(),
    ));
    if ai_enabled {
        zombie.insert(Thinker::build().picker(FirstToScore { threshold: 0.8 }).when(NearPlayer, ChaseAction));
    }
}

fn spawn_zombies(commands: &mut Commands, spawns: &[(Vec3, f32)], ai_enabled: bool) {
    for &(position, yaw) in spawns {
        spawn_zombie(commands, position, yaw, ai_enabled);
    }
}

#[derive(Resource, Default)]
struct ZombieSpawnPoints(Vec<Vec3>);

fn load_zombie_spawn_points(scenario: Res<Scenario>, mut commands: Commands) {
    let points = shared::zombie_spawn_points(*scenario);
    info!("{}: {} authored zombie spawn point(s)", scenario.title(), points.len());
    commands.insert_resource(ZombieSpawnPoints(points));
}

#[derive(Component)]
pub(crate) struct MapRoot;

#[derive(Resource, Default)]
pub(crate) struct WorldReady(pub(crate) bool);

pub(crate) fn world_ready(ready: Res<WorldReady>) -> bool {
    ready.0
}

fn track_world_ready(
    asset_server: Res<AssetServer>,
    map: Query<&WorldAssetRoot, With<MapRoot>>,
    pending_colliders: Query<(), With<ColliderConstructorHierarchy>>,
    mut ready: ResMut<WorldReady>,
) {
    if ready.0 {
        return;
    }
    let Ok(root) = map.single() else {
        return;
    };
    if !asset_server.is_loaded_with_dependencies(&root.0) {
        return;
    }
    if !pending_colliders.is_empty() {
        return;
    }
    ready.0 = true;
    info!("[world] map loaded and colliders built — starting simulation");
}

fn spawn_initial_zombies(
    points: Res<ZombieSpawnPoints>,
    target: Res<ZombieTargetCount>,
    mut done: Local<bool>,
    mut commands: Commands,
) {
    if *done {
        return;
    }
    *done = true;
    if points.0.is_empty() {
        error!("no zombie spawn points authored for this map — spawning nothing");
        return;
    }
    let count = (target.0 as usize).min(points.0.len());
    let spawns: Vec<(Vec3, f32)> = points.0[..count]
        .iter()
        .map(|p| (*p + Vec3::Y * ZOMBIE_REST_HEIGHT, 0.0))
        .collect();
    spawn_zombies(&mut commands, &spawns, true);
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPlayer;

fn near_player_scorer(
    transforms: Query<&Transform>,
    players: Query<&Transform, With<Player>>,
    mut query: Query<(&Actor, &mut Score), With<NearPlayer>>,
) {
    for (Actor(actor), mut score) in &mut query {
        let Ok(actor_transform) = transforms.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let in_range = players
            .iter()
            .any(|player_transform| actor_transform.translation.distance(player_transform.translation) <= ZOMBIE_CHASE_RANGE);
        score.set(if in_range { 1.0 } else { 0.0 });
    }
}

#[derive(Clone, Component, Debug, ActionBuilder)]
struct ChaseAction;

#[derive(Component, Default)]
struct ChaseTarget {
    entity: Option<Entity>,
    reevaluate_timer: f32,
}

fn chase_action_system(mut commands: Commands, mut query: Query<(&Actor, &mut ActionState, &ActionSpan)>) {
    for (Actor(actor), mut state, span) in &mut query {
        let _guard = span.span().enter();
        match *state {

            ActionState::Requested => {
                commands.entity(*actor).try_insert((Chase, ChaseTarget::default()));
                *state = ActionState::Executing;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).try_remove::<(Chase, ChaseTarget)>();
                *state = ActionState::Success;
            }
            _ => {}
        }
    }
}

fn turn_towards(rotation: &mut Quat, target: Quat, max_angle: f32) {
    let angle = rotation.angle_between(target);
    if angle <= max_angle || angle <= f32::EPSILON {
        *rotation = target;
    } else {
        *rotation = rotation.slerp(target, max_angle / angle);
    }
}

fn steer_around_obstacles(
    spatial_query: &SpatialQuery,
    origin: Vec3,
    desired_direction: Vec3,
    distance_to_target: f32,
    filter: &SpatialQueryFilter,
) -> Vec3 {
    let probe_distance = ZOMBIE_AVOID_PROBE_DISTANCE.min(distance_to_target);
    let is_clear = |direction: Vec3| {
        Dir3::new(direction).is_ok_and(|dir| spatial_query.cast_ray(origin, dir, probe_distance, false, filter).is_none())
    };

    if is_clear(desired_direction) {
        return desired_direction;
    }
    for angle_deg in ZOMBIE_AVOID_ANGLES_DEG {
        let rotated = Quat::from_rotation_y(angle_deg.to_radians()) * desired_direction;
        if is_clear(rotated) {
            return rotated;
        }
    }
    desired_direction
}

fn move_zombies(
    time: Res<Time>,
    freeze: Res<FreezeZombiesServer>,
    mut zombies: Query<
        (
            Entity,
            &mut Transform,
            &mut ZombieMotion,
            &mut ChaseTarget,
            &mut KnockbackVelocity,
            Option<&mut AgentTarget3d>,
            Option<&AgentDesiredVelocity3d>,
        ),
        (With<Zombie>, With<Chase>),
    >,
    players: Query<(Entity, &Transform, &PlayerThreat), (With<Player>, Without<Zombie>)>,
    mover: MoveAndSlide,
) {

    if freeze.0 {
        for (_, _, mut motion, ..) in &mut zombies {
            motion.moving = false;
        }
        return;
    }

    let positions: Vec<(Entity, Vec3)> = zombies.iter().map(|(e, t, ..)| (e, t.translation)).collect();

    let candidates: Vec<(Entity, Vec3, f32)> = players.iter().map(|(e, t, threat)| (e, t.translation, threat.0)).collect();

    let mut chase_load: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    for (.., chase_target, _, _, _) in &zombies {
        if let Some(target) = chase_target.entity {
            *chase_load.entry(target).or_insert(0) += 1;
        }
    }

    for (entity, mut transform, mut motion, mut chase_target, mut knockback, mut agent_target, desired_velocity) in
        &mut zombies
    {

        chase_target.reevaluate_timer -= time.delta_secs();
        let target_still_connected = chase_target.entity.is_some_and(|target| candidates.iter().any(|&(e, ..)| e == target));
        if !target_still_connected || chase_target.reevaluate_timer <= 0.0 {

            chase_target.entity = candidates
                .iter()
                .min_by(|(a_entity, a_pos, a_threat), (b_entity, b_pos, b_threat)| {
                    let a_load = *chase_load.get(a_entity).unwrap_or(&0) as f32;
                    let b_load = *chase_load.get(b_entity).unwrap_or(&0) as f32;
                    let a_score = transform.translation.distance(*a_pos) - a_threat * ZOMBIE_THREAT_DISTANCE_BONUS
                        + a_load * ZOMBIE_LOAD_BALANCE_PENALTY;
                    let b_score = transform.translation.distance(*b_pos) - b_threat * ZOMBIE_THREAT_DISTANCE_BONUS
                        + b_load * ZOMBIE_LOAD_BALANCE_PENALTY;
                    a_score.total_cmp(&b_score)
                })
                .map(|&(e, ..)| e);

            chase_target.reevaluate_timer = ZOMBIE_RETARGET_INTERVAL * (0.75 + entity_phase(entity) * 0.5);
        }
        let Some(target_translation) =
            chase_target.entity.and_then(|target| candidates.iter().find(|&&(e, ..)| e == target)).map(|&(_, p, _)| p)
        else {
            continue;
        };

        let to_target = target_translation - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();

        if let Some(agent_target) = agent_target.as_deref_mut() {
            *agent_target = AgentTarget3d::Point(target_translation);
        }

        let filter = SpatialQueryFilter::default();
        let horizontal_velocity = if distance > ZOMBIE_STOP_DISTANCE {
            motion.path_recalc_timer -= time.delta_secs();
            if motion.path_recalc_timer <= 0.0 {
                motion.path_recalc_timer = ZOMBIE_PATH_RECALC_INTERVAL;

                let path_direction = desired_velocity
                    .map(|v| v.velocity())
                    .map(|v| Vec3::new(v.x, 0.0, v.z).normalize_or_zero())
                    .filter(|direction| *direction != Vec3::ZERO);

                let desired_direction = path_direction.or_else(|| {
                    Some(
                        Vec3::new(
                            target_translation.x - transform.translation.x,
                            0.0,
                            target_translation.z - transform.translation.z,
                        )
                        .normalize_or_zero(),
                    )
                    .filter(|direction| *direction != Vec3::ZERO)
                });

                let mut separation = Vec3::ZERO;
                for (other_entity, other_pos) in &positions {
                    if *other_entity == entity {
                        continue;
                    }
                    let away = transform.translation - *other_pos;
                    let away = Vec3::new(away.x, 0.0, away.z);
                    let dist_sq = away.length_squared();
                    if dist_sq > 1e-6 && dist_sq < ZOMBIE_SEPARATION_RADIUS * ZOMBIE_SEPARATION_RADIUS {
                        let dist = dist_sq.sqrt();
                        separation += (away / dist) * (1.0 - dist / ZOMBIE_SEPARATION_RADIUS);
                    }
                }
                let desired_direction = desired_direction.map(|d| (d + separation * ZOMBIE_SEPARATION_STRENGTH).normalize_or_zero());

                motion.cached_path_dir = desired_direction
                    .map(|d| steer_around_obstacles(&mover.spatial_query, transform.translation, d, distance, &filter))
                    .unwrap_or(Vec3::ZERO);
            }
            motion.cached_path_dir * zombie_speed_for(distance)
        } else {
            Vec3::ZERO
        };

        if motion.grounded && motion.vertical_velocity < 0.0 {
            motion.vertical_velocity = -1.0;
        }
        motion.vertical_velocity += GRAVITY * time.delta_secs();

        let decay = (KNOCKBACK_DECAY_RATE * time.delta_secs()).min(1.0);
        knockback.0 *= 1.0 - decay;

        let velocity = horizontal_velocity + knockback.0 + Vec3::Y * motion.vertical_velocity;

        let shape = Collider::capsule(ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT - 2.0 * ZOMBIE_MOVE_RADIUS);
        let config = MoveAndSlideConfig::default();

        let was_grounded = motion.grounded;
        let mut grounded_this_frame = false;
        let output = mover.move_and_slide(
            &shape,
            transform.translation,
            transform.rotation,
            velocity,
            time.delta(),
            &config,
            &filter,
            |mut hit| {
                if resolve_slope_hit(&mut hit, velocity, was_grounded) {
                    grounded_this_frame = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );

        let dt = time.delta_secs();
        let step_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
        let blocked = was_grounded && blocked_horizontally(transform.translation, output.position, step_velocity, dt);
        let stepped_up = blocked
            .then(|| {
                try_step_up(
                    &mover,
                    &shape,
                    transform.rotation,
                    transform.translation,
                    step_velocity,
                    &config,
                    &filter,
                )
            })
            .flatten();

        transform.translation = stepped_up.unwrap_or(output.position);
        motion.grounded = grounded_this_frame || stepped_up.is_some();
        motion.vertical_velocity = if motion.grounded { motion.vertical_velocity.min(0.0) } else { motion.vertical_velocity };

        if horizontal_velocity != Vec3::ZERO {
            let target_rotation = Transform::default().looking_to(-horizontal_velocity, Vec3::Y).rotation;
            turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
        }

        motion.moving = horizontal_velocity != Vec3::ZERO;
    }
}

#[derive(Resource, Default)]
struct PendingRespawns(Vec<PendingRespawn>);

struct PendingRespawn {
    timer: f32,
}

fn tick_respawns(
    time: Res<Time>,
    mut commands: Commands,
    scenario: Res<Scenario>,
    mut respawns: ResMut<PendingRespawns>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    auto_maintain: Res<AutoMaintainZombies>,
    points: Res<ZombieSpawnPoints>,
) {
    let kind = *scenario;
    let dt = time.delta_secs();
    let mut i = 0;
    let mut to_spawn = Vec::new();
    while i < respawns.0.len() {
        respawns.0[i].timer -= dt;
        if respawns.0[i].timer <= 0.0 {
            respawns.0.swap_remove(i);
            if auto_maintain.0 {

                to_spawn.push(zombie_spawn_pos_for(kind, next_spawn_index.0, &points.0));
                next_spawn_index.0 += 1;
            }
        } else {
            i += 1;
        }
    }
    if !to_spawn.is_empty() {
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED);
    }
}

fn handle_fire_shot(
    trigger: On<FireShot>,
    mut commands: Commands,
    spatial_query: SpatialQuery,
    bone_globals: Query<&GlobalTransform>,
    mut zombies: Query<(Entity, &Transform, &ZombieHitboxBones, &mut Health, &mut KnockbackVelocity), With<Zombie>>,
    mut respawns: ResMut<PendingRespawns>,
    mut threats: Query<&mut PlayerThreat>,
    damage_multiplier: Res<DamageMultiplierServer>,
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
    let wall_filter = SpatialQueryFilter::from_excluded_entities(zombies.iter().map(|(e, ..)| e));
    let wall_distance = spatial_query
        .cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &wall_filter)
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
        if let Ok((_, _, _, mut health, mut knockback)) = zombies.get_mut(entity) {
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                commands.entity(entity).despawn();
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
struct Dead;

fn on_player_died(
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

fn on_request_restart(
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
