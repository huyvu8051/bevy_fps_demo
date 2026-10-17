
use bevy::prelude::*;
use bevy::animation::{AnimatedBy, AnimationTargetId};
use crate::*;

#[derive(Resource, Default)]
pub(crate) struct ZombieAnimDisabled(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct ZombieHealthBarsHidden(pub(crate) bool);

#[derive(Component)]
pub(crate) struct PlayerModelAnimRoot;

#[derive(Component)]
pub(crate) struct PlayerModelAnimationPlayer(pub(crate) Entity);

#[derive(Component, Default)]
pub(crate) struct PlayerModelSkeletonBones {
    pub(crate) left_upper_leg: Option<Entity>,
    pub(crate) right_upper_leg: Option<Entity>,
    pub(crate) right_arm: Option<Entity>,
    pub(crate) right_fore_arm: Option<Entity>,
    pub(crate) right_hand: Option<Entity>,
    pub(crate) left_arm: Option<Entity>,
    pub(crate) left_fore_arm: Option<Entity>,
    pub(crate) left_hand: Option<Entity>,
    pub(crate) head: Option<Entity>,
    pub(crate) hips: Option<Entity>,
}

#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub(crate) enum PlayerModelPose {
    #[default]
    Idle,
    Walk,
    Run,
}


#[derive(Component)]
pub(crate) struct ZombieHealthBar {
    zombie: Entity,
}

#[derive(Component)]
pub(crate) struct ZombieHealthBarFill;

#[derive(Component)]
pub(crate) struct ZombieHealthBarText;

#[derive(Component)]
pub(crate) struct Gun {
    pub(crate) base_offset: Vec3,
    pub(crate) recoil: f32,
    pub(crate) ammo: u32,

    pub(crate) reload_remaining: f32,

    pub(crate) ads_blend: f32,

    pub(crate) wall_raise: f32,

    pub(crate) shoulder_blend: f32,

    pub(crate) recoil_roll: f32,

    pub(crate) bob_phase: f32,
    pub(crate) bob_blend: f32,

    pub(crate) look_sway_roll: f32,
}

#[derive(Component)]
pub(crate) struct GunMesh;

#[derive(Component)]
pub(crate) struct GunMeshPart;

#[derive(Resource)]
pub(crate) struct ZombieKindAnims {
    classic: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
    soldier: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
    female: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
}

impl ZombieKindAnims {
    fn slot(&self, kind: ZombieKind) -> &(Handle<AnimationGraph>, Vec<AnimationNodeIndex>) {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Soldier => &self.soldier,
            ZombieKind::Female => &self.female,
        }
    }
}

pub(crate) fn build_zombie_kind_anims(
    asset_server: &AssetServer,
    graphs: &mut Assets<AnimationGraph>,
) -> ZombieKindAnims {
    let mut build = |kind: ZombieKind| {
        let clips: Vec<Handle<AnimationClip>> = (0..zombie_kind_clip_count(kind))
            .map(|i| asset_server.load(GltfAssetLabel::Animation(i).from_asset(zombie_kind_anim_path(kind))))
            .collect();
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        (graphs.add(graph), nodes)
    };
    ZombieKindAnims {
        classic: build(ZombieKind::Classic),
        soldier: build(ZombieKind::Soldier),
        female: build(ZombieKind::Female),
    }
}

#[derive(Resource)]
pub(crate) struct PlayerModelAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,

    pub(crate) run_back: AnimationNodeIndex,
}

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
    ]
    .into_iter()
    .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(tuning.anim_source_or_mesh().to_string())))
    .collect();
    let (graph, nodes) = AnimationGraph::from_clips(clips);
    PlayerModelAnim {
        graph: graphs.add(graph),
        idle: nodes[0],
        walk: nodes[1],
        run: nodes[2],
        run_back: nodes[3],
    }
}

pub(crate) fn on_zombie_replicated(
    trigger: On<Add, Zombie>,
    mut commands: Commands,

    showcase: Query<(), With<ZombieShowcaseClip>>,
) {
    let entity = trigger.event().entity;
    if showcase.contains(entity) {
        return;
    }
    commands.entity(entity).insert(zombie_cosmetic_bundle());
    spawn_zombie_health_bar(&mut commands, entity);
    spawn_radar_blip(&mut commands, entity);
}

