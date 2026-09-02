// --- Zombie + player-model client-side representation ---
// Extracted from lib.rs (Phase 4 of the gameplay-code-organization pass —
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

// Debug/dev: toggled by KeyM. Forces every zombie's animation paused
// (CPU sample + GPU skinning both skipped) regardless of distance/crowd
// tier — for isolating how much of the frame cost is actually animation,
// separate from the automatic LOD/crowd throttling in
// freeze_zombie_animations.
#[derive(Resource, Default)]
pub(crate) struct ZombieAnimDisabled(pub(crate) bool);

// Debug/dev: toggled by KeyJ. Hides every zombie health bar outright,
// skipping update_zombie_health_bars' screen-space projection/UI layout
// work entirely instead of just capping how many are shown.
#[derive(Resource, Default)]
pub(crate) struct ZombieHealthBarsHidden(pub(crate) bool);

// Debug/dev: toggled by KeyK. Forces every zombie onto the full-fidelity
// path in move_zombies (navmesh pathfind + raycast steering + MoveAndSlide
// collision), disabling the far-distance LOD shortcut entirely — useful to
// verify collision/pathing behavior for zombies that would normally be
// past ZOMBIE_LOD_DISTANCE.
#[derive(Resource, Default)]
pub(crate) struct ZombieLodDisabled(pub(crate) bool);

// Debug/dev: toggled by KeyT. Shows/hides the local player's own third-person
// PLAYER_MODEL_PATH visual — see its own doc comment for why this exists.
#[derive(Resource, Default)]
pub(crate) struct PlayerModelVisible(pub(crate) bool);

// Marks the standalone entity spawned for PLAYER_MODEL_PATH — see
// sync_player_model.
#[derive(Component)]
pub(crate) struct PlayerModel;

// Marks any PLAYER_MODEL_PATH scene root that should animate — both the
// local debug PlayerModel (above) and the child spawned for every other
// NetPlayer in on_net_player_replicated. wire_player_model_animations walks
// up from a newly-spawned AnimationPlayer looking for this, same pattern
// wire_zombie_animations uses for Zombie.
#[derive(Component)]
pub(crate) struct PlayerModelAnimRoot;

// Links a PlayerModelAnimRoot entity to the AnimationPlayer entity somewhere
// in its own scene subtree (glTF scenes put the AnimationPlayer on a child,
// not the root) — mirrors ZombieAnimationPlayer.
#[derive(Component)]
pub(crate) struct PlayerModelAnimationPlayer(pub(crate) Entity);

// Found the same way (walk up from a newly-spawned named bone to the
// PlayerModelAnimRoot ancestor, mirroring wire_zombie_hitbox_bones' own
// pattern in crates/shared/src/zombie.rs) — soldier.glb's own rig, NOT
// Mixamo-named like the zombies. Measured the ACTUAL parent/child tree
// directly from the glb (not guessed from the flat joint name list, which
// misled the first cut of this — Hips and Abdomen are BOTH on the spine
// branch, Hips -> Abdomen -> Torso -> Chest -> arms/head; legs are a
// completely separate branch, UpperLeg.L/UpperLeg.R attach straight to
// Body, not to Hips at all):
//   Body -> Hips -> Abdomen -> Torso -> Chest -> (Neck/Head, L/R Shoulder->arm)
//   Body -> UpperLeg.L -> LowerLeg.L -> Foot.L
//   Body -> UpperLeg.R -> LowerLeg.R -> Foot.R
// apply_player_model_travel_twist (movement_combat.rs) only touches the two
// UpperLeg bones — rotating the actual leg-chain roots toward the real
// travel direction — and leaves Hips/spine/arms alone entirely: the torso
// already faces the aim direction correctly (sync_player_model sets that
// on the whole model's root), so there's nothing to counter-rotate back
// there like the first (wrong) cut of this tried to.
#[derive(Component, Default)]
pub(crate) struct PlayerModelSkeletonBones {
    pub(crate) left_upper_leg: Option<Entity>,
    pub(crate) right_upper_leg: Option<Entity>,
}

