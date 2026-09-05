# Web port (wasm32)

Status: **works, full parity with desktop** — a release build compiles,
boots, renders, loads real assets, and plays real multiplayer against the
same dedicated server desktop/iOS clients use: stays connected (real
measured Ping, e.g. 31ms, not a placeholder), zombies spawn/take
damage/bleed, ammo drops on fire — verified live in Chrome, and over LAN
from a phone. Not yet: a public (non-LAN) deployment or a `.wasm` size
pass. See §6 for what's actually left.

## 1. Why this needed its own crate (`crates/web/`)

`client`'s own `[lib] crate-type` is deliberately `["staticlib", "rlib"]`
only — see that Cargo.toml's own comment: mixing in `"cdylib"` there once
baked a broken `LC_LOAD_DYLIB` into the iOS static build and crashed on a
real device. `wasm-bindgen` needs a `cdylib` compiled to
`wasm32-unknown-unknown` to produce a runnable `.wasm` module, so that
lives in a new, separate crate instead:

```
crates/web/
  Cargo.toml   # crate-type = ["cdylib"], depends on client + pinned wasm-bindgen
  src/lib.rs   # #[wasm_bindgen(start)] fn start() { client::main() } — nothing else
```

`wasm-bindgen` (the crate) and `wasm-bindgen` (the CLI tool) must be the
**exact same version** or the CLI refuses to run ("linked against a
different version of wasm-bindgen"). `crates/web/Cargo.toml` pins
`wasm-bindgen = "=0.2.126"` to match whatever's installed
(`wasm-bindgen --version`) — bump both together if the CLI is ever
upgraded.

## 2. Runtime bugs only a live browser load caught

`cargo check --target wasm32-unknown-unknown` catches compile errors, not
these — both only showed up as a crashed tab, found by actually loading
the build in Chrome (`claude-in-chrome` browser tool) and reading the
console.

- **`std::time::Instant`/`SystemTime` panic outright on wasm32**
  ("time not implemented on this platform") — no polyfill, no graceful
  fallback, just an immediate panic the first time it's called. Bevy's own
  internals already route around this; this project's own direct calls
  (`zombie_client.rs`'s per-frame perf-timing, `network_setup.rs`'s
  connect-time timestamp) didn't. Fixed with the `web-time` crate — a
  drop-in `Instant`/`SystemTime` replacement (real `std` on native,
  `performance.now()`/`Date.now()` on wasm32).
- **`std::env::set_var` panics outright on wasm32**
  ("cannot set env vars on this platform") — `std::env::var` (read) is
  fine there (always returns `Err`, a path every caller already handled),
  but `set_var` crashes immediately. The map-select menu used it to stash
  the picked map id. Replaced with a plain process-global
  `static MAP_ID_OVERRIDE: Mutex<Option<u8>>` cell
  (`network_setup::{set_map_id, map_id_override}`) — behaves identically
  on every platform, no signature changes needed anywhere that read it.

## 3. Filesystem: `std::fs` compiles on wasm32, but there's no real FS

`std::fs::read_to_string`/`write` **compile** fine for
`wasm32-unknown-unknown` — the module exists — but every call fails at
runtime (no filesystem in a browser sandbox). This project loads several
hand-tunable RON data files this way (car physics, arena layout, zombie
transform — see `docs/maps/`) and has an `EDITOR=1` F9 dev-save that
writes them back out.

- **Reads**: `shared::read_ron_data_file!` macro — native reads the file
  at runtime (unchanged behavior); wasm32 embeds it at compile time via
  `include_str!` instead. A web build always ships whatever the RON files
  held at build time; no runtime tuning. `arena.rs`'s loader takes a
  runtime `map_id` so it can't route through a macro that needs a literal
  path — it gets its own per-map-id `match` arm for wasm32 instead (adding
  a new arena map means adding a match arm there too).
- **Writes**: the whole F9-save system (`ItemGalleryCarsPlugin`,
  `ArenaLayoutPlugin`) is `#[cfg(not(target_arch = "wasm32"))]`-gated at
  `Plugin::build` — nothing persistent to write back to in a browser, so
  it doesn't exist there at all rather than failing every save.

## 4. Networking: two sockets, one server process

A browser can't open a raw UDP socket at all, so `native_transport`
(what desktop/iOS use) is a dead end for web. `renet2` offers two
web-capable transports:

| | WebTransport (`wt_*_transport`) | WebSocket (`ws_*_transport`) — **used here** |
|---|---|---|
| Underlying protocol | QUIC/UDP | TCP |
| Latency | lower | slightly higher, still fine |
| TLS requirement | valid cert, or self-signed + certificate-hash pinning (cert <14 days, client must know the exact hash) | none — `WebSocketAcceptor::Plain`; netcode's own encryption covers security either way |
| Testable without a real domain | hard | yes — `ws://` works over plain localhost |

Picked WebSocket for a first pass that's actually verifiable in a sandbox
without a domain/cert. Revisit WebTransport once there's a real deployment
target.

**Server** (`crates/server/src/main.rs`, `start_server`): now runs *two*
sockets on the same `NetcodeServerTransport` via
`NetcodeServerTransport::new_with_sockets` — one dedicated server serves
both native and web clients with shared game state:

- socket 0: the original `NativeSocket` (UDP) on `scenario.port()` —
  unchanged.
- socket 1: a new `WebSocketServer` on `scenario.ws_port()`
  (`shared::Scenario::ws_port()` = `port() + WS_PORT_OFFSET`, currently
  `+1000`). `WebSocketServer::new` needs a `tokio::runtime::Handle` to
  spawn its reader/writer tasks on — Bevy's own loop here is a plain
  `ScheduleRunnerPlugin` poll, not tokio, so `start_server` spins up one
  small multi-thread runtime and leaks it for the process's lifetime
  (nothing needs to shut it down early).

**Client** (`crates/client/src/network_setup.rs`,
`spawn_client_connection`): two implementations behind
`#[cfg(target_arch = "wasm32")]` — the native one is unchanged (real UDP,
`socket_id: 0`); the wasm32 one builds a `WebSocketClientConfig` from
`ws://<page-hostname>:<ws_port>` (host read from `window.location.hostname`
via `web_sys`, not hard-coded — a phone on the same LAN opens
`http://<dev-machine-ip>:8765` and the client connects back to that same
IP) and connects with `socket_id: 1`, matching the server's second socket.
No env-var-based host routing (`SERVER_ADDR`/`LOCAL`/`SERVER_HOST`, like
the native path has) — wasm32 has no env vars at all.

### 4.1 The bug that actually blocked multiplayer: `has_reliable_socket`

Everything above sounds like it should just work once wired up — it
doesn't, and the failure mode is brutal to diagnose: the client reaches
`Connected` for a moment, then the **server** immediately disconnects it
with `received message with invalid channel 1` and despawns its
`ConnectedClient` entity. `auto_reconnect` retries every 2s, forever, so
the whole thing looks like a permanently broken handshake — Ping/Loss
never move off `0`/`0%`, no replicated entity (including zombies) ever
shows up, and every naive theory ("wrong port", "wrong host", "menu
click landed on the wrong scenario") is a red herring since the
connection genuinely does complete at the netcode level before dying.

**Real cause**: `RenetClient::new(config, has_reliable_socket)` needs to
be told by hand whether the underlying socket already guarantees
reliability/ordering (`true` for WebSocket/TCP, `false` for real UDP) —
it does not introspect the `socket` argument to figure this out itself.
The **server** side gets this right automatically
(`renet2_netcode::server::add_connection` calls
`sockets[socket_id].is_reliable()` — `WebSocketServer::is_reliable()` is
hard-coded `true`) and downgrades that one connection's channels
server-side via `renet2::ConnectionConfig::downgrade_to_unreliable()`
(changes every channel's `SendType` to `Unreliable { ordered_reliable_
substrate: true }` — channel *count* is untouched, only the wire framing
per channel). The wasm32 client's own `RenetClient::new` call was
copy-pasted from the native path and kept the native path's `false`. With
only the server downgrading, client and server packet framing diverge
the instant any post-handshake message is sent — the server parses the
mismatched bytes as a bogus channel id and disconnects. Fix is one word:
pass `true` instead of `false` on the wasm32 path.

This one took a long investigation to find (documented in full for
next time): confirmed it wasn't the menu/scenario click (forced `MAP_ID`
directly, same result), confirmed it wasn't a stale/cached build,
checked whether it was an already-fixed upstream bug and bumped
`renet2_netcode`/`bevy_replicon_renet2`/`bevy_replicon` 0.13/0.13/0.38 →
0.14-0.15/0.15/0.39 to rule that out (no change — left the bump in place
since it's a working, harmless newer resolution, but it wasn't the fix),
then read `renet2`'s actual channel-downgrade source
(`remote_connection.rs`) end to end to find the asymmetry.

Cargo feature wiring (all additive — native builds are unaffected):

```toml
# crates/server/Cargo.toml
bevy_replicon_renet2 = { version = "0.15", features = ["native_transport", "ws_server_transport"] }
tokio = { version = "1", features = ["rt-multi-thread"] }

# crates/client/Cargo.toml — wasm32-only target table, placed at the END
# of the file (see the sharp edge below)
[target.'cfg(target_arch = "wasm32")'.dependencies]
bevy_replicon_renet2 = { version = "0.15", features = ["ws_client_transport"] }
url = "2"
web-sys = { version = "0.3", features = ["Window", "Location"] }
```

**Sharp edge hit and fixed**: a `[target.'cfg(...)'.dependencies]` table
placed in the *middle* of `[dependencies]` silently swallows every
following `key = value` line into itself instead of the real
`[dependencies]` table — TOML tables aren't indentation-scoped. Broke the
native build with "cannot find crate `bevy_editor_pls`" (that line ended
up inside the wasm32-only table) until the whole wasm32 block was moved to
the very end of `crates/client/Cargo.toml`.

## 5. Build + run locally

```bash
# 1. One-time: wasm target + tools
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli   # must match the wasm-bindgen crate version
                                  # pinned in crates/web/Cargo.toml exactly
brew install binaryen            # provides wasm-opt, see §5.1

# 2. Build the .wasm — the web-release profile (Cargo.toml), not plain
# --release; see §5.1 for why. Slow (LTO + codegen-units=1, ~7 minutes on
# this dev machine) — that's the deliberate size/build-time trade this
# profile makes, only pay it right before actually shipping a build.
cargo build -p web --target wasm32-unknown-unknown --profile web-release

# 3. Generate the JS glue + final .wasm
mkdir -p target/web_dist
wasm-bindgen --target web --out-dir target/web_dist --out-name web \
  target/wasm32-unknown-unknown/web-release/web.wasm

# 3b. Shrink it further with wasm-opt -Oz (binaryen) — see §5.1 for the
# actual size numbers this and step 2 together produce.
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o target/web_dist/web_bg.wasm target/web_dist/web_bg.wasm

# 4. index.html + favicon/apple-touch-icon — crates/web/index.html is the
# canonical one (has a loading progress bar, see its own comments); copy
# it in rather than hand-writing a new one. The two PNGs are what
# index.html's own <link rel="icon">/<link rel="apple-touch-icon"> tags
# point at — index.html references them by relative path, so they have to
# ship alongside it in target/web_dist, not just live next to the source
# index.html in the repo.
cp crates/web/index.html target/web_dist/index.html
cp crates/web/favicon-32.png crates/web/apple-touch-icon.png target/web_dist/

# 5. Serve it — needs assets/ reachable at the SAME relative path the
# native build uses (bevy_asset resolves "assets/..." relative to the
# page). Symlinking is enough for local testing:
ln -sfn "$(pwd)/crates/client/assets" target/web_dist/assets
cd target/web_dist && python3 -m http.server 8765

# 6. Run a real server so the web client has something to connect to —
# the web build always launches into Arena (map_id 2, see §4.1's sibling
# note in client::main — no menu on web anymore):
MAP_ID=2 cargo run -p server
```

### 5.1 `.wasm` size: 108MB → 25MB

The original 108MB `--release` build (plain `opt-level = 3`, no LTO, no
strip) reportedly wouldn't even open on an iPhone 14 Pro Max — almost
certainly iOS Safari's WebAssembly linear-memory/tab-memory limit (a
WebKit limit, much tighter than desktop Chrome's regardless of the
device's own RAM — a 6GB-RAM phone can still fail to load a page a
16GB-RAM desktop opens fine). Two independent levers, both real,
worth keeping in mind separately if this needs revisiting:

1. **`[profile.web-release]` (root `Cargo.toml`)** — `opt-level = "z"`
   (size over speed), `lto = true` (cross-crate dead code elimination,
   the single biggest lever here), `codegen-units = 1`, `strip = true`,
   `panic = "abort"`. A dedicated profile, not a change to
   `[profile.release]` itself, so native desktop/server release builds
   (which don't have this problem) keep their normal fast-compile,
   speed-optimized settings. **108MB → 33MB** on its own.
2. **`wasm-opt -Oz`** (binaryen, step 3b above) on top of that —
   **33MB → 25MB** after `wasm-bindgen` processes it down to ~30MB first.

Verified the `web-release` + `wasm-opt` build still runs correctly (same
Chrome test as always: loads, connects, Arena renders, touch UI, no
console panics) before trusting the size win.

Open `http://localhost:8765/index.html` (or `http://<lan-ip>:8765/...`
from another device on the same network — the client reads the page's own
hostname, see §4.1). Lands straight in Arena, no menu (see the wasm32
branch in `client::main`) — Ping/Loss in the HUD should read real, moving
numbers once connected (e.g. `31ms`/`0%`), not stuck at `0ms`/`0%` forever
or `--`/`--`.

## 6. What's left (not done here)

- **No gzip/brotli compression, no asset trimming for web specifically.**
  §5.1 covers the `.wasm` binary itself (108MB → 25MB); `crates/client/
  assets/` is still whatever the native build ships, untrimmed for a
  mobile-web audience — serving it compressed would help further but
  hasn't been done.
- **WebTransport.** Would cut latency vs WebSocket, but needs a real TLS
  cert (or the self-signed certificate-hash-pinning dance) to test
  properly — revisit once there's an actual domain to deploy against.
- **`bevy_editor_pls` on web.** Untested whether the dev editor (`EDITOR=1`,
  F2) even makes sense in a browser build — its own F9 save systems are
  already wasm32-gated off (§3), but the rest of the editor UI was never
  exercised there.
