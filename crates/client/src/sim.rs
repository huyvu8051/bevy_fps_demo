use avian3d::prelude::*;
use bevy_landmass::prelude::*;
use bevy::prelude::*;
use crate::combat::*;
use crate::map::*;
use big_brain::prelude::*;
use shared::movement::{blocked_horizontally, resolve_slope_hit, try_step_up};
use shared::{
    entity_phase, Chase, ClearAllZombies,
    GameOver, Health, KnockbackVelocity,
    Player,
    SetAutoMaintainZombies, SetFreezeZombies, SetZombieCount, SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, ZombiePose, ZombieWalkVariant, zombie_speed_for, 
    GRAVITY, KNOCKBACK_DECAY_RATE, Scenario,
    
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    PLAYER_THREAT_DECAY_RATE, ZOMBIE_LOAD_BALANCE_PENALTY, zombie_kind_for_phase, ZOMBIE_KIND_WEIGHTS,
    ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT, ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS,
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
            // move_zombies is the one system here that costs real time -- a
            // swept capsule against the whole map per zombie, every call. In
            // Update that ran at the refresh rate; at a fixed 64Hz it runs
            // roughly half as often on a 120Hz panel, for the same motion,
            // since every velocity here is already multiplied by delta_secs.
            //
            // It loses `.after(LandmassSystems::Output)` by moving: landmass
            // is in Update, which runs after FixedUpdate, so the desired
            // velocity read here is one frame old. Paths are recalculated
            // every 0.8s and the target is re-chosen every 1.5s, so a frame
            // of staleness is well inside what those intervals already allow.
            .add_systems(FixedUpdate, move_zombies.run_if(world_ready))
            .add_systems(
                Update,
                (tick_respawns.run_if(world_ready), decay_player_threat),
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



#[derive(Component, Default)]
pub(crate) struct PlayerThreat(pub(crate) f32);

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

        crate::zombie_reaction::ZombieHeadLook::default(),

        ZombiePose::default(),
        ZombieWalkVariant::default(),

        Visibility::default(),
    ));
    if ai_enabled {
        zombie.insert(Thinker::build().picker(FirstToScore { threshold: 0.8 }).when(NearPlayer, ChaseAction));
    }
}

pub(crate) fn spawn_zombies(commands: &mut Commands, spawns: &[(Vec3, f32)], ai_enabled: bool) {
    for &(position, yaw) in spawns {
        spawn_zombie(commands, position, yaw, ai_enabled);
    }
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

        knockback.0 *= (-KNOCKBACK_DECAY_RATE * time.delta_secs()).exp();

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
pub(crate) struct PendingRespawns(pub(crate) Vec<PendingRespawn>);

pub(crate) struct PendingRespawn {
    pub(crate) timer: f32,
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




