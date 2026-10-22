use bevy::prelude::*;

use crate::*;

#[derive(Resource)]
pub(crate) struct GunshotSound(pub(crate) Vec<Handle<AudioSource>>);

impl GunshotSound {
    pub(crate) fn of(&self, weapon: WeaponId) -> Handle<AudioSource> {
        self.0[weapon.index()].clone()
    }
}

#[derive(Resource)]
pub(crate) struct ReloadSound(pub(crate) Vec<Handle<AudioSource>>);

impl ReloadSound {
    pub(crate) fn of(&self, weapon: WeaponId) -> Handle<AudioSource> {
        self.0[weapon.index()].clone()
    }
}

#[derive(Resource)]
pub(crate) struct FootstepSounds {
    pub(crate) player: Vec<Handle<AudioSource>>,
    pub(crate) zombie: Vec<Handle<AudioSource>>,
}

#[derive(Component)]
pub(crate) struct GunshotVoice;

#[derive(Component)]
pub(crate) struct ZombieFootstepVoice;

pub(crate) const PLAYER_FOOTSTEP_SURFACE: &str = "gravel";
pub(crate) const PLAYER_FOOTSTEP_SAMPLES: usize = 10;
pub(crate) const ZOMBIE_FOOTSTEP_SURFACE: &str = "grass";
pub(crate) const ZOMBIE_FOOTSTEP_SAMPLES: usize = 9;

const FOOTSTEP_WALK_INTERVAL: f32 = 0.45;
const FOOTSTEP_RUN_INTERVAL: f32 = 0.30;
const FOOTSTEP_PLAYER_VOLUME: f32 = 0.45;
const FOOTSTEP_PITCH_SPREAD: f32 = 0.12;

const ZOMBIE_FOOTFALL_PHASES: [f32; 2] = [0.0, 0.5];
const ZOMBIE_FOOTSTEP_VOLUME: f32 = 1.5;
const ZOMBIE_FOOTSTEP_SPATIAL_SCALE: f32 = 0.15;
const ZOMBIE_FOOTSTEP_AUDIBLE_RANGE: f32 = 25.0;
const ZOMBIE_FOOTSTEP_MAX_VOICES: usize = 4;
const ZOMBIE_FOOTSTEP_MIN_GAP: f32 = 0.07;
const GUNSHOT_DUCK_FACTOR: f32 = 0.35;

fn hash01(seed: f32) -> f32 {
    (ops::sin(seed) * 43758.547).fract().abs()
}

fn pick_sample(samples: &[Handle<AudioSource>], seed: f32) -> Option<Handle<AudioSource>> {
    if samples.is_empty() {
        return None;
    }
    let index = (hash01(seed) * samples.len() as f32) as usize % samples.len();
    samples.get(index).cloned()
}

fn pitch_jitter(seed: f32) -> f32 {
    1.0 + (hash01(seed) - 0.5) * 2.0 * FOOTSTEP_PITCH_SPREAD
}

pub(crate) fn play_footsteps(
    time: Res<Time>,
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    sounds: Option<Res<FootstepSounds>>,
    player: Query<&Player, Without<NetPlayer>>,
    mut until_next: Local<f32>,
    mut commands: Commands,
) {
    let Some(sounds) = sounds else {
        return;
    };
    let grounded = player.single().map(|p| p.grounded).unwrap_or(false);
    if !grounded || !moving.0 {
        *until_next = 0.0;
        return;
    }

    *until_next -= time.delta_secs();
    if *until_next > 0.0 {
        return;
    }
    *until_next = if sprinting.0 { FOOTSTEP_RUN_INTERVAL } else { FOOTSTEP_WALK_INTERVAL };

    let seed = time.elapsed_secs() * 311.7;
    let Some(sample) = pick_sample(&sounds.player, seed) else {
        return;
    };
    commands.spawn((
        AudioPlayer(sample),
        PlaybackSettings::DESPAWN
            .with_volume(bevy::audio::Volume::Linear(FOOTSTEP_PLAYER_VOLUME))
            .with_speed(pitch_jitter(seed * 1.7)),
    ));
}

