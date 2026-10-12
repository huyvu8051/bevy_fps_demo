
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::ARENA_HALF_SIZE;

#[derive(Component, Default, Serialize, Deserialize)]
#[require(Transform)]
pub struct Zombie;

#[derive(Component, Default)]
pub struct ZombieMotion {
    pub vertical_velocity: f32,
    pub grounded: bool,
    pub path_recalc_timer: f32,
    pub cached_path_dir: Vec3,

    pub far: bool,
}

#[derive(Component, Default)]
pub struct KnockbackVelocity(pub Vec3);

#[derive(Component, Default)]
pub struct ZombieHitboxBones {
    pub head: Option<Entity>,
    pub belly: Option<Entity>,
    pub chest: Option<Entity>,
    pub left_arm: [Option<Entity>; 3],
    pub right_arm: [Option<Entity>; 3],
    pub left_leg: [Option<Entity>; 3],
    pub right_leg: [Option<Entity>; 3],
}

type HitboxSetter = fn(&mut ZombieHitboxBones, Entity);

const CLASSIC_HITBOX_SLOTS: &[(&str, HitboxSetter)] = &[
    ("mixamorig:Head", |b, e| b.head = Some(e)),
    ("mixamorig:Spine", |b, e| b.belly = Some(e)),
    ("mixamorig:Spine2", |b, e| b.chest = Some(e)),
    ("mixamorig:LeftArm", |b, e| b.left_arm[0] = Some(e)),
    ("mixamorig:LeftForeArm", |b, e| b.left_arm[1] = Some(e)),
    ("mixamorig:LeftHand", |b, e| b.left_arm[2] = Some(e)),
    ("mixamorig:RightArm", |b, e| b.right_arm[0] = Some(e)),
    ("mixamorig:RightForeArm", |b, e| b.right_arm[1] = Some(e)),
    ("mixamorig:RightHand", |b, e| b.right_arm[2] = Some(e)),
    ("mixamorig:LeftUpLeg", |b, e| b.left_leg[0] = Some(e)),
    ("mixamorig:LeftLeg", |b, e| b.left_leg[1] = Some(e)),
    ("mixamorig:LeftFoot", |b, e| b.left_leg[2] = Some(e)),
    ("mixamorig:RightUpLeg", |b, e| b.right_leg[0] = Some(e)),
    ("mixamorig:RightLeg", |b, e| b.right_leg[1] = Some(e)),
    ("mixamorig:RightFoot", |b, e| b.right_leg[2] = Some(e)),
];

const MESHY_HITBOX_SLOTS: &[(&str, HitboxSetter)] = &[
    ("Head", |b, e| b.head = Some(e)),
    ("Spine", |b, e| b.belly = Some(e)),
    ("Spine02", |b, e| b.chest = Some(e)),
    ("LeftArm", |b, e| b.left_arm[0] = Some(e)),
    ("LeftForeArm", |b, e| b.left_arm[1] = Some(e)),
    ("LeftHand", |b, e| b.left_arm[2] = Some(e)),
    ("RightArm", |b, e| b.right_arm[0] = Some(e)),
    ("RightForeArm", |b, e| b.right_arm[1] = Some(e)),
    ("RightHand", |b, e| b.right_arm[2] = Some(e)),
    ("LeftUpLeg", |b, e| b.left_leg[0] = Some(e)),
    ("LeftLeg", |b, e| b.left_leg[1] = Some(e)),
    ("LeftFoot", |b, e| b.left_leg[2] = Some(e)),
    ("RightUpLeg", |b, e| b.right_leg[0] = Some(e)),
    ("RightLeg", |b, e| b.right_leg[1] = Some(e)),
    ("RightFoot", |b, e| b.right_leg[2] = Some(e)),
];

pub fn zombie_hitbox_bone_slots(kind: ZombieKind) -> &'static [(&'static str, HitboxSetter)] {
    match kind {
        ZombieKind::Classic | ZombieKind::Female => CLASSIC_HITBOX_SLOTS,

        ZombieKind::Meshy | ZombieKind::Soldier => MESHY_HITBOX_SLOTS,
    }
}

