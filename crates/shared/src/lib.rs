// Plain gameplay data shared between the authoritative server and the
// client — no rendering/audio handles, so this crate can compile headless
// (server side). Doc comments trimmed to what's still true from this
// crate's point of view; see the systems that use these in
// crates/client/src/main.rs / crates/server/src/main.rs for full context.
//
// Organized into one module per concern (see each module's own doc
// comment for what it covers); everything is re-exported flat at the
// crate root (`pub use module::*` below) so existing call sites like
// `use shared::{Zombie, ZOMBIE_RADIUS, FireShot}` keep working unchanged
// regardless of which module something actually lives in — this split is
// purely organizational, not a public API change. `movement`/`sim_latency`
// are the exception: they were already separate, path-qualified modules
// (`shared::movement::...`) before this split, so they stay that way.

/// Real iOS device's own resource directory (`NSBundle.main.resourcePath`)
/// — a real device's app bundle lives at a sandboxed path chosen at
/// install time, nothing like the dev machine's `CARGO_MANIFEST_DIR` this
/// crate's other native-target file reads assume. Confirmed necessary by
/// an actual device crash (not just theoretical): `read_ron_data_file!`'s
/// old 2-way wasm32/native split reads `CARGO_MANIFEST_DIR`-relative on
/// every non-wasm32 target, which happens to still resolve on the
/// Simulator (its "sandboxed" bundle is a real path on the host Mac's own
/// filesystem) but panics with "No such file or directory" on a real
/// device — the exact bug `client::load_character_ron_by_dir` hit first
/// (fixed the same way, via `cadisplaylink::native_metal_probe::
/// bundle_resource_path` in that crate) before this crate's own RON
/// files turned out to need it too.
///
/// A near-duplicate of client's own `bundle_resource_path` (can't call
/// into client from here — this crate is a dependency of client, not the
/// other way around) — kept deliberately tiny (one Foundation call) so
/// duplicating it is cheaper than inverting that dependency.
#[cfg(target_os = "ios")]
pub fn ios_bundle_resource_path() -> Option<String> {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    unsafe {
        let bundle_class = class!(NSBundle);
        let bundle: *mut Object = msg_send![bundle_class, mainBundle];
        let path: *mut Object = msg_send![bundle, resourcePath];
        if path.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Loads one of the hand-tunable RON data files under
/// `crates/client/assets/data/` (car physics, arena layout, zombie
/// transform...) — `$file` is just the filename (e.g.
/// `"map1_level_pieces.ron"`), a compile-time string literal (same
/// requirement `include_str!` has, since that's what the wasm arm
/// expands to); the `assets/data/` prefix is added here so call sites
/// don't each hand-roll their own path join.
///
/// Three platform branches, not two — see `ios_bundle_resource_path`'s
/// own doc comment for why native had to split further into iOS vs.
/// everything else (desktop, mainly):
/// - `wasm32`: no real filesystem in the browser sandbox (confirmed:
///   `std::fs::read_to_string` compiles fine for wasm32-unknown-unknown,
///   it just always returns an `Err` at runtime — there's no syscall
///   behind it), so this target embeds the file's contents at compile
///   time via `include_str!` instead: a web build always ships whatever
///   the file held when it was last built, no runtime tuning, and the F9
///   save systems are wasm-gated off entirely (see
///   `ItemGalleryCarsPlugin`/`ArenaLayoutPlugin`) since there's nothing
///   persistent to write back to anyway.
/// - `target_os = "ios"`: reads from `NSBundle.main.resourcePath`, the
///   real device's own copy of the bundled asset.
/// - everything else (desktop native): reads the file at runtime via
///   `CARGO_MANIFEST_DIR`, same as before — so `EDITOR=1` hand-edits or
///   an F9 dev-editor save take effect on next launch with no rebuild.
#[macro_export]
macro_rules! read_ron_data_file {
    ($file:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/data/", $file)).to_string()
        }
        #[cfg(target_os = "ios")]
        {
            let resource_path = $crate::ios_bundle_resource_path()
                .unwrap_or_else(|| panic!("failed to read RON data file {}: NSBundle.main.resourcePath was nil", $file));
            let path = std::path::Path::new(&resource_path).join("assets/data").join($file);
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read RON data file {path:?}: {e}"))
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
        {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/data/", $file);
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read RON data file {path}: {e}"))
        }
    }};
}

pub mod movement;
pub mod sim_latency;

pub mod arena;
pub mod combat;
pub mod debug_bot;
pub mod game_state;
pub mod hitbox;
pub mod item_gallery;
pub mod level_piece;
pub mod map1;
pub mod network;
pub mod player;
pub mod zombie;

pub use arena::*;
pub use combat::*;
pub use debug_bot::*;
pub use game_state::*;
pub use hitbox::*;
pub use item_gallery::*;
pub use level_piece::*;
pub use map1::*;
pub use network::*;
pub use player::*;
pub use zombie::*;