// See PlayerModelSkeletonBones' own doc comment. Same "walk up ChildOf from
// a freshly-named bone" idiom as wire_zombie_hitbox_bones — cheap-rejects
// any name that isn't one of the two bones this cares about before paying
// for the walk.
pub(crate) fn wire_player_model_skeleton_bones(
    child_of: Query<&ChildOf>,
    mut roots: Query<&mut PlayerModelSkeletonBones, With<PlayerModelAnimRoot>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (bone_entity, name) in &new_names {
        let n = name.as_str();
        if n != "UpperLeg.L" && n != "UpperLeg.R" {
            continue;
        }
        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok(mut bones) = roots.get_mut(current) {
                match n {
                    "UpperLeg.L" => bones.left_upper_leg = Some(bone_entity),
                    "UpperLeg.R" => bones.right_upper_leg = Some(bone_entity),
                    _ => {}
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

// Was a plain bool (Walk vs Idle only) before Run/Shoot/Death got wired up
// — same shape/reasoning as ZombiePose. Shoot/Death aren't part of the
// continuous state this tracks (they're one-shot, played directly by
// on_player_fired_fx/on_player_died_fx without going through this
// enum/comparison at all — see those systems), so this only ever holds
// Idle/Walk/Run.
// Reflect + register_type (GamePlugin, lib.rs) so bevy_editor_pls's
// inspector (EDITOR=1, F2) can actually show/edit this on a selected
// entity — an enum dropdown to force Idle/Walk/Run for testing without
// needing to actually move the player. Debug-only convenience; nothing
// else needs this to be Reflect.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub(crate) enum PlayerModelPose {
    #[default]
    Idle,
    Walk,
    Run,
}

// Set by on_player_died_fx once that player's Death pose starts playing —
// excluded from update_remote_player_model_animation's own query so the
// next ordinary Idle/Walk/Run transition (which will keep arriving; the
// dead player's own NetTransform doesn't stop replicating) doesn't cut the
// Death animation short and snap back to Idle.
#[derive(Component)]
pub(crate) struct PlayerModelDead;

// Screen-space health bar tracking a specific zombie — update_zombie_health_bars
// repositions it every frame and despawns it once the tracked zombie is gone.
#[derive(Component)]
pub(crate) struct ZombieHealthBar {
    zombie: Entity,
}

// The colored fill inside a ZombieHealthBar, whose width update_zombie_health_bars
// scales to the tracked zombie's remaining health fraction.
#[derive(Component)]
pub(crate) struct ZombieHealthBarFill;

// The "n.n/max" text above a ZombieHealthBar, kept in sync by
// update_zombie_health_bars alongside the fill's width.
#[derive(Component)]
pub(crate) struct ZombieHealthBarText;

#[derive(Component)]
pub(crate) struct Gun {
    pub(crate) base_offset: Vec3,
    pub(crate) recoil: f32,
    pub(crate) ammo: u32,
    // 0.0 = not reloading. Ticks down from RELOAD_DURATION.
    pub(crate) reload_remaining: f32,
    // 0.0 = hip-fire pose (base_offset), 1.0 = fully raised to GUN_ADS_OFFSET.
    // Eases toward whichever is_aiming() currently says, in gun_view_animation.
    pub(crate) ads_blend: f32,
    // 0.0 = normal height, 1.0 = fully raised (WALL_GUN_RAISE_HEIGHT) to
    // clear a wall in front. Eased toward the camera's forward-wall-distance
    // probe in gun_view_animation.
    pub(crate) wall_raise: f32,
    // 0.0 = right shoulder (normal), 1.0 = fully swapped to the left
    // shoulder. Eased directly from the raw lean-left key, independently of
    // Lean.current — its target only ever toggles between 0 and 1 (never a
    // ±1 range), so a reversal never sees a doubled distance/speed the way
    // driving it straight off Lean.current did.
    pub(crate) shoulder_blend: f32,
    // Random -1.0..1.0 left/right lean set fresh each shot (see shoot()),
    // scaled by RECOIL_ROLL_DEG and recoil_t in gun_view_animation — purely
    // cosmetic, same as RECOIL_TILT_UP_DEG, so the visual kick doesn't
    // always snap along dead-straight vertical line.
    pub(crate) recoil_roll: f32,
    // Walk-bob: bob_phase only advances while moving (see LocalPlayerMoving,
    // gun_view_animation), and bob_blend eases 0..1 toward "currently
    // moving" so the bob fades in/out smoothly instead of snapping to zero
    // the instant WASD is released.
    pub(crate) bob_phase: f32,
    pub(crate) bob_blend: f32,
    // Inertia-style lean: eases toward a target proportional to this
    // frame's mouse-look yaw delta (negated — turning right should make the
    // gun's rear lag/lean left, PUBG-style), then springs back to 0 once
    // the mouse stops. See LOOK_SWAY_* constants.
    pub(crate) look_sway_roll: f32,
}

// Tags every render-mesh descendant of the Gun entity (see wire_gun_mesh),
// so shoot()'s raycast can exclude the viewmodel itself. Needed once ADS
// pulled the gun in close to and centered on the camera (GUN_ADS_OFFSET) —
// it now sits directly in the forward raycast's path, where GUN_OFFSET's
// off-center hip-fire position never did.
#[derive(Component)]
pub(crate) struct GunMesh;

// One AnimationGraph per ZombieKind for the 5 gameplay poses — mirrors the
// server's ServerZombieAnim so client and server pick the same clip for a
// pose. Meshy only has real Walking/Running clips; the rest fall back per
// zombie_kind_pose_clips.
pub(crate) struct ZombieKindGameplayAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    // [idle, walk, run, crawl, bite] — order of zombie_kind_pose_clips.
    poses: [AnimationNodeIndex; 5],
}

impl ZombieKindGameplayAnim {
    pub(crate) fn node(&self, pose: ZombiePose) -> AnimationNodeIndex {
        self.poses[match pose {
            ZombiePose::Idle => 0,
            ZombiePose::Walk => 1,
            ZombiePose::Run => 2,
            ZombiePose::Crawl => 3,
            ZombiePose::Bite => 4,
        }]
    }
    pub(crate) fn idle(&self) -> AnimationNodeIndex {
        self.poses[0]
    }
}

#[derive(Resource)]
pub(crate) struct ZombieAnim {
    classic: ZombieKindGameplayAnim,
    meshy: ZombieKindGameplayAnim,
}

impl ZombieAnim {
    pub(crate) fn slot(&self, kind: ZombieKind) -> &ZombieKindGameplayAnim {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
        }
    }
}

pub(crate) fn build_zombie_anim(asset_server: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> ZombieAnim {
    let mut build = |kind: ZombieKind| {
        let path = zombie_kind_mesh_path(kind);
        let clips: Vec<Handle<AnimationClip>> = zombie_kind_pose_clips(kind)
            .into_iter()
            .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(path)))
            .collect();
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        ZombieKindGameplayAnim {
            graph: graphs.add(graph),
            poses: [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]],
        }
    };
    ZombieAnim {
        classic: build(ZombieKind::Classic),
        meshy: build(ZombieKind::Meshy),
    }
}

