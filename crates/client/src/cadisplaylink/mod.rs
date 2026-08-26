//! CADisplayLink-driven iOS entry point. Only ever reachable from Swift via
//! the extern "C" functions in ffi.rs — never touched by desktop builds.
//!
//! The original experiment this module started as (see git history) ran a
//! full `bevy_render`/wgpu App on the CADisplayLink loop, same as the
//! normal winit-driven app minus the event loop — that path (and the
//! `ios_view.rs` raw-window-handle bridge it needed) was removed once it
//! became clear wgpu's own CPU encoding cost, not the winit event loop,
//! was the real problem (see README.md's "Build & chạy trên iOS" section).
//! What's left is the two things that actually ship: `native_metal_probe`
//! (the hand-written Metal renderer, driving a real headless gameplay
//! `App` via `native_game_app`/`native_game_extract`) and `fps_probe`'s
//! own minimal camera+room scene for raw FPS/CPU measurement.

mod ffi;
pub mod gltf_scene;
pub mod metal_math;
pub mod metal_texture;
pub mod native_game_app;
pub mod native_game_extract;
pub mod native_menu;
pub mod native_metal_probe;
pub mod skinned_gltf;
