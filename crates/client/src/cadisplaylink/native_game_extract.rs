//! Phase 6 Step 2 of the wgpu-replacement plan: per-frame extraction from
//! the headless real-game `App` (see `native_game_app.rs`) into a plain,
//! owned snapshot our Metal renderer can read — the seam between Bevy ECS
//! and the custom renderer, mirroring `native_metal_probe.rs`'s existing
//! `patrol_system` extract pattern (tick the World, run an ad-hoc
//! `SystemState`/`QueryState`, collect into an owned `Vec` rather than
//! keeping a live borrow).
//!
//! Deliberately data-only: no Metal/rendering types appear here at all, so
//! this module compiles and is testable independent of Step 4's actual
//! draw-call wiring.

use bevy::asset::Assets;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res, SystemState};
use bevy::image::Image;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::light::{DirectionalLight, PointLight};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::render::render_resource::TextureFormat;
use bevy::transform::components::{GlobalTransform, Transform};

use shared::{NetPlayer, Zombie};

/// Raw RGBA8 pixel data for one mesh's base-color texture, extracted from
/// Bevy's `Image` asset — `None` on the `ExtractedMesh` when the material
/// has no texture (flat `base_color` factor only) or when its CPU-side
/// pixel format isn't one of the common RGBA8 variants glTF base-color
/// textures normally load as (extraction just skips it in that case,
/// falling back to a flat-tinted draw rather than failing).
pub struct ExtractedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One drawable static mesh — arena walls/floor (`spawn_test_arena`,
/// `crates/client/src/lib.rs:3915`) and the gun's glTF-scene-instantiated
/// child meshes (`GUN_PATH = "models/ak47_scope.glb"`) both show up here.
/// Both carry plain `Mesh3d`/`MeshMaterial3d<StandardMaterial>` once
/// Bevy's scene spawner has run; querying `GlobalTransform` (not local
/// `Transform`) is what makes the gun's meshes — parented to the gun
/// root, itself `ChildOf(camera)` — come out in correct world space
/// without any manual parent-chain math here. Requires
/// `bevy::transform::TransformPlugin` in the headless App (see
/// native_game_app.rs) to actually propagate `Transform` -> `GlobalTransform`
/// through the hierarchy each tick; first attempt at this used local
/// `Transform` directly and rendered the gun as a garbled giant mesh,
/// confirming the propagation step really is necessary, not optional.
pub struct ExtractedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty if the source mesh has no UV attribute (e.g. the arena's
    /// `Cuboid` primitives, which do have one — but guard rather than
    /// assume) — the renderer falls back to `[0.0, 0.0]` per vertex, which
    /// combined with `texture: None` just shows the flat `base_color` tint.
    pub uvs: Vec<[f32; 2]>,
    /// Empty if the source `Mesh` isn't indexed (shouldn't happen for
    /// anything built via `meshes.add(Cuboid::new(..))` or loaded glTF, both
    /// of which are always indexed, but guard rather than assume).
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    pub texture: Option<ExtractedTexture>,
    /// World-space transform at extraction time (from `GlobalTransform`,
    /// converted to a plain `Transform` for the renderer's convenience —
    /// see `bake_static_meshes` in native_metal_probe.rs).
    pub transform: Transform,
}

/// Camera pose. `eye` is the `FpsCamera` entity's own world-space
/// translation. `rotation` is that entity's actual world-space rotation —
/// deliberately read straight off `GlobalTransform` rather than
/// reconstructed from `FpsCamera.yaw/.pitch`, because the *final* rotation
/// each tick is `apply_view_recoil`'s composition of yaw + pitch +
/// `ViewRecoil`'s kick + `Lean`'s roll (crates/client/src/lib.rs:5144) —
/// reconstructing from the raw component fields silently drops recoil kick
/// and lean roll, which is exactly why gun recoil was visible (baked into
/// the gun mesh's own transform) but the camera/crosshair never shook to
/// match. `yaw`/`pitch` are kept too (pre-recoil, straight off the
/// component) since they're still useful for e.g. UI that cares about the
/// player's intended look direction rather than its recoil-jittered one.
pub struct CameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],
    pub yaw: f32,
    pub pitch: f32,
    /// The real `Gun.ads_blend` (0 = hip fire, 1 = fully aimed) — drives
    /// whether/how strongly a renderer should show a scope-zoom overlay,
    /// instead of a probe-only guess at whether the player is aiming.
    pub ads_blend: f32,
    /// The real `ScopeZoom` resource value (magnification factor, e.g.
    /// 6..25 — see `SCOPE_MIN_ZOOM`/`SCOPE_MAX_ZOOM` in
    /// crates/client/src/lib.rs), so a scope overlay's FOV can be computed
    /// the same way the real game does (`SCOPE_BASE_FOV / zoom`) instead
    /// of a hardcoded guess.
    pub scope_zoom: f32,
    /// The real `Gun.wall_raise` (0 = normal, 1 = fully raised to clear a
    /// wall) — `update_scope_lens` (crates/client/src/lib.rs:6050) hides
    /// the scope even mid-ADS once this crosses
    /// `SCOPE_HIDDEN_WALL_RAISE_THRESHOLD`, so a renderer needs it too to
    /// match the real on/off condition exactly (`ads_blend` alone isn't
    /// enough).
    pub wall_raise: f32,
}

