use bevy::prelude::*;

use crate::*;
use shared::ZombieLimb;

/// Damage one limb soaks up before it comes off, in multiples of a single limb
/// hit. Torso and head damage never counts toward it.
const SEVER_DAMAGE: f32 = 1.0;
const SEVERED_BONE_SCALE: f32 = 0.001;
const STUMP_SPRAY_COUNT: usize = 18;
const STUMP_SPRAY_SPEED: f32 = 3.4;

#[derive(Resource, Default)]
pub(crate) struct LimbDamage(std::collections::HashMap<Entity, [LimbWound; 4]>);

#[derive(Clone, Copy, Default)]
struct LimbWound {
    damage: f32,
    /// Shallowest segment hit, so a shoulder shot takes the whole arm rather
    /// than leaving it hanging off the elbow.
    cut_at: usize,
    hit: bool,
}

impl LimbDamage {
    pub(crate) fn record(&mut self, zombie: Entity, limb: ZombieLimb, segment: usize, damage: f32) {
        let wound = &mut self.0.entry(zombie).or_default()[limb.index()];
        if !wound.hit || segment < wound.cut_at {
            wound.cut_at = segment;
        }
        wound.hit = true;
        wound.damage += damage;
    }

    pub(crate) fn severed(&self, zombie: Entity) -> Vec<SeveredLimb> {
        self.0
            .get(&zombie)
            .map(|wounds| {
                ZombieLimb::ALL
                    .into_iter()
                    .filter(|limb| wounds[limb.index()].damage >= SEVER_DAMAGE)
                    .map(|limb| SeveredLimb { limb, cut_at: wounds[limb.index()].cut_at, sprayed: false })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn forget(&mut self, zombie: Entity) {
        self.0.remove(&zombie);
    }
}

pub(crate) struct SeveredLimb {
    limb: ZombieLimb,
    cut_at: usize,
    sprayed: bool,
}

#[derive(Component, Default)]
pub(crate) struct SeveredLimbs(pub(crate) Vec<SeveredLimb>);

impl SeveredLimbs {
    /// Keeps the spray flags already set, so adding a second limb does not
    /// re-fire blood from the first.
    pub(crate) fn merge(&mut self, fresh: Vec<SeveredLimb>) {
        for cut in fresh {
            if !self.0.iter().any(|known| known.limb == cut.limb) {
                self.0.push(cut);
            }
        }
    }
}

fn limb_chain(bones: &ZombieHitboxBones, limb: ZombieLimb) -> &[Option<Entity>; 3] {
    match limb {
        ZombieLimb::LeftArm => &bones.left_arm,
        ZombieLimb::RightArm => &bones.right_arm,
        ZombieLimb::LeftLeg => &bones.left_leg,
        ZombieLimb::RightLeg => &bones.right_leg,
    }
}

/// Collapses the joint a limb comes off at. The zombie is one skinned mesh
/// with no separate limb pieces to hide, so scaling the joint to nothing takes
/// everything weighted to it and its children down to a point.
///
/// Runs after the animation has written its bone transforms. In Update the
/// clip would get the last word and the limb would flicker back.
pub(crate) fn sever_limbs(
    mut commands: Commands,
    blood: Res<BloodAssets>,
    limb_damage: Res<LimbDamage>,
    globals: Query<&GlobalTransform>,
    mut wounded: Query<(Entity, &ZombieHitboxBones, Option<&mut SeveredLimbs>)>,
    mut bones: Query<&mut Transform>,
    alive: Query<(), With<Zombie>>,
) {
    for (entity, hitboxes, cuts) in &mut wounded {
        let Some(mut cuts) = cuts else {
            let fresh = limb_damage.severed(entity);
            if !fresh.is_empty() {
                commands.entity(entity).insert(SeveredLimbs(fresh));
            }
            continue;
        };
        cuts.merge(limb_damage.severed(entity));
        for cut in &mut cuts.0 {
            let Some(joint) = limb_chain(hitboxes, cut.limb)[cut.cut_at] else { continue };
            if let Ok(mut transform) = bones.get_mut(joint) {
                transform.scale = Vec3::splat(SEVERED_BONE_SCALE);
            }
            if cut.sprayed {
                continue;
            }
            cut.sprayed = true;
            let state = if alive.contains(entity) { "ALIVE" } else { "dead" };
            info!("[dismember] {state} zombie={entity} lost {:?} at joint {}", cut.limb, cut.cut_at);
            let Ok(at) = globals.get(joint).map(|g| g.translation()) else { continue };
            for i in 0..STUMP_SPRAY_COUNT {
                let seed = at.x * 31.7 + at.z * 13.1 + i as f32 * 7.9;
                let dir = Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7).abs() * 0.9, ops::sin(seed * 2.1))
                    .normalize_or_zero();
                commands.spawn((
                    Mesh3d(blood.particle_mesh.clone()),
                    MeshMaterial3d(blood.particle_material.clone()),
                    Transform::from_translation(at),
                    BloodParticle { velocity: dir * STUMP_SPRAY_SPEED, remaining: BLOOD_PARTICLE_LIFETIME },
                ));
            }
        }
    }
}

/// A severed limb has no bone left to follow, so it must not get a ragdoll
/// body either; otherwise an invisible arm drags the corpse around.
pub(crate) fn is_severed(cuts: Option<&SeveredLimbs>, bones: &ZombieHitboxBones, bone: Entity) -> bool {
    let Some(cuts) = cuts else { return false };
    cuts.0.iter().any(|cut| {
        limb_chain(bones, cut.limb)[cut.cut_at..].iter().any(|slot| *slot == Some(bone))
    })
}

/// Zombies cleared or respawned without dying never reach forget() elsewhere,
/// so their tally would sit in the map for the rest of the session.
pub(crate) fn forget_gone_zombies(mut removed: RemovedComponents<Zombie>, mut limb_damage: ResMut<LimbDamage>) {
    for entity in removed.read() {
        limb_damage.forget(entity);
    }
}
