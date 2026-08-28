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

// Was a plain bool (Walk vs Idle only) before Run/Shoot/Death got wired up
// — same shape/reasoning as ZombiePose. Shoot/Death aren't part of the
// continuous state this tracks (they're one-shot, played directly by
// on_player_fired_fx/on_player_died_fx without going through this
// enum/comparison at all — see those systems), so this only ever holds
// Idle/Walk/Run.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
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

#[derive(Resource)]
pub(crate) struct ZombieAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,
    pub(crate) crawl: AnimationNodeIndex,
    pub(crate) bite: AnimationNodeIndex,
}

#[derive(Resource)]
pub(crate) struct PlayerModelAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,
    pub(crate) shoot: AnimationNodeIndex,
    pub(crate) death: AnimationNodeIndex,
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
pub(crate) fn on_zombie_replicated(trigger: On<Add, Zombie>, mut commands: Commands, asset_server: Res<AssetServer>) {
    let entity = trigger.event().entity;
    commands.entity(entity).insert(zombie_cosmetic_bundle());
    commands.entity(entity).with_child(zombie_mesh_bundle(&asset_server));
    spawn_zombie_health_bar(&mut commands, entity);
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
        .with_child(zombie_mesh_bundle(asset_server))
        .id();
    spawn_zombie_health_bar(commands, zombie_entity);
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
pub(crate) fn zombie_mesh_bundle(asset_server: &AssetServer) -> impl Bundle {
    (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_PATH))),
        Transform::from_xyz(0.0, ZOMBIE_MODEL_Y_OFFSET, 0.0).with_scale(Vec3::splat(ZOMBIE_SCALE)),
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
    let perf_start = std::time::Instant::now();
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
    let perf_start = std::time::Instant::now();
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
    child_of: Query<&ChildOf>,
    zombies: Query<(), With<Zombie>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| zombies.contains(e)) else {
            continue;
        };
        // try_insert, not insert: with 100 zombies all loading their glTF
        // scenes at once, one can despawn (killed instantly, or the
        // respawn queue replacing it) between this query running and the
        // command actually applying — same benign race chase_action_system
        // already works around below. Plain insert() surfaced it as a
        // "entity despawned" warning, once per zombie, right at startup.
        commands
            .entity(player_entity)
            .try_insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();
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
    parents: Query<&NetTransform, With<NetPlayer>>,
    mut roots: Query<
        (&ChildOf, &PlayerModelAnimationPlayer, &mut PlayerModelPose),
        (With<PlayerModelAnimRoot>, Without<PlayerModelDead>),
    >,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
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
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    mut roots: Query<(&PlayerModelAnimationPlayer, &mut PlayerModelPose), (With<PlayerModelAnimRoot>, With<PlayerModel>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
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
    if new_pose == *pose {
        return;
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
    mut zombies: Query<(&Transform, &NetTransform, &Health, &ZombieAnimationPlayer, &mut ZombiePose), With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    for (zombie_transform, net_transform, health, link, mut pose) in &mut zombies {
        let distance = zombie_transform.translation.distance(player_transform.translation);
        let new_pose = zombie_pose_for(distance, health.0 / ZOMBIE_MAX_HEALTH, net_transform.moving);
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            let node = match new_pose {
                ZombiePose::Idle => anim.idle,
                ZombiePose::Walk => anim.walk,
                ZombiePose::Run => anim.run,
                ZombiePose::Crawl => anim.crawl,
                ZombiePose::Bite => anim.bite,
            };
            player.play(node).repeat();
        }
    }
}

