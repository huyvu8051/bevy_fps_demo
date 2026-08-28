// --- VFX: scope camera, hit feedback, particles/decals, gun view anim ---
// Extracted from lib.rs (Phase 5 of the gameplay-code-organization pass —
// see chat/commit history). Every item here is `pub(crate)` so lib.rs's
// single master `.chain()`'d Update system list (unchanged — pure code
// motion, no execution-order change) can still reference these by name.
//
// `use crate::*;` pulls in every crate-root item this module needs
// (constants, other components/resources, helper fns) — root-private
// items ARE visible to a descendant module like this one per Rust's
// normal privacy rules, just not name-resolved without an explicit
// `use`/path, hence the glob rather than hunting down each one by hand.

use bevy::prelude::*;
use crate::*;

// Debug/dev toggle for the Cheats panel's "Scope" button — disables the
// secondary ScopeCamera (the AK47 scope's own render-to-texture camera,
// see setup()) and its lens quad entirely while ADS'd, instead of just
// hiding the gun/map render toggles' own targets. When off, ADS still
// narrows FOV the normal way (AIM_FOV_MULTIPLIER) but shows no scope
// picture-in-picture — useful to cut the extra camera pass on weaker
// devices, or just to compare with/without it. See update_scope_lens.
#[derive(Resource)]
pub(crate) struct ScopeCameraEnabled(pub(crate) bool);
impl Default for ScopeCameraEnabled {
    fn default() -> Self {
        Self(true)
    }
}

// The second camera that renders the scope's actual zoomed view onto
// SCOPE_TEXTURE_SIZE² render target, sampled by ScopeLens's material.
// Starts inactive — update_scope_lens only turns it on while ADS'd, so it
// isn't paying for a second render pass the rest of the time.
#[derive(Component)]
pub(crate) struct ScopeCamera;

// The small quad in front of the gun showing the scope camera's render
// target — the thing the player actually looks "through".
#[derive(Component)]
pub(crate) struct ScopeLens;

// Marks the static reticle chevron so shoot()'s raycast can exclude it —
// see VfxAssets.vfx_meshes.
#[derive(Component)]
pub(crate) struct ScopeReticle;

// Current scope magnification, adjusted by mouse wheel while aiming (see
// adjust_scope_zoom) and clamped to [SCOPE_MIN_ZOOM, SCOPE_MAX_ZOOM].
#[derive(Resource)]
pub(crate) struct ScopeZoom(pub(crate) f32);

