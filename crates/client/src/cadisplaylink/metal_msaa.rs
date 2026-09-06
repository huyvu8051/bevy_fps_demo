//! Resize-on-demand MSAA/depth render targets — zero dependency on this
//! project's own gameplay/asset types (no `bevy`, no `shared`), so this is
//! another piece pulled out on the way toward a genuinely reusable "native
//! Metal renderer for Bevy on iOS" plugin (see native_metal_renderer.rs's own
//! module doc comment for the wider context this experiment exists in;
//! metal_math.rs/metal_texture.rs were split out the same way).
//!
//! Owns the 3 render targets a single-pass MSAA-toggle renderer needs: a
//! multisampled depth buffer, its sample-count-1 twin (used when MSAA is
//! off), and the multisampled color buffer the main pass resolves into the
//! drawable. All 3 are cached and only rebuilt when the requested size
//! actually changes (device rotation, window resize on macOS Catalyst) —
//! rebuilding a Metal texture every frame regardless would be wasted GPU
//! allocation churn for a value that's almost always identical frame to
//! frame.

use metal::{Device, MTLPixelFormat};

pub(crate) struct MsaaTargets {
    sample_count: u64,
    depth: Option<metal::Texture>,
    depth_no_msaa: Option<metal::Texture>,
    color: Option<metal::Texture>,
}

impl MsaaTargets {
    pub(crate) fn new(sample_count: u64) -> Self {
        Self { sample_count, depth: None, depth_no_msaa: None, color: None }
    }

    // Multisampled — must match the main pass's color attachment sample
    // count exactly, or Metal rejects the pass.
    pub(crate) fn ensure_depth(&mut self, device: &Device, width: u64, height: u64) -> metal::Texture {
        if let Some(tex) = &self.depth {
            if tex.width() == width && tex.height() == height {
                return tex.clone();
            }
        }
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_texture_type(metal::MTLTextureType::D2Multisample);
        descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_sample_count(self.sample_count);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let tex = device.new_texture(&descriptor);
        self.depth = Some(tex.clone());
        tex
    }

    // The main pass's real render target — multisampled, resolved into the
    // drawable's own (single-sample) texture at the end of the pass. Same
    // resize-on-demand caching as `ensure_depth`.
    pub(crate) fn ensure_color(&mut self, device: &Device, width: u64, height: u64) -> metal::Texture {
        if let Some(tex) = &self.color {
            if tex.width() == width && tex.height() == height {
                return tex.clone();
            }
        }
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_texture_type(metal::MTLTextureType::D2Multisample);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_sample_count(self.sample_count);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let tex = device.new_texture(&descriptor);
        self.color = Some(tex.clone());
        tex
    }

    // Sample-count-1 twin of `ensure_depth`, used when MSAA is off (the
    // main pass then renders straight into the drawable, no color/resolve
    // step at all).
    pub(crate) fn ensure_depth_no_msaa(&mut self, device: &Device, width: u64, height: u64) -> metal::Texture {
        if let Some(tex) = &self.depth_no_msaa {
            if tex.width() == width && tex.height() == height {
                return tex.clone();
            }
        }
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let tex = device.new_texture(&descriptor);
        self.depth_no_msaa = Some(tex.clone());
        tex
    }
}