pub fn wire_zombie_hitbox_bones(
    child_of: Query<&ChildOf>,
    mut zombies: Query<(&mut ZombieHitboxBones, Option<&ZombieKind>), With<Zombie>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (bone_entity, name) in &new_names {
        let n = name.as_str();

        if n != "Spine02" && !CLASSIC_HITBOX_SLOTS.iter().any(|&(s, _)| s == n) {
            continue;
        }

        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok((mut bones, kind)) = zombies.get_mut(current) {
                let slots = zombie_hitbox_bone_slots(kind.copied().unwrap_or_default());
                if let Some(&(_, set)) = slots.iter().find(|&&(s, _)| s == n) {
                    set(&mut bones, bone_entity);
                }
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

pub const ZOMBIE_ATTACK_INTERVAL: f32 = 1.0;

#[derive(Component)]
pub struct ZombieAttackTimer(pub f32);

impl Default for ZombieAttackTimer {
    fn default() -> Self {
        Self(ZOMBIE_ATTACK_INTERVAL)
    }
}

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct Chase;

pub const ZOMBIE_AI_ENABLED: bool = true;

pub const ZOMBIE_CHASE_RANGE: f32 = 60.0;

pub const ZOMBIE_WALK_SPEED: f32 = 1.5;
pub const ZOMBIE_RUN_SPEED: f32 = 3.0;

pub fn zombie_speed_for(distance: f32) -> f32 {
    if distance <= ZOMBIE_RUN_DISTANCE {
        ZOMBIE_RUN_SPEED
    } else {
        ZOMBIE_WALK_SPEED
    }
}

pub const ZOMBIE_SCALE: f32 = 0.25;

const ZOMBIE_MESH_REFERENCE_HEIGHT: f32 = 8.1468;
const ZOMBIE_WORLD_HEIGHT: f32 = ZOMBIE_MESH_REFERENCE_HEIGHT * ZOMBIE_SCALE;

pub(crate) const ZOMBIE_SCALE_FACTOR: f32 = ZOMBIE_SCALE / 0.1;

const ZOMBIE_MESH_REFERENCE_TORSO_WIDTH: f32 = 1.7042;

pub const ZOMBIE_RADIUS: f32 = ZOMBIE_MESH_REFERENCE_TORSO_WIDTH * ZOMBIE_SCALE / 2.0;

pub const ZOMBIE_LENGTH: f32 = ZOMBIE_WORLD_HEIGHT - 2.0 * ZOMBIE_RADIUS;

pub const ZOMBIE_OUTER_RADIUS: f32 = ZOMBIE_RADIUS * 4.0;
pub const ZOMBIE_OUTER_HEIGHT: f32 = ZOMBIE_WORLD_HEIGHT;

pub const ZOMBIE_MOVE_RADIUS: f32 = 0.3;

pub const ZOMBIE_MODEL_Y_OFFSET: f32 = -(ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0);

pub const ZOMBIE_PATH: &str = "characters/zombie/mesh/zombie.glb";

pub const ZOMBIE_ANIM_BITE: usize = 0;
pub const ZOMBIE_ANIM_CRAWL: usize = 1;
pub const ZOMBIE_ANIM_IDLE: usize = 2;
pub const ZOMBIE_ANIM_RUN: usize = 3;
pub const ZOMBIE_ANIM_WALK: usize = 4;

pub const ZOMBIE_MESHY_PATH: &str = "characters/zombie_meshy/mesh/zombie_meshy.glb";

pub const ZOMBIE_SOLDIER_PATH: &str = "characters/zombie_soldier/mesh/zombie_soldier.glb";

pub const ZOMBIE_SOLDIER_ANIM_PATH: &str = "characters/zombie_soldier/anims/soldier_running.glb";

/// A Meshy-AI model generated with the Mixamo rig option, so its bones share
/// the `mixamorig:` names Classic's were renamed to.
pub const ZOMBIE_FEMALE_PATH: &str = "characters/zombie_female/mesh/zombie_female.glb";

/// `zombie.glb`'s clips, re-rooted so cross-file reuse actually binds.
///
/// Bevy bakes a clip's curve target paths starting at the glTF's own
/// scene-root node name; `zombie.glb`'s root is "RootNode > Zombie > bone",
/// but `wire_zombie_scene_animation_targets` roots live-spawned paths at
/// whatever node is named "target_character" directly above the bones. This
/// is `zombie.glb` with its scene pointed at the "Zombie" node, renamed to
/// "target_character" — same curve data, baked under the matching name chain.
pub const ZOMBIE_CLASSIC_POSES_ANIM_PATH: &str = "characters/zombie/anims/classic_poses.glb";

pub fn zombie_kind_anim_path(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => ZOMBIE_PATH,
        ZombieKind::Meshy => ZOMBIE_MESHY_PATH,
        ZombieKind::Soldier => ZOMBIE_SOLDIER_ANIM_PATH,
        ZombieKind::Female => ZOMBIE_CLASSIC_POSES_ANIM_PATH,
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub enum ZombieKind {
    #[default]
    Classic,
    Meshy,
    Soldier,
    Female,
}

pub const ZOMBIE_KIND_WEIGHTS: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub fn zombie_kind_for_phase(phase: f32, weights: [f32; 4]) -> ZombieKind {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return ZombieKind::default();
    }
    let target = phase * total;
    let mut cumulative = 0.0;
    for (kind, &weight) in
        [ZombieKind::Classic, ZombieKind::Meshy, ZombieKind::Soldier, ZombieKind::Female].into_iter().zip(&weights)
    {
        cumulative += weight;
        if target < cumulative {
            return kind;
        }
    }
    ZombieKind::Female
}

impl std::fmt::Display for ZombieKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ZombieKind::Classic => "classic",
            ZombieKind::Meshy => "meshy",
            ZombieKind::Soldier => "soldier",
            ZombieKind::Female => "female",
        })
    }
}

