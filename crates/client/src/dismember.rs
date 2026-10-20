use bevy::prelude::*;

use crate::*;
use shared::ZombiePart;

/// Damage one limb soaks up before it comes off, in multiples of a single limb
/// hit. Torso and head damage never counts toward it.
const SEVER_DAMAGE: f32 = 1.0;
const SEVERED_BONE_SCALE: f32 = 0.001;
const STUMP_SPRAY_COUNT: usize = 18;
const STUMP_SPRAY_SPEED: f32 = 3.4;

#[derive(Resource, Default)]
pub(crate) struct LimbDamage(std::collections::HashMap<Entity, [LimbWound; 5]>);

#[derive(Clone, Copy, Default)]
struct LimbWound {
    damage: f32,
    /// Shallowest segment hit, so a shoulder shot takes the whole arm rather
    /// than leaving it hanging off the elbow.
    cut_at: usize,
    hit: bool,
}

impl LimbDamage {
    pub(crate) fn record(&mut self, zombie: Entity, part: ZombiePart, segment: usize, damage: f32) {
        let wound = &mut self.0.entry(zombie).or_default()[part.index()];
        if !wound.hit || segment < wound.cut_at {
            wound.cut_at = segment;
        }
        wound.hit = true;
        wound.damage += damage;
    }

