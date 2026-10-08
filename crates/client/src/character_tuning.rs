
use bevy::prelude::*;
use crate::*;

#[derive(serde::Deserialize, Clone)]
pub(crate) struct CharacterAnimIndices {
    pub(crate) idle: usize,
    pub(crate) walk: usize,
    pub(crate) run: usize,
    pub(crate) shoot: usize,
    pub(crate) death: usize,

    #[serde(default)]
    pub(crate) run_back: usize,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct CharacterRon {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lore_ref: String,
    pub(crate) mesh: String,

    #[serde(default)]
    pub(crate) anim_source: String,
    pub(crate) scale: f32,
    pub(crate) y_offset: f32,
    pub(crate) facing_deg: f32,
    pub(crate) animations: CharacterAnimIndices,
    pub(crate) audio: Vec<String>,
}

pub(crate) const AVAILABLE_CHARACTER_DIRS: &[&str] = &["characters/soldier_meshy"];

fn load_character_ron_by_dir(dir: &str) -> CharacterRon {
    #[cfg(target_os = "ios")]
    let text: String = {
        let resource_path = crate::cadisplaylink::native_metal_renderer::bundle_resource_path()
            .unwrap_or_else(|| panic!("failed to read {dir}/character.ron: NSBundle.main.resourcePath was nil"));
        let path = std::path::Path::new(&resource_path).join("assets").join(dir).join("character.ron");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {dir}/character.ron at {path:?}: {e}"))
    };
    #[cfg(not(target_os = "ios"))]
    let text: String = std::fs::read_to_string(format!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/{}/character.ron"), dir))
        .unwrap_or_else(|e| panic!("failed to read {dir}/character.ron: {e}"));
    ron::de::from_str(&text).unwrap_or_else(|e| panic!("failed to parse {dir}/character.ron: {e}"))
}

pub(crate) fn character_dir_from_mesh_path(mesh_path: &str) -> String {
    mesh_path.rsplit_once("/mesh/").map(|(dir, _)| dir.to_string()).unwrap_or_default()
}

pub(crate) fn format_character_ron(c: &CharacterRon) -> String {
    format!(
        "// Metadata for the \"{id}\" character — see docs/CHARACTER_IMPORT_CHECKLIST.md's\n\
         // \"Scale and origin\" section for why this can't just trust the mesh's native\n\
         // scale/pivot.\n\
         //\n\
         // scale/y_offset/facing_deg are F9-tunable (EDITOR=1): third-person (KeyU),\n\
         // Comma/Period nudges scale, Minus/Equal nudges y_offset — see\n\
         // nudge_player_model_tuning/save_player_model_tuning\n\
         // (crates/client/src/movement_combat.rs). `animations` isn't nudge-able,\n\
         // only hand-edited.\n\
         (\n\
         \u{20}   id: \"{id}\",\n\
         \u{20}   display_name: \"{display_name}\",\n\
         \u{20}   lore_ref: \"{lore_ref}\",\n\
         \n\
         \u{20}   mesh: \"{mesh}\",\n\
         \u{20}   // See docs/CHARACTER_IMPORT_CHECKLIST.md before replacing this mesh.\n\
         \u{20}   anim_source: \"{anim_source}\",\n\
         \u{20}   scale: {scale},\n\
         \u{20}   y_offset: {y_offset},\n\
         \u{20}   facing_deg: {facing_deg},\n\
         \n\
         \u{20}   // Indices into anim_source, not mesh.\n\
         \u{20}   animations: (\n\
         \u{20}       idle: {idle},\n\
         \u{20}       walk: {walk},\n\
         \u{20}       run: {run},\n\
         \u{20}       run_back: {run_back},\n\
         \u{20}       shoot: {shoot},\n\
         \u{20}       death: {death},\n\
         \u{20}   ),\n\
         \n\
         \u{20}   audio: [{audio}],\n\
         )\n",
        id = c.id,
        display_name = c.display_name,
        lore_ref = c.lore_ref,
        mesh = c.mesh,
        anim_source = c.anim_source,
        scale = ron_f32(c.scale),
        y_offset = ron_f32(c.y_offset),
        facing_deg = ron_f32(c.facing_deg),
        idle = c.animations.idle,
        walk = c.animations.walk,
        run = c.animations.run,
        run_back = c.animations.run_back,
        shoot = c.animations.shoot,
        death = c.animations.death,
        audio = c.audio.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", "),
    )
}

#[derive(Resource, Clone)]
pub(crate) struct PlayerModelTuning {
    pub(crate) scale: f32,
    pub(crate) y_offset: f32,
    pub(crate) facing_deg: f32,
    pub(crate) animations: CharacterAnimIndices,
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lore_ref: String,
    pub(crate) mesh: String,
    pub(crate) anim_source: String,
    pub(crate) audio: Vec<String>,
}

impl PlayerModelTuning {

    pub(crate) fn load(dir: &str) -> Self {
        let c = load_character_ron_by_dir(dir);
        Self {
            scale: c.scale,
            y_offset: c.y_offset,
            facing_deg: c.facing_deg,
            animations: c.animations,
            id: c.id,
            display_name: c.display_name,
            lore_ref: c.lore_ref,
            mesh: c.mesh,
            anim_source: c.anim_source,
            audio: c.audio,
        }
    }

    pub(crate) fn anim_source_or_mesh(&self) -> &str {
        if self.anim_source.is_empty() {
            &self.mesh
        } else {
            &self.anim_source
        }
    }
}

impl Default for PlayerModelTuning {
    fn default() -> Self {
        Self::load(AVAILABLE_CHARACTER_DIRS[0])
    }
}