#[derive(Component, Clone, Copy)]
pub struct ZombieShowcaseClip(pub usize);

pub fn zombie_kind_mesh_path(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => ZOMBIE_PATH,
        ZombieKind::Meshy => ZOMBIE_MESHY_PATH,
        ZombieKind::Soldier => ZOMBIE_SOLDIER_PATH,
        ZombieKind::Female => ZOMBIE_FEMALE_PATH,
    }
}

/// `None` for kinds that ship their own material instead of the tiered
/// `base_color_qN.jpg` set the other kinds swap between.
pub fn zombie_kind_texture_path(kind: ZombieKind, tier: u8) -> Option<String> {
    let dir = match kind {
        ZombieKind::Classic => "zombie",
        ZombieKind::Meshy => "zombie_meshy",
        ZombieKind::Soldier => "zombie_soldier",
        ZombieKind::Female => return None,
    };
    Some(format!("characters/{dir}/mesh/textures/base_color_q{}.jpg", tier.min(5)))
}

pub fn zombie_kind_clip_count(kind: ZombieKind) -> usize {
    match kind {
        ZombieKind::Classic => 5,
        ZombieKind::Meshy => 7,

        ZombieKind::Soldier => 1,
        ZombieKind::Female => 5,
    }
}

pub fn zombie_kind_pose_clips(kind: ZombieKind) -> [usize; 5] {
    match kind {
        ZombieKind::Classic => [
            ZOMBIE_ANIM_IDLE,
            ZOMBIE_ANIM_WALK,
            ZOMBIE_ANIM_RUN,
            ZOMBIE_ANIM_CRAWL,
            ZOMBIE_ANIM_BITE,
        ],
        ZombieKind::Meshy => [0, 6, 5, 0, 0],
        ZombieKind::Soldier => [0, 0, 0, 0, 0],
        ZombieKind::Female => [
            ZOMBIE_ANIM_IDLE,
            ZOMBIE_ANIM_WALK,
            ZOMBIE_ANIM_RUN,
            ZOMBIE_ANIM_CRAWL,
            ZOMBIE_ANIM_BITE,
        ],
    }
}

pub fn zombie_kind_metal_model_name(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => "zombie_classic",
        ZombieKind::Meshy => "zombie_meshy",
        ZombieKind::Soldier => "zombie_soldier",
        ZombieKind::Female => "zombie_female",
    }
}

pub const ZOMBIE_PATH_RECALC_INTERVAL: f32 = 0.8;

pub const ZOMBIE_RETARGET_INTERVAL: f32 = 1.5;

pub const PLAYER_THREAT_PER_SHOT: f32 = 1.0;
pub const PLAYER_THREAT_DECAY_RATE: f32 = 0.5;
pub const ZOMBIE_THREAT_DISTANCE_BONUS: f32 = 8.0;

pub const ZOMBIE_LOAD_BALANCE_PENALTY: f32 = 1.0;