#[derive(Component)]
pub(crate) struct HitMarker {
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct MuzzleFlash {
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct Smoke {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

// A thin glowing streak from muzzle to impact — see TRACER_CHANCE's own doc
// comment for why this doesn't spawn on every shot. Pure VFX, spawned after
// the real hit is already resolved, so it never affects hit detection.
#[derive(Component)]
pub(crate) struct BulletTracer {
    pub(crate) remaining: f32,
}

// A single flying blood droplet — spurts outward from a zombie hit, falls
// under its own gravity (separate from GRAVITY, blood falls faster/looks
// punchier), and despawns once its lifetime runs out.
#[derive(Component)]
pub(crate) struct BloodParticle {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

// A single flying spark — the environment-hit counterpart of BloodParticle,
// see spawn_spark_effects.
#[derive(Component)]
pub(crate) struct SparkParticle {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

// A static decal left where blood actually landed — either right at the
// hit point, or on a floor found by raycasting straight down from there.
#[derive(Component)]
pub(crate) struct BloodStain {
    pub(crate) remaining: f32,
}

#[derive(Resource)]
pub(crate) struct HitMarkerAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct MuzzleFxAssets {
    pub(crate) flash_mesh: Handle<Mesh>,
    pub(crate) flash_material: Handle<StandardMaterial>,
    pub(crate) smoke_mesh: Handle<Mesh>,
    pub(crate) smoke_material: Handle<StandardMaterial>,
}

// Unit cylinder (radius 1, height 1, centered on origin) — spawn_bullet_tracer
// scales it per-shot (X/Z = TRACER_RADIUS, Y = the shot's actual length)
// rather than building a fresh mesh every time.
#[derive(Resource)]
pub(crate) struct TracerAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct BloodAssets {
    pub(crate) particle_mesh: Handle<Mesh>,
    pub(crate) particle_material: Handle<StandardMaterial>,
    pub(crate) stain_mesh: Handle<Mesh>,
    pub(crate) stain_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct SparkAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

// Spawns a burst of flying blood particles at a zombie hit, plus two static
// stains: one right at the hit point (blood on the wound/whatever surface
// was actually struck), and one on the floor beneath it if a downward
// raycast finds one within BLOOD_FLOOR_RAYCAST_DISTANCE (blood dripping
// down and landing) — reuses the shoot()'s own SpatialQuery rather than
// taking one as a fresh parameter, since shoot() is already at Bevy's
// 16-parameter-per-system limit (see VfxAssets' doc comment).
// `bone` is `Some` only when the hit zombie survived (shoot() checks
// health after applying damage) — a wound stain parented to a despawning
// corpse would just despawn right along with it, so there's no point. It's
// the specific BONE nearest the hit (see nearest_zombie_bone/
// spawn_blood_stain_on_zombie's own doc comment for why a bone, not the
// zombie's root entity), not "the zombie" in general.
// `killed` skips the wound stain entirely instead of falling back to a
// world-space one at hit_point: that fallback sits right where the corpse
// just was, so it visually reads as "blood still stuck on the body" for
// its own full BLOOD_STAIN_LIFETIME even though the body itself (and any
// bone-parented decal on it) is already gone. The floor drips below are
// left untouched — those are real drips that already fell and landed
// separately, not blood riding along on the corpse.
pub(crate) fn spawn_blood_effects(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    hit_point: Vec3,
    hit_normal: Vec3,
    bone: Option<(Entity, &GlobalTransform)>,
    killed: bool,
) {
    for i in 0..BLOOD_PARTICLE_COUNT {
        // Same "deterministic pseudo-randomness via sin" trick used
        // elsewhere (recoil jitter, bullet spread) — offset each particle's
        // seed by its index so they don't all fly the same direction.
        let seed = time.elapsed_secs() * 977.0 + i as f32 * 131.7;
        let dir = Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7).abs() * 0.6, ops::sin(seed * 2.1))
            .normalize_or_zero();
        commands.spawn((
            Mesh3d(blood_assets.particle_mesh.clone()),
            MeshMaterial3d(blood_assets.particle_material.clone()),
            Transform::from_translation(hit_point),
            BloodParticle {
                velocity: dir * BLOOD_PARTICLE_SPEED,
                remaining: BLOOD_PARTICLE_LIFETIME,
            },
        ));
    }

    // The wound itself — follows that bone's own animation if it's still
    // alive, otherwise just sits in world space wherever it landed.
    match bone {
        Some((entity, bone_global)) => {
            spawn_blood_stain_on_zombie(commands, blood_assets, entity, bone_global, hit_point, hit_normal);
        }
        None if !killed => spawn_blood_stain(commands, blood_assets, None, hit_point, hit_normal),
        None => {}
    }

    // A random number of drips land on whatever's below — not always
    // exactly one, and not all from the exact same spot, so a burst doesn't
    // read as a single too-tidy dot.
    let range = (BLOOD_FLOOR_STAIN_MAX - BLOOD_FLOOR_STAIN_MIN + 1) as f32;
    let count = (BLOOD_FLOOR_STAIN_MIN as f32 + ops::sin(time.elapsed_secs() * 457.0).abs() * range) as usize;
    let count = count.clamp(BLOOD_FLOOR_STAIN_MIN, BLOOD_FLOOR_STAIN_MAX);

    let Ok(down) = Dir3::new(Vec3::NEG_Y) else {
        return;
    };
    let filter = SpatialQueryFilter::default();
    for i in 0..count {
        let seed = time.elapsed_secs() * 613.0 + i as f32 * 219.3;
        let jitter = Vec3::new(ops::sin(seed), 0.0, ops::sin(seed * 1.7)) * BLOOD_FLOOR_STAIN_SPREAD;
        let origin = hit_point + jitter;
        if let Some(floor_hit) = spatial_query.cast_ray(origin, down, BLOOD_FLOOR_RAYCAST_DISTANCE, true, &filter) {
            let floor_point = origin + Vec3::NEG_Y * floor_hit.distance;
            // Floor drips are always world-space (not parented) even if the
            // zombie survived — they've already fallen and landed
            // separately from the body, so there's nothing to follow.
            spawn_blood_stain(commands, blood_assets, None, floor_point, floor_hit.normal);
        }
    }
}

// Gates the "you dealt real damage" feedback (floating number + blood) on
// damage actually being > 0. Before this, a shot that hit the broad-phase
// zombie capsule (ZOMBIE_OUTER_RADIUS/ZOMBIE_OUTER_HEIGHT — deliberately
// wider than the visible mesh, see its own doc comment) but landed outside
// every real bone
// hitbox (zombie_hit_damage_bones) still spawned a floating damage number
// and a blood spray with 0 actual HP lost — a clean miss looked and felt
// exactly like a landed hit, which is why zombies could take forever to die
// despite what looked like a stream of hits. A 0-damage "hit" now reads as
// a graze (spark burst, same as a wall hit) instead, so what you see
// matches what the zombie's health actually did. Called from on_shot_fx
// (remote players' shots) and on_shot_confirmed (the local shooter's own
// shot, once the server confirms it — see ShotConfirmed's own doc comment
// for why this waits instead of using a local guess); both spawn their own
// HitMarker decal separately, immediately, regardless of this — that's an
// honest "the raycast physically stopped here" signal either way, not
// duplicated here.
pub(crate) fn spawn_zombie_hit_feedback(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spark_assets: &SparkAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    point: Vec3,
    normal: Vec3,
    damage: f32,
    bone_for_blood: Option<(Entity, &GlobalTransform)>,
    killed: bool,
) {
    if damage > 0.0 {
        spawn_floating_damage(commands, point, damage);
        spawn_blood_effects(commands, blood_assets, spatial_query, time, point, normal, bone_for_blood, killed);
    } else {
        spawn_spark_effects(commands, spark_assets, time, point, normal);
    }
}

// Spawns a burst of flying sparks at a non-zombie (wall/environment) hit —
// the mirror image of spawn_blood_effects. Biased outward from hit_normal
// (lerped toward a random direction, not fully random) so they read as
// bouncing off the surface rather than scattering every which way.
pub(crate) fn spawn_spark_effects(commands: &mut Commands, spark_assets: &SparkAssets, time: &Time, hit_point: Vec3, hit_normal: Vec3) {
    let normal = hit_normal.normalize_or_zero();
    let range = (SPARK_PARTICLE_MAX - SPARK_PARTICLE_MIN + 1) as f32;
    let count = SPARK_PARTICLE_MIN + (ops::sin(time.elapsed_secs() * 349.0).abs() * range) as usize;
    let count = count.clamp(SPARK_PARTICLE_MIN, SPARK_PARTICLE_MAX);

    for i in 0..count {
        let seed = time.elapsed_secs() * 883.0 + i as f32 * 157.1;
        let random_dir =
            Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7), ops::sin(seed * 2.1)).normalize_or_zero();
        let dir = (normal + random_dir).normalize_or_zero();
        // Varies each spark's speed a bit too, so the burst doesn't look
        // like a uniform ring.
        let speed = SPARK_PARTICLE_SPEED * (0.6 + ops::sin(seed * 3.7).abs() * 0.6);
        commands.spawn((
            Mesh3d(spark_assets.mesh.clone()),
            MeshMaterial3d(spark_assets.material.clone()),
            Transform::from_translation(hit_point),
            SparkParticle {
                velocity: dir * speed,
                remaining: SPARK_PARTICLE_LIFETIME,
            },
        ));
    }
}

// A thin glowing streak from `start` to `end` — shown on only a fraction of
// shots (TRACER_CHANCE), same sin-based deterministic "randomness" already
// used for spread/recoil jitter elsewhere in this file (no rand crate). The
// seed passed in lets each call site (local shooter vs. each remote shot)
// use its own unrelated-looking frequency, so two shots landing the same
// frame don't roll identically. Pure VFX — called only after the real hit
// is already resolved, so this can never affect damage.
pub(crate) fn spawn_bullet_tracer(commands: &mut Commands, tracer_assets: &TracerAssets, seed: f32, start: Vec3, end: Vec3, show_tracers: bool) {
    if !show_tracers {
        return;
    }
    // (sin(seed) + 1) / 2 maps to a roughly-uniform [0, 1] value — below
    // TRACER_CHANCE means "this shot gets a tracer".
    if (ops::sin(seed) + 1.0) * 0.5 >= TRACER_CHANCE {
        return;
    }
    let delta = end - start;
    let length = delta.length();
    if length < f32::EPSILON {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, delta / length);
    commands.spawn((
        Mesh3d(tracer_assets.mesh.clone()),
        MeshMaterial3d(tracer_assets.material.clone()),
        Transform::from_translation(start + delta * 0.5)
            .with_rotation(rotation)
            .with_scale(Vec3::new(TRACER_RADIUS, length, TRACER_RADIUS)),
        BulletTracer {
            remaining: TRACER_LIFETIME,
        },
    ));
}

// `parent` is Some when this stain should be attached to (and move with)
// that entity instead of sitting still in world space.
pub(crate) fn spawn_blood_stain(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    parent: Option<Entity>,
    point: Vec3,
    normal: Vec3,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, normal.normalize_or_zero());
    let transform = Transform::from_translation(point + normal.normalize_or_zero() * 0.01).with_rotation(rotation);
    let mut stain = commands.spawn((
        Mesh3d(blood_assets.stain_mesh.clone()),
        MeshMaterial3d(blood_assets.stain_material.clone()),
        transform,
        BloodStain {
            remaining: BLOOD_STAIN_LIFETIME,
        },
    ));
    if let Some(parent) = parent {
        stain.insert(ChildOf(parent));
    }
}

// Parented onto the specific BONE nearest the hit (see nearest_zombie_bone),
// not the zombie's root entity — the root's own Transform only carries
// position + yaw (interpolate_net_transform), never the per-bone animation
// (arms swinging, torso bobbing) that actually moves the visible skin. A
// stain parented to the root stayed rigidly offset from the root while the
// real mesh underneath animated independently, reading as floating instead
// of stuck to skin. Parenting to the actual animated bone entity instead
// means it inherits that bone's own GlobalTransform — no different than any
// other cosmetic child of a bone (the mesh itself is one). GlobalTransform::
// reparented_to handles both the bone's current rotation and ZOMBIE_SCALE —
// without this, a child entity's declared size would end up multiplied by
// that scale, same issue as the capsule Collider in spawn_zombie.
pub(crate) fn spawn_blood_stain_on_zombie(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    bone: Entity,
    bone_global: &GlobalTransform,
    hit_point: Vec3,
    hit_normal: Vec3,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, hit_normal.normalize_or_zero());
    let world_transform =
        GlobalTransform::from(Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * 0.01).with_rotation(rotation));
    let local_transform = world_transform.reparented_to(bone_global);

    commands.entity(bone).with_children(|parent| {
        parent.spawn((
            Mesh3d(blood_assets.stain_mesh.clone()),
            MeshMaterial3d(blood_assets.stain_material.clone()),
            local_transform,
            BloodStain {
                remaining: BLOOD_STAIN_LIFETIME,
            },
        ));
    });
}

