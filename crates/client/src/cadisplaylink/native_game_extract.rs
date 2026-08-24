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
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::transform::components::Transform;

use shared::{NetPlayer, Zombie};

/// One drawable static mesh — arena walls/floor (`spawn_test_arena`,
/// `crates/client/src/lib.rs:3915`) and the gun's glTF-scene-instantiated
/// child meshes (`GUN_PATH = "models/ak47_scope.glb"`) both show up here,
/// since both carry plain `Mesh3d`/`MeshMaterial3d<StandardMaterial>` once
/// Bevy's scene spawner has run — no separate code path needed for either.
pub struct ExtractedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty if the source `Mesh` isn't indexed (shouldn't happen for
    /// anything built via `meshes.add(Cuboid::new(..))` or loaded glTF, both
    /// of which are always indexed, but guard rather than assume).
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    /// Local `Transform` at extraction time. Root-level entities only
    /// (arena/gun-root) — the arena has no parent so this is already its
    /// world transform; the gun's own child meshes are relative to the gun
    /// root, which is itself `ChildOf(camera)` (camera-relative viewmodel,
    /// not world space) — composing the full chain is Step 4's job once an
    /// actual draw call needs it.
    pub transform: Transform,
}

/// Camera pose, replicating `apply_view_recoil`'s rotation composition
/// (`crates/client/src/lib.rs:5144`) minus the recoil/lean terms (headless
/// has no input driving those yet — Step 3): `eye` is the `FpsCamera`
/// entity's own `Transform.translation` (already kept in sync with the
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

    // Camera: the FpsCamera entity's own Transform + yaw/pitch. See this
    // module's doc comment on CameraSnapshot for why recoil/lean are
    // omitted here (Step 3 concern, not extraction).
    let mut camera_state: SystemState<Query<(&Transform, &crate::FpsCamera)>> =
        SystemState::new(world);
    if let Some((transform, cam)) = camera_state.get(world).iter().next() {
        snapshot.camera = Some(CameraSnapshot {
            eye: transform.translation.to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
        });
    }

    // Zombies: plain marker + Transform, per the plan's own extraction spec.
    let mut zombie_state: SystemState<Query<&Transform, With<Zombie>>> =
        SystemState::new(world);
    snapshot.zombies = zombie_state.get(world).iter().copied().collect();

    // Remote players: NetPlayer entities that aren't this client's own —
    // LocalPlayer is inserted by the game's own `mark_local_player` system
    // onto whichever NetPlayer matches this client's MyClientId.
    let mut remote_state: SystemState<
        Query<&Transform, (With<NetPlayer>, Without<crate::LocalPlayer>)>,
    > = SystemState::new(world);
    snapshot.remote_players = remote_state.get(world).iter().copied().collect();

    // Static meshes: arena walls/floor + gun scene meshes, both plain
    // Mesh3d/MeshMaterial3d<StandardMaterial> entities — see this module's
    // doc comment on ExtractedMesh.
    let mut mesh_state: SystemState<(
        Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &Transform)>,
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
            transform: *transform,
        });
    }

    snapshot
}
