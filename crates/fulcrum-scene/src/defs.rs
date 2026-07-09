//! Serde-friendly mirror components (`*Def`) for path-bearing built-ins, plus the resolver
//! systems that turn them into the real components once assets can load.
//!
//! Data files never contain handles — they name assets by path. A prefab's `"Sprite"` entry
//! deserializes to [`SpriteDef`]; a cosmetic resolver system then loads the referenced assets
//! and inserts the real [`Sprite`]. The def stays on the entity so extraction (scene saving,
//! inspector) is symmetric.

use bevy_ecs::prelude::{Commands, Component, Entity, Query, Without};
use fulcrum_anim::{AnimationPlayer, Animator, AnimatorLoader, AsepriteLoader};
use fulcrum_core::{Color, Vec2};
use fulcrum_render::{AssetLoader, HAlign, Sprite, Text, Tilemap, TilemapLoader};
use serde::{Deserialize, Serialize};

fn default_text_size() -> f32 {
    16.0
}

/// Data-file form of [`Sprite`]: assets by path. Either `texture` (whole image) or
/// `sheet` + `region` (Aseprite export + named frame).
#[derive(Component, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SpriteDef {
    /// Path to a plain texture.
    pub texture: Option<String>,
    /// Path to an Aseprite JSON export.
    pub sheet: Option<String>,
    /// Named region within `sheet` (an exported frame filename).
    pub region: Option<String>,
    /// Tint (defaults to white).
    pub color: Color,
    /// Size override in world units.
    pub size: Option<Vec2>,
    /// Mirror horizontally.
    pub flip_x: bool,
    /// Mirror vertically.
    pub flip_y: bool,
    /// Draw order.
    pub z: f32,
}

/// Data-file form of [`AnimationPlayer`]: `"file.json#tag"`.
#[derive(Component, Serialize, Deserialize, Clone, Default)]
pub struct AnimationPlayerDef {
    /// Clip reference: an Aseprite JSON path, `#`, and a tag name.
    pub clip: String,
}

/// Data-file form of [`Text`].
#[derive(Component, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TextDef {
    /// The text to show.
    pub value: String,
    /// Font path (empty = built-in pixel font).
    pub font: Option<String>,
    /// Size in pixels.
    pub size: f32,
    /// Color.
    pub color: Color,
    /// Per-line alignment.
    pub h_align: HAlign,
    /// Draw order.
    pub z: f32,
}

impl Default for TextDef {
    fn default() -> Self {
        Self {
            value: String::new(),
            font: None,
            size: default_text_size(),
            color: Color::WHITE,
            h_align: HAlign::Left,
            z: 0.0,
        }
    }
}

/// Data-file form of [`Animator`]: the state machine by path.
#[derive(Component, Serialize, Deserialize, Clone, Default)]
pub struct AnimatorDef {
    /// Path to a `.animsm.ron` state machine.
    pub machine: String,
}

/// Data-file form of [`Tilemap`]: the map asset by path.
#[derive(Component, Serialize, Deserialize, Clone, Default)]
pub struct TilemapDef {
    /// Path to a `.map.ron` file.
    pub asset: String,
    /// Draw order.
    #[serde(default)]
    pub z: f32,
}

impl SpriteDef {
    fn base_sprite(&self, texture: fulcrum_asset::Handle<fulcrum_render::Texture>) -> Sprite {
        let mut sprite = Sprite::new(texture).with_color(self.color).with_z(self.z);
        sprite.custom_size = self.size;
        sprite.flip_x = self.flip_x;
        sprite.flip_y = self.flip_y;
        sprite
    }
}

