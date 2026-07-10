//! Fulcrum rendering: owns the winit window/event-loop runner, the wgpu device and surface, and
//! (as the engine grows) the sprite-batch renderer with fixed-timestep render interpolation.

pub mod atlas;
pub mod batch;
pub mod camera;
pub mod gizmos;
pub mod gpu;
pub mod particles;
pub mod reload;
pub mod sprite;
pub mod text;
pub mod texture;
pub mod tilemap;
pub mod window;

pub use atlas::SpriteSheet;
pub use batch::{ExtractedSprite, RenderStats, UiQuads};
pub use camera::{Camera2D, ScalingMode};
pub use gizmos::Gizmos;
pub use gpu::{GpuContext, RenderOverlay};
pub use particles::{EffectLoader, EmitMode, ParticleEffectAsset, ParticleEmitter, SpawnEffectExt};
pub use sprite::{Sprite, SpriteRegion};
pub use text::{DefaultFont, Font, GlyphCache, HAlign, Text, UiGlyph};
pub use texture::{AssetLoader, Texture, WhitePixel};
pub use tilemap::{TileLayer, Tilemap, TilemapAsset, TilemapLoader, load_tilemap_data};
pub use window::{WindowEventHooks, WindowHandle, WindowInfo, WindowPlugin};
