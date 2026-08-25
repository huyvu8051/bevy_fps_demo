// Client-side mirror of the server's per-player AI takeover switch.
//
// The actual decision logic (target selection, aim tracking, swarm-aware
// retreat/approach/dodge, firing) used to live entirely in this module —
// it now runs server-side instead (see crates/server's
// server_autopilot_pick_target/server_autopilot_think, and SetAutopilot's
// own doc comment in crates/shared for exactly why: so autopilot keeps
// working through an AFK window, or even a dropped connection, not only
// while this client happens to be running its own decision loop). Moving
// it there needed no gameplay logic duplicated a second time — the server
// already had an almost-identical AI (bot_pick_target/move_debug_bots/
// debug_bot_fire) driving DebugBot entities; server-side autopilot mirrors
// that same pattern against a real Player entity instead.
//
// What's left here is just enough for the toggle button (Settings menu,
// via cadisplaylink::native_menu::autopilot/set_autopilot) to have
// something to read/flip locally for instant UI feedback, and for the
// systems that used to special-case "autopilot is deciding, not me" —
// mouse_look/touch_look (aim), send_player_input (movement — skips local
// prediction while this is on, see its own doc comment), shoot (firing) —
// to keep doing so, now purely by "don't bother, the server has this"
// rather than "read what autopilot.rs decided".
use bevy::prelude::*;

/// Client-side mirror of ServerAutopilot (crates/server) — NOT itself
/// authoritative. Set instantly on toggle for responsive UI (same
/// zero-round-trip-latency pattern as e.g. AutoMaintainUi), then a
/// SetAutopilot event tells the server to actually flip its own copy.
/// Defaults to true to match the server's own Default (see
/// ServerAutopilot's own impl) — this should never need to disagree with
/// the server outside of the brief window before a toggle's event arrives.
#[derive(Resource)]
pub(crate) struct Autopilot(pub(crate) bool);
impl Default for Autopilot {
    fn default() -> Self {
        Autopilot(true)
    }
}