/// Resolve defs that only need plain assets (textures, fonts).
pub(crate) fn resolve_plain_defs(
    mut commands: Commands,
    sprites: Query<(Entity, &SpriteDef), Without<Sprite>>,
    texts: Query<(Entity, &TextDef), Without<Text>>,
    mut assets: AssetLoader,
) {
    for (entity, def) in &sprites {
        // Sheet-based sprites are handled by `resolve_aseprite_defs`.
        if def.sheet.is_some() {
            continue;
        }
        let Some(texture_path) = &def.texture else {
            log::error!("SpriteDef with neither `texture` nor `sheet`; skipping");
            continue;
        };
        let texture = assets.load(texture_path);
        let sprite = def.base_sprite(texture);
        commands.entity(entity).insert(sprite);
    }
    for (entity, def) in &texts {
        let mut text = Text::new(def.value.clone())
            .with_size(def.size)
            .with_color(def.color)
            .with_align(def.h_align)
            .with_z(def.z);
        if let Some(font_path) = &def.font {
            text.font = assets.load_font(font_path);
        }
        commands.entity(entity).insert(text);
    }
}

/// Resolve tilemap defs (separate system: `TilemapLoader` shares asset storage with
/// `AssetLoader`, and one system may not hold both).
pub(crate) fn resolve_tilemap_defs(
    mut commands: Commands,
    tilemaps: Query<(Entity, &TilemapDef), Without<Tilemap>>,
    mut maps: TilemapLoader,
) {
    for (entity, def) in &tilemaps {
        match maps.load(&def.asset) {
            Ok(asset) => {
                commands.entity(entity).insert(Tilemap { asset, z: def.z });
            }
            Err(error) => log::error!("{error}; tilemap not spawned"),
        }
    }
}

/// Resolve animator defs (own system: `AnimatorLoader` shares asset storage with
/// `AsepriteLoader`).
pub(crate) fn resolve_animator_defs(
    mut commands: Commands,
    animators: Query<(Entity, &AnimatorDef), Without<Animator>>,
    mut loader: AnimatorLoader,
) {
    for (entity, def) in &animators {
        match loader.load(&def.machine) {
            Ok(machine) => {
                commands.entity(entity).insert((
                    Animator::new(machine),
                    AnimationPlayer::play(fulcrum_asset::Handle::INVALID),
                ));
            }
            Err(error) => log::error!("{error}; animator not resolved"),
        }
    }
}

/// Resolve defs that need Aseprite imports (sheet sprites, animation players).
pub(crate) fn resolve_aseprite_defs(
    mut commands: Commands,
    sprites: Query<(Entity, &SpriteDef), Without<Sprite>>,
    players: Query<(Entity, &AnimationPlayerDef), Without<AnimationPlayer>>,
    mut aseprite: AsepriteLoader,
) {
    for (entity, def) in &sprites {
        let Some(sheet_path) = &def.sheet else {
            continue;
        };
        let import = match aseprite.load(sheet_path) {
            Ok(import) => import,
            Err(error) => {
                log::error!("{error}; sprite not resolved");
                continue;
            }
        };
        let index = match &def.region {
            Some(region) => match aseprite
                .sheets()
                .get(import.sheet)
                .and_then(|s| s.index_of(region))
            {
                Some(index) => index,
                None => {
                    log::error!(
                        "sheet `{sheet_path}` has no region `{}`; using frame 0",
                        def.region.as_deref().unwrap_or("")
                    );
                    0
                }
            },
            None => 0,
        };
        let mut sprite = Sprite::from_sheet(import.sheet, index);
        sprite.color = def.color;
        sprite.custom_size = def.size;
        sprite.flip_x = def.flip_x;
        sprite.flip_y = def.flip_y;
        sprite.z = def.z;
        commands.entity(entity).insert(sprite);
    }
    for (entity, def) in &players {
        let Some((file, tag)) = def.clip.split_once('#') else {
            log::error!("AnimationPlayer clip `{}` is not `file.json#tag`", def.clip);
            continue;
        };
        let import = match aseprite.load(file) {
            Ok(import) => import,
            Err(error) => {
                log::error!("{error}; animation not resolved");
                continue;
            }
        };
        let Some(&clip) = import.clips.get(tag) else {
            log::error!("`{file}` has no tag `{tag}`");
            continue;
        };
        commands.entity(entity).insert(AnimationPlayer::play(clip));
    }
}
