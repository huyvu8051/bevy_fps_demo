//! wasm32 entry point for the browser build — see this crate's own
//! Cargo.toml comment for why it exists separately from `client` (cdylib
//! vs client's iOS-sensitive staticlib+rlib). Everything else — the whole
//! App, every plugin, every system — is unchanged `client` code; this
//! crate is nothing but the `#[wasm_bindgen(start)]` glue wasm-bindgen
//! needs to call into it from the page's JS.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    // Bevy's LogPlugin (pulled in by client::main()'s DefaultPlugins) sets
    // up console_error_panic_hook/tracing-wasm itself on wasm32 — no extra
    // setup needed here beyond just calling into the same entry point the
    // desktop binary and iOS staticlib both use.
    client::main();
}
