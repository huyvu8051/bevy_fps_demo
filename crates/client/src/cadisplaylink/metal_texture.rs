//! Plain Metal texture upload — zero dependency on this project's own
//! gameplay/asset types (no `bevy`, no `shared`, no `gltf`), so this is
//! another piece pulled out on the way toward a genuinely reusable "native
//! Metal renderer for Bevy on iOS" plugin (see native_metal_renderer.rs's own
//! module doc comment for the wider context this experiment exists in;
//! metal_math.rs was the first piece split out this same way).

use metal::{Device, MTLPixelFormat, MTLRegion};

/// Uploads `rgba` (tightly-packed RGBA8, `width * height * 4` bytes) as a
/// new shader-readable `Device`-local `Texture` — used for every glTF
/// base-color texture this renderer draws (baked static meshes, skinned
/// zombie/soldier models).
pub(crate) fn upload_texture(device: &Device, width: u32, height: u32, rgba: &[u8]) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
    descriptor.set_width(width as u64);
    descriptor.set_height(height as u64);
    descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
    descriptor.set_usage(metal::MTLTextureUsage::ShaderRead);
    let texture = device.new_texture(&descriptor);
    let region = MTLRegion::new_2d(0, 0, width as u64, height as u64);
    texture.replace_region(region, 0, rgba.as_ptr() as *const _, (width * 4) as u64);
    texture
}