pub(crate) fn on_zombie_kind_replicated(
    trigger: On<Add, ZombieKind>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    kinds: Query<&ZombieKind>,

    showcase: Query<(), With<ZombieShowcaseClip>>,
) {
    let entity = trigger.event().entity;
    if showcase.contains(entity) {
        return;
    }
    let Ok(&kind) = kinds.get(entity) else { return };
    commands.entity(entity).with_child(zombie_mesh_bundle(&asset_server, kind));
}

pub(crate) fn spawn_local_zombie(commands: &mut Commands, asset_server: &AssetServer, spawn_pos: Vec3) {
    let zombie_entity = commands
        .spawn((
            Transform::from_translation(spawn_pos),
            Zombie,

            Health(ZOMBIE_MAX_HEALTH),
            zombie_cosmetic_bundle(),
        ))
        .with_child(zombie_mesh_bundle(asset_server, ZombieKind::Classic))
        .id();
    spawn_zombie_health_bar(commands, zombie_entity);
    spawn_radar_blip(commands, zombie_entity);
}

pub(crate) fn zombie_cosmetic_bundle() -> impl Bundle {
    (
        Visibility::default(),
        ZombieHitboxBones::default(),
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        ZombiePose::default(),
    )
}

pub(crate) fn zombie_mesh_bundle(asset_server: &AssetServer, kind: ZombieKind) -> impl Bundle {
    (
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(zombie_kind_mesh_path(kind)))),
        Transform::IDENTITY,
    )
}

