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
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, SystemState};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::transform::components::{GlobalTransform, Transform};

use shared::{NetPlayer, Zombie};

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
    /// Empty if the source `Mesh` isn't indexed (shouldn't happen for
    /// anything built via `meshes.add(Cuboid::new(..))` or loaded glTF, both
    /// of which are always indexed, but guard rather than assume).
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    /// World-space transform at extraction time (from `GlobalTransform`,
    /// converted to a plain `Transform` for the renderer's convenience —
    /// see `bake_static_meshes` in native_metal_probe.rs).
    pub transform: Transform,
}

/// Camera pose, replicating `apply_view_recoil`'s rotation composition
/// (`crates/client/src/lib.rs:5144`) minus the recoil/lean terms (headless
/// has no input driving those yet — Step 3): `eye` is the `FpsCamera`
/// entity's own world-space translation (already kept in sync with the
/// player entity by the game's own systems), `yaw`/`pitch` straight from
/// `FpsCamera` so a renderer can build `Quat::from_euler(YXZ, yaw, pitch, 0)`
/// itself without this module depending on any Metal/view-matrix code.
pub struct CameraSnapshot {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Default)]
pub struct RenderSnapshot {
    pub camera: Option<CameraSnapshot>,
    pub zombies: Vec<Transform>,
    pub remote_players: Vec<Transform>,
    pub static_meshes: Vec<ExtractedMesh>,
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self { eye: [0.0, 0.0, 0.0], yaw: 0.0, pitch: 0.0 }
    }
}

/// Runs once per tick against the headless `App`'s `World` — cheap enough
/// (a handful of small queries plus re-reading two `Assets<T>` collections)
/// to call every frame rather than caching, matching
/// `native_metal_probe.rs`'s own per-frame extract call.
pub fn extract_render_snapshot(world: &mut bevy::ecs::world::World) -> RenderSnapshot {
    let mut snapshot = RenderSnapshot::default();

    // Camera: the FpsCamera entity's own world-space translation +
    // yaw/pitch. See this module's doc comment on CameraSnapshot for why
    // recoil/lean are omitted here (Step 3 concern, not extraction).
    let mut camera_state: SystemState<Query<(&GlobalTransform, &crate::FpsCamera)>> =
        SystemState::new(world);
    if let Some((transform, cam)) = camera_state.get(world).iter().next() {
        snapshot.camera = Some(CameraSnapshot {
            eye: transform.translation().to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
        });
    }

    // Zombies: plain marker + world-space transform, per the plan's own
    // extraction spec.
    let mut zombie_state: SystemState<Query<&GlobalTransform, With<Zombie>>> =
        SystemState::new(world);
    snapshot.zombies =
        zombie_state.get(world).iter().map(|t| t.compute_transform()).collect();

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

    // Static meshes: arena walls/floor + gun scene meshes, both plain
    // Mesh3d/MeshMaterial3d<StandardMaterial> entities — see this module's
    // doc comment on ExtractedMesh for why GlobalTransform (not local
    // Transform) is what makes the parented gun meshes come out correct.
    let mut mesh_state: SystemState<(
        Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform)>,
        Res<Assets<Mesh>>,
        Res<Assets<StandardMaterial>>,
    )> = SystemState::new(world);
    let (mesh_query, meshes, materials) = mesh_state.get(world);
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
        let indices = mesh
            .indices()
            .map(|i| i.iter().map(|v| v as u32).collect())
            .unwrap_or_default();

        snapshot.static_meshes.push(ExtractedMesh {
            positions,
            normals,
            indices,
            base_color: {
                let s = material.base_color.to_srgba();
                [s.red, s.green, s.blue, s.alpha]
            },
            transform: transform.compute_transform(),
        });
    }

    snapshot
}
