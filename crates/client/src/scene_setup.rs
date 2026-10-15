
use bevy::animation::graph::AnimationGraph;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use crate::*;

#[derive(Resource)]
pub(crate) struct SceneRenderTarget(pub(crate) Handle<Image>);

pub(crate) fn setup_common(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_model_tuning: Res<PlayerModelTuning>,
    render_resolution_scale: Res<RenderResolutionScale>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let (window_width, window_height) = windows
        .single()
        .map(|window| (window.resolution.physical_width(), window.resolution.physical_height()))
        .unwrap_or((1280, 720));
    let mut scene_image = Image::new_fill(
        scaled_scene_extent(window_width, window_height, render_resolution_scale.0),
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scene_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let scene_image_handle = images.add(scene_image);
    commands.insert_resource(SceneRenderTarget(scene_image_handle.clone()));

    commands.spawn((Camera2d, IsDefaultUiCamera));
    spawn_splash_screen(&mut commands, &asset_server);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        ImageNode::new(scene_image_handle),
        Pickable::IGNORE,
    ));

    commands.insert_resource(HitMarkerAssets {
        mesh: meshes.add(Sphere::new(0.03)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.15, 0.1),
            emissive: LinearRgba::rgb(4.0, 0.3, 0.2),
            unlit: true,
            ..default()
        }),
    });

    commands.insert_resource(GunshotSound(asset_server.load("audio/gunshot.wav")));
    commands.insert_resource(ReloadSound(asset_server.load("audio/reload.wav")));

    commands.insert_resource(MuzzleFxAssets {
        flash_mesh: meshes.add(Rectangle::new(0.18, 0.18)),
        flash_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.5),
            base_color_texture: Some(asset_server.load("textures/muzzle.png")),
            emissive: LinearRgba::rgb(6.0, 3.5, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        smoke_mesh: meshes.add(Rectangle::new(0.25, 0.25)),
        smoke_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.75, 0.75, 0.5),
            base_color_texture: Some(asset_server.load("textures/smoke.png")),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(TracerAssets {
        mesh: meshes.add(Cylinder::new(1.0, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(10.0, 6.0, 1.0),
            unlit: true,
            ..default()
        }),
    });

    commands.insert_resource(BloodAssets {
        particle_mesh: meshes.add(Rectangle::new(BLOOD_PARTICLE_SIZE, BLOOD_PARTICLE_SIZE)),
        particle_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.02, 0.02),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        stain_mesh: meshes.add(Rectangle::new(BLOOD_STAIN_SIZE, BLOOD_STAIN_SIZE)),
        stain_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.01, 0.01, 0.85),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(SparkAssets {
        mesh: meshes.add(Rectangle::new(SPARK_PARTICLE_SIZE, SPARK_PARTICLE_SIZE)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.75, 0.2),
            emissive: LinearRgba::rgb(25.0, 12.0, 1.5),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(ZombieAnim::build(&mut graphs, |kind, index| {
        asset_server.load(GltfAssetLabel::Animation(index).from_asset(zombie_kind_anim_path(kind)))
    }));
    commands.insert_resource(build_zombie_kind_anims(&asset_server, &mut graphs));
    commands.insert_resource(build_player_model_anim(&asset_server, &mut graphs, &player_model_tuning));
    commands.insert_resource(ScopeZoom(SCOPE_DEFAULT_ZOOM));
    commands.insert_resource(GameOver(false));
    if network_setup::map_id_override().is_some() {
        next_state.set(AppState::InGame);
    } else {
        next_state.set(AppState::Menu);
    }
}

fn review_yaw_pitch() -> (f32, f32) {
    (0.0, 0.0)
}

pub(crate) fn setup_gameplay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    touch_enabled: Res<TouchControlsEnabled>,
    player_model_tuning: Res<PlayerModelTuning>,
    scene_target: Option<Res<SceneRenderTarget>>,
    mut pending: ResMut<MapSelectionPending>,
) {
    let Some(scene_target) = scene_target else {
        return;
    };
    pending.0 = false;
    let scenario = current_scenario();
    let player_spawn = scenario.player_spawn();

    commands.spawn((
        Name::new(scenario.title()),
        crate::sim::MapRoot,
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(scenario.require_gltf_scene_path()))),
        Transform::IDENTITY,
        shared::gltf_scene_colliders(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: if dynamic_lights_enabled() { MAIN_SUN_ILLUMINANCE } else { 0.0 },
            shadow_maps_enabled: light_shadows_enabled(),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
        MainSun,
    ));

    commands.spawn((
        Name::new("Player"),
        Transform::from_translation(player_spawn),
        Player {
            vertical_velocity: 0.0,
            grounded: false,
        },
        Health(PLAYER_MAX_HEALTH),
        PlayerStance::default(),
        crate::sim::PlayerThreat::default(),
    ));

    commands.spawn((
        Name::new("PlayerModel (local, debug — KeyT/CameraMode)"),
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(player_model_tuning.mesh.clone()))),
        Transform::from_translation(player_spawn).with_scale(Vec3::splat(player_model_tuning.scale)),
        Visibility::Hidden,
        PlayerModel,
        PlayerModelAnimRoot,
        PlayerModelSkeletonBones::default(),
    ));

    let (review_yaw, review_pitch) = review_yaw_pitch();
    let camera = commands
        .spawn((
            Name::new("FpsCamera (first-person, main render camera)"),
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: BASE_FOV,
                ..default()
            }),
            Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
            FpsCamera { yaw: review_yaw, pitch: review_pitch },
            ViewRecoil::default(),
            Lean::default(),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
                ..default()
            },
            Bloom::default(),
            Msaa::Off,
            RenderLayers::from_layers(&[WORLD_LAYER, VIEWMODEL_LAYER]),
            RenderTarget::from(scene_target.0.clone()),
        ))
        .id();

    let gun_entity = commands
        .spawn((
            Name::new("Gun (first-person viewmodel)"),
            Transform::from_translation(GUN_OFFSET)
                .with_rotation(Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians()))
                .with_scale(Vec3::splat(GUN_SCALE)),
            Visibility::default(),
            Gun {
                base_offset: GUN_OFFSET,
                recoil: 0.0,
                ammo: MAGAZINE_SIZE,
                reload_remaining: 0.0,
                ads_blend: 0.0,
                wall_raise: 0.0,
                shoulder_blend: 0.0,
                recoil_roll: 0.0,
                bob_phase: 0.0,
                bob_blend: 0.0,
                look_sway_roll: 0.0,
            },
            ChildOf(camera),
        ))
        .id();
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity);

    commands.spawn((
        Name::new("ThirdPersonCamera (KeyU, inactive until CameraMode::ThirdPerson)"),
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: BASE_FOV,
            ..default()
        }),
        Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
        ThirdPersonCamera,
        RenderLayers::from_layers(&[WORLD_LAYER]),
        RenderTarget::from(scene_target.0.clone()),
    ));

    let scope_size = Extent3d {
        width: SCOPE_TEXTURE_SIZE,
        height: SCOPE_TEXTURE_SIZE,
        ..default()
    };
    let mut scope_image = Image::new_fill(
        scope_size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scope_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let scope_image_handle = images.add(scope_image);

    let scope_camera = commands
        .spawn((
            Name::new("ScopeCamera (renders the ADS lens texture)"),
            Camera3d::default(),
            Camera {
                order: -1,
                is_active: false,
                ..default()
            },
            RenderTarget::from(scope_image_handle.clone()),
            Projection::Perspective(PerspectiveProjection {
                fov: SCOPE_BASE_FOV / SCOPE_DEFAULT_ZOOM,
                ..default()
            }),
            Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
                ..default()
            },
            Msaa::Off,
            RenderLayers::from_layers(&[WORLD_LAYER, SCOPE_RETICLE_LAYER]),
            Bloom::default(),
            ScopeCamera,
        ))
        .id();

    commands.spawn((
        Mesh3d(meshes.add(
            Triangle2d::new(
                Vec2::new(0.0, 0.0),
                Vec2::new(-SCOPE_RETICLE_WIDTH / 2.0, -SCOPE_RETICLE_HEIGHT / 2.0),
                Vec2::new(SCOPE_RETICLE_WIDTH / 2.0, -SCOPE_RETICLE_HEIGHT / 2.0),
            )
            .mesh(),
        )),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            emissive: LinearRgba::rgb(35.0, 0.0, 0.0),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, -SCOPE_RETICLE_DISTANCE)),
        RenderLayers::layer(SCOPE_RETICLE_LAYER),
        ScopeReticle,
        ChildOf(scope_camera),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Circle::new(SCOPE_LENS_SIZE / 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(scope_image_handle),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(SCOPE_LENS_OFFSET),
        Visibility::Hidden,
        RenderLayers::layer(VIEWMODEL_LAYER),
        ScopeLens,
        ChildOf(camera),
    ));

    spawn_crosshair(&mut commands);
    spawn_health_ui(&mut commands);
    spawn_ammo_ui(&mut commands);
    spawn_compass_ui(&mut commands);
    spawn_radar_ui(&mut commands);
    spawn_inspect_ui(&mut commands);
    spawn_settings_menu(&mut commands);
    spawn_settings_toggle_button(&mut commands);
    spawn_mouse_ui_lock_indicator(&mut commands);
    if touch_enabled.0 {
        spawn_touch_controls(&mut commands);
    }
}