/// The real `ScopeCamera` entity's own state (crates/client/src/lib.rs,
/// `struct ScopeCamera` at line 1065) — reading this directly, instead of
/// reimplementing `sync_scope_camera`/`update_scope_lens`'s math
/// ourselves, is what makes the scope view's camera-shoulder-swap sweep,
/// recoil kick, reload dip/tilt, and walk-bob all "just work" without a
/// single line of that logic duplicated here: those two real systems
/// already compute the exact final `Transform`/`Camera.is_active`/
/// `Projection.fov` every tick inside this same headless App, we only
/// need to read the result. `desktop` (the real winit/wgpu build) shows
/// this exact system working correctly already, since it's 100% the same
/// `build_game_common` code — nothing about this needed reimplementing,
/// only extracting.
pub struct ScopeCameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],
    /// Real `Camera.is_active`, already gated on the exact
    /// `ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD && wall_raise <
    /// SCOPE_HIDDEN_WALL_RAISE_THRESHOLD` condition by `update_scope_lens`
    /// — supersedes hand-reconstructing that condition from `ads_blend`/
    /// `wall_raise` alone.
    pub active: bool,
    /// Real `Projection::Perspective.fov` (radians), already
    /// `SCOPE_BASE_FOV / ScopeZoom` per `update_scope_lens`.
    pub fov: f32,
}

#[derive(Default)]
pub struct RenderSnapshot {
    pub camera: Option<CameraSnapshot>,
    pub scope_camera: Option<ScopeCameraSnapshot>,
    pub zombies: Vec<ZombieRenderState>,
    pub remote_players: Vec<Transform>,
    pub static_meshes: Vec<ExtractedMesh>,
    pub light: Option<LightSnapshot>,
    pub sun: Option<SunSnapshot>,
}

/// The scene's real `DirectionalLight` ("MainSun", crate::lib — spawned
/// unconditionally for both USE_TEST_ARENA and the Sponza scene, and per
/// its own doc comment there ("the arena instead just gets a brighter
/// ambient/directional light") is actually the *primary* light for the
/// test arena, with the single `PointLight` LightSnapshot already extracts
/// more of a secondary/fill light — missed on the first lighting-port pass,
/// which only extracted the point light.
pub struct SunSnapshot {
    /// The direction the light travels *toward* (the entity's own forward
    /// vector) — same convention native_metal_probe.rs's old hardcoded
    /// `light_dir` constant used, so `max(dot(normal, -sunDir), 0)` matches.
    pub direction: [f32; 3],
    pub color: [f32; 3],
}