pub(crate) fn update_blood_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut BloodParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut query {
        particle.velocity.y += BLOOD_PARTICLE_GRAVITY * dt;
        transform.translation += particle.velocity * dt;

        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn update_spark_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut SparkParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut query {
        particle.velocity.y += SPARK_PARTICLE_GRAVITY * dt;
        transform.translation += particle.velocity * dt;

        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn despawn_blood_stains(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut BloodStain)>) {
    for (entity, mut stain) in &mut query {
        stain.remaining -= time.delta_secs();
        if stain.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Shows the slider only while actually ADS'd — inverse of update_
// crosshair's own CrosshairRoot toggle (that one hides on aim, this one
// shows on aim), same is_aiming(...) check so both agree on the exact
// same frame.
pub(crate) fn update_scope_zoom_slider_visibility(
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut track: Query<&mut Node, With<TouchScopeZoomSliderTrack>>,
) {
    let Ok(mut node) = track.single_mut() else {
        return;
    };
    let aiming = cursor.single().is_ok_and(|c| is_aiming(c, &mouse, touch_aiming.0));
    node.display = if aiming { Display::Flex } else { Display::None };
}

// Draws the real hitbox shapes zombie_hit_damage_bones (crates/shared)
// tests a hit against — the outer broad-phase capsule (white), the torso
// capsule (belly<->chest), 8 limb capsule segments (shoulder->elbow->hand,
// hip->knee->foot), and the head sphere — so what you see on screen lines
// up exactly with what determines damage, not an approximation of it.
// Falls back to the old height-band rings (drawn relative to the zombie's
// root, not any bone) only for a zombie whose bones haven't resolved yet —
// see wire_zombie_hitbox_bones.
// Split out of draw_zombie_hitboxes — `Gizmos` (used there) already borrows
// `Res<GizmoConfigStore>` internally to look up per-group config, so also
// taking `ResMut<GizmoConfigStore>` directly in that same system panics at
// startup (B0002: a ResMut conflicting with a Res access already in the
// same system's param set). Setting the depth bias in its own system,
// chained to run first, avoids the two ever being live in the same param
// list at once.
pub(crate) fn set_hitbox_gizmo_depth_bias(mut config_store: ResMut<GizmoConfigStore>) {
    // Depth bias -1.0 so debug hitboxes/gizmos render in front of meshes —
    // idempotent (same value every call), so no harm setting it every
    // frame rather than only once at Startup.
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
}

pub(crate) fn draw_zombie_hitboxes(
    show: Res<ShowHitboxes>,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
    autopilot_state: Res<AutopilotState>,
    show_aim: Res<ShowAutopilotTarget>,
) {
    if !show.0 {
        return;
    }

    for (zombie_entity, transform, bones) in &zombies {
        let is_autopilot_target = autopilot_state.target == Some(zombie_entity);

        // The actual collision shape handle_fire_shot/debug_bot_fire test
        // shots against (Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT),
        // centered on this Transform — see ZOMBIE_MODEL_Y_OFFSET's own doc
        // comment for why the rendered mesh is offset to put its feet at the
        // cylinder's bottom). Drawn in white so it's visually distinct from
        // the colored damage-region markers below — if this cylinder's top
        // ends up rendering above the visible head, that's ZOMBIE_SCALE/
        // ZOMBIE_MESH_REFERENCE_HEIGHT not actually matching the mesh's real
        // height, not a bug in the raycast itself.
        gizmos.primitive_3d(
            &Cylinder { radius: ZOMBIE_OUTER_RADIUS, half_height: ZOMBIE_OUTER_HEIGHT / 2.0 },
            Isometry3d::from_translation(transform.translation),
            Color::WHITE,
        );

        let mut any_bone = false;
        let bone_pos = |e: Entity| globals.get(e).ok().map(GlobalTransform::translation);
        let mut joints_to_draw = Vec::new();

        // Single sphere at the Head bone's own raw resolved position — no
        // offset, no separate neck sphere (reverted per request). No second
        // joint to span a capsule to, so it stays a plain sphere.
        if let Some(pos) = bones.head.and_then(bone_pos) {
            gizmos.sphere(Isometry3d::from_translation(pos), HEAD_HITBOX_RADIUS, Color::srgb(1.0, 0.2, 0.2));
            joints_to_draw.push(pos);
            any_bone = true;
        }
        if let (Some(belly), Some(chest)) = (bones.belly.and_then(bone_pos), bones.chest.and_then(bone_pos)) {
            draw_bone_capsule(&mut gizmos, belly, chest, TORSO_HITBOX_RADIUS, Color::srgb(1.0, 0.8, 0.0));
            joints_to_draw.push(belly);
            joints_to_draw.push(chest);
            any_bone = true;
        }
        for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
            for joint_entity in chain.iter() {
                if let Some(pos) = joint_entity.and_then(bone_pos) {
                    joints_to_draw.push(pos);
                }
            }
            for pair in chain.windows(2) {
                if let (Some(a), Some(b)) = (pair[0].and_then(bone_pos), pair[1].and_then(bone_pos)) {
                    draw_bone_capsule(&mut gizmos, a, b, LIMB_HITBOX_RADIUS, Color::srgb(0.2, 0.6, 1.0));
                    any_bone = true;
                }
            }
        }

        // Draw the joint positions as small distinct spheres (green, or cyan if targeted by autopilot and target trace is enabled)
        let joint_color = if is_autopilot_target && show_aim.0 {
            Color::srgb(0.0, 1.0, 1.0) // Cyan
        } else {
            Color::srgb(0.0, 1.0, 0.0) // Green
        };
        for joint_pos in &joints_to_draw {
            gizmos.sphere(Isometry3d::from_translation(*joint_pos), 0.04, joint_color);
        }

        // Highlight the targeted zombie's aim point with a red sphere (if target trace is enabled)
        if is_autopilot_target && show_aim.0 {
            let resolved_bones = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(|g| g.translation()));
            if let Some((aim_point, _)) = shared::random_aim_point(&resolved_bones, zombie_entity) {
                gizmos.sphere(Isometry3d::from_translation(aim_point), 0.08, Color::srgb(1.0, 0.0, 0.0));
            }
        }

        if any_bone {
            continue;
        }
        // No bones resolved yet (scene still loading) — show the geometric
        // fallback so there's still *something* visible.
        let root = transform.translation;
        let ring_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_HEAD_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 0.2, 0.2),
        );
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_CHEST_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 0.6, 0.0),
        );
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_BELLY_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 1.0, 0.0),
        );
    }
}

