# Building for iOS

Status: **not yet buildable from this machine** — full Xcode isn't installed (only
Command Line Tools, which lack the iOS SDK). This documents what's already done
and the steps left once Xcode is installed.

## Already done

- `rustup target add aarch64-apple-ios aarch64-apple-ios-sim` — both installed.
- `cargo install cargo-mobile2` — installed (`cargo mobile --help` works).
- Touch controls (virtual joystick, drag-to-look, fire button) already exist in
  `src/main.rs`, gated by `TouchControlsEnabled`: auto-on when
  `cfg!(target_os = "ios")` or `"android"`, or forced on under a desktop build via
  `TOUCH_UI_PREVIEW=1 cargo run --bin client` (useful for iterating on the touch
  UI/input logic without a device — see the `TouchControlsEnabled`/
  `touch_move_axis`/`touch_look`/`touch_fire_button` doc comments in `src/main.rs`
  for how it wires into the existing keyboard/mouse input pipeline).

## Don't run `cargo mobile init` directly in this crate

Tried this already — **it's destructive**. `cargo-mobile2`'s `init` is built for
scaffolding a *brand-new* project from a template, not adding a mobile target to
an existing one: run inside `crates/client/`, it overwrote `Cargo.toml` with a
generic Bevy 0.16 template (wrong version, and it dropped every real dependency —
`shared`, `avian3d`, `bevy_replicon`, `bevy_replicon_renet2`) and generated a
throwaway `src/lib.rs` with none of the actual game in it. It also side-effect
installs several Homebrew packages (`xcodegen`, `libimobiledevice`, ...) before
getting to the part that needs `sudo` (`cocoapods`, installed via `gem`).

**The safe way to use it**: run `cargo mobile init` in an **empty scratch
directory**, not here, to see the Xcode project structure it generates (an
`gen/apple/` folder with an `.xcodeproj`/`.xcworkspace` plus `Info.plist`).
Copy *only* that generated Xcode project scaffolding over into this crate by
hand, wiring it to build the real `crates/client` staticlib instead of the
template's — do not let it touch this crate's `Cargo.toml`.

## Steps once Xcode is installed

1. Install full Xcode from the App Store (not just Command Line Tools) — needed
   for the iOS SDK and `xcodebuild`. `xcode-select -p` should print a path
   under `/Applications/Xcode.app/...` once done, not
   `/Library/Developer/CommandLineTools`.
2. `cargo mobile init` needs a few interactive answers only you can give: an app
   identifier (e.g. `com.huyvu8051.bevyfpsdemo`) and an **Apple Developer Team
   ID** (from your Apple ID — Xcode → Settings → Accounts, or
   developer.apple.com/account if you have a paid membership). Run it in a
   scratch directory as described above, then hand-merge the generated Xcode
   project into this crate rather than running it here directly.
3. `Info.plist`: probably doesn't need `NSLocalNetworkUsageDescription`/Bonjour
   entries — that's specifically for local-subnet discovery (mDNS/Bonjour), and
   this client just opens a plain outbound UDP socket to a public address (see
   `SERVER_ADDR` below), which iOS doesn't gate behind a permission prompt. Only
   add it if testing shows otherwise once you can actually run it.
4. Assets: `crates/client/assets/` (GLB models, audio) needs to end up inside
   the `.app` bundle — the generated Xcode project should have a "Copy Bundle
   Resources" build phase for this; confirm the asset-loading code still finds
   them at runtime (Bevy's default `AssetPlugin` resolves paths relative to the
   running executable, which for an iOS app *is* the bundle root, so this
   should work without code changes — verify once buildable).
5. Server connection: set `SERVER_ADDR` before building/running to point at the
   dedicated server — see the root-level playit.gg tunnel address if testing
   against the k3s deployment (`argocd/manifests/bevy-fps-demo` in the `infra`
   repo), or a LAN IP for local testing. There's no way to pass env vars to an
   installed iOS app the way `cargo run` does on desktop — this will need to
   become a compile-time default or an in-app settings field before this is
   useful beyond Xcode's own "run on device" (which does forward the build
   environment).
6. Provisioning: a free personal Apple ID lets you install to your own device
   for 7 days at a time via Xcode; TestFlight or longer-lived installs need the
   paid Apple Developer Program ($99/year).

## Android

Not started. Same `cargo-mobile2` tool covers Android too (`cargo-ndk` +
Gradle project instead of Xcode), and the same "don't run `init` in this crate
directly" warning applies. `INTERNET` permission in `AndroidManifest.xml` is
needed for the UDP socket (unlike iOS, this one Android *does* require an
explicit manifest entry).
