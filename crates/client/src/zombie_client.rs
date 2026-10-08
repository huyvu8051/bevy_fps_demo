
use bevy::prelude::*;
use bevy::animation::{AnimatedBy, AnimationTargetId};
use crate::*;

#[derive(Resource, Default)]
pub(crate) struct ZombieAnimDisabled(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct ZombieHealthBarsHidden(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct ZombieLodDisabled(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct PlayerModelVisible(pub(crate) bool);

#[derive(Component)]
pub(crate) struct PlayerModel;

#[derive(Component)]
pub(crate) struct PlayerModelAnimRoot;

#[derive(Component)]
pub(crate) struct PlayerModelAnimationPlayer(pub(crate) Entity);

#[derive(Component, Default)]
pub(crate) struct PlayerModelSkeletonBones {
    pub(crate) left_upper_leg: Option<Entity>,
    pub(crate) right_upper_leg: Option<Entity>,
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
pub(crate) struct PlayerModelDead;

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

pub(crate) struct ZombieKindGameplayAnim {
    pub(crate) graph: Handle<AnimationGraph>,

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
    soldier: ZombieKindGameplayAnim,
}

impl ZombieAnim {
    pub(crate) fn slot(&self, kind: ZombieKind) -> &ZombieKindGameplayAnim {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
            ZombieKind::Soldier => &self.soldier,
        }
    }
}

pub(crate) fn build_zombie_anim(asset_server: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> ZombieAnim {
    let mut build = |kind: ZombieKind| {
        let path = zombie_kind_anim_path(kind);
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
        soldier: build(ZombieKind::Soldier),
    }
}

#[derive(Resource)]
pub(crate) struct ZombieKindAnims {
    classic: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
    meshy: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
    soldier: (Handle<AnimationGraph>, Vec<AnimationNodeIndex>),
}

impl ZombieKindAnims {
    fn slot(&self, kind: ZombieKind) -> &(Handle<AnimationGraph>, Vec<AnimationNodeIndex>) {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
            ZombieKind::Soldier => &self.soldier,
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
        meshy: build(ZombieKind::Meshy),
        soldier: build(ZombieKind::Soldier),
    }
}

#[derive(Resource)]
pub(crate) struct PlayerModelAnim {
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,

    pub(crate) run_back: AnimationNodeIndex,
    pub(crate) shoot: AnimationNodeIndex,
    pub(crate) death: AnimationNodeIndex,
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
        tuning.animations.shoot,
        tuning.animations.death,
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
        shoot: nodes[4],
        death: nodes[5],
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

pub(crate) fn spawn_item_gallery_zombie_showcase(commands: &mut Commands, asset_server: &AssetServer) {
    for (row, kind) in [ZombieKind::Classic, ZombieKind::Meshy, ZombieKind::Soldier].into_iter().enumerate() {
        let count = zombie_kind_clip_count(kind);
        let row_z = shared::ITEM_GALLERY_ZOMBIE_ROW_Z[row];
        for (i, (x, z)) in shared::item_gallery_zombie_row(count, row_z).into_iter().enumerate() {
            commands

                .spawn((
                    Name::new(format!("zombie_{kind}_clip{i}")),
                    Transform::from_xyz(x, 0.0, z),
                    Zombie,
                    kind,
                    ZombieShowcaseClip(i),
                    Health(ZOMBIE_MAX_HEALTH),
                    zombie_cosmetic_bundle(),
                ))
                .with_child(zombie_mesh_bundle(asset_server, kind));
        }
    }
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

pub(crate) fn apply_zombie_texture_quality(
    asset_server: Res<AssetServer>,
    texture_quality: Res<ZombieTextureQuality>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    zombies: Query<(Entity, &ZombieKind)>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut cached: Local<std::collections::HashMap<ZombieKind, Handle<StandardMaterial>>>,
) {
    for (entity, &kind) in &zombies {
        if cached.contains_key(&kind) {
            continue;
        }
        if let Some(handle) = find_material_in_descendants(entity, &children, &mesh_materials) {
            cached.insert(kind, handle);
        }
    }
    if !texture_quality.is_changed() {
        return;
    }
    for (&kind, handle) in cached.iter() {
        let texture: Handle<Image> = asset_server.load(shared::zombie_kind_texture_path(kind, texture_quality.0));
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color_texture = Some(texture);
        }
    }
}

pub(crate) fn apply_zombie_distance_lod_texture(
    enabled: Res<DistanceLodEnabled>,
    bands: Res<LodBandDistance>,
    texture_levels: Res<LodZombieTextureLevel>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<(Entity, &Transform, &ZombieKind)>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut cached: Local<std::collections::HashMap<ZombieKind, Handle<StandardMaterial>>>,
    mut last_tier: Local<std::collections::HashMap<ZombieKind, u8>>,
) {
    if !enabled.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.xz();

    let mut nearest: std::collections::HashMap<ZombieKind, f32> = std::collections::HashMap::new();
    for (entity, transform, &kind) in &zombies {
        if !cached.contains_key(&kind)
            && let Some(handle) = find_material_in_descendants(entity, &children, &mesh_materials)
        {
            cached.insert(kind, handle);
        }
        let distance = (transform.translation.xz() - player_pos).length();
        nearest.entry(kind).and_modify(|d| *d = d.min(distance)).or_insert(distance);
    }

    for (&kind, &distance) in &nearest {
        let Some(handle) = cached.get(&kind) else {
            continue;
        };
        let band = bands.band_of(distance);
        let target_tier = texture_levels.0[band];
        if last_tier.get(&kind) == Some(&target_tier) {
            continue;
        }
        let texture: Handle<Image> = asset_server.load(shared::zombie_kind_texture_path(kind, target_tier));
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color_texture = Some(texture);
        }
        last_tier.insert(kind, target_tier);
    }
}

pub(crate) fn find_material_in_descendants(
    entity: Entity,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
) -> Option<Handle<StandardMaterial>> {
    if let Ok(material) = mesh_materials.get(entity) {
        return Some(material.0.clone());
    }
    let kids = children.get(entity).ok()?;
    kids.iter().find_map(|child| find_material_in_descendants(child, children, mesh_materials))
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

            commands.entity(bar_entity).try_despawn();
            continue;
        };

        if !visible_zombies.contains(&bar.zombie) {
            node.display = Display::None;
            continue;
        }

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

        let hz = if distance <= ZOMBIE_ANIM_FULL_DISTANCE {
            global_max_hz
        } else {
            ZOMBIE_ANIM_THROTTLE_HZ.min(global_max_hz)
        };

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

pub(crate) fn update_zombie_lod(
    mut timings: ResMut<PerfTimings>,
    lod_disabled: Res<ZombieLodDisabled>,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &mut ZombieMotion), (With<Zombie>, Without<Player>)>,
    time: Res<Time>,
    mut elapsed: Local<f32>,
) {

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
    mut roots: Query<&mut PlayerModelSkeletonBones, With<PlayerModelAnimRoot>>,
    new_names: Query<Entity, Added<Name>>,
) {
    for entity in &new_names {
        let Ok(name) = names.get(entity) else { continue };
        match name.as_str() {
            "LeftUpLeg" | "RightUpLeg" => {
                let Some(root) = find_animation_root(entity, &child_of, |e| roots.contains(e)) else {
                    continue;
                };
                let Ok(mut bones) = roots.get_mut(root) else { continue };
                if name.as_str() == "LeftUpLeg" {
                    bones.left_upper_leg = Some(entity);
                } else {
                    bones.right_upper_leg = Some(entity);
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

    if over.0 {
        return;
    }
    for (child_of, link, mut pose) in &mut roots {
        let Ok(net_transform) = parents.get(child_of.0) else {
            continue;
        };

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

pub(crate) fn update_local_player_model_animation(
    over: Res<PlayerModelPoseOverride>,
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    move_axis: Res<LocalPlayerMoveAxis>,
    mut roots: Query<(&PlayerModelAnimationPlayer, &mut PlayerModelPose), (With<PlayerModelAnimRoot>, With<PlayerModel>)>,
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