/// The scene's real `PointLight`, extracted so the Metal renderer can
/// actually shade from where the light is instead of a hand-picked fixed
/// direction — lighting was never ported when the rest of the real game
/// moved onto this renderer (see native_metal_probe.rs's own fragment
/// shaders, which used a constant `light_dir` unrelated to any real light
/// in the scene). USE_TEST_ARENA (crate::lib) spawns exactly one overhead
/// `PointLight`, so `first()` is enough for now — if a scene with more
/// than one light ever needs supporting, this is the one place to extend
/// into a small array instead of a single `Option`.
pub struct LightSnapshot {
    pub position: [f32; 3],
    /// sRGB, 0..1 per channel — already in the same range the renderer's
    /// existing vertex/base-color colors use, unlike `PointLight::intensity`
    /// (lumens, can be in the millions) which this deliberately doesn't
    /// extract: the shader uses distance/range attenuation for falloff and
    /// this color purely as a tint, not a physically-based exposure value.
    pub color: [f32; 3],
    pub range: f32,
}

/// A zombie's world transform plus which of zombie.glb's 5 embedded poses
/// (Idle/Walk/Run/Crawl/Bite — see `crate::ZombiePose`/
/// `update_zombie_animation`) its real `AnimationPlayer` is actually
/// playing right now, and how far into that clip. Extracting this instead
/// of hardcoding a clip index is the same "extract the real thing, don't
/// reimplement it" principle this module already applies to the camera/
/// scope camera — the pose decision (distance/Health thresholds) already
/// lives in `update_zombie_animation`; duplicating that logic here would
/// just be a second place for the two to drift apart.
pub struct ZombieRenderState {
    pub transform: Transform,
    /// Matches zombie.glb's own animation list order (0=Bite, 1=Crawl,
    /// 2=Idle, 3=Run, 4=Walk) — what `skinned_gltf::SkinnedModel::
    /// compute_joint_matrices`'s `clip_index` parameter expects. Falls
    /// back to 2 (Idle) if no `AnimationPlayer` is found/playing yet (e.g.
    /// the zombie's glTF scene is still loading asynchronously).
    pub anim_clip_index: usize,
    pub anim_time: f32,
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            eye: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            yaw: 0.0,
            pitch: 0.0,
            ads_blend: 0.0,
            scope_zoom: 8.0,
            wall_raise: 0.0,
        }
    }
}

