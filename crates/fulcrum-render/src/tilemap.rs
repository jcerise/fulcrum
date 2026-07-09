//! Tilemaps: layered grids of sprite-sheet tiles with chunked, cached, camera-culled rendering.
//!
//! Conventions: tile `(0, 0)` is the **bottom-left** of the map (matching +Y-up world space);
//! the map entity's `Transform2D::translation` is the world position of that tile's min corner
//! (tilemap transforms are translation-only — rotation/scale are ignored). In the RON file,
//! layer rows are written top-to-bottom as you'd see them, and converted on load.

use bevy_ecs::prelude::{Component, Query, Res, ResMut};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{FulcrumConfig, FxHashMap, Rect, Transform2D, Vec2, vec2};
use serde::Deserialize;

use crate::atlas::SpriteSheet;
use crate::batch::{ExtraQuads, ExtractedSprite, RenderStats};
use crate::camera::Camera2D;
use crate::texture::{Texture, load_texture};
use crate::window::WindowInfo;

/// Tiles per chunk edge: meshes rebuild and cull at this granularity.
const CHUNK: u32 = 32;

/// On-disk format (`*.map.ron`). `0` = empty; any other value indexes the sheet at `value - 1`.
#[derive(Deserialize)]
#[serde(rename = "Tilemap")]
struct TilemapDef {
    /// Path to the tile texture, relative to the asset root.
    texture: String,
    /// Tile size in pixels.
    tile_size: (f32, f32),
    /// Grid columns/rows in the tile texture.
    sheet_cols: u32,
    sheet_rows: u32,
    layers: Vec<LayerDef>,
}

#[derive(Deserialize)]
#[serde(rename = "Layer")]
struct LayerDef {
    name: String,
    /// Rows written top-to-bottom (as seen on screen); converted to bottom-up on load.
    tiles: Vec<Vec<u32>>,
}

/// One layer of tiles, row-major with `(0, 0)` at the bottom-left.
pub struct TileLayer {
    /// Layer name, used by [`TilemapAsset::tile_at`]/[`set_tile`](TilemapAsset::set_tile).
    pub name: String,
    /// `width * height` tile values; `0` = empty, else sheet region `value - 1`.
    pub tiles: Vec<u32>,
    /// Width in tiles.
    pub width: u32,
    /// Height in tiles.
    pub height: u32,
}

/// A pre-built tile quad in map-local coordinates (translation applied at extract time).
struct CachedQuad {
    corners: [Vec2; 4],
    uv: [[f32; 2]; 4],
}

#[derive(Default)]
struct ChunkCache {
    quads: Vec<CachedQuad>,
    built: bool,
}

/// A loaded tilemap: layers of tile data plus per-chunk cached meshes.
pub struct TilemapAsset {
    /// The sheet tiles index into.
    pub sheet: Handle<SpriteSheet>,
    /// Tile size in world units.
    pub tile_size: Vec2,
    /// The layers, drawn in order (later layers on top).
    pub layers: Vec<TileLayer>,
    /// Per-layer chunk mesh caches.
    chunks: Vec<FxHashMap<(u32, u32), ChunkCache>>,
    /// Rebuilds performed last extract (for the dirty-chunk acceptance check).
    rebuilt_last_frame: usize,
}

/// Draws a [`TilemapAsset`] at the entity's translation.
#[derive(Component)]
pub struct Tilemap {
    /// The map to draw.
    pub asset: Handle<TilemapAsset>,
    /// Draw order among sprites.
    pub z: f32,
}

fn parse_def(path: &str, bytes: &[u8]) -> Result<TilemapDef, AssetError> {
    let source = std::str::from_utf8(bytes).map_err(|_| AssetError::Decode {
        path: path.to_string(),
        message: "not UTF-8".into(),
    })?;
    ron::from_str(source).map_err(|error| AssetError::Decode {
        path: path.to_string(),
        message: error.to_string(),
    })
}

