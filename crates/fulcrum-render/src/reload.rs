//! Hot-reload handlers for GPU-side assets: textures, fonts, and tilemaps.

use bevy_ecs::prelude::{Local, Res, ResMut};
use fulcrum_asset::{AssetEvent, AssetServer, AssetWatcher, Assets, Debounce};
use fulcrum_core::EventReader;
use fulcrum_core::EventWriter;

use crate::gpu::GpuContext;
use crate::text::{Font, GlyphCache};
use crate::texture::{Texture, decode_rgba, upload_rgba};
use crate::tilemap::TilemapAsset;

/// Drain the filesystem watcher into `AssetEvent`s (debounced).
pub(crate) fn pump_asset_events(
    watcher: Option<Res<AssetWatcher>>,
    mut events: EventWriter<AssetEvent>,
    mut debounce: Local<Debounce>,
) {
    let Some(watcher) = watcher else { return };
    for path in watcher.drain() {
        if debounce.allow(&path) {
            log::info!("asset changed: {path}");
            events.write(AssetEvent { path });
        }
    }
}

/// Reload changed textures (in place, same handles), fonts (clearing the glyph cache), and
/// tilemaps (re-parsed; chunk meshes rebuild lazily).
#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
pub(crate) fn reload_render_assets(
    mut events: EventReader<AssetEvent>,
    server: Res<AssetServer>,
    gpu: Res<GpuContext>,
    mut textures: ResMut<Assets<Texture>>,
    mut fonts: ResMut<Assets<Font>>,
    mut cache: ResMut<GlyphCache>,
    mut tilemaps: ResMut<Assets<TilemapAsset>>,
    mut renderer: Option<ResMut<crate::batch::SpriteRenderer>>,
) {
    for event in events.read() {
        let path = &event.path;
        if let Some(handle) = textures.handle_for_path(path) {
            match server
                .read_bytes(path)
                .and_then(|bytes| decode_rgba(path, &bytes))
            {
                Ok((pixels, width, height)) => {
                    let texture = upload_rgba(&gpu, path, &pixels, width, height);
                    textures.replace(handle, texture);
                    if let Some(renderer) = renderer.as_mut() {
                        renderer.invalidate_texture(handle.id());
                    }
                    log::info!("reloaded texture {path}");
                }
                Err(error) => log::error!("hot reload: {error}"),
            }
        }
        if let Some(handle) = fonts.handle_for_path(path) {
            match server
                .read_bytes(path)
                .and_then(|bytes| Font::from_bytes(path, &bytes))
            {
                Ok(font) => {
                    fonts.replace(handle, font);
                    cache.clear();
                    log::info!("reloaded font {path}");
                }
                Err(error) => log::error!("hot reload: {error}"),
            }
        }
        if tilemaps.handle_for_path(path).is_some() {
            crate::tilemap::reload_tilemap(&server, &mut tilemaps, path);
        }
    }
}