// Per-kind graph holding EVERY clip in the file, indexed by clip number —
// only the ItemGallery showcase rig uses this (one zombie pinned per clip,
// see wire_zombie_animations / spawn_item_gallery_zombie_showcase). Normal
// gameplay zombies keep using ZombieAnim's 5 named slots.
#[derive(Resource)]
pub(crate) struct ZombieKindAnims {
    classic: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
    meshy: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
}

impl ZombieKindAnims {
    fn slot(&self, kind: ZombieKind) -> &(Handle<AnimationGraph>, Vec<AnimationNodeIndex>) {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
        }
    }
}

pub(crate) fn build_zombie_kind_anims(
    asset_server: &AssetServer,
    graphs: &mut Assets<AnimationGraph>,
) -> ZombieKindAnims {
    let mut build = |kind: ZombieKind| {
        let clips: Vec<Handle<AnimationClip>> = (0..zombie_kind_clip_count(kind))
            .map(|i| asset_server.load(GltfAssetLabel::Animation(i).from_asset(zombie_kind_mesh_path(kind))))
            .collect();
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        (graphs.add(graph), nodes)
    };
    ZombieKindAnims {
        classic: build(ZombieKind::Classic),
        meshy: build(ZombieKind::Meshy),
    }
}

#[derive(Resource)]
pub(crate) struct PlayerModelAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,
    // Played instead of `run` while moving backward — see
    // CharacterAnimIndices.run_back's own doc comment (lib.rs).
    pub(crate) run_back: AnimationNodeIndex,
    pub(crate) shoot: AnimationNodeIndex,
    pub(crate) death: AnimationNodeIndex,
}

// Builds a fresh PlayerModelAnim from tuning's current mesh + animation
// indices — used both by setup()'s initial spawn and by
// character_cycle_button (settings_menu.rs) to rebuild this resource from
// scratch when the active character switches (a different mesh's clips
// live at different indices, so the old AnimationGraph's nodes would point
// at the wrong — or nonexistent — clips for the new one).
pub(crate) fn build_player_model_anim(
    asset_server: &AssetServer,
    graphs: &mut Assets<AnimationGraph>,
    tuning: &PlayerModelTuning,
) -> PlayerModelAnim {
    let clips: Vec<Handle<AnimationClip>> = [
        tuning.animations.idle,
        tuning.animations.walk,
        tuning.animations.run,
        tuning.animations.run_back,
        tuning.animations.shoot,
        tuning.animations.death,
    ]
    .into_iter()
    .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(tuning.mesh.clone())))
    .collect();
    let (graph, nodes) = AnimationGraph::from_clips(clips);
    PlayerModelAnim {
        graph: graphs.add(graph),
        idle: nodes[0],
        walk: nodes[1],
        run: nodes[2],
        run_back: nodes[3],
        shoot: nodes[4],
        death: nodes[5],
    }
}

// Shared by the initial spawn in setup() and respawn_zombies, so a killed
// zombie comes back identical to a freshly-spawned one.
// Reactive spawn: the server owns Zombie AI/movement now (see
// crates/server) — this fires when a replicated Zombie entity shows up
// (already carrying Transform/NetTransform via Zombie's own
// #[require(Transform)]) and just attaches the purely cosmetic/client-local
// pieces onto that same entity: the GLTF scene, health, hitbox bones for
// zombie_hit_damage, and a persistent Sensor collider so shoot() can
// raycast against it (see the collider's own comment below).
pub(crate) fn on_zombie_replicated(
    trigger: On<Add, Zombie>,
    mut commands: Commands,
    // ItemGallery showcase zombies also carry `Zombie` but build their own
    // mesh/kind in spawn_item_gallery_zombie_showcase — don't stack a health
    // bar on top of them here.
    showcase: Query<(), With<ZombieShowcaseClip>>,
) {
    let entity = trigger.event().entity;
    if showcase.contains(entity) {
        return;
    }
    commands.entity(entity).insert(zombie_cosmetic_bundle());
    spawn_zombie_health_bar(&mut commands, entity);
    // The mesh child is spawned by on_zombie_kind_replicated instead — the
    // server replicates ZombieKind on every networked zombie, and it may
    // arrive in the same tick as Zombie or just after.
}

// Separate from on_zombie_replicated so the mesh is built from the real
// replicated kind, not a guess. Every server zombie carries ZombieKind
// (Classic default in spawn_zombies / render-compare rig), so this always
// fires for a networked zombie.
pub(crate) fn on_zombie_kind_replicated(
    trigger: On<Add, ZombieKind>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    kinds: Query<&ZombieKind>,
    // showcase rig builds its own mesh in spawn_item_gallery_zombie_showcase.
    showcase: Query<(), With<ZombieShowcaseClip>>,
) {
    let entity = trigger.event().entity;
    if showcase.contains(entity) {
        return;
    }
    let Ok(&kind) = kinds.get(entity) else { return };
    commands.entity(entity).with_child(zombie_mesh_bundle(&asset_server, kind, true));
}