/// Build the runtime asset from a parsed definition (validates rectangular layers and flips
/// rows to bottom-up). Pure; unit-testable.
fn build_asset(
    path: &str,
    def: &TilemapDef,
    sheet: Handle<SpriteSheet>,
) -> Result<TilemapAsset, AssetError> {
    let mut layers = Vec::with_capacity(def.layers.len());
    for layer in &def.layers {
        let height = layer.tiles.len() as u32;
        let width = layer.tiles.first().map(|row| row.len()).unwrap_or(0) as u32;
        if layer.tiles.iter().any(|row| row.len() as u32 != width) {
            return Err(AssetError::Decode {
                path: path.to_string(),
                message: format!("layer `{}` rows have differing lengths", layer.name),
            });
        }
        // File rows are top-to-bottom; storage is bottom-up.
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for row in layer.tiles.iter().rev() {
            tiles.extend_from_slice(row);
        }
        layers.push(TileLayer {
            name: layer.name.clone(),
            tiles,
            width,
            height,
        });
    }
    let chunk_maps = layers.iter().map(|_| FxHashMap::default()).collect();
    Ok(TilemapAsset {
        sheet,
        tile_size: vec2(def.tile_size.0, def.tile_size.1),
        layers,
        chunks: chunk_maps,
        rebuilt_last_frame: 0,
    })
}

impl TilemapAsset {
    /// Build a map programmatically (e.g. procedural generation).
    pub fn new(sheet: Handle<SpriteSheet>, tile_size: Vec2, layers: Vec<TileLayer>) -> Self {
        let chunk_maps = layers.iter().map(|_| FxHashMap::default()).collect();
        Self {
            sheet,
            tile_size,
            layers,
            chunks: chunk_maps,
            rebuilt_last_frame: 0,
        }
    }

    fn layer_index(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|layer| layer.name == name)
    }

    /// The tile value at `(x, y)` in `layer` (bottom-left origin), or `None` if out of range.
    pub fn tile_at(&self, layer: &str, x: u32, y: u32) -> Option<u32> {
        let layer = &self.layers[self.layer_index(layer)?];
        if x >= layer.width || y >= layer.height {
            return None;
        }
        Some(layer.tiles[(y * layer.width + x) as usize])
    }

    /// Set the tile at `(x, y)`; the containing chunk re-meshes on the next frame.
    pub fn set_tile(&mut self, layer: &str, x: u32, y: u32, tile: u32) {
        let Some(index) = self.layer_index(layer) else {
            log::error!("set_tile: no layer named `{layer}`");
            return;
        };
        let data = &mut self.layers[index];
        if x >= data.width || y >= data.height {
            log::error!("set_tile: ({x}, {y}) out of bounds for `{layer}`");
            return;
        }
        data.tiles[(y * data.width + x) as usize] = tile;
        if let Some(chunk) = self.chunks[index].get_mut(&(x / CHUNK, y / CHUNK)) {
            chunk.built = false;
        }
    }

    /// Which tile a world position lands on, given the map entity's transform.
    pub fn world_to_tile(&self, map_transform: &Transform2D, world: Vec2) -> Option<(u32, u32)> {
        let local = world - map_transform.translation;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let x = (local.x / self.tile_size.x) as u32;
        let y = (local.y / self.tile_size.y) as u32;
        let layer = self.layers.first()?;
        (x < layer.width && y < layer.height).then_some((x, y))
    }

    /// Rebuild one chunk's quads from tile data. `texture_size` normalizes UVs.
    fn rebuild_chunk(
        &mut self,
        layer_index: usize,
        chunk_x: u32,
        chunk_y: u32,
        sheet: &SpriteSheet,
        texture_size: Vec2,
    ) {
        let layer = &self.layers[layer_index];
        let mut quads = Vec::new();
        let x_end = (chunk_x * CHUNK + CHUNK).min(layer.width);
        let y_end = (chunk_y * CHUNK + CHUNK).min(layer.height);
        for y in (chunk_y * CHUNK)..y_end {
            for x in (chunk_x * CHUNK)..x_end {
                let value = layer.tiles[(y * layer.width + x) as usize];
                if value == 0 {
                    continue;
                }
                let Some(region) = sheet.regions.get(value as usize - 1) else {
                    continue;
                };
                let min = vec2(x as f32 * self.tile_size.x, y as f32 * self.tile_size.y);
                let max = min + self.tile_size;
                let (u0, u1) = (region.min.x / texture_size.x, region.max.x / texture_size.x);
                let (v0, v1) = (region.min.y / texture_size.y, region.max.y / texture_size.y);
                quads.push(CachedQuad {
                    corners: [
                        vec2(min.x, min.y),
                        vec2(max.x, min.y),
                        vec2(max.x, max.y),
                        vec2(min.x, max.y),
                    ],
                    uv: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                });
            }
        }
        self.chunks[layer_index].insert((chunk_x, chunk_y), ChunkCache { quads, built: true });
        self.rebuilt_last_frame += 1;
    }
}

