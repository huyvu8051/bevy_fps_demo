use avian3d::prelude::*;
use bevy::prelude::*;

use crate::*;
use shared::ZombiePart;

/// Damage one part soaks up before it comes off, in multiples of a single limb
/// hit. Torso damage never counts toward it.
const SEVER_DAMAGE: f32 = 1.0;
const SEVERED_BONE_SCALE: f32 = 0.001;

const BURST_COUNT: usize = 18;
const BURST_SPEED: f32 = 3.4;

/// Both faces of a cut keep pumping for a moment after it opens, rather than
/// one puff at the instant of severance.
const JET_SECONDS: f32 = 1.5;
const JET_INTERVAL: f32 = 0.04;
const JET_COUNT: usize = 3;
const JET_SPEED: f32 = 2.4;
const JET_SPREAD: f32 = 0.45;

const GORE_RADIUS: f32 = 0.075;
const GORE_BASE_LENGTH: f32 = 0.40;
const GORE_DENSITY: f32 = 900.0;
const GORE_LIFETIME: f32 = 20.0;
const GORE_THROW_SPEED: f32 = 2.6;
const GORE_SPIN: f32 = 9.0;
const MAX_GORE: usize = 24;

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
                    .filter(|part| wounds[part.index()].damage >= SEVER_DAMAGE)
                    .map(|part| SeveredPart {
                        part,
                        cut_at: wounds[part.index()].cut_at,
                        opened: false,
                        along: Vec3::Y,
                        bleed: JET_SECONDS,
                        next_drop: 0.0,
                    })
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
    opened: bool,
    /// Direction the missing part used to run in, so the stump sprays outward
    /// rather than into the body.
    along: Vec3,
    bleed: f32,
    next_drop: f32,
}

#[derive(Component, Default)]
pub(crate) struct SeveredParts(pub(crate) Vec<SeveredPart>);

impl SeveredParts {
    /// Keeps the state already set, so adding a second cut does not reopen the
    /// first one's jet.
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

fn spray(commands: &mut Commands, blood: &BloodAssets, at: Vec3, along: Vec3, count: usize, speed: f32, seed: f32) {
    let sideways = along.any_orthonormal_vector();
    let other = along.cross(sideways);
    for i in 0..count {
        let scatter = seed + i as f32 * 7.9;
        let spread = (sideways * ops::sin(scatter * 1.9) + other * ops::sin(scatter * 2.7)) * JET_SPREAD;
        let dir = (along + spread).normalize_or_zero();
        commands.spawn((
            Mesh3d(blood.particle_mesh.clone()),
            MeshMaterial3d(blood.particle_material.clone()),
            Transform::from_translation(at),
            BloodParticle { velocity: dir * speed, remaining: BLOOD_PARTICLE_LIFETIME },
        ));
    }
}

/// Collapses the joint a part comes off at. The zombie is one skinned mesh
/// with no separate pieces to hide, so scaling the joint to nothing takes
/// everything weighted to it and its children down to a point.
///
/// Runs after the animation has written its bone transforms. In Update the
/// clip would get the last word and the part would flicker back.
pub(crate) fn sever_limbs(
    mut commands: Commands,
    time: Res<Time>,
    blood: Res<BloodAssets>,
    gore: Res<GoreAssets>,
    limb_damage: Res<LimbDamage>,
    globals: Query<&GlobalTransform>,
    mut wounded: Query<(Entity, &ZombieHitboxBones, Option<&mut SeveredParts>)>,
    mut bones: Query<&mut Transform>,
    alive: Query<(), With<Zombie>>,
) {
    let dt = time.delta_secs();
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
            let chain = part_chain(hitboxes, cut.part);
            let Some(joint) = chain[cut.cut_at] else { continue };
            if let Ok(mut transform) = bones.get_mut(joint) {
                transform.scale = Vec3::splat(SEVERED_BONE_SCALE);
            }
            let Ok(joint_global) = globals.get(joint) else { continue };
            let at = joint_global.translation();

            if !cut.opened {
                cut.opened = true;
                // Aim down the rest of the chain. A wrist cut or the head has
                // nothing past it, so fall back to the bone's own up axis.
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
                cut.along = along;
                let state = if alive.contains(entity) { "ALIVE" } else { "dead" };
                info!("[dismember] {state} zombie={entity} lost {:?} at joint {}", cut.part, cut.cut_at);
                spray(&mut commands, &blood, at, along, BURST_COUNT, BURST_SPEED, at.x * 31.7 + at.z * 13.1);
                throw_part(&mut commands, &gore, cut.part, at, along, length, at.x * 17.3 + at.z * 29.1);
            }

            if cut.bleed <= 0.0 {
                continue;
            }
            cut.bleed -= dt;
            cut.next_drop -= dt;
            if cut.next_drop <= 0.0 {
                cut.next_drop = JET_INTERVAL;
                spray(&mut commands, &blood, at, cut.along, JET_COUNT, JET_SPEED, at.y * 97.1 + cut.bleed * 53.3);
            }
        }
    }
}