// Debug-only: spawns a brand new, non-networked zombie purely on this
// client (Digit1's "spawn one in front of me" and the respawn-on-kill
// stand-in — see ZOMBIE_RESPAWN_DELAY's comment on why that one's a stopgap
// until damage/death is server-authoritative too). Stationary — no AI runs
// for it since that's server-only now, so it's really just a target dummy.
pub(crate) fn spawn_local_zombie(commands: &mut Commands, asset_server: &AssetServer, spawn_pos: Vec3) {
    let zombie_entity = commands
        .spawn((
            Transform::from_translation(spawn_pos),
            Zombie,
            // Not replicated, so unlike a real zombie it needs its own local
            // Health — nothing will ever set one via replication for it.
            // Also unkillable via shoot() now: the server has never heard of
            // this entity, so its raycast can't land on it.
            Health(ZOMBIE_MAX_HEALTH),
            zombie_cosmetic_bundle(),
        ))
        .with_child(zombie_mesh_bundle(asset_server, ZombieKind::Classic, true))
        .id();
    spawn_zombie_health_bar(commands, zombie_entity);
}

// Scenario::ItemGallery animation-review rig: two rows of stationary
// zombies (one row per mesh kind), one zombie per animation clip of that
// kind, each pinned to loop its one clip (ZombieShowcaseClip →
// wire_zombie_animations). Client-local like spawn_local_zombie — no
// NetTransform / ZombieMotion, so movement / pose / LOD systems all skip
// them and they just stand and cycle their clip. Called from setup()'s
// ItemGallery branch alongside spawn_item_gallery_test_props.
pub(crate) fn spawn_item_gallery_zombie_showcase(commands: &mut Commands, asset_server: &AssetServer) {
    for (row, kind) in [ZombieKind::Classic, ZombieKind::Meshy].into_iter().enumerate() {
        let count = zombie_kind_clip_count(kind);
        let row_z = shared::ITEM_GALLERY_ZOMBIE_ROW_Z[row];
        for (i, (x, z)) in shared::item_gallery_zombie_row(count, row_z).into_iter().enumerate() {
            commands
                // No RigidBody / Collider — walk-through, same as every
                // zombie (movement blocking is never actor-vs-actor here;
                // crowd shove is steering-force + on-hit KnockbackVelocity).
                // The floor is a fixed known height, so the root Y is placed
                // directly (zombie_kind_showcase_root_y) instead of dropped.
                .spawn((
                    Name::new(format!("zombie_{kind}_clip{i}")),
                    Transform::from_xyz(x, zombie_kind_showcase_root_y(kind), z),
                    Zombie,
                    kind,
                    ZombieShowcaseClip(i),
                    Health(ZOMBIE_MAX_HEALTH),
                    zombie_cosmetic_bundle(),
                ))
                .with_child(zombie_mesh_bundle(asset_server, kind, false));
        }
    }
}

// Definition (not just its registration in ItemGalleryCarsPlugin::build)
// is wasm32-gated too — see that fn's own doc comment for why an EDITOR=1
// F9 saver can't exist there at all.
#[cfg(not(target_arch = "wasm32"))]
const ZOMBIE_MESHY_TRANSFORM_RON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/data/zombie_meshy_transform.ron");

// F9 (EDITOR=1, ItemGallery): reads a live meshy showcase zombie's
// mesh-child scale / rotation / Y-offset and writes them to
// zombie_meshy_transform.ron, so "rotate/scale/nudge it in the editor,
// eyeball, repeat" ends with a real save. All 7 meshy showcase zombies
// share one transform, so any one is read. The root Y is physics-owned
// (gravity), so only the mesh child matters here. Runs on the same F9 as
// the car handler.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_zombie_meshy_showcase_transform(
    keys: Res<ButtonInput<KeyCode>>,
    roots: Query<(&Children, &ZombieKind), With<ZombieShowcaseClip>>,
    child_tf: Query<&Transform, (With<SceneRoot>, Without<ZombieShowcaseClip>)>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let Some((children, _)) = roots.iter().find(|(_, k)| **k == ZombieKind::Meshy) else {
        return; // not the ItemGallery showcase — nothing to save
    };
    let Some(mesh) = children.iter().find_map(|c| child_tf.get(c).ok()) else {
        return;
    };

    let (ry, rx, rz) = mesh.rotation.to_euler(EulerRot::YXZ);
    let out = format!(
        "// Meshy showcase zombie transform — Scenario::ItemGallery only.\n\
         //   (scale, mesh_y, rot_x_deg, rot_y_deg, rot_z_deg)\n\
         // F9 (EDITOR=1) writes this; hand-editing works too.\n\
         ({}, {}, {}, {}, {})\n",
        ron_f32(mesh.scale.x),
        ron_f32(mesh.translation.y),
        ron_f32(rx.to_degrees()),
        ron_f32(ry.to_degrees()),
        ron_f32(rz.to_degrees()),
    );
    match std::fs::write(ZOMBIE_MESHY_TRANSFORM_RON, out) {
        Ok(()) => {
            info!("saved meshy zombie transform -> {ZOMBIE_MESHY_TRANSFORM_RON}");
            // Runs after the car handler in the Update tuple — fold into
            // its "saved N cars" line if that one just succeeded.
            toast.ok_also("F9: saved meshy zombie transform", " + meshy zombie");
        }
        Err(e) => {
            error!("F9 meshy zombie save failed writing {ZOMBIE_MESHY_TRANSFORM_RON}: {e}");
            toast.err(format!("F9: meshy zombie save FAILED — {e}"));
        }
    }
}

// No collider here anymore — hit detection is server-authoritative now
// (crates/server's handle_fire_shot raycasts against its own zombie
// colliders), so the client doesn't need one of its own to shoot with.
// Deliberately doesn't include the mesh itself — see zombie_mesh_bundle.
// Visibility: the Zombie entity itself never had one before (nothing on it
// needed rendering directly) — now that zombie_mesh_bundle is a *child*
// or it, Bevy's visibility-propagation hook warns (bevy_ecs warning[B0004])
// if an ancestor is missing Visibility, since inherited visibility walks up
// the parent chain. Explicit here rather than relying on Zombie's own
// #[require] so this stays a client-only concern.
pub(crate) fn zombie_cosmetic_bundle() -> impl Bundle {
    (
        Visibility::default(),
        ZombieHitboxBones::default(),
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        ZombiePose::default(),
    )
}

