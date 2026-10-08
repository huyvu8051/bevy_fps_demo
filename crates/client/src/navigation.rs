
use bevy::prelude::*;

/// Toggled by the NavDbg cheat button; persisted in settings. Has no visual
/// effect until a navmesh debug view is wired up again.
#[derive(Resource, Default)]
pub(crate) struct NavDebugVisible(pub(crate) bool);

/// Marks level geometry that should carve a hole in the navmesh.
#[derive(Component)]
pub(crate) struct ClientNavObstacle;

#[derive(Component)]
pub(crate) struct ArenaCarNavObstacle;