/// A severed part has no bone left to follow, so it must not get a ragdoll
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

const HEAD_GORE_RADIUS: f32 = 0.105;

#[derive(Resource)]
pub(crate) struct GoreAssets {
    limb_mesh: Handle<Mesh>,
    head_mesh: Handle<Mesh>,
    limb_material: Handle<StandardMaterial>,
    head_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct GorePiece {
    remaining: f32,
    /// Half the capsule's length, which is where the cut face sits in local
    /// space, so the jet tracks the piece as it tumbles.
    half_length: f32,
    bleed: f32,
    next_drop: f32,
}

/// Both textures are deliberately generic -- muscle and a bloodied skull --
/// so one set serves every zombie kind and nobody can tell which body a part
/// came off.
pub(crate) fn build_gore_assets(
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> GoreAssets {
    let flesh = |texture: &str| StandardMaterial {
        base_color_texture: Some(asset_server.load(texture.to_string())),
        perceptual_roughness: 0.45,
        ..default()
    };
    GoreAssets {
        limb_mesh: meshes.add(Capsule3d::new(GORE_RADIUS, GORE_BASE_LENGTH)),
        head_mesh: meshes.add(Sphere::new(HEAD_GORE_RADIUS)),
        limb_material: materials.add(flesh("textures/gore/limb.png")),
        head_material: materials.add(flesh("textures/gore/head.png")),
    }
}

/// A stand-in for the part that was just hidden. The body is one skinned mesh,
/// so there is no real arm to throw; this is a capsule roughly its size, which
/// reads as a limb at the speed it leaves at.
fn throw_part(commands: &mut Commands, gore: &GoreAssets, part: ZombiePart, from: Vec3, along: Vec3, length: f32, seed: f32) {
    let length = length.max(GORE_BASE_LENGTH * 0.5);
    let head = part == ZombiePart::Head;
    let (mesh, material, collider, scale, half_length) = if head {
        (
            gore.head_mesh.clone(),
            gore.head_material.clone(),
            Collider::sphere(HEAD_GORE_RADIUS),
            Vec3::ONE,
            HEAD_GORE_RADIUS,
        )
    } else {
        (
            gore.limb_mesh.clone(),
            gore.limb_material.clone(),
            Collider::capsule(GORE_RADIUS, length),
            Vec3::new(1.0, length / GORE_BASE_LENGTH, 1.0),
            length * 0.5,
        )
    };
    let scatter = Vec3::new(ops::sin(seed * 1.7), ops::sin(seed * 2.3).abs(), ops::sin(seed * 3.1));
    commands.spawn((
        Name::new("SeveredPart"),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(from + along * length * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, along))
            .with_scale(scale),
        RigidBody::Dynamic,
        collider,
        ColliderDensity(GORE_DENSITY),
        SweptCcd::default(),
        LinearVelocity(along * GORE_THROW_SPEED + scatter * GORE_THROW_SPEED * 0.4 + Vec3::Y * 1.2),
        AngularVelocity(scatter * GORE_SPIN),
        GorePiece {
            remaining: GORE_LIFETIME,
            half_length,
            bleed: JET_SECONDS,
            next_drop: 0.0,
        },
    ));
}

/// Severed parts are cheap but not free, and they are rigid bodies the solver
/// keeps revisiting. Oldest go first once over budget.
pub(crate) fn update_gore(
    time: Res<Time>,
    mut commands: Commands,
    blood: Res<BloodAssets>,
    mut pieces: Query<(Entity, &mut GorePiece, &GlobalTransform)>,
) {
    let dt = time.delta_secs();
    let mut over_budget = pieces.iter().len().saturating_sub(MAX_GORE);
    for (entity, mut piece, global) in &mut pieces {
        if over_budget > 0 && piece.remaining > 1.0 {
            piece.remaining = 1.0;
            over_budget -= 1;
        }
        piece.remaining -= dt;
        if piece.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        if piece.bleed <= 0.0 {
            continue;
        }
        piece.bleed -= dt;
        piece.next_drop -= dt;
        if piece.next_drop > 0.0 {
            continue;
        }
        piece.next_drop = JET_INTERVAL;
        // The cut face is the end nearest the body, which the throw put at the
        // capsule's local -Y.
        let down_axis = global.rotation() * Vec3::NEG_Y;
        let face = global.translation() + down_axis * piece.half_length;
        spray(&mut commands, &blood, face, down_axis, JET_COUNT, JET_SPEED, face.x * 61.7 + piece.bleed * 41.9);
    }
}