// The visible mesh, on its own child entity instead of directly on the
// zombie's own Transform — that Transform is driven every frame by
// interpolate_net_transform (translation+rotation only, from replicated
// NetTransform), so any position offset baked into it directly would get
// clobbered the very next frame. A child's Transform is independent, which
// is what lets ZOMBIE_MODEL_Y_OFFSET actually stick (same reasoning
// PLAYER_MODEL_PATH's child already relied on). wire_zombie_animations/
// wire_zombie_hitbox_bones walk back up through ChildOf to find the Zombie
// ancestor regardless of how many hops that takes, so this extra nesting
// level doesn't break either of them.
// `for_gameplay`: networked zombies (root at the grounded rest height
// move_zombies keeps it at) need zombie_kind_gameplay_mesh_transform; the
// ItemGallery showcase (fixed showcase root Y) needs the plain one. Same
// for Classic either way.
pub(crate) fn zombie_mesh_bundle(asset_server: &AssetServer, kind: ZombieKind, for_gameplay: bool) -> impl Bundle {
    let transform = if for_gameplay {
        zombie_kind_gameplay_mesh_transform(kind)
    } else {
        zombie_kind_mesh_transform(kind)
    };
    (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(zombie_kind_mesh_path(kind)))),
        transform,
    )
}

pub(crate) fn spawn_zombie_health_bar(commands: &mut Commands, zombie: Entity) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(ZOMBIE_HEALTH_BAR_WIDTH_PX),
                height: Val::Px(ZOMBIE_HEALTH_BAR_HEIGHT_PX),
                // Hidden until update_zombie_health_bars positions it — avoids
                // a one-frame flash at the top-left corner (Val::Px(0.0)
                // default) before the first world_to_viewport projection.
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ZombieHealthBar { zombie },
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.9, 0.2)),
                ZombieHealthBarFill,
            ));
            // Sits just above the bar itself rather than overlapping it —
            // the bar is only ZOMBIE_HEALTH_BAR_HEIGHT_PX (6px) tall, too
            // thin to hold readable text.
            bar.spawn((
                Text::new(format!("{ZOMBIE_MAX_HEALTH}/{ZOMBIE_MAX_HEALTH}")),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-14.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ZombieHealthBarText,
            ));
        });
}

// Projects each tracked zombie's head position to screen space every frame
// and resizes its fill bar to match remaining health. Despawns the bar once
// its zombie is gone (killed or otherwise despawned) instead of relying on
// any despawn-children-together mechanism, since the bar is a sibling UI
// entity, not a child of the zombie.
pub(crate) fn update_zombie_health_bars(
    mut commands: Commands,
    mut timings: ResMut<PerfTimings>,
    hidden: Res<ZombieHealthBarsHidden>,
    camera: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<(Entity, &Transform, &Health, &ZombieMotion), With<Zombie>>,
    mut bars: Query<(Entity, &ZombieHealthBar, &mut Node, &Children)>,
    mut fills: Query<&mut Node, (With<ZombieHealthBarFill>, Without<ZombieHealthBar>)>,
    mut texts: Query<&mut Text, With<ZombieHealthBarText>>,
) {
    let perf_start = web_time::Instant::now();
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let Ok(player_transform) = player.single() else {
        return;
    };

    if hidden.0 {
        for (_, _, mut node, _) in &mut bars {
            node.display = Display::None;
        }
        timings.health_bars_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    // Bars are only shown at all for zombies that are both near (not
    // motion.far) AND among the ZOMBIE_HEALTH_BAR_MAX_VISIBLE closest to
    // the player — a crowd of hundreds of nearby zombies would otherwise
    // still update/layout a bar for every single one of them.
    let mut candidates: Vec<(Entity, f32)> = zombies
        .iter()
        .filter(|(_, _, _, motion)| !motion.far)
        .map(|(entity, transform, ..)| (entity, transform.translation.distance(player_transform.translation)))
        .collect();
    candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    candidates.truncate(ZOMBIE_HEALTH_BAR_MAX_VISIBLE);
    let visible_zombies: std::collections::HashSet<Entity> = candidates.into_iter().map(|(entity, _)| entity).collect();

    for (bar_entity, bar, mut node, children) in &mut bars {
        let Ok((_, zombie_transform, health, _)) = zombies.get(bar.zombie) else {
            // try_despawn instead of despawn: observed in testing that a bar
            // can already be gone by the time this runs (exact interleaving
            // with other despawn commands the same frame isn't pinned down),
            // and a plain despawn logs a noisy "entity already despawned"
            // warning for what's a harmless no-op either way.
            commands.entity(bar_entity).try_despawn();
            continue;
        };

        // Skips the screen-space projection + UI layout update entirely
        // unless this zombie made the near+closest-N cut above.
        if !visible_zombies.contains(&bar.zombie) {
            node.display = Display::None;
            continue;
        }

        let head_pos = zombie_transform.translation + Vec3::Y * ZOMBIE_HEALTH_BAR_HEIGHT;
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, head_pos) else {
            // Behind the camera's near plane — hide instead of drawing a
            // bar at some nonsensical projected position.
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = Val::Px(screen_pos.x - ZOMBIE_HEALTH_BAR_WIDTH_PX / 2.0);
        node.top = Val::Px(screen_pos.y);

        let fraction = (health.0 / ZOMBIE_MAX_HEALTH).clamp(0.0, 1.0);
        for &child in children {
            if let Ok(mut fill_node) = fills.get_mut(child) {
                fill_node.width = Val::Percent(fraction * 100.0);
            }
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 = format!("{:.1}/{ZOMBIE_MAX_HEALTH}", health.0.max(0.0));
            }
        }
    }
    timings.health_bars_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
}

