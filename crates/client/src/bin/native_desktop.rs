//! The game on the desktop through `metal_renderer` instead of `bevy_render`.
//!
//! iOS has run this path for a while, driven from a UIKit host over the
//! `native_metal_probe_*` FFI. This drives the same FFI from winit, so the
//! desktop gets it without an Xcode project and without giving up `cargo run`.
//!
//! winit rather than hand-written AppKit because the window is the easy part;
//! keyboard and mouse handling is not, and winit already has it. All that is
//! borrowed from it is the `NSView` behind the window.
//!
//!   cargo run -p client --release --bin native_desktop
//!
//! Measured against the windowed build on the same map, this is the question
//! the rigs were built to answer -- see desert_rust_bevy's README.

// Not `#![cfg(target_os = "macos")]`: that empties the whole file on iOS,
// main included, and cargo then fails the binary for having no entry point --
// which is what broke the device deploy.
#[cfg(target_os = "macos")]
mod mac {
use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

// Called directly rather than through the C symbols: the iOS host is a
// separate binary and needs the FFI, this one is the same crate and linking
// against its own no_mangle exports only invites the linker to drop them.
use client::cadisplaylink::ffi::{
    native_metal_probe_create_real_game, native_metal_probe_key_changed,
    native_metal_probe_mouse_button_changed, native_metal_probe_mouse_connected,
    native_metal_probe_mouse_moved, native_metal_probe_release,
    native_metal_probe_render_frame,
};

pub fn run() {
    let event_loop = EventLoop::new().expect("no event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut Host::default()).expect("event loop failed");
}

#[derive(Default)]
struct Host {
    window: Option<Window>,
    game: Option<*mut c_void>,
    /// `EXIT_AFTER=<seconds>` leaves the same way Escape does. A profile is
    /// only written when the app shuts down properly, and killing it from the
    /// outside is what left every chrome trace empty.
    quit_at: Option<std::time::Instant>,
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = event_loop
            .create_window(Window::default_attributes().with_title("Last Day in Saigon"))
            .expect("no window");

        // The renderer reads the view's layer, bounds and contentsScale once
        // at create, so the window has to exist first.
        let RawWindowHandle::AppKit(handle) = window
            .window_handle()
            .expect("no window handle")
            .as_raw()
        else {
            panic!("not an AppKit window");
        };
        let view = handle.ns_view.as_ptr() as *mut Object;
        let scale = window.scale_factor() as f32;
        // setWantsLayer alone gives a plain CALayer, and metal_renderer casts
        // whatever `[view layer]` returns straight to CAMetalLayer -- which
        // aborts inside an extern "C" frame, with the message swallowed. The
        // layer has to be the right class before the renderer ever sees it.
        unsafe {
            let layer: *mut Object = msg_send![objc::class!(CAMetalLayer), new];
            let _: () = msg_send![layer, setContentsScale: scale as f64];
            // A framebufferOnly layer cannot be read back, and a screenshot of
            // this path is the only way to see it without holding the device.
            if std::env::var("CAPTURE_SCREENSHOT_TO").is_ok() {
                let _: () = msg_send![layer, setFramebufferOnly: false];
            }
            let _: () = msg_send![view, setLayer: layer];
            let _: () = msg_send![view, setWantsLayer: true];
        }

        let game = unsafe { native_metal_probe_create_real_game(view, scale) };
        assert!(!game.is_null(), "the game host failed to start");
        unsafe { native_metal_probe_mouse_connected(game, true) };

        let _ = window.set_cursor_grab(CursorGrabMode::Locked);
        window.set_cursor_visible(false);

        // Without this nothing ever asks for the first frame, so render_frame
        // -- and with it app.update() -- never runs: the window opens, the
        // renderer initialises, and the map never finishes loading.
        window.request_redraw();
        self.quit_at = std::env::var("EXIT_AFTER")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map(|after| std::time::Instant::now() + std::time::Duration::from_secs_f32(after));
        self.window = Some(window);
        self.game = Some(game);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(game) = self.game else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if code == KeyCode::Escape {
                        event_loop.exit();
                        return;
                    }
                    if let Some(id) = key_id(code) {
                        let down = event.state == ElementState::Pressed;
                        unsafe { native_metal_probe_key_changed(game, id, down) };
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let index = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    _ => return,
                };
                let down = state == ElementState::Pressed;
                unsafe { native_metal_probe_mouse_button_changed(game, index, down) };
            }
            WindowEvent::RedrawRequested => {
                if self.quit_at.is_some_and(|at| std::time::Instant::now() >= at) {
                    event_loop.exit();
                    return;
                }
                unsafe { native_metal_probe_render_frame(game) };
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        let Some(game) = self.game else { return };
        if let DeviceEvent::MouseMotion { delta } = event {
            unsafe { native_metal_probe_mouse_moved(game, delta.0 as f32, delta.1 as f32) };
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(game) = self.game.take() {
            unsafe { native_metal_probe_release(game) };
        }
    }
}

/// The host has its own table, not the platform's: letters are 0..=25 in
/// alphabetical order, digits 26..=35, then a handful of named keys. See
/// `map_key_id` in cadisplaylink::ffi, which is the other half of this.
fn key_id(code: KeyCode) -> Option<u16> {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
        KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        Digit0, Digit1, Digit2, Digit3, Digit4,
        Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    if let Some(i) = LETTERS.iter().position(|k| *k == code) {
        return Some(i as u16);
    }
    if let Some(i) = DIGITS.iter().position(|k| *k == code) {
        return Some(26 + i as u16);
    }
    Some(match code {
        Space => 36,
        Escape => 37,
        ShiftLeft => 38,
        ShiftRight => 39,
        ControlLeft => 40,
        ControlRight => 41,
        AltLeft => 42,
        AltRight => 43,
        BracketLeft => 44,
        BracketRight => 45,
        F1 => 46,
        Tab => 47,
        Enter => 48,
        ArrowUp => 49,
        ArrowDown => 50,
        ArrowLeft => 51,
        ArrowRight => 52,
        _ => return None,
    })
}
}

fn main() {
    #[cfg(target_os = "macos")]
    mac::run();
}