    pub(crate) fn severed(&self, zombie: Entity) -> Vec<SeveredPart> {
        self.0
            .get(&zombie)
            .map(|wounds| {
                ZombiePart::ALL
                    .into_iter()
                    .filter(|limb| wounds[limb.index()].damage >= SEVER_DAMAGE)
                    .map(|limb| SeveredPart { part: limb, cut_at: wounds[limb.index()].cut_at, sprayed: false })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn forget(&mut self, zombie: Entity) {
        self.0.remove(&zombie);
    }
}

pub(crate) struct SeveredPart {
    part: ZombiePart,
    cut_at: usize,
    sprayed: bool,
}

#[derive(Component, Default)]
pub(crate) struct SeveredParts(pub(crate) Vec<SeveredPart>);

impl SeveredParts {
    /// Keeps the spray flags already set, so adding a second limb does not
    /// re-fire blood from the first.
    pub(crate) fn merge(&mut self, fresh: Vec<SeveredPart>) {
        for cut in fresh {
            if !self.0.iter().any(|known| known.part == cut.part) {
                self.0.push(cut);
            }
        }
    }
}

/// The head is a single joint rather than a three joint chain, so it rides in
/// the first slot with the rest empty.
fn part_chain(bones: &ZombieHitboxBones, part: ZombiePart) -> [Option<Entity>; 3] {
    match part {
        ZombiePart::Head => [bones.head, None, None],
        ZombiePart::LeftArm => bones.left_arm,
        ZombiePart::RightArm => bones.right_arm,
        ZombiePart::LeftLeg => bones.left_leg,
        ZombiePart::RightLeg => bones.right_leg,
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
    mut wounded: Query<(Entity, &ZombieHitboxBones, Option<&mut SeveredParts>)>,
    mut bones: Query<&mut Transform>,
    alive: Query<(), With<Zombie>>,
    gore: Res<GoreAssets>,
) {
    for (entity, hitboxes, cuts) in &mut wounded {
        let Some(mut cuts) = cuts else {
            let fresh = limb_damage.severed(entity);
            if !fresh.is_empty() {
                commands.entity(entity).insert(SeveredParts(fresh));
            }
            continue;
        };
        cuts.merge(limb_damage.severed(entity));
        for cut in &mut cuts.0 {
            let Some(joint) = part_chain(hitboxes, cut.part)[cut.cut_at] else { continue };
            if let Ok(mut transform) = bones.get_mut(joint) {
                transform.scale = Vec3::splat(SEVERED_BONE_SCALE);
            }
            if cut.sprayed {
                continue;
            }
            cut.sprayed = true;
            let state = if alive.contains(entity) { "ALIVE" } else { "dead" };
            info!("[dismember] {state} zombie={entity} lost {:?} at joint {}", cut.part, cut.cut_at);
            let Ok(joint_global) = globals.get(joint) else { continue };
            let at = joint_global.translation();

            // Aim the thrown part down the rest of the chain. A wrist cut or
            // the head has nothing past it, so fall back to the bone own up
            // axis at a default length.
            let chain = part_chain(hitboxes, cut.part);
            let tip = chain[cut.cut_at + 1..]
                .iter()
                .flatten()
                .last()
                .and_then(|&bone| globals.get(bone).ok())
                .map(|g| g.translation());
            let (along, length) = match tip {
                Some(tip) if tip.distance(at) > 0.05 => ((tip - at).normalize(), tip.distance(at)),
                _ => (joint_global.up().normalize_or_zero(), GORE_BASE_LENGTH),
            };
            throw_part(&mut commands, &gore, at, along, length, at.x * 17.3 + at.z * 29.1);
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
pub(crate) fn is_severed(cuts: Option<&SeveredParts>, bones: &ZombieHitboxBones, bone: Entity) -> bool {
    let Some(cuts) = cuts else { return false };
    cuts.0.iter().any(|cut| {
        part_chain(bones, cut.part)[cut.cut_at..].iter().any(|slot| *slot == Some(bone))
    })
}

/// Zombies cleared or respawned without dying never reach forget() elsewhere,
/// so their tally would sit in the map for the rest of the session.
pub(crate) fn forget_gone_zombies(mut removed: RemovedComponents<Zombie>, mut limb_damage: ResMut<LimbDamage>) {
    for entity in removed.read() {
        limb_damage.forget(entity);
    }
}

const GORE_RADIUS: f32 = 0.075;
const GORE_BASE_LENGTH: f32 = 0.40;
const GORE_DENSITY: f32 = 900.0;
const GORE_LIFETIME: f32 = 20.0;
const GORE_THROW_SPEED: f32 = 2.6;
const GORE_SPIN: f32 = 9.0;
const MAX_GORE: usize = 24;

#[derive(Resource)]
pub(crate) struct GoreAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct GorePiece {
    remaining: f32,
}

pub(crate) fn build_gore_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> GoreAssets {
    GoreAssets {
        mesh: meshes.add(Capsule3d::new(GORE_RADIUS, GORE_BASE_LENGTH)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.29, 0.25),
            perceptual_roughness: 0.85,
            ..default()
        }),
    }
}

/// A stand-in for the limb that was just hidden. The body is one skinned mesh,
/// so there is no real arm to throw; this is a capsule roughly its size, which
/// reads as a limb at the speed it leaves at.
fn throw_part(
    commands: &mut Commands,
    gore: &GoreAssets,
    from: Vec3,
    along: Vec3,
    length: f32,
    seed: f32,
) {
    let length = length.max(GORE_BASE_LENGTH * 0.5);
    let scatter = Vec3::new(ops::sin(seed * 1.7), ops::sin(seed * 2.3).abs(), ops::sin(seed * 3.1));
    commands.spawn((
        Name::new("SeveredPart"),
        Mesh3d(gore.mesh.clone()),
        MeshMaterial3d(gore.material.clone()),
        Transform::from_translation(from + along * length * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, along))
            .with_scale(Vec3::new(1.0, length / GORE_BASE_LENGTH, 1.0)),
        RigidBody::Dynamic,
        Collider::capsule(GORE_RADIUS, length),
        ColliderDensity(GORE_DENSITY),
        SweptCcd::default(),
        LinearVelocity(along * GORE_THROW_SPEED + scatter * GORE_THROW_SPEED * 0.4 + Vec3::Y * 1.2),
        AngularVelocity(scatter * GORE_SPIN),
        GorePiece { remaining: GORE_LIFETIME },
    ));
}

/// Severed parts are cheap but not free, and they are rigid bodies the solver
/// keeps revisiting. Oldest go first once over budget.
pub(crate) fn update_gore(
    time: Res<Time>,
    mut commands: Commands,
    mut pieces: Query<(Entity, &mut GorePiece)>,
) {
    let mut over_budget = pieces.iter().len().saturating_sub(MAX_GORE);
    for (entity, mut piece) in &mut pieces {
        if over_budget > 0 && piece.remaining > 1.0 {
            piece.remaining = 1.0;
            over_budget -= 1;
        }
        piece.remaining -= time.delta_secs();
        if piece.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
