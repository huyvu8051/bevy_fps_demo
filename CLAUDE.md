# Bevy FPS Demo - Developer Guidelines

## Coding Style & Instructions
- **Concise Code**: Write clean, idiomatic, self-documenting Rust code.
- **Minimal Comments**: DO NOT write multi-line explanatory essays, architectural post-mortems, or verbose rationale in comments. Only write short, essential comments when mathematical formulas, unsafe blocks, or non-obvious hardware constraints strictly require them.
- **Maintain Existing Comments**: Do not touch or reformat unrelated existing comments unless requested.

## Architecture
- Engine: Bevy 0.18 + Avian3D 0.6 + Bevy Replicon 0.39.
- Crate structure:
  - `crates/client`: Desktop, Web (wasm32), iOS game client.
  - `crates/server`: Dedicated game server, zombie navigation & physics authority.
  - `crates/shared`: Common components, network protocol, level definitions, and movement math.

## Common Commands
- Check: `cargo check`
- Test: `cargo test`
- Run client (desktop): `cargo run -p client`
- Run server: `cargo run -p server`
