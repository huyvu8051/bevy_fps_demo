
use metal::{Device, MTLPixelFormat, MTLRegion};

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