/// Runs once per tick against the headless `App`'s `World` — cheap enough
/// (a handful of small queries plus re-reading two `Assets<T>` collections)
/// to call every frame rather than caching, matching
/// `native_metal_probe.rs`'s own per-frame extract call.
pub fn extract_render_snapshot(world: &mut bevy::ecs::world::World) -> RenderSnapshot {
    let mut snapshot = RenderSnapshot::default();

    // Camera: the FpsCamera entity's own world-space translation +
    // rotation (see CameraSnapshot's own doc comment for why the real
    // rotation, not a yaw/pitch reconstruction, is what carries
    // recoil/lean). `Gun.ads_blend`/`ScopeZoom` ride along here too since
    // a renderer needs them in lockstep with the camera pose to decide
    // whether/how to show a scope overlay.
    let mut camera_state: SystemState<(
        Query<(&GlobalTransform, &crate::FpsCamera)>,
        Query<&crate::Gun>,
        bevy::ecs::system::Res<crate::ScopeZoom>,
    )> = SystemState::new(world);
    let (camera_query, gun_query, scope_zoom) = camera_state.get(world);
    if let Some((transform, cam)) = camera_query.iter().next() {
        snapshot.camera = Some(CameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
            ads_blend: gun_query.iter().next().map(|g| g.ads_blend).unwrap_or(0.0),
            scope_zoom: scope_zoom.0,
            wall_raise: gun_query.iter().next().map(|g| g.wall_raise).unwrap_or(0.0),
        });
    }

    // Scope camera: read the real ScopeCamera entity's own final state
    // directly — see ScopeCameraSnapshot's own doc comment for why this
    // supersedes reimplementing sync_scope_camera/update_scope_lens.
    let mut scope_state: SystemState<
        Query<(&GlobalTransform, &bevy::camera::Camera, &bevy::camera::Projection), With<crate::ScopeCamera>>,
    > = SystemState::new(world);
    if let Some((transform, camera, projection)) = scope_state.get(world).iter().next() {
        let fov = match projection {
            bevy::camera::Projection::Perspective(p) => p.fov,
            _ => 0.0,
        };
        snapshot.scope_camera = Some(ScopeCameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            active: camera.is_active,
            fov,
        });
    }

    // Zombies: world-space transform plus real animation-pose state — see
    // ZombieRenderState's own doc comment. `ZombieAnimationPlayer(Entity)`
    // (crate::lib) links each zombie root to the descendant entity that
    // actually carries the `AnimationPlayer` (the glTF scene loads
    // asynchronously, so it isn't the same entity and may not exist yet).
    let mut zombie_state: SystemState<(
        Query<(&GlobalTransform, Option<&crate::ZombieAnimationPlayer>), With<Zombie>>,
        Query<&bevy::animation::AnimationPlayer>,
        bevy::ecs::system::Res<crate::ZombieAnim>,
    )> = SystemState::new(world);
    let (zombie_query, anim_players, zombie_anim) = zombie_state.get(world);
    snapshot.zombies = zombie_query
        .iter()
        .map(|(transform, link)| {
            // Look up which of the 5 AnimationNodeIndex handles ZombieAnim
            // hands out is actually playing, then map it back to zombie.glb's
            // own raw clip index (skinned_gltf's compute_joint_matrices takes
            // a clip index, not a graph node) — see ZombiePose's priority
            // order in update_zombie_animation for how that pose was picked.
            let (anim_clip_index, anim_time) = link
                .and_then(|link| anim_players.get(link.0).ok())
                .and_then(|player| player.playing_animations().next())
                .map(|(node, active)| {
                    let clip_index = if *node == zombie_anim.bite {
                        0
                    } else if *node == zombie_anim.crawl {
                        1
                    } else if *node == zombie_anim.run {
                        3
                    } else if *node == zombie_anim.walk {
                        4
                    } else {
                        2 // idle, and the fallback for any unrecognized node
                    };
                    (clip_index, active.seek_time())
                })
                .unwrap_or((2, 0.0));
            ZombieRenderState { transform: transform.compute_transform(), anim_clip_index, anim_time }
        })
        .collect();

    // Remote players: NetPlayer entities that aren't this client's own.
    // `Without<LocalPlayer>` alone isn't reliable here — observed empirically
    // (debug log: camera at [0, 1.46, 14], the one "remote" player at
    // [0, 0.81, 14] — same X/Z as the camera, only offset in Y by the
    // ordinary eye-height-above-hip-pivot amount) meaning `mark_local_player`
    // (an observer on `Add<NetworkId>`, see crates/client/src/lib.rs:1299)
    // isn't tagging the local client's own entity in this headless setup,
    // root cause not yet found. Belt-and-suspenders fix: also drop any
    // NetPlayer whose horizontal (X/Z) position coincides with the camera's
    // — deliberately horizontal-only, since the camera sits above the
    // player's own pivot by a fixed eye-height offset even for the correct
    // entity, so a 3D distance check (tried first) missed this case. A
    // genuine remote player is never this close horizontally.
    const LOCAL_PLAYER_EXCLUSION_RADIUS_XZ: f32 = 1.0;
    let camera_eye_xz = snapshot.camera.as_ref().map(|c| bevy::math::Vec2::new(c.eye[0], c.eye[2]));
    let mut remote_state: SystemState<
        Query<&GlobalTransform, (With<NetPlayer>, bevy::ecs::query::Without<crate::LocalPlayer>)>,
    > = SystemState::new(world);
    snapshot.remote_players = remote_state
        .get(world)
        .iter()
        .map(|t| t.compute_transform())
        .filter(|t| match camera_eye_xz {
            Some(eye_xz) => {
                let pos_xz = bevy::math::Vec2::new(t.translation.x, t.translation.z);
                pos_xz.distance(eye_xz) > LOCAL_PLAYER_EXCLUSION_RADIUS_XZ
            }
            None => true,
        })
        .collect();

    // Light: the scene's real PointLight — see LightSnapshot's own doc
    // comment for why only position/color/range (not raw lumen intensity)
    // are extracted.
    let mut light_state: SystemState<Query<(&PointLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.light = light_state.get(world).iter().next().map(|(light, transform)| {
        let c = light.color.to_srgba();
        LightSnapshot {
            position: transform.translation().to_array(),
            color: [c.red, c.green, c.blue],
            range: light.range,
        }
    });

    // Sun: the always-present DirectionalLight("MainSun") — see
    // SunSnapshot's own doc comment for why this is a separate extraction
    // from the point light above.
    let mut sun_state: SystemState<Query<(&DirectionalLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.sun = sun_state.get(world).iter().next().map(|(sun, transform)| {
        let c = sun.color.to_srgba();
        SunSnapshot { direction: transform.forward().to_array(), color: [c.red, c.green, c.blue] }
    });

    // Static meshes: arena walls/floor + gun scene meshes, both plain
    // Mesh3d/MeshMaterial3d<StandardMaterial> entities — see this module's
    // doc comment on ExtractedMesh for why GlobalTransform (not local
    // Transform) is what makes the parented gun meshes come out correct.
    //
    // `Without<SkinnedMesh>` excludes zombie.glb/soldier.glb's own
    // Mesh3d/MeshMaterial3d child entities (Bevy's scene spawner
    // instantiates those from each Zombie/NetPlayer's SceneRoot, same as
    // it does for the gun's meshes here) — those already get drawn once,
    // correctly GPU-skinned, via `real_game_extracted`'s own
    // EntityKind::Zombie/Soldier skinned-model path (native_metal_probe.rs)
    // using the probe's separately-loaded zombie_model/soldier_model.
    // Without this filter, this query ALSO baked their raw bind-pose
    // vertex positions (no skinning applied — this query has no way to
    // apply joint transforms) as a second, oversized/unanimated "static"
    // mesh sitting on top of the correctly animated one — the exact same
    // "garbled giant mesh" failure mode as the gun's own GlobalTransform-
    // propagation bug noted above, just from skipping skinning instead of
    // skipping GlobalTransform.
    let mut mesh_state: SystemState<(
        Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform), Without<SkinnedMesh>>,
        Res<Assets<Mesh>>,
        Res<Assets<StandardMaterial>>,
        Res<Assets<Image>>,
    )> = SystemState::new(world);
    let (mesh_query, meshes, materials, images) = mesh_state.get(world);
    for (mesh3d, material3d, transform) in mesh_query.iter() {
        let Some(mesh) = meshes.get(&mesh3d.0) else { continue };
        let Some(material) = materials.get(&material3d.0) else { continue };

        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => continue,
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) => n.clone(),
            _ => Vec::new(),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) => uv.clone(),
            _ => Vec::new(),
        };
        let indices = mesh
            .indices()
            .map(|i| i.iter().map(|v| v as u32).collect())
            .unwrap_or_default();

        // Only the common RGBA8 variants glTF base-color textures normally
        // decode to — anything else (compressed formats like KTX2/Basis,
        // etc.) this module has no reason to own a conversion for.
        let has_texture_handle = material.base_color_texture.is_some();
        let texture = material.base_color_texture.as_ref().and_then(|handle| {
            let image = images.get(handle)?;
            let data = image.data.as_ref()?;
            match image.texture_descriptor.format {
                TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => Some(ExtractedTexture {
                    width: image.texture_descriptor.size.width,
                    height: image.texture_descriptor.size.height,
                    rgba: data.clone(),
                }),
                _ => None,
            }
        });
        // A material WITH a texture assigned that we couldn't decode (as
        // opposed to a material with no texture at all, which legitimately
        // renders via its flat base_color tint) previously fell back to an
        // opaque white placeholder — visible on the gun's scope lens as a
        // solid white/gray disk sitting inside the optic, since that
        // element's actual texture uses an unsupported format. Hiding the
        // mesh entirely is a deliberately temporary stand-in for real
        // format support until decoding whatever format that texture
        // actually is becomes worth doing.
        if has_texture_handle && texture.is_none() {
            continue;
        }

        snapshot.static_meshes.push(ExtractedMesh {
            positions,
            normals,
            uvs,
            indices,
            base_color: {
                let s = material.base_color.to_srgba();
                [s.red, s.green, s.blue, s.alpha]
            },
            texture,
            transform: transform.compute_transform(),
        });
    }

    snapshot
}
