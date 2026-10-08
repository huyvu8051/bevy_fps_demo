
#[cfg(target_os = "ios")]
pub fn ios_bundle_resource_path() -> Option<String> {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    unsafe {
        let bundle_class = class!(NSBundle);
        let bundle: *mut Object = msg_send![bundle_class, mainBundle];
        let path: *mut Object = msg_send![bundle, resourcePath];
        if path.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

#[macro_export]
macro_rules! read_ron_data_file {
    ($file:expr) => {{
        #[cfg(target_os = "ios")]
        {
            let resource_path = $crate::ios_bundle_resource_path()
                .unwrap_or_else(|| panic!("failed to read RON data file {}: NSBundle.main.resourcePath was nil", $file));
            let path = std::path::Path::new(&resource_path).join("assets/data").join($file);
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read RON data file {path:?}: {e}"))
        }
        #[cfg(not(target_os = "ios"))]
        {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/data/", $file);
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read RON data file {path}: {e}"))
        }
    }};
}

pub mod movement;
pub mod sim_latency;

pub mod arena;
pub mod arena_cars;
pub mod combat;
pub mod debug_bot;
pub mod game_state;
pub mod hitbox;
pub mod level_piece;
pub mod network;
pub mod player;
pub mod zombie;

pub use arena::*;
pub use arena_cars::*;
pub use combat::*;
pub use debug_bot::*;
pub use game_state::*;
pub use hitbox::*;
pub use level_piece::*;
pub use network::*;
pub use player::*;
pub use zombie::*;