// Finds whichever bone (head/belly/chest/any limb joint) of `bones` is
// nearest to `point` and has actually resolved a GlobalTransform — used to
// pick which specific bone a wound decal should parent onto (see
// spawn_blood_stain_on_zombie's own doc comment for why a bone, not the
// zombie's root, is the one that actually animates). Purely a VFX
// placement choice, not the same "nearest region" zombie_hit_nearest_region
// (crates/shared) computes for damage-miss diagnostics — this returns an
// actual Entity to parent onto, not a named region/radius.
pub(crate) fn nearest_zombie_bone(bones: &ZombieHitboxBones, point: Vec3, globals: &Query<&GlobalTransform>) -> Option<(Entity, GlobalTransform)> {
    bones
        .head
        .into_iter()
        .chain(bones.belly)
        .chain(bones.chest)
        .chain(bones.left_arm.into_iter().flatten())
        .chain(bones.right_arm.into_iter().flatten())
        .chain(bones.left_leg.into_iter().flatten())
        .chain(bones.right_leg.into_iter().flatten())
        .filter_map(|entity| globals.get(entity).ok().map(|g| (entity, *g)))
        .min_by(|(_, a), (_, b)| a.translation().distance_squared(point).total_cmp(&b.translation().distance_squared(point)))
}

