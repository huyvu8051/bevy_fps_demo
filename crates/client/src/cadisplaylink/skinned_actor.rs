//! The renderer core's own notion of "a GPU-skinned thing to draw this
//! frame" — deliberately knows nothing about zombies, soldiers, or any
//! other gameplay concept. Used to be a closed `EntityKind::{Zombie,
//! Soldier}` enum in native_metal_probe.rs, hardcoding both which
//! `SkinnedModel` to draw AND its scale/offset/facing correction directly
//! into the draw loop — meaning adding a third skinned actor type (a new
//! monster, a vehicle, anything) meant editing the renderer itself. Moved
//! to a name-keyed registry instead: whoever extracts a frame's actors
//! (this project's own native_game_extract.rs, or a different host app's
//! equivalent) registers whatever named models it has and tags each
//! extracted actor with one of those names — the renderer core only ever
//! asks "what's registered under this name", never "is this a Zombie".

use super::skinned_gltf::SkinnedModel;
use bevy::transform::components::Transform;
use std::collections::HashMap;

/// A loaded skinned model plus the per-model placement correction needed to
/// align it with wherever its entity's own `Transform` actually sits — a
/// model's own rig origin/scale/facing convention is a property of that
/// asset, not something the renderer should hardcode per gameplay type.
/// (Concretely: this project's zombie.glb needs shrinking to ZOMBIE_SCALE
/// and lifting from capsule-center to feet; soldier.glb needs a fixed yaw
/// correction because its forward axis doesn't match the entity's own
/// facing convention. Both are just data now, not renderer branches.)
pub struct SkinnedModelSlot {
    pub model: SkinnedModel,
    pub scale: f32,
    pub y_offset: f32,
    /// Extra yaw (radians), applied on top of the entity's own rotation.
    pub facing_correction: f32,
}

/// Name -> loaded model. The host app populates this once at startup
/// (register whatever skinned models it has); the renderer core only ever
/// reads it by name at draw time, via `SkinnedActorDraw::model`.
#[derive(Default)]
pub struct SkinnedModelRegistry(HashMap<&'static str, SkinnedModelSlot>);

impl SkinnedModelRegistry {
    pub fn register(&mut self, name: &'static str, model: SkinnedModel, scale: f32, y_offset: f32, facing_correction: f32) {
        self.0.insert(name, SkinnedModelSlot { model, scale, y_offset, facing_correction });
    }

    pub fn get(&self, name: &str) -> Option<&SkinnedModelSlot> {
        self.0.get(name)
    }
}

/// How to get this frame's per-joint skinning matrices for one actor.
/// `JointMatrices` is preferred — already-animated matrices read straight
/// out of a real ECS skeleton (see native_game_extract.rs's
/// `joint_matrices_for`), so there's only ever one implementation of
/// keyframe sampling/hierarchy math in play (whatever `bevy_animation`
/// itself does), matching the desktop/wgpu path exactly by construction.
/// `Clip` is the fallback for an actor with no live ECS skeleton wired up
/// to extract from yet — `SkinnedModel::compute_joint_matrices` samples
/// its own hand-parsed copy of the glTF's animation data instead, which
/// can in principle drift from `bevy_animation`'s own sampling over time.
/// Currently only this project's remote-player/soldier extraction uses
/// this fallback (zombies use `JointMatrices`) — see native_metal_probe.rs's
/// own draw_scene doc comment for the plan to close this gap too.
pub enum SkinnedActorPose {
    JointMatrices(Vec<[[f32; 4]; 4]>),
    Clip { clip_index: usize, time: f32 },
}

/// One skinned actor to draw this frame, exactly as extracted from the
/// host app's own ECS/game state — this project's implementation is
/// native_game_extract.rs's zombie/remote-player queries, mapped to this
/// shape in native_metal_probe.rs's render_frame. `model` is a
/// `SkinnedModelRegistry` key, not a gameplay type.
pub struct SkinnedActorDraw {
    pub transform: Transform,
    pub model: &'static str,
    pub pose: SkinnedActorPose,
}