pub(crate) fn spawn_zombie_health_bar(commands: &mut Commands, zombie: Entity) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(ZOMBIE_HEALTH_BAR_WIDTH_PX),
                height: Val::Px(ZOMBIE_HEALTH_BAR_HEIGHT_PX),

                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ZombieHealthBar { zombie },
            NativeUiRoot,
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

            bar.spawn((
                Text::new(format!("{ZOMBIE_MAX_HEALTH}/{ZOMBIE_MAX_HEALTH}")),
                TextFont {
                    font_size: FontSize::Px(12.0),
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

pub(crate) fn update_zombie_health_bars(
    mut commands: Commands,
    mut timings: ResMut<PerfTimings>,
    hidden: Res<ZombieHealthBarsHidden>,
    camera: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
    zombies: Query<(&Transform, &Health), With<Zombie>>,
    mut bars: Query<(Entity, &ZombieHealthBar, &mut Node, &Children)>,
    mut fills: Query<&mut Node, (With<ZombieHealthBarFill>, Without<ZombieHealthBar>)>,
    mut texts: Query<&mut Text, With<ZombieHealthBarText>>,
) {
    let perf_start = std::time::Instant::now();
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    if hidden.0 {
        for (_, _, mut node, _) in &mut bars {
            node.display = Display::None;
        }
        timings.health_bars_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    for (bar_entity, bar, mut node, children) in &mut bars {
        let Ok((zombie_transform, health)) = zombies.get(bar.zombie) else {

            commands.entity(bar_entity).try_despawn();
            continue;
        };

        let head_pos = zombie_transform.translation + Vec3::Y * ZOMBIE_HEALTH_BAR_HEIGHT;
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, head_pos) else {

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

pub(crate) fn freeze_zombie_animations(
    freeze: Res<FreezeZombies>,
    anim_disabled: Res<ZombieAnimDisabled>,
    zombies: Query<&ZombieAnimationPlayer, With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let should_pause = freeze.0 || anim_disabled.0;
    for link in &zombies {
        let Ok(mut player) = players.get_mut(link.0) else {
            continue;
        };
        for (_, active) in player.playing_animations_mut() {
            if should_pause {
                active.pause();
            } else {
                active.resume();
            }
        }
    }
}

const ZOMBIE_SPLIT_SCENE_ROOT_NAME: &str = "target_character";

pub(crate) fn wire_zombie_scene_animation_targets(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
    names: Query<&Name>,
    zombies: Query<(), With<Zombie>>,
    new_names: Query<Entity, Added<Name>>,
) {
    for entity in &new_names {
        let Ok(name) = names.get(entity) else { continue };
        if name.as_str() != ZOMBIE_SPLIT_SCENE_ROOT_NAME {
            continue;
        }

        if find_animation_root(entity, &child_of, |e| zombies.contains(e)).is_none() {
            continue;
        }
        commands.entity(entity).try_insert(AnimationPlayer::default());
        wire_animation_target_path(&mut commands, entity, entity, Vec::new(), &children, &names);
    }
}

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

        if let Some(ZombieShowcaseClip(clip)) = showcase {

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

const PLAYER_MODEL_SCENE_ROOT_NAME: &str = "target_character";

pub(crate) fn wire_player_model_scene_nodes(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform>,
    mut roots: Query<&mut PlayerModelSkeletonBones, With<PlayerModelAnimRoot>>,
    new_names: Query<Entity, Added<Name>>,
) {
    for entity in &new_names {
        let Ok(name) = names.get(entity) else { continue };
        match name.as_str() {
            "LeftUpLeg" | "RightUpLeg" | "RightArm" | "RightForeArm" | "RightHand" | "LeftArm"
            | "LeftForeArm" | "LeftHand" | "Head" | "Hips" => {
                let Some(root) = find_animation_root(entity, &child_of, |e| roots.contains(e)) else {
                    continue;
                };
                let Ok(mut bones) = roots.get_mut(root) else { continue };
                // Capture the bind rotation here, not later: this function is what
                // attaches the AnimationPlayer, so nothing has posed the bone yet.
                if let Ok(transform) = transforms.get(entity) {
                    commands.entity(entity).try_insert(BoneRest(transform.rotation));
                }
                // this rig carries Mixamo bone names without the mixamorig: prefix
                match name.as_str() {
                    "LeftUpLeg" => bones.left_upper_leg = Some(entity),
                    "RightUpLeg" => bones.right_upper_leg = Some(entity),
                    "RightArm" => bones.right_arm = Some(entity),
                    "RightForeArm" => bones.right_fore_arm = Some(entity),
                    "RightHand" => bones.right_hand = Some(entity),
                    "LeftArm" => bones.left_arm = Some(entity),
                    "LeftHand" => bones.left_hand = Some(entity),
                    "Head" => bones.head = Some(entity),
                    "Hips" => bones.hips = Some(entity),
                    _ => bones.left_fore_arm = Some(entity),
                }

            }
            PLAYER_MODEL_SCENE_ROOT_NAME => {

                if find_animation_root(entity, &child_of, |e| roots.contains(e)).is_none() {
                    continue;
                }
                commands.entity(entity).try_insert(AnimationPlayer::default());
                wire_animation_target_path(&mut commands, entity, entity, Vec::new(), &children, &names);
            }
            _ => {}
        }
    }
}

fn wire_animation_target_path(
    commands: &mut Commands,
    player: Entity,
    entity: Entity,
    mut path: Vec<Name>,
    children: &Query<&Children>,
    names: &Query<&Name>,
) {
    let Ok(name) = names.get(entity) else { return };
    path.push(name.clone());
    commands.entity(entity).try_insert((AnimationTargetId::from_names(path.iter()), AnimatedBy(player)));
    if let Ok(kids) = children.get(entity) {
        for &child in kids {
            wire_animation_target_path(commands, player, child, path.clone(), children, names);
        }
    }
}

pub(crate) fn wire_player_model_animations(
    mut commands: Commands,
    anim: Res<PlayerModelAnim>,
    child_of: Query<&ChildOf>,
    roots: Query<(), With<PlayerModelAnimRoot>>,
    viewmodel: Query<(), With<ViewmodelBody>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| roots.contains(e)) else {
            continue;
        };
        // The viewmodel body needs the root marker so its bones get wired, but
        // not the clip: a walk cycle rewriting the arms every frame fights the
        // grip IK, which showed up as the arm span jittering between 0.18 and
        // 0.41m instead of settling.
        if viewmodel.contains(root) {
            continue;
        }
        commands
            .entity(player_entity)
            .try_insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();
        commands
            .entity(root)
            .try_insert((PlayerModelAnimationPlayer(player_entity), PlayerModelPose::default()));
    }
}


pub(crate) fn update_local_player_model_animation(
    over: Res<PlayerModelPoseOverride>,
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    move_axis: Res<LocalPlayerMoveAxis>,
    mut roots: Query<(&PlayerModelAnimationPlayer, &mut PlayerModelPose), (With<PlayerModelAnimRoot>, With<ViewmodelBody>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,

    mut last_run_back: Local<bool>,
) {

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

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub(crate) struct PlayerModelPoseOverride(pub(crate) bool);

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

pub(crate) fn update_zombie_animation(
    anim: Res<ZombieAnim>,
    player: Query<&Transform, With<Player>>,

    mut zombies: Query<
        (&Transform, &ZombieMotion, &Health, &ZombieAnimationPlayer, Option<&ZombieKind>, &mut ZombiePose),
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
            let slot = anim.slot(kind.copied().unwrap_or_default());
            player.stop_all();
            player.play(slot.node(new_pose)).repeat();
        }
    }
}