// Plays the server-decided outcome of a shot — for every client EXCEPT the
// shooter, who already predicted and showed this same hit locally in
// shoot() and is excluded from receiving it at all (see handle_fire_shot's
// SendMode::BroadcastExcept). A remote player's shot was never predicted by
// this client, so it always gets the full effect here.
pub(crate) fn on_shot_fx(
    trigger: On<ShotFx>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    show_hit_markers: Res<ShowHitMarkers>,
    show_tracers: Res<ShowBulletTracers>,
) {
    match *trigger.event() {
        ShotFx::ZombieHit { origin, point, normal, damage, killed } => {
            if damage > 0.0 {
                info!("[shot] (remote player) inner bone capsule hit: damage={damage:.1} killed={killed}");
            } else {
                info!("[shot] (remote player) outer capsule hit but missed every inner bone capsule (0 damage)");
            }
            if show_hit_markers.0 {
                commands.spawn((
                    Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                    MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                    Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                    HitMarker {
                        remaining: HIT_MARKER_LIFETIME,
                    },
                ));
            }
            // Seeded off `point` (not just time) so multiple remote shots
            // landing the same frame don't all roll the same tracer chance.
            spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 619.0 + point.x * 13.7 + point.z * 7.3, origin, point, show_tracers.0);

            // The server doesn't send which zombie entity it was (avoiding
            // cross-network entity mapping for a purely cosmetic detail) —
            // whichever zombie is nearest the hit point, then whichever of
            // ITS bones is nearest, is visually indistinguishable from the
            // real one and much simpler (see nearest_zombie_bone's own doc
            // comment for why a bone, not the zombie root).
            let nearest_bone = if killed {
                None
            } else {
                zombies
                    .iter()
                    .min_by(|(_, a, _), (_, b, _)| {
                        a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point))
                    })
                    .and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals))
            };
            let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
            spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed);
        }
        ShotFx::SurfaceHit { origin, point, normal } => {
            if show_hit_markers.0 {
                commands.spawn((
                    Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                    MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                    Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                    HitMarker {
                        remaining: HIT_MARKER_LIFETIME,
                    },
                ));
            }
            spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 619.0 + point.x * 13.7 + point.z * 7.3, origin, point, show_tracers.0);
            spawn_spark_effects(&mut commands, &vfx_assets.sparks, &time, point, normal);
        }
    }
}

// Recoil/reload/bob numbers derived purely from Gun's own state — used to
// be computed by hand, identically, in THREE places (gun_view_animation,
// sync_scope_camera, update_scope_lens): same recoil_t/reload_t/dip/
// bob_aim_scale/bob_x/bob_y arithmetic, three separate copies of one
// formula. That's exactly the kind of duplication that quietly drifts —
// tweak GUN_BOB_AMOUNT_X once and forget the other two call sites, and the
// gun/camera/lens stop moving in lockstep without any compile error to
// catch it. One pure function of `&Gun`, called by all three, instead.
pub(crate) struct GunPoseTimings {
    recoil_t: f32,
    dip: f32,
    bob_x: f32,
    bob_y: f32,
}

pub(crate) fn gun_pose_timings(gun: &Gun) -> GunPoseTimings {
    let recoil_t = gun.recoil / RECOIL_DURATION;
    let reload_t = gun.reload_remaining / RELOAD_DURATION;
    // Parabola: 0 at both ends of the reload, 1 at the midpoint — the gun
    // dips down/tilts away and comes back up as the reload finishes.
    let dip = 4.0 * reload_t * (1.0 - reload_t);
    let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
    let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
    let bob_y = ops::sin(gun.bob_phase * 2.0).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;
    GunPoseTimings { recoil_t, dip, bob_x, bob_y }
}

// The FULL sway + recoil-yaw-kick + recoil-roll-kick + reload/recoil-tilt
// rotation stack — used by update_scope_lens (the physical glass quad the
// player sees on screen; its own rotation reads as "the whole optic
// wobbling/kicking with the gun" the same way gun_view_animation's own
// mesh rotation does). NOT used for sync_scope_camera (see
// gun_camera_tilt_rotation's own doc comment right below for why the
// ScopeCamera itself only gets the tilt term, not this full stack) and
// NOT used by gun_view_animation either: the gun mesh's own rotation
// composes an extra base model-facing rotation (GUN_ROTATION_Y_DEG) and
// wall-raise tilt IN BETWEEN these same terms, which changes the
// composition order — quaternion multiplication doesn't commute, so
// forcing the mesh through this exact stack would change its own visual
// result in a way there's no device here to verify; gun_view_animation
// still uses the same underlying GunPoseTimings numbers, just composes
// its own rotation around them.
pub(crate) fn gun_recoil_sway_rotation(gun: &Gun, timings: &GunPoseTimings) -> Quat {
    Quat::from_rotation_z(gun.look_sway_roll.to_radians())
        * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
        * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
        * Quat::from_rotation_x((RELOAD_TILT_DEG * timings.dip + RECOIL_TILT_UP_DEG * timings.recoil_t).to_radians())
}

