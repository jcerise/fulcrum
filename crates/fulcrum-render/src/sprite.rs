//! The [`Sprite`] component: what an entity looks like.

use bevy_ecs::prelude::Component;
use fulcrum_asset::Handle;
use fulcrum_core::{Color, Vec2};

use crate::atlas::SpriteSheet;
use crate::texture::Texture;

/// A sub-region of a [`SpriteSheet`] for a sprite to draw.
#[derive(Clone, Copy, Debug)]
pub struct SpriteRegion {
    /// The sheet holding the region.
    pub sheet: Handle<SpriteSheet>,
    /// Index into the sheet's region list.
    pub index: u32,
}

/// A textured quad drawn at the entity's [`Transform2D`](fulcrum_core::Transform2D).
///
/// Spawn an entity with both components and the engine renders it — batched, sorted by
/// [`z`](Self::z), and interpolated between simulation ticks.
#[derive(Component, Clone, Debug)]
pub struct Sprite {
    /// The texture to draw. Ignored when [`region`](Self::region) is set (the sheet's texture
    /// is used instead).
    pub texture: Handle<Texture>,
    /// Draw a sheet region instead of the whole `texture`.
    pub region: Option<SpriteRegion>,
    /// Tint multiplied with the texture (default white = untinted).
    pub color: Color,
    /// Size in world units. `None` = the texture's size in pixels.
    pub custom_size: Option<Vec2>,
    /// Which point of the sprite sits on the entity's translation, in `0..=1` of the sprite's
    /// size. `(0.5, 0.5)` (default) = centered; `(0.0, 0.0)` = bottom-left.
    pub anchor: Vec2,
    /// Mirror horizontally.
    pub flip_x: bool,
    /// Mirror vertically.
    pub flip_y: bool,
    /// Draw order: higher `z` draws in front.
    pub z: f32,
}

impl Sprite {
    /// An untinted, unflipped, centered sprite at `z = 0` drawn at texture size.
    pub fn new(texture: Handle<Texture>) -> Self {
        Self {
            texture,
            region: None,
            color: Color::WHITE,
            custom_size: None,
            anchor: Vec2::splat(0.5),
            flip_x: false,
            flip_y: false,
            z: 0.0,
        }
    }

    /// A sprite drawing region `index` of `sheet`, sized to the region by default.
    pub fn from_sheet(sheet: Handle<SpriteSheet>, index: u32) -> Self {
        Self {
            region: Some(SpriteRegion { sheet, index }),
            ..Self::new(Handle::INVALID)
        }
    }

    /// Builder-style tint.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Builder-style size override (world units).
    pub fn with_size(mut self, size: Vec2) -> Self {
        self.custom_size = Some(size);
        self
    }

    /// Builder-style draw order.
    pub fn with_z(mut self, z: f32) -> Self {
        self.z = z;
        self
    }
}
