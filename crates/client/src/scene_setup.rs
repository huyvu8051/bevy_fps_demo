
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
            base_color: Color::linear_rgb(4.0, 0.3, 0.2),
            unlit: true,
            ..default()
        }),
    });

    commands.insert_resource(GunshotSound(asset_server.load("audio/gunshot.wav")));
    commands.insert_resource(ReloadSound(asset_server.load("audio/reload.wav")));
    commands.insert_resource(FootstepSounds {
        player: (0..PLAYER_FOOTSTEP_SAMPLES)
            .map(|i| asset_server.load(format!("audio/footsteps/{PLAYER_FOOTSTEP_SURFACE}/{i}.ogg")))
            .collect(),
        zombie: (0..ZOMBIE_FOOTSTEP_SAMPLES)
            .map(|i| asset_server.load(format!("audio/footsteps/{ZOMBIE_FOOTSTEP_SURFACE}/{i}.ogg")))
            .collect(),
    });

    commands.insert_resource(MuzzleFxAssets {
        flash_mesh: meshes.add(Rectangle::new(FLASH_QUAD_SIZE, FLASH_QUAD_SIZE)),
        ember_mesh: meshes.add(Rectangle::new(MUZZLE_EMBER_SIZE, MUZZLE_EMBER_SIZE)),
        smoke_mesh: meshes.add(Rectangle::new(0.25, 0.25)),
    });

    commands.insert_resource(TracerAssets {
        mesh: meshes.add(spindle_mesh(TRACER_SEGMENTS)),
        fade_steps: (0..TRACER_FADE_STEPS)
            .map(|step| {
                let fade = 1.0 - step as f32 / TRACER_FADE_STEPS as f32;
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba::new(10.0, 6.0, 1.0, fade)),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });

    let barrel_ribbon = meshes.add(empty_ribbon_mesh());
    commands.insert_resource(BarrelSmokeRibbon { mesh: barrel_ribbon.clone() });
    commands.spawn((
        Name::new("BarrelSmokeRibbon"),
        Mesh3d(barrel_ribbon),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.70, 0.69, 0.67),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
    ));

    commands.insert_resource(build_bullet_hole_assets(&asset_server, &mut meshes, &mut materials));
    commands.insert_resource(build_casing_assets(&mut meshes, &mut materials));
    commands.insert_resource(build_gore_assets(&mut meshes, &mut materials));

    commands.insert_resource(BloodAssets {
        particle_mesh: meshes.add(Rectangle::new(BLOOD_PARTICLE_SIZE, BLOOD_PARTICLE_SIZE)),
        particle_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.05, 0.04),
            base_color_texture: Some(images.add(droplet_image(BLOOD_DROPLET_TEXTURE_SIZE))),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        stain_mesh: meshes.add(Rectangle::new(BLOOD_STAIN_SIZE, BLOOD_STAIN_SIZE)),
        floor_stain_mesh: meshes.add(Rectangle::new(BLOOD_FLOOR_STAIN_SIZE, BLOOD_FLOOR_STAIN_SIZE)),
        stain_materials: (0..BLOOD_STAIN_VARIANTS)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(1.0, 0.42, 0.38, 0.92),
                    base_color_texture: Some(asset_server.load(format!("textures/blood/splatter_{i}.png"))),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.12,
                    metallic: 0.0,
                    reflectance: 0.75,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
        wall_stain_mesh: meshes.add(Rectangle::new(BLOOD_WALL_STAIN_SIZE, BLOOD_WALL_STAIN_SIZE)),
        burst_mesh: meshes.add(Rectangle::new(BLOOD_BURST_SIZE, BLOOD_BURST_SIZE)),
        burst_materials: (0..BLOOD_STAIN_VARIANTS)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(1.0, 0.16, 0.12, 1.0),
                    base_color_texture: Some(asset_server.load(format!("textures/blood/splatter_{i}.png"))),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });

    commands.insert_resource(SparkAssets {
        mesh: meshes.add(Rectangle::new(SPARK_PARTICLE_SIZE, SPARK_PARTICLE_SIZE)),
        material: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(25.0, 12.0, 1.5),
            base_color_texture: Some(images.add(droplet_image(BLOOD_DROPLET_TEXTURE_SIZE))),
            alpha_mode: AlphaMode::Blend,
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
    if game_flow::map_id_override().is_some() {
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
    mut overlay_materials: ResMut<Assets<DamageOverlayMaterial>>,
    mut scope_lens_materials: ResMut<Assets<ScopeLensMaterial>>,
    touch_enabled: Res<TouchControlsEnabled>,
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
        crate::map::MapRoot,
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
            SpatialListener::default(),
            ScreenEffects::default(),
            FpsCamera { yaw: review_yaw, pitch: review_pitch },
            ViewRecoil::default(),
            Lean::default(),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
                ..default()
            },
            (
                Bloom::default(),
                Msaa::Off,
                bevy::core_pipeline::prepass::DepthPrepass,
                distance_fog_for(scenario),
            ),
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
                settle_jitter: Vec3::ZERO,
                settle_current: Vec3::ZERO,
                settle_rolled: false,
                recoil: 0.0,
                ammo: MAGAZINE_SIZE,
                reload_remaining: 0.0,
                ads_blend: 0.0,
                wall_raise: 0.0,
                shoulder_blend: 0.0,
                ready_blend: 0.0,
                ready_armed: false,
                recoil_roll: 0.0,
                bob_phase: 0.0,
                bob_blend: 0.0,
                look_sway_roll: 0.0,
            },
            ChildOf(camera),
        ))
        .id();
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity);

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
            // Soft particles sample the depth prepass. Without it the scope
            // view specialises muzzle_smoke.wgsl with no prepass_depth in
            // scope and the shader fails to build for this camera.
            bevy::core_pipeline::prepass::DepthPrepass,
            RenderLayers::from_layers(&[WORLD_LAYER, SCOPE_RETICLE_LAYER]),
            distance_fog_for(scenario),
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
            base_color: Color::linear_rgb(35.0, 0.0, 0.0),
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
        MeshMaterial3d(scope_lens_materials.add(scope_lens_material(scope_image_handle))),
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
    spawn_damage_overlay(&mut commands, &mut overlay_materials);
    spawn_settings_menu(&mut commands);
    spawn_settings_toggle_button(&mut commands);
    spawn_mouse_ui_lock_indicator(&mut commands);
    if touch_enabled.0 {
        spawn_touch_controls(&mut commands);
    }
}