// Keeps the scope's render-to-texture camera glued to the main camera's
// position/orientation every frame — it has no parent/player-follow logic
// of its own, it just mirrors the real camera exactly.
pub(crate) fn sync_scope_camera(
    camera: Query<&Transform, (With<FpsCamera>, Without<ScopeCamera>, Without<Gun>)>,
    gun: Query<(&Gun, &Transform), (Without<FpsCamera>, Without<ScopeCamera>)>,
    mut scope_camera: Query<&mut Transform, (With<ScopeCamera>, Without<FpsCamera>, Without<Gun>)>,
) {
    let (Ok(camera_transform), Ok((gun, gun_transform)), Ok(mut scope_transform)) =
        (camera.single(), gun.single(), scope_camera.single_mut())
    else {
        return;
    };
    *scope_transform = *camera_transform;

    // Converts a delta expressed in the same local X/Y/Z space Gun's own
    // Transform.translation uses (see gun_view_animation) into a world
    // offset for the (unparented) scope camera: local X/Y map straight onto
    // right()/up(), but local Z is inverted relative to forward() since the
    // camera faces its own local -Z.
    let local_to_world = |local: Vec3| {
        camera_transform.right() * local.x + camera_transform.up() * local.y
            - camera_transform.forward() * local.z
    };

    // The gun mesh's own already-computed local Transform.translation
    // (gun_view_animation, this same frame) — shoulder-swap, ADS sweep,
    // recoil kick, reload dip, walk-bob, wall-raise, all already baked
    // in. Reused directly instead of a second, hand-written copy of that
    // same formula (see GunPoseTimings' own doc comment for the
    // shared-FORMULA-but-still-two-call-sites version of this that used
    // to be here) — "the camera goes wherever the gun mesh goes" as
    // literally as possible: it reads the gun's own real position result,
    // not a parallel calculation that merely aims for the same numbers.
    // Not literally ChildOf(Gun) — Gun's own rotation carries an extra
    // GUN_ROTATION_Y_DEG model-facing correction that would wrongly spin
    // the camera's view direction if inherited, so only translation is
    // shared this way; rotation stays computed separately below.
    scope_transform.translation += local_to_world(gun_transform.translation)
        // The eye sits at a genuinely different height than wherever the
        // gun mesh's own origin is (SCOPE_HIP_HEIGHT_OFFSET while not
        // aiming, SCOPE_CAMERA_HEIGHT_OFFSET once ADS'd) — this isn't
        // part of the gun's own translation, so it's still a separate
        // additive term.
        + camera_transform.up() * (SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend) + SCOPE_CAMERA_HEIGHT_OFFSET * gun.ads_blend);
    // Full sync with ScopeLens's own rotation (was a tilt-only subset) —
    // per request, ScopeCamera now matches every axis ScopeLens gets:
    // sway, recoil yaw-kick, recoil roll-kick, and tilt.
    // NOTE: the roll component (look_sway_roll + RECOIL_ROLL_DEG) is
    // mathematically a no-op on the actual picture the player sees
    // through the scope: ScopeCamera rolling by θ renders its captured
    // texture rotated -θ, and ScopeLens (the quad displaying that
    // texture) is rotated +θ on screen — they cancel to 0 net tilt in
    // what the player actually perceives. Only the translate sync (above)
    // and the yaw-kick/tilt rotation terms have any real visible effect
    // on the scope picture; the roll term only still matters for
    // ScopeCamera's OWN captured framing being self-consistent with the
    // lens glass's shape, not for "the picture visibly tilting".
    scope_transform.rotation *= gun_recoil_sway_rotation(gun, &gun_pose_timings(gun));
}

// Scroll wheel steps ScopeZoom up/down while aiming — ignored (and drained,
// so a scroll during ADS doesn't "carry over" and suddenly apply once you
// start aiming again) the rest of the time.
pub(crate) fn adjust_scope_zoom(
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut zoom: ResMut<ScopeZoom>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    autopilot: Res<Autopilot>,
    autopilot_state: Res<AutopilotState>,
    zombies: Query<&Transform, With<Zombie>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if !is_aiming(cursor, &mouse, touch_aiming.0) {
        wheel.clear();
        return;
    }
    // Auto-zoom only while Autopilot is actually driving — AutopilotState.
    // target lingers (last-locked entity) even after autopilot is turned
    // back off, since autopilot_pick_target stops updating it rather than
    // clearing it; without this check that stale target kept overriding
    // real scroll-wheel zoom for a manually-aiming player indefinitely.
    if autopilot.0 {
        if let Some(target_entity) = autopilot_state.target {
            if let (Ok(camera_global), Ok(zombie_transform)) =
                (camera.single(), zombies.get(target_entity))
            {
                let distance = camera_global.translation().distance(zombie_transform.translation);
                let t = ((distance - AUTO_ZOOM_NEAR_DISTANCE)
                    / (AUTO_ZOOM_FAR_DISTANCE - AUTO_ZOOM_NEAR_DISTANCE))
                    .clamp(0.0, 1.0);
                zoom.0 = SCOPE_MIN_ZOOM + (SCOPE_MAX_ZOOM - SCOPE_MIN_ZOOM) * t;
                wheel.clear();
                return;
            }
        }
    }
    for event in wheel.read() {
        zoom.0 = (zoom.0 * SCOPE_ZOOM_STEP_FACTOR.powf(event.y.signum()))
            .clamp(SCOPE_MIN_ZOOM, SCOPE_MAX_ZOOM);
    }
}