// FreezeZombies stops move_zombies (position) but not, on its own, the
// AnimationPlayer's own playback — this pauses/resumes every currently
// playing animation on each zombie to match, so a frozen zombie doesn't
// keep walking/idling in place. Runs every frame rather than reacting to
// FreezeZombies changing; pause()/resume() are idempotent so that's fine.
// Also pauses animation (CPU sampling + GPU skinning) for zombies past
// ZOMBIE_LOD_DISTANCE (see ZombieMotion.far) — same "not worth paying for
// something too far to be scrutinized" trade-off as move_zombies' own LOD
// path, folded into this system rather than a separate one so the two
// pause/resume sources can't fight each other over system ordering.
pub(crate) fn freeze_zombie_animations(
    time: Res<Time>,
    freeze: Res<FreezeZombies>,
    anim_disabled: Res<ZombieAnimDisabled>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<(&Transform, &ZombieAnimationPlayer, &ZombieMotion), (With<Zombie>, Without<Player>)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    if anim_disabled.0 {
        for (_, link, _) in &zombies {
            if let Ok(mut player) = players.get_mut(link.0) {
                for (_, active) in player.playing_animations_mut() {
                    active.pause();
                }
            }
        }
        return;
    }
    // Global cap based on total live zombie count — see the
    // ZOMBIE_ANIM_CROWD_* constants' doc comment. f32::INFINITY means "no
    // global cap", so the per-zombie distance tier below decides alone.
    let total_zombie_count = zombies.iter().count();
    let global_max_hz = if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_3 {
        ZOMBIE_ANIM_CROWD_HZ_3
    } else if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_2 {
        ZOMBIE_ANIM_CROWD_HZ_2
    } else if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_1 {
        ZOMBIE_ANIM_CROWD_HZ_1
    } else {
        f32::INFINITY
    };

    for (zombie_transform, link, motion) in &zombies {
        let Ok(mut player) = players.get_mut(link.0) else {
            continue;
        };
        let distance = zombie_transform.translation.distance(player_transform.translation);
        // Within ZOMBIE_ANIM_FULL_DISTANCE, full fidelity unless the global
        // crowd cap kicks in; beyond it, the normal medium-tier rate,
        // further capped by the crowd limit too if that's lower.
        let hz = if distance <= ZOMBIE_ANIM_FULL_DISTANCE {
            global_max_hz
        } else {
            ZOMBIE_ANIM_THROTTLE_HZ.min(global_max_hz)
        };
        // Cheap per-entity phase offset so throttled zombies don't all
        // toggle pause/resume on the exact same frame.
        let phase = ops::sin(zombie_transform.translation.x * 12.9898).abs();
        let throttle_cycle = (time.elapsed_secs() * hz + phase * 2.0) % 2.0;
        let throttled_paused_this_tick = hz.is_finite() && throttle_cycle < 1.0;
        let should_pause = freeze.0 || motion.far || throttled_paused_this_tick;
        for (_, active) in player.playing_animations_mut() {
            if should_pause {
                active.pause();
            } else {
                active.resume();
            }
        }
    }
}

// Zombie AI/pathfinding/movement (big-brain + vleue_navigator +
// move_zombies/steer_around_obstacles) now live in crates/server —
// zombies are server-authoritative, replicated down as NetTransform.
//
// This client only decides its own render-cost LOD tier per zombie —
// distinct from (and simpler than) the server's movement-cost LOD, but
// reusing the same ZOMBIE_LOD_DISTANCE threshold — since ZombieMotion.far is
// still what freeze_zombie_animations/update_zombie_health_bars key off of.
pub(crate) fn update_zombie_lod(
    mut timings: ResMut<PerfTimings>,
    lod_disabled: Res<ZombieLodDisabled>,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &mut ZombieMotion), (With<Zombie>, Without<Player>)>,
    time: Res<Time>,
    mut elapsed: Local<f32>,
) {
    // No dt-dependent math here (just a per-zombie distance threshold), so
    // unlike zombies_damage_player this doesn't need accumulated-dt
    // bookkeeping to stay correct — motion.far just ends up up to
    // 1/INPUT_SEND_HZ stale, imperceptible for a render-cost LOD tier.
    *elapsed += time.delta_secs();
    if *elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    *elapsed = 0.0;
    let perf_start = web_time::Instant::now();
    let Ok(player_transform) = player.single() else {
        return;
    };
    let mut near_count = 0usize;
    let mut total = 0usize;
    for (zombie_transform, mut motion) in &mut zombies {
        total += 1;
        let distance = zombie_transform.translation.distance(player_transform.translation);
        motion.far = !lod_disabled.0 && distance > ZOMBIE_LOD_DISTANCE;
        if !motion.far {
            near_count += 1;
        }
    }
    timings.move_zombies_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
    timings.zombie_count = total;
    timings.near_zombie_count = near_count;
}

