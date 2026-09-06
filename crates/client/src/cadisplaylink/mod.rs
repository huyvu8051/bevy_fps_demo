//! CADisplayLink-driven iOS entry point — the sole, permanent iOS
//! rendering path. Only ever reachable from Swift via the extern "C"
//! functions in ffi.rs — never touched by desktop builds.
//!
//! The original experiment this module started as (see git history) ran a
//! full `bevy_render`/wgpu App on the CADisplayLink loop, same as the
//! normal winit-driven app minus the event loop — that path (and the
//! `ios_view.rs` raw-window-handle bridge it needed) was removed once it
//! became clear wgpu's own CPU encoding cost, not the winit event loop,
//! was the real problem (see README.md's "Build & chạy trên iOS" section).
//! What's left, and what actually ships, is `native_metal_renderer`: the
//! hand-written Metal renderer (`NativeMetalRenderer`), driving a real
//! headless gameplay `App` via `native_game_app`/`native_game_extract`.

mod ffi;
pub mod gltf_scene;
pub mod metal_math;
pub mod metal_msaa;
pub mod metal_texture;
pub mod native_game_app;
pub mod native_game_extract;
pub mod native_menu;
pub mod native_metal_renderer;
pub mod skinned_actor;
pub mod skinned_gltf;