// Turns the scope camera/lens on as soon as ADS starts blending in at all
// (off again once the gun raises for a wall), applies the current
// ScopeZoom to the scope camera's FOV, and slides the lens quad in from
// GUN_OFFSET (the gun's own hip-fire pose) to SCOPE_LENS_OFFSET using the
// exact same gun.ads_blend lerp gun_view_animation drives the gun mesh's
// own translation with, then mirrors its X onto whichever shoulder the gun
// is currently on — so the lens visually travels in/out together with the
// gun instead of just popping in once ADS is mostly done.
pub(crate) fn update_scope_lens(
    gun: Query<&Gun>,
    zoom: Res<ScopeZoom>,
    scope_camera_enabled: Res<ScopeCameraEnabled>,
    mut scope_camera: Query<(&mut Camera, &mut Projection), With<ScopeCamera>>,
    mut lens: Query<(&mut Transform, &mut Visibility), (With<ScopeLens>, Without<ScopeReticle>)>,
    mut reticle: Query<&mut Transform, (With<ScopeReticle>, Without<ScopeLens>)>,
) {
    let Ok(gun) = gun.single() else {
        return;
    };
    let visible = scope_camera_enabled.0
        && gun.ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD
        && gun.wall_raise < SCOPE_HIDDEN_WALL_RAISE_THRESHOLD;

    if let Ok((mut camera, mut projection)) = scope_camera.single_mut() {
        camera.is_active = visible;
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = SCOPE_BASE_FOV / zoom.0;
        }
    }

    // Keeps the reticle roughly constant on screen across zoom levels (see
    // SCOPE_RETICLE_WIDTH/HEIGHT's doc comment) — it's sized to look right
    // at SCOPE_DEFAULT_ZOOM, so scale down as zoom increases past that.
    if let Ok(mut reticle_transform) = reticle.single_mut() {
        reticle_transform.scale = Vec3::splat(SCOPE_DEFAULT_ZOOM / zoom.0);
    }

    if let Ok((mut transform, mut visibility)) = lens.single_mut() {
        // X and Z both sweep in from GUN_OFFSET to SCOPE_LENS_OFFSET with
        // ads_blend — same lateral + "pulled in closer" travel as the gun
        // mesh and the scope camera (sync_scope_camera). Y also dips down a
        // little below SCOPE_LENS_OFFSET.y while not aiming. Recoil kick and
        // reload dip/tilt stack on top the same way they do on the gun
        // mesh's own Transform.translation — this is a plain local-space
        // translation just like Gun's, so no right()/forward() conversion
        // is needed here (unlike sync_scope_camera, which has to rebuild a
        // world offset for the free-floating scope camera).
        let lens_x = GUN_OFFSET.x + (SCOPE_LENS_OFFSET.x - GUN_OFFSET.x) * gun.ads_blend;
        let timings = gun_pose_timings(gun);
        let swapped_x = lens_x * (1.0 - 2.0 * gun.shoulder_blend) - RELOAD_LEFT_DIP * timings.dip + timings.bob_x;
        let lens_y = SCOPE_LENS_OFFSET.y - SCOPE_LENS_HIDDEN_DROP * (1.0 - gun.ads_blend)
            + SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend)
            + RECOIL_KICK_UP * timings.recoil_t
            - RELOAD_DIP * timings.dip
            - timings.bob_y;
        let lens_z = GUN_OFFSET.z + (SCOPE_LENS_OFFSET.z - GUN_OFFSET.z) * gun.ads_blend + RECOIL_KICK * timings.recoil_t;
        transform.translation = Vec3::new(swapped_x, lens_y, lens_z);
        transform.rotation = gun_recoil_sway_rotation(gun, &timings);
        *visibility = if visible { Visibility::Visible } else { Visibility::Hidden };
    }
}

// Drives the gun mesh's Transform from Gun's recoil/reload_remaining state —
// both are just countdown timers, this is the only system that turns them
// into an actual pose each frame (also where reload_remaining hitting 0
// refills the magazine).
pub(crate) fn gun_view_animation(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    game_over: Res<GameOver>,
    look_delta: Res<MouseLookDelta>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<&Transform, (With<FpsCamera>, Without<Gun>)>,
    spatial_query: SpatialQuery,
    mut query: Query<(&mut Transform, &mut Gun), Without<FpsCamera>>,
    local_moving: Res<LocalPlayerMoving>,
) {
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));
    let shoulder_target = if !game_over.0 && keys.pressed(KeyCode::KeyQ) { 1.0 } else { 0.0 };
    // Wall-raise probe: how close is the wall directly in front of the
    // camera? 0.0 = clear, 1.0 = right up against it.
    let wall_target = camera.single().ok().map_or(0.0, |camera_transform| {
        let filter = SpatialQueryFilter::default();
        spatial_query
            .cast_ray(
                camera_transform.translation,
                camera_transform.forward(),
                WALL_GUN_RAISE_DISTANCE,
                true,
                &filter,
            )
            .map_or(0.0, |hit| {
                1.0 - (hit.distance / WALL_GUN_RAISE_DISTANCE).clamp(0.0, 1.0)
            })
    });
    for (mut transform, mut gun) in &mut query {
        if gun.recoil > 0.0 {
            gun.recoil = (gun.recoil - time.delta_secs()).max(0.0);
        }
        if gun.reload_remaining > 0.0 {
            gun.reload_remaining = (gun.reload_remaining - time.delta_secs()).max(0.0);
            if gun.reload_remaining == 0.0 {
                gun.ammo = MAGAZINE_SIZE;
            }
        }

        let ads_target = if aiming { 1.0 } else { 0.0 };
        let ads_step = (GUN_ADS_SPEED * time.delta_secs()).min(1.0);
        gun.ads_blend += (ads_target - gun.ads_blend) * ads_step;

        let wall_raise_step = (WALL_GUN_RAISE_SPEED * time.delta_secs()).min(1.0);
        gun.wall_raise += (wall_target - gun.wall_raise) * wall_raise_step;

        let shoulder_step = (LEAN_SPEED * time.delta_secs()).min(1.0);
        gun.shoulder_blend += (shoulder_target - gun.shoulder_blend) * shoulder_step;

        // LocalPlayerMoving instead of is_moving(&keys) directly — that
        // raw key check is blind to touch input and, more importantly, to
        // Autopilot's synthetic movement (which never touches
        // ButtonInput<KeyCode> at all, see crate::autopilot), so the gun
        // wouldn't bob while Autopilot walked the player around, and would
        // conversely bob for no reason if the player happened to still be
        // resting fingers on WASD while Autopilot was steering. LocalPlayerMoving
        // is already the single unified signal send_player_input sets from
        // whichever input source actually produced this frame's move_axis.
        let moving = local_moving.0;
        let bob_target = if moving { 1.0 } else { 0.0 };
        let bob_blend_step = (GUN_BOB_BLEND_SPEED * time.delta_secs()).min(1.0);
        gun.bob_blend += (bob_target - gun.bob_blend) * bob_blend_step;
        if moving {
            gun.bob_phase += GUN_BOB_SPEED * time.delta_secs();
        }
        // Sign flipped from the initial guess — reported backwards in-game.
        let sway_target =
            (look_delta.0.x * LOOK_SWAY_SENSITIVITY).clamp(-LOOK_SWAY_MAX_DEG, LOOK_SWAY_MAX_DEG);
        let sway_step = (LOOK_SWAY_SPEED * time.delta_secs()).min(1.0);
        gun.look_sway_roll += (sway_target - gun.look_sway_roll) * sway_step;

        // Same GunPoseTimings sync_scope_camera/update_scope_lens read —
        // see its own doc comment for why this used to be three separate
        // hand-written copies of this exact arithmetic.
        let timings = gun_pose_timings(&gun);

        let base_pos = gun.base_offset.lerp(GUN_ADS_OFFSET, gun.ads_blend);
        // Holding the lean-left key swaps the gun over to the left shoulder
        // by mirroring its X offset, instead of just nudging it —
        // shoulder_blend eases 0 (right shoulder, normal) -> 1 (fully
        // mirrored to the left). Leaning right keeps the normal right-
        // shoulder pose.
        let swapped_x = base_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, base_pos.y, base_pos.z)
            + Vec3::new(
                timings.bob_x,
                RECOIL_KICK_UP * timings.recoil_t - RELOAD_DIP * timings.dip - timings.bob_y
                    + WALL_GUN_RAISE_HEIGHT * gun.wall_raise,
                RECOIL_KICK * timings.recoil_t,
            );
        // Recoil tilt is applied as the outer rotation (camera-local space),
        // not composed inside the model's own base orientation like the
        // reload tilt is — doing it inside meant the -90° base Y rotation
        // twisted "tilt around local X" into something closer to a sideways
        // roll than an up/down pitch (reported as the gun leaning right on
        // recoil instead of the muzzle rising).
        let base_rotation = Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians())
            * Quat::from_rotation_x(RELOAD_TILT_DEG.to_radians() * timings.dip);
        // Sign flipped: the previous sign raised the stock/rear instead of
        // the muzzle.
        // Same outer/camera-space composition as the recoil tilt above, same
        // sign so it pitches the muzzle up rather than down.
        let wall_raise_tilt = WALL_GUN_RAISE_ROT_DEG.to_radians() * gun.wall_raise;
        transform.rotation = Quat::from_rotation_x(wall_raise_tilt)
            * Quat::from_rotation_z(gun.look_sway_roll.to_radians())
            * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_x(RECOIL_TILT_UP_DEG.to_radians() * timings.recoil_t)
            * base_rotation;
    }
}