// Determining the root type before touching anything (rather than assigning
// ZombieAnim's graph unconditionally, the way this used to work back when
// zombies were the only animated character in the scene) matters now that
// wire_player_model_animations runs against the exact same
// Added<AnimationPlayer> query — assigning first and checking ancestry after
// would let both systems clobber each other's graph on whichever entity
// type they don't actually own.
pub(crate) fn wire_zombie_animations(
    mut commands: Commands,
    anim: Res<ZombieAnim>,
    kind_anims: Res<ZombieKindAnims>,
    child_of: Query<&ChildOf>,
    zombies: Query<(Option<&ZombieShowcaseClip>, Option<&ZombieKind>), With<Zombie>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| zombies.contains(e)) else {
            continue;
        };
        let (showcase, kind) = zombies
            .get(root)
            .map_or((None, None), |(s, k)| (s.copied(), k.copied()));

        // try_insert, not insert: with 100 zombies all loading their glTF
        // scenes at once, one can despawn (killed instantly, or the
        // respawn queue replacing it) between this query running and the
        // command actually applying — same benign race chase_action_system
        // already works around below. Plain insert() surfaced it as a
        // "entity despawned" warning, once per zombie, right at startup.
        if let Some(ZombieShowcaseClip(clip)) = showcase {
            // ItemGallery showcase zombie — pin to its one clip forever,
            // using the kind's full-clip graph rather than ZombieAnim's
            // 5 named slots. update_zombie_animation skips it after this.
            let (graph, clips) = kind_anims.slot(kind.unwrap_or_default());
            commands
                .entity(player_entity)
                .try_insert(AnimationGraphHandle(graph.clone()));
            if let Some(&node) = clips.get(clip % clips.len().max(1)) {
                player.play(node).repeat();
            }
        } else {
            let slot = anim.slot(kind.unwrap_or_default());
            commands
                .entity(player_entity)
                .try_insert(AnimationGraphHandle(slot.graph.clone()));
            player.play(slot.idle()).repeat();
        }
        commands.entity(root).try_insert(ZombieAnimationPlayer(player_entity));
    }
}

// Mirror of wire_zombie_animations for PLAYER_MODEL_PATH — covers both the
// local debug PlayerModel and every remote NetPlayer's model child, since
// both carry PlayerModelAnimRoot (see its own doc comment).
pub(crate) fn wire_player_model_animations(
    mut commands: Commands,
    anim: Res<PlayerModelAnim>,
    child_of: Query<&ChildOf>,
    roots: Query<(), With<PlayerModelAnimRoot>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| roots.contains(e)) else {
            continue;
        };
        commands
            .entity(player_entity)
            .try_insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();
        commands
            .entity(root)
            .try_insert((PlayerModelAnimationPlayer(player_entity), PlayerModelPose::default()));
    }
}

// Remote/bot player models: driven by the parent NetPlayer entity's
// replicated NetTransform.moving (see move_players/move_debug_bots on the
// server, which now actually set it) — same signal update_zombie_animation
// already uses for zombies. Doesn't touch the local debug model (it has no
// ChildOf, so it never matches this query — see
// update_local_player_model_animation instead).
pub(crate) fn update_remote_player_model_animation(
    over: Res<PlayerModelPoseOverride>,
    parents: Query<&NetTransform, With<NetPlayer>>,
    mut roots: Query<
        (&ChildOf, &PlayerModelAnimationPlayer, &mut PlayerModelPose),
        (With<PlayerModelAnimRoot>, Without<PlayerModelDead>),
    >,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    // See PlayerModelPoseOverride's own doc comment — while set, a manual
    // pose (F2 inspector, EDITOR=1) would get stomped back to whatever
    // real movement says every single frame otherwise.
    if over.0 {
        return;
    }
    for (child_of, link, mut pose) in &mut roots {
        let Ok(net_transform) = parents.get(child_of.0) else {
            continue;
        };
        // sprinting alone doesn't imply moving (holding Shift while stood
        // still is a real, if unusual, input) — gate Run on both, same as
        // the sprint boolean itself only being computed while genuinely
        // trying to move on the sending client (send_player_input's own
        // `sprinting` local).
        let new_pose = if net_transform.moving && net_transform.sprinting {
            PlayerModelPose::Run
        } else if net_transform.moving {
            PlayerModelPose::Walk
        } else {
            PlayerModelPose::Idle
        };
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            let node = match new_pose {
                PlayerModelPose::Idle => anim.idle,
                PlayerModelPose::Walk => anim.walk,
                PlayerModelPose::Run => anim.run,
            };
            player.play(node).repeat();
        }
    }
}