/// `PreRender` system: push visible chunks' cached quads, rebuilding dirty ones.
#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
pub(crate) fn extract_tilemaps(
    maps: Query<(&Tilemap, &Transform2D)>,
    mut assets: ResMut<Assets<TilemapAsset>>,
    sheets: Res<Assets<SpriteSheet>>,
    textures: Res<Assets<Texture>>,
    camera: Res<Camera2D>,
    window: Res<WindowInfo>,
    mut extra: ResMut<ExtraQuads>,
    mut stats: ResMut<RenderStats>,
) {
    let window_size = vec2(window.width as f32, window.height as f32);
    let view = camera.visible_aabb(window_size);
    let mut visible_chunks = 0usize;

    for (map, transform) in &maps {
        let Some(asset) = assets.get_mut(map.asset) else {
            continue;
        };
        asset.rebuilt_last_frame = 0;
        let Some(sheet) = sheets.get(asset.sheet) else {
            continue;
        };
        let Some(texture) = textures.get(sheet.texture) else {
            continue;
        };
        let texture_size = vec2(texture.width as f32, texture.height as f32);
        let chunk_world = asset.tile_size * CHUNK as f32;

        // Visible rect in map-local space.
        let local = Rect::new(
            view.min - transform.translation,
            view.max - transform.translation,
        );

        for layer_index in 0..asset.layers.len() {
            let layer = &asset.layers[layer_index];
            let chunks_x = layer.width.div_ceil(CHUNK);
            let chunks_y = layer.height.div_ceil(CHUNK);
            let first_x = (local.min.x / chunk_world.x).floor().max(0.0) as u32;
            let first_y = (local.min.y / chunk_world.y).floor().max(0.0) as u32;
            let last_x =
                ((local.max.x / chunk_world.x).ceil() as i64).clamp(0, chunks_x as i64) as u32;
            let last_y =
                ((local.max.y / chunk_world.y).ceil() as i64).clamp(0, chunks_y as i64) as u32;

            for chunk_y in first_y..last_y {
                for chunk_x in first_x..last_x {
                    let needs_build = !asset.chunks[layer_index]
                        .get(&(chunk_x, chunk_y))
                        .is_some_and(|chunk| chunk.built);
                    if needs_build {
                        asset.rebuild_chunk(layer_index, chunk_x, chunk_y, sheet, texture_size);
                    }
                    let chunk = &asset.chunks[layer_index][&(chunk_x, chunk_y)];
                    visible_chunks += 1;
                    for quad in &chunk.quads {
                        extra.0.push(ExtractedSprite {
                            z: map.z,
                            texture: sheet.texture,
                            corners: quad.corners.map(|c| c + transform.translation),
                            uv: quad.uv,
                            color: [1.0, 1.0, 1.0, 1.0],
                        });
                    }
                }
            }
        }
    }
    stats.tilemap_chunks = visible_chunks;
}

/// One-line tilemap loading: `let map = maps.load("maps/level1.map.ron")?;`
#[derive(SystemParam)]
pub struct TilemapLoader<'w> {
    server: Res<'w, AssetServer>,
    textures: ResMut<'w, Assets<Texture>>,
    sheets: ResMut<'w, Assets<SpriteSheet>>,
    tilemaps: ResMut<'w, Assets<TilemapAsset>>,
    gpu: Res<'w, crate::gpu::GpuContext>,
    config: Res<'w, FulcrumConfig>,
}

