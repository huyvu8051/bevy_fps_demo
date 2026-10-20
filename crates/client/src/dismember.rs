use bevy::prelude::*;

use crate::*;
use shared::ZombieLimb;

/// Damage one limb has to soak up before it comes off, in multiples of a
/// single limb hit. Torso and head kills leave the body whole.
const SEVER_DAMAGE: f32 = 1.0;
const SEVERED_BONE_SCALE: f32 = 0.001;
const STUMP_SPRAY_COUNT: usize = 18;
const STUMP_SPRAY_SPEED: f32 = 3.4;

#[derive(Resource, Default)]
pub(crate) struct LimbDamage(std::collections::HashMap<Entity, [LimbWound; 4]>);

#[derive(Clone, Copy, Default)]
pub(crate) struct LimbWound {
    damage: f32,
    /// Shallowest segment hit, so a shot near the shoulder takes the whole arm
    /// rather than leaving it hanging off the elbow.
    cut_at: usize,
}

impl LimbDamage {
    pub(crate) fn record(&mut self, zombie: Entity, limb: ZombieLimb, segment: usize, damage: f32) {
        let wound = &mut self.0.entry(zombie).or_default()[limb.index()];
        if wound.damage == 0.0 || segment < wound.cut_at {
            wound.cut_at = segment;
        }
        wound.damage += damage;
    }

    pub(crate) fn take(&mut self, zombie: Entity) -> Vec<(ZombieLimb, usize)> {
        self.0
            .remove(&zombie)
            .map(|wounds| {
                ZombieLimb::ALL
                    .into_iter()
                    .filter(|limb| wounds[limb.index()].damage >= SEVER_DAMAGE)
                    .map(|limb| (limb, wounds[limb.index()].cut_at))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn forget(&mut self, zombie: Entity) {
        self.0.remove(&zombie);
    }
}

#[derive(Component)]
pub(crate) struct SeveredLimbs(pub(crate) Vec<(ZombieLimb, usize)>);

/// Set once the stump has been sprayed, so the blood does not re-fire every
/// frame while the scale is being held down.
#[derive(Component)]
pub(crate) struct LimbsCut;

fn limb_chain(bones: &ZombieHitboxBones, limb: ZombieLimb) -> &[Option<Entity>; 3] {
    match limb {
        ZombieLimb::LeftArm => &bones.left_arm,
        ZombieLimb::RightArm => &bones.right_arm,
        ZombieLimb::LeftLeg => &bones.left_leg,
        ZombieLimb::RightLeg => &bones.right_leg,
    }
}

/// Collapses the bone the limb comes off at. The zombie is one skinned mesh
/// with no separate limb pieces to hide, so scaling the joint to nothing takes
/// everything weighted to it and its children down to a point. Blood over the
/// join covers the pinch where the geometry closes up.
pub(crate) fn sever_limbs(
    mut commands: Commands,
    blood: Res<BloodAssets>,
    severed: Query<(Entity, &SeveredLimbs, &ZombieHitboxBones, Option<&LimbsCut>)>,
    globals: Query<&GlobalTransform>,
    mut bones: Query<&mut Transform>,
) {
    for (entity, cuts, hitboxes, already_cut) in &severed {
        for &(limb, segment) in &cuts.0 {
            let Some(joint) = limb_chain(hitboxes, limb)[segment] else { continue };
            let Ok(at) = globals.get(joint).map(|g| g.translation()) else { continue };
            if let Ok(mut transform) = bones.get_mut(joint) {
                transform.scale = Vec3::splat(SEVERED_BONE_SCALE);
            }
            if already_cut.is_some() {
                continue;
            }
            info!("[dismember] zombie={entity} lost {limb:?} at joint {segment}");
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
        commands.entity(entity).insert(LimbsCut);
    }
}

/// A severed limb has no bone left to follow, so it must not get a ragdoll
/// body either; otherwise an invisible arm drags the corpse around.
pub(crate) fn is_severed(cuts: Option<&SeveredLimbs>, bones: &ZombieHitboxBones, bone: Entity) -> bool {
    let Some(cuts) = cuts else { return false };
    cuts.0.iter().any(|&(limb, segment)| {
        limb_chain(bones, limb)[segment..].iter().any(|slot| *slot == Some(bone))
    })
}

/// Zombies cleared or respawned without dying never reach take(), so their
/// tally would sit in the map for the rest of the session.
pub(crate) fn forget_gone_zombies(mut removed: RemovedComponents<Zombie>, mut limb_damage: ResMut<LimbDamage>) {
    for entity in removed.read() {
        limb_damage.forget(entity);
    }
}
