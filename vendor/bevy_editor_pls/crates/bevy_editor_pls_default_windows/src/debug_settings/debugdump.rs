use bevy::prelude::*;
use std::{
    io::Write,
    process::{Command, Stdio},
};

// Schedule/render-graph dot-dump disabled while patching this vendored
// fork for Bevy 0.18 (bevy_mod_debugdump's own bevy dependency is still
// on 0.15, causing duplicate-crate type errors) — irrelevant to the
// level-editing use case this fork is being patched for anyway. `setup`
// just leaves DotGraphs empty; the debug_settings window UI still shows
// (see mod.rs) but the dump buttons produce nothing.
#[derive(Resource)]
pub struct DotGraphs {
    pub update_schedule: Option<String>,
    pub fixed_update_schedule: Option<String>,
    pub render_main_schedule: Option<String>,
    pub render_extract_schedule: Option<String>,
    pub render_graph: Option<String>,
}

pub fn setup(app: &mut App) {
    app.insert_resource(DotGraphs {
        update_schedule: None,
        fixed_update_schedule: None,
        render_main_schedule: None,
        render_extract_schedule: None,
        render_graph: None,
    });
}

pub fn execute_dot(dot: &str, format: &str) -> Result<Vec<u8>, std::io::Error> {
    let mut child = Command::new("dot")
        .arg("-T")
        .arg(format)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child.stdin.as_mut().unwrap().write_all(dot.as_bytes())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, String::from_utf8_lossy(&output.stderr)));
    }

    Ok(output.stdout)
}