impl TilemapLoader<'_> {
    /// Load a `.map.ron` tilemap (and its tile texture), deduplicated by path.
    pub fn load(&mut self, path: &str) -> Result<Handle<TilemapAsset>, AssetError> {
        let _ = &self.config; // reserved for future per-map settings
        if let Some(handle) = self.tilemaps.handle_for_path(path) {
            return Ok(handle);
        }
        let bytes = self.server.read_bytes(path)?;
        let def = parse_def(path, &bytes)?;
        let texture = load_texture(&self.server, &mut self.textures, &self.gpu, &def.texture);
        let sheet = self.sheets.insert(SpriteSheet::from_grid(
            texture,
            vec2(def.tile_size.0, def.tile_size.1),
            def.sheet_cols,
            def.sheet_rows,
        ));
        let asset = build_asset(path, &def, sheet)?;
        Ok(self.tilemaps.insert_with_path(path, asset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = r#"Tilemap(
        texture: "tiles.png",
        tile_size: (16.0, 16.0),
        sheet_cols: 2,
        sheet_rows: 2,
        layers: [
            Layer(name: "ground", tiles: [
                [3, 3, 3],
                [1, 0, 2],
            ]),
        ],
    )"#;

    fn asset() -> TilemapAsset {
        let def = parse_def("test.map.ron", MAP.as_bytes()).unwrap();
        build_asset("test.map.ron", &def, Handle::INVALID).unwrap()
    }

    #[test]
    fn parses_and_flips_rows_bottom_up() {
        let map = asset();
        assert_eq!(map.layers[0].width, 3);
        assert_eq!(map.layers[0].height, 2);
        // Bottom row of the file is y = 0.
        assert_eq!(map.tile_at("ground", 0, 0), Some(1));
        assert_eq!(map.tile_at("ground", 1, 0), Some(0));
        assert_eq!(map.tile_at("ground", 2, 0), Some(2));
        assert_eq!(map.tile_at("ground", 0, 1), Some(3));
        assert_eq!(map.tile_at("ground", 3, 0), None, "out of range");
        assert_eq!(map.tile_at("nope", 0, 0), None, "unknown layer");
    }

    #[test]
    fn set_tile_updates_and_dirties() {
        let mut map = asset();
        // Build the chunk first so there's something to dirty.
        let sheet = SpriteSheet::from_grid(Handle::INVALID, vec2(16.0, 16.0), 2, 2);
        map.rebuild_chunk(0, 0, 0, &sheet, vec2(32.0, 32.0));
        assert!(map.chunks[0][&(0, 0)].built);
        map.set_tile("ground", 1, 0, 4);
        assert_eq!(map.tile_at("ground", 1, 0), Some(4));
        assert!(!map.chunks[0][&(0, 0)].built, "chunk marked dirty");
    }

    #[test]
    fn world_to_tile_respects_map_transform() {
        let map = asset();
        let transform = Transform2D::from_xy(100.0, 200.0);
        assert_eq!(
            map.world_to_tile(&transform, vec2(100.0, 200.0)),
            Some((0, 0))
        );
        assert_eq!(
            map.world_to_tile(&transform, vec2(147.9, 216.5)),
            Some((2, 1))
        );
        assert_eq!(
            map.world_to_tile(&transform, vec2(99.0, 200.0)),
            None,
            "left of map"
        );
        assert_eq!(
            map.world_to_tile(&transform, vec2(150.0, 240.0)),
            None,
            "above map"
        );
    }

    #[test]
    fn chunk_meshes_skip_empty_tiles_and_map_uvs() {
        let mut map = asset();
        let sheet = SpriteSheet::from_grid(Handle::INVALID, vec2(16.0, 16.0), 2, 2);
        map.rebuild_chunk(0, 0, 0, &sheet, vec2(32.0, 32.0));
        let chunk = &map.chunks[0][&(0, 0)];
        assert_eq!(chunk.quads.len(), 5, "6 tiles minus 1 empty");
        // Tile value 1 -> region 0 -> UVs in the top-left quadrant of a 2x2 sheet.
        let first = &chunk.quads[0];
        assert_eq!(first.corners[0], vec2(0.0, 0.0));
        assert_eq!(
            first.uv[3],
            [0.0, 0.0],
            "top-left corner samples region top-left"
        );
        assert_eq!(
            first.uv[1],
            [0.5, 0.5],
            "bottom-right samples region bottom-right"
        );
    }
}
