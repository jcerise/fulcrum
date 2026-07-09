//! GPU textures: decoding, upload, and the game-facing [`AssetLoader`].

use bevy_ecs::prelude::{Res, ResMut};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};

use crate::gpu::GpuContext;

/// Path key under which the shared error-placeholder texture is stored.
const PLACEHOLDER_PATH: &str = "<placeholder>";

/// A GPU texture with its default view. Sampling is `Nearest` by default (set by the sprite
/// pipeline) — this is a pixel-art-friendly engine.
pub struct Texture {
    /// The GPU texture.
    pub texture: wgpu::Texture,
    /// Default full view of the texture.
    pub view: wgpu::TextureView,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Decode encoded image bytes (PNG) into tightly-packed RGBA8 pixels. Pure function, no GPU —
/// unit-testable.
pub fn decode_rgba(path: &str, bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), AssetError> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| AssetError::Decode {
            path: path.to_string(),
            message: e.to_string(),
        })?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}

/// Upload RGBA8 pixels as a wgpu texture.
pub(crate) fn upload_rgba(
    gpu: &GpuContext,
    label: &str,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Texture {
    upload_raw(&gpu.device, &gpu.queue, label, pixels, width, height)
}

/// Upload RGBA8 pixels given raw device/queue (usable before `GpuContext` is a resource).
pub(crate) fn upload_raw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Texture {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Texture {
        texture,
        view,
        width,
        height,
    }
}

/// Load a texture from the asset root, deduplicating by path. A missing or undecodable file
/// logs an error and returns the shared 2×2 magenta placeholder — asset loading never panics.
pub fn load_texture(
    server: &AssetServer,
    textures: &mut Assets<Texture>,
    gpu: &GpuContext,
    path: &str,
) -> Handle<Texture> {
    if let Some(handle) = textures.handle_for_path(path) {
        return handle;
    }
    match server
        .read_bytes(path)
        .and_then(|bytes| decode_rgba(path, &bytes))
    {
        Ok((pixels, width, height)) => {
            let texture = upload_rgba(gpu, path, &pixels, width, height);
            textures.insert_with_path(path, texture)
        }
        Err(error) => {
            log::error!("{error}; using placeholder texture");
            placeholder(textures, gpu)
        }
    }
}

/// The shared magenta placeholder, created on first use.
fn placeholder(textures: &mut Assets<Texture>, gpu: &GpuContext) -> Handle<Texture> {
    if let Some(handle) = textures.handle_for_path(PLACEHOLDER_PATH) {
        return handle;
    }
    // 2×2 opaque magenta.
    let pixels: [u8; 16] = [
        255, 0, 255, 255, 255, 0, 255, 255, //
        255, 0, 255, 255, 255, 0, 255, 255,
    ];
    let texture = upload_rgba(gpu, PLACEHOLDER_PATH, &pixels, 2, 2);
    textures.insert_with_path(PLACEHOLDER_PATH, texture)
}

/// One-line asset loading for game systems:
///
/// ```ignore
/// fn setup(mut commands: Commands, mut assets: AssetLoader) {
///     let ship = assets.load("ship.png");
///     // spawn a sprite with `ship` ...
/// }
/// ```
#[derive(SystemParam)]
pub struct AssetLoader<'w> {
    server: Res<'w, AssetServer>,
    textures: ResMut<'w, Assets<Texture>>,
    sheets: ResMut<'w, Assets<crate::atlas::SpriteSheet>>,
    gpu: Res<'w, GpuContext>,
}

impl AssetLoader<'_> {
    /// Load a texture by path (relative to the asset root), deduplicated: the same path always
    /// yields the same handle.
    pub fn load(&mut self, path: &str) -> Handle<Texture> {
        load_texture(&self.server, &mut self.textures, &self.gpu, path)
    }

    /// Register a sprite sheet (e.g. built with `SpriteSheet::from_grid`) and get its handle.
    pub fn add_sheet(
        &mut self,
        sheet: crate::atlas::SpriteSheet,
    ) -> Handle<crate::atlas::SpriteSheet> {
        self.sheets.insert(sheet)
    }
}