pub(crate) fn despawn_hit_markers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut HitMarker)>,
) {
    for (entity, mut marker) in &mut query {
        marker.remaining -= time.delta_secs();
        if marker.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn despawn_muzzle_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut MuzzleFlash)>,
) {
    for (entity, mut flash) in &mut query {
        flash.remaining -= time.delta_secs();
        if flash.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn despawn_bullet_tracers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut BulletTracer)>,
) {
    for (entity, mut tracer) in &mut query {
        tracer.remaining -= time.delta_secs();
        if tracer.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Puffs drift up/forward and swell as they age, then vanish once their
// lifetime runs out.
pub(crate) fn update_smoke(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut Transform, &mut Smoke)>) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut smoke) in &mut query {
        transform.translation += smoke.velocity * dt;
        transform.scale *= 1.0 + SMOKE_GROWTH_PER_SEC * dt;

        smoke.remaining -= dt;
        if smoke.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Broadcast the instant ANY client fires (see PlayerFiredFx's own doc
// comment in crate::shared for why no such signal existed before this
// pass) — finds that specific shooter's own NetPlayer entity via its
// replicated NetworkId and plays a brief, non-looping Gun_Shoot pose on
// their third-person model, for every OTHER client watching (the shooter's
// own client already knows it fired locally and has no third-person view
// of itself in normal play). A linear scan over connected players/roots is
// fine at this player count — same shape as the local/remote animation
// systems above, no indexed lookup infrastructure exists for this either.
pub(crate) fn on_player_fired_fx(
    trigger: On<PlayerFiredFx>,
    net_players: Query<(Entity, &NetworkId), With<NetPlayer>>,
    roots: Query<(&ChildOf, &PlayerModelAnimationPlayer), (With<PlayerModelAnimRoot>, Without<PlayerModelDead>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some((shooter, _)) = net_players.iter().find(|(_, id)| id.get() == trigger.shooter_network_id) else {
        return;
    };
    let Some((_, link)) = roots.iter().find(|(child_of, _)| child_of.0 == shooter) else {
        return;
    };
    if let Ok(mut player) = players.get_mut(link.0) {
        // Not .repeat() — a one-shot pose. update_remote_player_model_animation
        // will naturally take back over and restart Idle/Walk/Run the next
        // time this player's NetTransform.moving/sprinting actually changes;
        // until then this just plays once and holds its last frame, which
        // reads fine for a quick recoil-of-the-arms gesture.
        player.play(anim.shoot);
    }
}

// Same shooter-lookup shape as on_player_fired_fx above, but marks the
// player's model PlayerModelDead first so the ordinary Idle/Walk/Run
// system (which keeps running — NetTransform doesn't stop replicating just
// because this player died) doesn't cut the Death pose short.
pub(crate) fn on_player_died_fx(
    trigger: On<PlayerDiedFx>,
    mut commands: Commands,
    net_players: Query<(Entity, &NetworkId), With<NetPlayer>>,
    roots: Query<(Entity, &ChildOf, &PlayerModelAnimationPlayer), With<PlayerModelAnimRoot>>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some((who, _)) = net_players.iter().find(|(_, id)| id.get() == trigger.who_network_id) else {
        return;
    };
    let Some((root, _, link)) = roots.iter().find(|(_, child_of, _)| child_of.0 == who) else {
        return;
    };
    commands.entity(root).insert(PlayerModelDead);
    if let Ok(mut player) = players.get_mut(link.0) {
        player.stop_all();
        player.play(anim.death);
    }
}