fn animation_phase(
    anim_players: &Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    graphs: &Assets<AnimationGraph>,
    clips: &Assets<AnimationClip>,
    anim_entity: Entity,
) -> Option<f32> {
    let (player, graph_handle) = anim_players.get(anim_entity).ok()?;
    let (node, active) = player.playing_animations().find(|(_, active)| !active.is_paused())?;
    let graph = graphs.get(&graph_handle.0)?;
    let AnimationNodeType::Clip(clip) = &graph.get(*node)?.node_type else {
        return None;
    };
    let duration = clips.get(clip)?.duration();
    if duration <= 0.0 {
        return None;
    }
    Some((active.seek_time() / duration).rem_euclid(1.0))
}

fn crossed_footfall(previous: f32, current: f32) -> bool {
    if current < previous {
        return true;
    }
    ZOMBIE_FOOTFALL_PHASES
        .iter()
        .any(|boundary| *boundary > 0.0 && previous < *boundary && current >= *boundary)
}

pub(crate) fn play_zombie_footsteps(
    time: Res<Time>,
    sounds: Option<Res<FootstepSounds>>,
    listener: Query<&GlobalTransform, With<FpsCamera>>,
    zombies: Query<(Entity, &Transform, &ZombieMotion, &ZombieAnimationPlayer), With<Zombie>>,
    anim_players: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    graphs: Res<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    live_voices: Query<(), With<ZombieFootstepVoice>>,
    gunshots: Query<(), With<GunshotVoice>>,
    mut last_phase: Local<std::collections::HashMap<Entity, f32>>,
    mut last_step_at: Local<f32>,
    mut commands: Commands,
) {
    let Some(sounds) = sounds else {
        return;
    };
    let Ok(ear) = listener.single() else {
        return;
    };
    let ear = ear.translation();
    let now = time.elapsed_secs();

    last_phase.retain(|entity, _| zombies.contains(*entity));

    let mut due: Vec<(f32, Entity)> = Vec::new();
    for (entity, transform, motion, anim_link) in &zombies {
        let distance = transform.translation.distance(ear);
        if !motion.moving || distance > ZOMBIE_FOOTSTEP_AUDIBLE_RANGE {
            last_phase.remove(&entity);
            continue;
        }
        let Some(phase) = animation_phase(&anim_players, &graphs, &clips, anim_link.0) else {
            continue;
        };
        let Some(previous) = last_phase.insert(entity, phase) else {
            continue;
        };
        if crossed_footfall(previous, phase) {
            due.push((distance, entity));
        }
    }

    if due.is_empty() || now - *last_step_at < ZOMBIE_FOOTSTEP_MIN_GAP {
        return;
    }

    let budget = ZOMBIE_FOOTSTEP_MAX_VOICES.saturating_sub(live_voices.iter().count());
    if budget == 0 {
        return;
    }

    due.sort_by(|(a, _), (b, _)| a.total_cmp(b));
    let ducked = !gunshots.is_empty();

    for (_, entity) in due.into_iter().take(budget) {
        let Ok((_, transform, _, _)) = zombies.get(entity) else {
            continue;
        };
        let seed = now * 97.3 + shared::entity_phase(entity) * 51.0;
        let Some(sample) = pick_sample(&sounds.zombie, seed) else {
            continue;
        };
        let volume = if ducked { ZOMBIE_FOOTSTEP_VOLUME * GUNSHOT_DUCK_FACTOR } else { ZOMBIE_FOOTSTEP_VOLUME };
        commands.spawn((
            AudioPlayer(sample),
            PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(volume))
                .with_speed(pitch_jitter(seed * 2.3))
                .with_spatial(true)
                .with_spatial_scale(bevy::audio::SpatialScale::new(ZOMBIE_FOOTSTEP_SPATIAL_SCALE)),
            Transform::from_translation(transform.translation),
            ZombieFootstepVoice,
        ));
        *last_step_at = now;
    }
}
