
use bevy::prelude::*;
use shared::MapKind;

use crate::item_gallery_cars::F9SaveToast;
use crate::{current_scenario, LevelPieceIndex, LevelPieceKind};

pub(crate) struct Map1LayoutPlugin;

impl Plugin for Map1LayoutPlugin {
    fn build(&self, app: &mut App) {

        if std::env::var("EDITOR").is_ok() {
            app.add_systems(Update, save_map1_layout_on_key.run_if(crate::debug_keys_allowed));
        }
    }
}

fn save_map1_layout_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    boxes: Query<(&Transform, &LevelPieceIndex, &shared::LevelBox)>,
    panels: Query<(&Transform, &LevelPieceIndex, &shared::LevelPanel)>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    if current_scenario().map_kind() != MapKind::Campaign(1) {
        return;
    }

    let mut box_rows: Vec<(usize, shared::BoxPiece)> = boxes
        .iter()
        .filter(|(_, index, _)| index.kind == LevelPieceKind::Box)
        .map(|(t, index, level_box)| {

            let piece: shared::BoxPiece = (t.translation, t.rotation, level_box.size * t.scale, level_box.color, level_box.roughness);
            (index.index, piece)
        })
        .collect();
    box_rows.sort_by_key(|(i, _)| *i);
    let box_pieces: Vec<shared::BoxPiece> = box_rows.into_iter().map(|(_, p)| p).collect();

    let mut panel_rows: Vec<(usize, shared::PanelPiece)> = panels
        .iter()
        .filter(|(_, index, _)| index.kind == LevelPieceKind::Panel)
        .map(|(t, index, panel)| {

            let translation = if panel.collider_offset {
                let thickness = panel.collider_thickness.unwrap_or(0.0);
                t.translation - t.rotation * Vec3::new(0.0, 0.0, thickness / 2.0)
            } else {
                t.translation
            };
            let piece: shared::PanelPiece =
                (translation, t.rotation, panel.width, panel.height, panel.color, panel.roughness, panel.collider_thickness, panel.collider_offset);
            (index.index, piece)
        })
        .collect();
    panel_rows.sort_by_key(|(i, _)| *i);
    let panel_pieces: Vec<shared::PanelPiece> = panel_rows.into_iter().map(|(_, p)| p).collect();

    let path = format!("{}/assets/data/map/1.ron", env!("CARGO_MANIFEST_DIR"));
    let text = shared::map1::map1_ron_text(&(box_pieces.clone(), panel_pieces.clone()));
    match std::fs::write(&path, text) {
        Ok(()) => {
            let count = box_pieces.len() + panel_pieces.len();
            info!("saved {count} map1 pieces -> {path}");
            toast.ok(format!("F9: saved {count} map1 pieces"));
        }
        Err(e) => {
            error!("F9 save failed writing {path}: {e}");
            toast.err(format!("F9: map1 save FAILED — {e}"));
        }
    }
}