// The local debug model has no NetPlayer parent to read NetTransform.moving
// from (it's a standalone entity, see PlayerModel's own doc comment) — reads
// LocalPlayerMoving instead, which send_player_input sets from this frame's
// own move_axis directly (no need to wait on a replication round trip for
// something only this client ever sees anyway).
pub(crate) fn update_local_player_model_animation(
    over: Res<PlayerModelPoseOverride>,
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    move_axis: Res<LocalPlayerMoveAxis>,
    mut roots: Query<(&PlayerModelAnimationPlayer, &mut PlayerModelPose), (With<PlayerModelAnimRoot>, With<PlayerModel>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
    // Whether Run was last played as anim.run_back rather than anim.run —
    // *pose alone can't tell (both are PlayerModelPose::Run), so reversing
    // direction while already sprinting would otherwise fall through the
    // `new_pose == *pose` early-return below and never actually swap the
    // playing clip. Not worth a real Resource for a single bool only this
    // fn reads/writes.
    mut last_run_back: Local<bool>,
) {
    // See PlayerModelPoseOverride's own doc comment — while set, a manual
    // pose (F2 inspector, EDITOR=1) would get stomped back to whatever
    // real movement says every single frame otherwise.
    if over.0 {
        return;
    }
    let Ok((link, mut pose)) = roots.single_mut() else {
        return;
    };
    let new_pose = if moving.0 && sprinting.0 {
        PlayerModelPose::Run
    } else if moving.0 {
        PlayerModelPose::Walk
    } else {
        PlayerModelPose::Idle
    };
    // See CharacterAnimIndices.run_back's own doc comment (lib.rs) — Walk
    // has no backward clip to switch to, so this only matters for Run.
    let run_back = new_pose == PlayerModelPose::Run && move_axis.0.y < 0.0;
    if new_pose == *pose && run_back == *last_run_back {
        return;
    }
    *pose = new_pose;
    *last_run_back = run_back;
    if let Ok(mut player) = players.get_mut(link.0) {
        player.stop_all();
        let node = match new_pose {
            PlayerModelPose::Idle => anim.idle,
            PlayerModelPose::Walk => anim.walk,
            PlayerModelPose::Run => {
                if run_back {
                    anim.run_back
                } else {
                    anim.run
                }
            }
        };
        player.play(node).repeat();
    }
}

// Debug-only (EDITOR=1): flip this in the F2 inspector's Resources tab to
// freeze update_local_player_model_animation/update_remote_player_model_
// animation's own real-movement-driven overwrite, so a manually-picked
// PlayerModelPose (selected on a player entity in the inspector) actually
// sticks and plays instead of snapping back to Idle the very next frame —
// this is what was silently fighting a "F2, change PlayerModelPose,
// dropdown value doesn't change" attempt before this resource existed.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub(crate) struct PlayerModelPoseOverride(pub(crate) bool);

// Companion to PlayerModelPoseOverride: while it's on, this is the only
// thing left driving the AnimationPlayer — reacts to the pose actually
// changing (Changed<PlayerModelPose>, whether from the inspector or
// anywhere else) and plays the matching clip, since a plain component edit
// alone never calls AnimationPlayer::play by itself.
pub(crate) fn apply_player_model_pose_override(
    over: Res<PlayerModelPoseOverride>,
    anim: Res<PlayerModelAnim>,
    roots: Query<(&PlayerModelAnimationPlayer, &PlayerModelPose), (With<PlayerModelAnimRoot>, Changed<PlayerModelPose>)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if !over.0 {
        return;
    }
    for (link, pose) in &roots {
        let Ok(mut player) = players.get_mut(link.0) else {
            continue;
        };
        player.stop_all();
        let node = match *pose {
            PlayerModelPose::Idle => anim.idle,
            PlayerModelPose::Walk => anim.walk,
            PlayerModelPose::Run => anim.run,
        };
        player.play(node).repeat();
    }
}

// Tags every new Mesh3d that turns out to be a descendant of the Gun entity
// with GunMesh, once the ak47_scope.glb scene finishes loading (same async
// timing problem as wire_zombie_animations/wire_zombie_hitbox_bones) — see
// GunMesh's doc comment for why this needs to exist.
pub(crate) fn wire_gun_mesh(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    gun: Query<(), With<Gun>>,
    new_meshes: Query<Entity, Added<Mesh3d>>,
) {
    for mesh_entity in &new_meshes {
        let mut current = mesh_entity;
        for _ in 0..20 {
            if gun.contains(current) {
                commands
                    .entity(mesh_entity)
                    .insert((GunMesh, RenderLayers::layer(VIEWMODEL_LAYER)));
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

// Switches a zombie between its Walk and Idle clips exactly when `Chase` is
// added/removed, rather than every frame.
// Switches a zombie between its Walk and Idle clips based on the server's
// replicated NetTransform.moving flag (Chase itself isn't replicated — see
// NetTransform's own comment on why).
// All 5 of zombie.glb's clips, picked by priority (highest first) from
// state this client already computes/has locally — no new replication
// needed, same "each client checks distance/Health itself" precedent
// zombies_damage_player already set for the identical ZOMBIE_STOP_DISTANCE
// melee check:
//   1. Bite  — within ZOMBIE_STOP_DISTANCE of the local player (the same
//      range zombies_damage_player uses to actually deal damage) —
//      overrides everything else while in melee range, wounded or not.
//   2. Crawl — Health at/below ZOMBIE_CRAWL_HEALTH_FRACTION of max, not
//      currently biting — a "nearly dead" telegraph with no prior
//      precedent in this codebase (there was no wounded-state concept
//      before this).
//   3. Run   — moving (NetTransform.moving) and within ZOMBIE_RUN_DISTANCE
//      of the player, not crawling/biting — a "closing in" tension beat.
//   4. Walk  — moving, none of the above.
//   5. Idle  — not moving, none of the above.
pub(crate) fn update_zombie_animation(
    anim: Res<ZombieAnim>,
    player: Query<&Transform, With<Player>>,
    // Without<ZombieShowcaseClip>: ItemGallery showcase zombies stay pinned
    // to the single clip wire_zombie_animations gave them (they also carry
    // no NetTransform, so this query would skip them anyway — belt and braces).
    mut zombies: Query<
        (&Transform, &NetTransform, &Health, &ZombieAnimationPlayer, Option<&ZombieKind>, &mut ZombiePose),
        (With<Zombie>, Without<ZombieShowcaseClip>),
    >,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    for (zombie_transform, net_transform, health, link, kind, mut pose) in &mut zombies {
        let distance = zombie_transform.translation.distance(player_transform.translation);
        let new_pose = zombie_pose_for(distance, health.0 / ZOMBIE_MAX_HEALTH, net_transform.moving);
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            player.play(anim.slot(kind.copied().unwrap_or_default()).node(new_pose)).repeat();
        }
    }
}

