
use bevy::animation::graph::AnimationGraph;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::mesh::{Indices, PrimitiveTopology};
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
    // MAP_ID means a measurement or a debug run, which goes straight into the
    // map. Two seconds of branding in front of that is two seconds of waiting
    // on every single run.
    if game_flow::map_id_override().is_none() {
        spawn_splash_screen(&mut commands, &asset_server);
    }

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

    // One handle per weapon, in WeaponId order, so a weapon that gets its own
    // recording only has to say so in the table.
    commands.insert_resource(GunshotSound(
        WeaponId::ALL.iter().map(|w| asset_server.load(w.def().sound.fire)).collect(),
    ));
    commands.insert_resource(ReloadSound(
        WeaponId::ALL.iter().map(|w| asset_server.load(w.def().sound.reload)).collect(),
    ));
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
        smoke_atlas: asset_server.load(SMOKE_ATLAS_PATH),
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
    commands.insert_resource(build_casing_assets(&asset_server, &mut meshes, &mut materials));
    commands.insert_resource(build_gore_assets(&asset_server, &mut meshes, &mut materials));

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
    commands.insert_resource(ScopeZoom(WeaponId::default().def().scope.default_zoom));
    commands.insert_resource(GameOver(false));
    // Everything now goes through the loading gate; it remembers where it was
    // headed.
    commands.insert_resource(AfterLoading(if game_flow::map_id_override().is_some() {
        AppState::InGame
    } else {
        AppState::Menu
    }));
    next_state.set(AppState::Loading);
}

fn review_yaw_pitch() -> (f32, f32) {
    (0.0, 0.0)
}