pub const DIRECTOR_MELEE_RANGE: f32 = 2.5;

pub const DIRECTOR_PRESSURE_SMOOTHING: f32 = 0.5;
pub const DIRECTOR_CALM_PRESSURE_THRESHOLD: f32 = 1.0;
pub const DIRECTOR_CALM_DURATION_SECS: f32 = 20.0;
pub const DIRECTOR_HORDE_SIZE: usize = 10;
pub const DIRECTOR_HORDE_SPAWN_RADIUS: f32 = 6.0;
pub const ZOMBIE_STOP_DISTANCE: f32 = 1.2;

#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
#[reflect(Component)]
pub enum ZombiePose {
    #[default]
    Idle,
    Walk,
    Run,
    Crawl,
    Bite,
}

pub const ZOMBIE_CRAWL_HEALTH_FRACTION: f32 = 0.35;

pub const ZOMBIE_RUN_DISTANCE: f32 = 20.0;

#[derive(Component)]
pub struct ZombieAnimationPlayer(pub Entity);

pub fn find_animation_root(entity: Entity, child_of: &Query<&ChildOf>, is_root: impl Fn(Entity) -> bool) -> Option<Entity> {
    let mut current = entity;
    for _ in 0..20 {
        if is_root(current) {
            return Some(current);
        }
        current = child_of.get(current).ok()?.0;
    }
    None
}

pub fn zombie_pose_for(distance: f32, health_fraction: f32, moving: bool) -> ZombiePose {
    if distance <= ZOMBIE_STOP_DISTANCE {
        ZombiePose::Bite
    } else if health_fraction <= ZOMBIE_CRAWL_HEALTH_FRACTION {
        ZombiePose::Crawl
    } else if moving && distance <= ZOMBIE_RUN_DISTANCE {
        ZombiePose::Run
    } else if moving {
        ZombiePose::Walk
    } else {
        ZombiePose::Idle
    }
}

pub const ZOMBIE_LOD_DISTANCE: f32 = 15.0;

pub const ZOMBIE_LOD_REST_HEIGHT: f32 = ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0;
pub const ZOMBIE_AVOID_PROBE_DISTANCE: f32 = 2.5;
pub const ZOMBIE_AVOID_ANGLES_DEG: [f32; 6] = [25.0, -25.0, 50.0, -50.0, 80.0, -80.0];
pub const ZOMBIE_SEPARATION_RADIUS: f32 = 1.1;
pub const ZOMBIE_SEPARATION_STRENGTH: f32 = 1.2;
pub const KNOCKBACK_DECAY_RATE: f32 = 6.0;

pub const ZOMBIE_TURN_RATE_DEG: f32 = 180.0;

pub const fn zombie_spawn_grid(half_x: f32, half_z: f32, y: f32) -> [Vec3; 100] {
    let mut spawns = [Vec3::ZERO; 100];
    let mut i = 0usize;
    while i < 100 {
        let row = (i / 10) as f32;
        let col = (i % 10) as f32;
        let x = -half_x + col * (2.0 * half_x / 9.0);
        let z = -half_z + row * (2.0 * half_z / 9.0);
        spawns[i] = Vec3::new(x, y, z);
        i += 1;
    }
    spawns
}

pub const ZOMBIE_SPAWNS: [Vec3; 100] =
    zombie_spawn_grid(ARENA_HALF_SIZE - 3.0, ARENA_HALF_SIZE - 3.0, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0);

pub fn random_zombie_spawn_pos(seed: usize) -> Vec3 {
    const INV_G: f32 = 0.754_877_7;
    const INV_G2: f32 = 0.569_840_3;
    let half = ARENA_HALF_SIZE - 3.0;
    let x = (-1.0 + 2.0 * (seed as f32 * INV_G).fract()) * half;
    let z = (-1.0 + 2.0 * (seed as f32 * INV_G2).fract()) * half;
    Vec3::new(x, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, z)
}

#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SpawnMoreZombies;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetZombieCount(pub u32);

#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ClearAllZombies;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetAutoMaintainZombies(pub bool);

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetFreezeZombies(pub bool);

pub const ZOMBIE_RESPAWN_DELAY: f32 = 1.0;

pub const KNOCKBACK_FORCE: f32 = 3.5 * ZOMBIE_SCALE_FACTOR;
