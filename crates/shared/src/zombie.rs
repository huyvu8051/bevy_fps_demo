
use bevy::prelude::*;
use serde::{Deserialize, Serialize};


#[derive(Component, Default, Serialize, Deserialize)]
#[require(Transform)]
pub struct Zombie;

#[derive(Component, Default)]
pub struct ZombieMotion {
    pub vertical_velocity: f32,
    pub grounded: bool,
    pub path_recalc_timer: f32,
    pub cached_path_dir: Vec3,
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

pub fn zombie_hitbox_bone_slots(kind: ZombieKind) -> &'static [(&'static str, HitboxSetter)] {
    match kind {
        ZombieKind::Classic | ZombieKind::Soldier | ZombieKind::Female => CLASSIC_HITBOX_SLOTS,
    }
}

pub fn wire_zombie_hitbox_bones(
    child_of: Query<&ChildOf>,
    mut zombies: Query<(&mut ZombieHitboxBones, Option<&ZombieKind>), With<Zombie>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (bone_entity, name) in &new_names {
        let n = name.as_str();

        if !CLASSIC_HITBOX_SLOTS.iter().any(|&(s, _)| s == n) {
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

const ZOMBIE_WORLD_HEIGHT: f32 = 2.0367;

pub const ZOMBIE_RADIUS: f32 = 0.213025;

pub const ZOMBIE_LENGTH: f32 = ZOMBIE_WORLD_HEIGHT - 2.0 * ZOMBIE_RADIUS;

pub const ZOMBIE_OUTER_RADIUS: f32 = ZOMBIE_RADIUS * 4.0;
pub const ZOMBIE_OUTER_HEIGHT: f32 = ZOMBIE_WORLD_HEIGHT;

pub const ZOMBIE_MOVE_RADIUS: f32 = 0.3;

pub const ZOMBIE_PATH: &str = "characters/zombie/mesh/zombie.glb";

pub const ZOMBIE_SOLDIER_PATH: &str = "characters/zombie_soldier/mesh/zombie_soldier.glb";

pub const ZOMBIE_FEMALE_PATH: &str = "characters/zombie_female/mesh/zombie_female.glb";

pub fn zombie_kind_anim_path(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => ZOMBIE_PATH,
        ZombieKind::Soldier => ZOMBIE_SOLDIER_PATH,
        ZombieKind::Female => ZOMBIE_FEMALE_PATH,
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub enum ZombieKind {
    #[default]
    Classic,
    Soldier,
    Female,
}

pub const ZOMBIE_KIND_WEIGHTS: [f32; 3] = [1.0, 1.0, 1.0];

pub fn zombie_kind_for_phase(phase: f32, weights: [f32; 3]) -> ZombieKind {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return ZombieKind::default();
    }
    let target = phase * total;
    let mut cumulative = 0.0;
    for (kind, &weight) in
        [ZombieKind::Classic, ZombieKind::Soldier, ZombieKind::Female].into_iter().zip(&weights)
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
        ZombieKind::Soldier => ZOMBIE_SOLDIER_PATH,
        ZombieKind::Female => ZOMBIE_FEMALE_PATH,
    }
}

pub fn zombie_kind_clip_count(kind: ZombieKind) -> usize {
    match kind {
        ZombieKind::Classic => 21,
        ZombieKind::Soldier => 3,
        ZombieKind::Female => 4,
    }
}

pub fn zombie_kind_pose_clips(kind: ZombieKind) -> [usize; ZombiePose::COUNT] {
    match kind {
        ZombieKind::Classic => [20, 16, 0, 20, 2],
        ZombieKind::Soldier | ZombieKind::Female => [2, 1, 0, 2, 2],
    }
}

pub fn zombie_kind_metal_model_name(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => "zombie_classic",
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

impl ZombiePose {
    pub const COUNT: usize = 5;

    pub fn clip_slot(self) -> usize {
        match self {
            ZombiePose::Idle => 0,
            ZombiePose::Walk => 1,
            ZombiePose::Run => 2,
            ZombiePose::Crawl => 3,
            ZombiePose::Bite => 4,
        }
    }
}

pub struct ZombieKindAnim {
    pub graph: Handle<AnimationGraph>,
    poses: [AnimationNodeIndex; ZombiePose::COUNT],
}

impl ZombieKindAnim {
    pub fn node(&self, pose: ZombiePose) -> AnimationNodeIndex {
        self.poses[pose.clip_slot()]
    }

    pub fn idle(&self) -> AnimationNodeIndex {
        self.poses[ZombiePose::Idle.clip_slot()]
    }
}

#[derive(Resource)]
pub struct ZombieAnim {
    classic: ZombieKindAnim,
    soldier: ZombieKindAnim,
    female: ZombieKindAnim,
}

impl ZombieAnim {
    pub fn slot(&self, kind: ZombieKind) -> &ZombieKindAnim {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Soldier => &self.soldier,
            ZombieKind::Female => &self.female,
        }
    }

    pub fn build(
        graphs: &mut Assets<AnimationGraph>,
        mut load_clip: impl FnMut(ZombieKind, usize) -> Handle<AnimationClip>,
    ) -> Self {
        let mut build_kind = |kind: ZombieKind| {
            let mut graph = AnimationGraph::new();
            let root = graph.root;
            let poses = zombie_kind_pose_clips(kind)
                .map(|index| graph.add_clip(load_clip(kind, index), 1.0, root));
            ZombieKindAnim { graph: graphs.add(graph), poses }
        };
        Self {
            classic: build_kind(ZombieKind::Classic),
            soldier: build_kind(ZombieKind::Soldier),
            female: build_kind(ZombieKind::Female),
        }
    }
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

pub const ZOMBIE_REST_HEIGHT: f32 = ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0;
pub const ZOMBIE_AVOID_PROBE_DISTANCE: f32 = 2.5;
pub const ZOMBIE_AVOID_ANGLES_DEG: [f32; 6] = [25.0, -25.0, 50.0, -50.0, 80.0, -80.0];
pub const ZOMBIE_SEPARATION_RADIUS: f32 = 1.1;
pub const ZOMBIE_SEPARATION_STRENGTH: f32 = 1.2;
pub const KNOCKBACK_DECAY_RATE: f32 = 6.0;

pub const ZOMBIE_TURN_RATE_DEG: f32 = 180.0;

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

pub const KNOCKBACK_FORCE: f32 = 8.75;
