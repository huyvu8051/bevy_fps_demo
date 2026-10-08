# Building for iOS

**Superseded** — this document described the state *before* Xcode was
installed and before any of the iOS-specific rendering work existed. Xcode
is now installed, the project builds and deploys routinely (Simulator +
physical device), and iOS no longer even uses Bevy's own render backend
(wgpu) — see the root `README.md`'s "Build & chạy trên iOS" section for the
current, accurate build/run steps, and
`crates/client/src/cadisplaylink/native_metal_renderer.rs`'s own doc comment
for why (wgpu's per-frame CPU encoding cost on iOS; replaced with a
hand-written Rust + `metal` crate renderer, drawing the real game's own
ECS state extracted from a headless `App` — no window, no wgpu, no
`bevy_render` on iOS at all).

Kept here only for historical reference on the `cargo-mobile2` scaffolding
gotcha below, which is still true and still worth knowing if the Xcode
project ever needs to be regenerated from scratch.

## Don't run `cargo mobile init` directly in this crate

Tried this early on — **it's destructive**. `cargo-mobile2`'s `init` is
built for scaffolding a *brand-new* project from a template, not adding a
mobile target to an existing one: run inside `crates/client/`, it
overwrote `Cargo.toml` with a generic Bevy 0.16 template (wrong version,
and it dropped every real dependency — `shared`, `avian3d`,
`bevy_replicon`, `bevy_replicon_renet2`) and generated a throwaway
`src/lib.rs` with none of the actual game in it. It also side-effect
installs several Homebrew packages (`xcodegen`, `libimobiledevice`, ...)
before getting to the part that needs `sudo` (`cocoapods`, installed via
`gem`).

**The safe way to use it** (if ever regenerating the Xcode project from
scratch): run `cargo mobile init` in an **empty scratch directory**, not
here, to see the Xcode project structure it generates. Copy *only* that
generated scaffolding over by hand, wiring it to build the real
`crates/client` staticlib instead of the template's — do not let it touch
this crate's `Cargo.toml`. In practice this project's actual
`crates/client/gen/apple/iOS/bevy_in_iOS.xcodeproj` is hand-maintained
directly (Swift `ViewController`/`MetalView`/`CADisplayLink`, no winit,
no `xcodegen`) — see the root README for why.

## Android

Not started. `cargo-ndk` + Gradle instead of Xcode, same
"don't run `init` in this crate directly" warning would apply.
`INTERNET` permission in `AndroidManifest.xml` would be needed for the UDP
socket (unlike iOS, which doesn't gate a plain outbound UDP socket behind
a permission prompt).
