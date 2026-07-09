//! [`SpriteSheet`]: named sub-regions of one texture, so many sprites batch into one draw call.

use fulcrum_asset::Handle;
use fulcrum_core::math::Rect;
use fulcrum_core::{FxHashMap, Vec2};

use crate::texture::Texture;

/// An asset describing rectangular regions within a texture. Regions are pixel rects in image
/// space: origin at the texture's top-left, +Y down.
pub struct SpriteSheet {
    /// The backing texture.
    pub texture: Handle<Texture>,
    /// Pixel regions, indexed by [`SpriteRegion::index`](crate::sprite::SpriteRegion).
    pub regions: Vec<Rect>,
    /// Optional names for regions (Aseprite import fills this in a later step).
    pub names: FxHashMap<String, u32>,
}

impl SpriteSheet {
    /// A uniform grid of `cols` x `rows` tiles of `tile` pixels each, indexed row-major from
    /// the top-left.
    pub fn from_grid(texture: Handle<Texture>, tile: Vec2, cols: u32, rows: u32) -> Self {
        let mut regions = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let min = Vec2::new(col as f32 * tile.x, row as f32 * tile.y);
                regions.push(Rect::from_min_size(min, tile));
            }
        }
        Self {
            texture,
            regions,
            names: FxHashMap::default(),
        }
    }

    /// Look up a region index by name.
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.names.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fulcrum_core::vec2;

    #[test]
    fn from_grid_is_row_major_from_top_left() {
        let sheet = SpriteSheet::from_grid(Handle::INVALID, vec2(16.0, 8.0), 3, 2);
        assert_eq!(sheet.regions.len(), 6);
        assert_eq!(
            sheet.regions[0],
            Rect::from_min_size(vec2(0.0, 0.0), vec2(16.0, 8.0))
        );
        assert_eq!(
            sheet.regions[2],
            Rect::from_min_size(vec2(32.0, 0.0), vec2(16.0, 8.0))
        );
        // First tile of the second row starts below the first row.
        assert_eq!(
            sheet.regions[3],
            Rect::from_min_size(vec2(0.0, 8.0), vec2(16.0, 8.0))
        );
    }
}
