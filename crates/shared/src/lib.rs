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

/// Loads one of the hand-tunable RON data files under
/// `crates/client/assets/data/` (car physics, arena layout, zombie
/// transform...) at a `$path` that must be a compile-time string literal
/// (same requirement `include_str!` has, since that's what the wasm arm
/// expands to) — see `item_gallery::item_gallery_car_physics_raw`'s doc
/// comment for why a `CARGO_MANIFEST_DIR`-relative `concat!` literal is
/// the right shape for `$path` here.
///
/// Native targets read the file at runtime, same as before — so `EDITOR=1`
/// hand-edits or an F9 dev-editor save take effect on next launch with no
/// rebuild. `wasm32` has no real filesystem in the browser sandbox
/// (confirmed: `std::fs::read_to_string` compiles fine for
/// wasm32-unknown-unknown, it just always returns an `Err` at runtime —
/// there's no syscall behind it), so that target instead embeds the
/// file's contents at compile time via `include_str!`: a web build always
/// ships whatever the file held when it was last built, no runtime
/// tuning, and the F9 save systems are wasm-gated off entirely (see
/// `ItemGalleryCarsPlugin`/`ArenaLayoutPlugin`) since there's nothing
/// persistent to write back to anyway.
#[macro_export]
macro_rules! read_ron_data_file {
    ($path:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            include_str!($path).to_string()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::read_to_string($path).unwrap_or_else(|e| panic!("failed to read RON data file {}: {e}", $path))
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