fn push_bar(positions: &mut Vec<[f32; 3]>, indices: &mut Vec<u32>, cx: f32, cy: f32, hw: f32, hh: f32) {
    let base = positions.len() as u32;
    positions.extend_from_slice(&[
        [cx - hw, cy - hh, 0.0],
        [cx + hw, cy - hh, 0.0],
        [cx + hw, cy + hh, 0.0],
        [cx - hw, cy + hh, 0.0],
    ]);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A duplex crosshair: thick posts from the edge that taper to fine lines,
/// mil-dots along the fine section, and a gap at dead centre.
///
/// Built in radians at unit distance, like everything else the scope draws, so
/// the constants read as what the player sees through the glass. Flat quads
/// rather than lines: line primitives are one pixel wide whatever the
/// magnification, which makes the posts vanish as the scope zooms in.
fn duplex_reticle_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let reach = SCOPE_RETICLE_REACH;
    let taper = reach * SCOPE_RETICLE_TAPER;
    let gap = reach * SCOPE_RETICLE_CENTRE_GAP;
    let post = SCOPE_RETICLE_POST_WIDTH / 2.0;
    let line = SCOPE_RETICLE_LINE_WIDTH / 2.0;
    let dot = SCOPE_RETICLE_DOT_SIZE / 2.0;

    for side in [-1.0f32, 1.0] {
        let post_mid = side * (taper + reach) / 2.0;
        let post_half = (reach - taper) / 2.0;
        let line_mid = side * (gap + taper) / 2.0;
        let line_half = (taper - gap) / 2.0;

        push_bar(&mut positions, &mut indices, post_mid, 0.0, post_half, post);
        push_bar(&mut positions, &mut indices, 0.0, post_mid, post, post_half);
        push_bar(&mut positions, &mut indices, line_mid, 0.0, line_half, line);
        push_bar(&mut positions, &mut indices, 0.0, line_mid, line, line_half);

        // Spaced along the fine section only, as on the real glass: dots out
        // on the thick posts would be swallowed by them.
        let step = (taper - gap) / (SCOPE_RETICLE_DOTS_PER_ARM + 1) as f32;
        for i in 1..=SCOPE_RETICLE_DOTS_PER_ARM {
            let at = side * (gap + step * i as f32);
            push_bar(&mut positions, &mut indices, at, 0.0, dot, dot);
            push_bar(&mut positions, &mut indices, 0.0, at, dot, dot);
        }
    }

    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
        .with_inserted_indices(Indices::U32(indices))
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
    active: Res<ActiveWeapon>,
    cascades: Res<ShadowCascades>,
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
        cascade_config(&cascades),
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
            RenderLayers::from_layers(&[WORLD_LAYER, VIEWMODEL_LAYER, SCOPE_MASK_LAYER]),
            RenderTarget::from(scene_target.0.clone()),
        ))
        .id();

    let gun_entity = commands
        .spawn((
            Name::new("Gun (first-person viewmodel)"),
            Transform::from_translation(active.def().rest_offset),
            Visibility::default(),
            Gun {
                optical_fix: Quat::IDENTITY,
                ads_from_ready: 0.0,
                ads_wanted: false,
                base_offset: active.def().rest_offset,
                muzzle_local: Transform::IDENTITY,
                eject_local: Transform::IDENTITY,
                settle_jitter: Vec3::ZERO,
                settle_current: Vec3::ZERO,
                settle_twist_jitter: 0.0,
                settle_twist_current: 0.0,
                settle_rolled: false,
                recoil: 0.0,
                recoil_target: 0.0,
                ammo: active.def().magazine,
                reload_remaining: 0.0,
                ads_blend: 0.0,
                wall_raise: 0.0,
                shoulder_blend: 0.0,
                ready_blend: 0.0,
                ready_armed: false,
                recoil_roll: 0.0,
                bob_phase: 0.0,
                bob_blend: 0.0,
                look_sway: Vec2::ZERO,
            },
            ChildOf(camera),
        ))
        .id();
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity, active.def().mesh_path);

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
            Name::new("ScopeCamera (renders the picture the eyepiece shows)"),
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
            NoShadowPass,
            distance_fog_for(scenario),
            // No Bloom here, unlike the main camera. Measured at 0.66ms of the
            // 3.9ms a sighted frame costs -- a sixth of it -- for a glow drawn
            // into a 512px disc at twelve times magnification, where there is
            // almost nothing bright enough left in frame to bloom.
            ScopeCamera,
        ))
        .id();

    // A sight's mark is a virtual image laid over the view, not an object out
    // in the field, so nothing in front of it hides it. cull_mode is off
    // because the mark is turned to face down the barrel, and which way that
    // leaves it facing depends on how the socket was authored in Blender.
    let mark_material = |colour: Color| StandardMaterial {
        base_color: colour,
        unlit: true,
        depth_bias: 1000.0,
        cull_mode: None,
        ..default()
    };
    let reticles = ReticleAssets {
        chevron: (
            meshes.add(
                Triangle2d::new(
                    Vec2::ZERO,
                    Vec2::new(-SCOPE_CHEVRON_WIDTH / 2.0, -SCOPE_CHEVRON_HEIGHT / 2.0),
                    Vec2::new(SCOPE_CHEVRON_WIDTH / 2.0, -SCOPE_CHEVRON_HEIGHT / 2.0),
                )
                .mesh(),
            ),
            materials.add(mark_material(Color::linear_rgb(35.0, 0.0, 0.0))),
        ),
        duplex: (
            meshes.add(duplex_reticle_mesh()),
            materials.add(mark_material(Color::BLACK)),
        ),
    };

    commands.spawn((
        Name::new("ReloadHandTarget"),
        Transform::default(),
        ReloadHandTarget,
        ChildOf(camera),
    ));

    // The sight picture in fov-zoom mode is a hole in this, so the inner
    // radius is resized every frame against whatever field the camera is on.
    // The outer has to reach the corners, which on 16:9 sit 2.04 times the
    // half height out -- and the whole ring is scaled by the hole, so the
    // ratio has to carry that 2.04 and the 0.42 the hole is: 12 leaves room
    // for a wider window than that.
    commands.spawn((
        Name::new("ScopeFovMask"),
        Mesh3d(meshes.add(Annulus::new(1.0, 12.0).mesh().resolution(64).build())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, -1.0),
        Visibility::Hidden,
        RenderLayers::layer(SCOPE_MASK_LAYER),
        ScopeFovMask,
        ChildOf(camera),
    ));

    let reticle = commands
        .spawn((
            Mesh3d(reticles.chevron.0.clone()),
            MeshMaterial3d(reticles.chevron.1.clone()),
            // Hung off the camera, but aimed down the barrel each frame. Parenting
            // here guarantees it is in front of the lens and facing it; the
            // direction is what carries the weapon.
            Transform::default(),
            RenderLayers::layer(SCOPE_RETICLE_LAYER),
            ScopeReticle,
            ChildOf(scope_camera),
        ))
        .id();
    commands.insert_resource(reticles);

    // A child of the mark, so it is carried down the barrel and scaled with it
    // and only its own colour has to be said here. Render layers do not
    // propagate, so it names its own. Hidden until a weapon that wants it.
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(SCOPE_RETICLE_CENTRE_DOT_SIZE / 2.0).mesh())),
        MeshMaterial3d(materials.add(mark_material(Color::linear_rgb(35.0, 0.0, 0.0)))),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(SCOPE_RETICLE_LAYER),
        ScopeReticleCentreDot,
        ChildOf(reticle),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Circle::new(1.0))),
        MeshMaterial3d(scope_lens_materials.add(scope_lens_material(scope_image_handle))),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(VIEWMODEL_LAYER),
        ScopeScreen,
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

