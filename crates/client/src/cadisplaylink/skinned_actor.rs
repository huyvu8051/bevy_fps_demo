
use super::skinned_gltf::SkinnedModel;
use bevy::transform::components::Transform;
use std::collections::HashMap;

pub struct SkinnedModelSlot {
    pub model: SkinnedModel,
    pub scale: f32,
    pub y_offset: f32,

    pub facing_correction: f32,
}

#[derive(Default)]
pub struct SkinnedModelRegistry(HashMap<&'static str, SkinnedModelSlot>);

impl SkinnedModelRegistry {
    pub fn register(&mut self, name: &'static str, model: SkinnedModel, scale: f32, y_offset: f32, facing_correction: f32) {
        self.0.insert(name, SkinnedModelSlot { model, scale, y_offset, facing_correction });
    }

    pub fn get(&self, name: &str) -> Option<&SkinnedModelSlot> {
        self.0.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut SkinnedModelSlot> {
        self.0.get_mut(name)
    }
}

#[allow(dead_code)]
pub enum SkinnedActorPose {
    JointMatrices(Vec<[[f32; 4]; 4]>),
    Clip { clip_index: usize, time: f32 },
}

pub struct SkinnedActorDraw {
    pub transform: Transform,
    pub model: &'static str,
    pub pose: SkinnedActorPose,
}
