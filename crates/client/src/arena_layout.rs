
use bevy::prelude::*;
use shared::{ArenaShape, Scenario};

use crate::arena_cars::{ron_f32, F9SaveToast};
use crate::{current_scenario, ArenaBoxDims};

pub(crate) struct ArenaLayoutPlugin;

impl Plugin for ArenaLayoutPlugin {

    fn build(&self, app: &mut App) {

        if std::env::var("EDITOR").is_ok() {

            app.add_systems(Update, save_arena_layout_on_key.run_if(crate::debug_keys_allowed));
        }
    }
}

fn save_arena_layout_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    boxes: Query<(&Transform, &ArenaBoxDims)>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let scenario = current_scenario();
    if scenario != Scenario::StressTest {
        return;
    }
    let map_id = scenario.map_id();

    let mut rows: Vec<(usize, String, &'static str)> = boxes
        .iter()
        .map(|(t, dims)| {

            let (ry, rx, rz) = t.rotation.to_euler(EulerRot::YXZ);

            let size = dims.size * t.scale;
            let core = format!(
                "({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                dims.shape.as_ron(),
                ron_f32(size.x),
                ron_f32(size.y),
                ron_f32(size.z),
                ron_f32(t.translation.x),
                ron_f32(t.translation.y),
                ron_f32(t.translation.z),
                ron_f32(rx.to_degrees()),
                ron_f32(ry.to_degrees()),
                ron_f32(rz.to_degrees()),
            );
            let label = if dims.index == 0 {
                "floor"
            } else if dims.shape == ArenaShape::Cylinder {
                "pillar"
            } else if t.rotation.angle_between(Quat::IDENTITY) > 1.0e-3 {
                "ramp"
            } else {
                "wall"
            };
            (dims.index, core, label)
        })
        .collect();
    rows.sort_by_key(|(i, _, _)| *i);

    let path = format!(
        "{}/assets/data/arena_layout_map{map_id}.ron",
        env!("CARGO_MANIFEST_DIR")
    );

    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            error!("F9 save aborted: can't read {path}: {e}");
            toast.err(format!("F9: can't read arena RON — {e}"));
            return;
        }
    };
    let header: String = existing
        .lines()
        .take_while(|l| l.trim() != "[")
        .map(|l| format!("{l}\n"))
        .collect();

    let width = rows.iter().map(|(_, core, _)| core.len()).max().unwrap_or(0);
    let mut out = header;
    out.push_str("[\n");
    for (_, core, label) in &rows {
        out.push_str(&format!("    {:<width$} // {label}\n", format!("{core},")));
    }
    out.push_str("]\n");

    match std::fs::write(&path, out) {
        Ok(()) => {
            info!("saved {} arena boxes -> {path}", rows.len());
            toast.ok(format!("F9: saved {} arena boxes (map {map_id})", rows.len()));
        }
        Err(e) => {
            error!("F9 save failed writing {path}: {e}");
            toast.err(format!("F9: arena save FAILED — {e}"));
        }
    }
}
