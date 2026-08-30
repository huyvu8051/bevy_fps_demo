// --- MapKind::Arena geometry: F9 save back to the per-map-id RON --------
//
// The flat arena (floor + walls + pillars + centre ramp) is spawned from
// crates/client/assets/data/arena_layout_map<ID>.ron by spawn_test_arena
// (crates/client/src/lib.rs), one file per Arena map id (2 = StressTest,
// 4 = LegacyArena). This module is the write-back half: with EDITOR=1 on
// one of those maps, drag / rotate / scale a wall or the ramp in the dev
// editor (F2) and press F9 — every ArenaBox's live Transform + size is
// written back to that map's own RON, in the original row order.
//
// Deliberately a near-copy of item_gallery_cars.rs's F9 saver (same
// EulerRot::YXZ split, same header-preservation, same F9SaveToast) rather
// than a shared abstraction — the two data files have different row
// shapes and the duplication is small and readable.

use bevy::prelude::*;
use shared::{ArenaShape, MapKind};

use crate::item_gallery_cars::{ron_f32, F9SaveToast};
use crate::{current_scenario, ArenaBoxDims};

pub(crate) struct ArenaLayoutPlugin;

impl Plugin for ArenaLayoutPlugin {
    fn build(&self, app: &mut App) {
        // Gated on EDITOR=1 exactly like ItemGalleryCarsPlugin's own F9
        // saver — F9SaveToast / spawn_f9_toast_ui / tick_f9_toast are only
        // registered there under the same guard, so this system must not
        // exist without them (it reads ResMut<F9SaveToast>). Also wasm32-
        // gated for the same reason as that plugin — see its own comment.
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var("EDITOR").is_ok() {
            app.add_systems(Update, save_arena_layout_on_key);
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
    if scenario.map_kind() != MapKind::Arena {
        return; // F9 on a non-arena map is the cars'/zombie's to handle
    }
    let map_id = scenario.map_id();

    // (row index, "(shape, ...9 numbers...)", trailing-comment label)
    let mut rows: Vec<(usize, String, &'static str)> = boxes
        .iter()
        .map(|(t, dims)| {
            // Same EulerRot::YXZ split spawn_test_arena / shared::arena_layout
            // rebuild from — returns (y_angle, x_angle, z_angle).
            let (ry, rx, rz) = t.rotation.to_euler(EulerRot::YXZ);
            // Fold the editor's scale gizmo into the saved size so it
            // round-trips (spawn rebuilds every piece at scale 1).
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

    // Preserve the header: keep every line up to and including the lone `[`.
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
