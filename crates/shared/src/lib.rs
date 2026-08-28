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
