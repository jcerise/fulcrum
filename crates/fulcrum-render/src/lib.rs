//! Fulcrum rendering: owns the winit window/event-loop runner, the wgpu device and surface, and
//! (as the engine grows) the sprite-batch renderer with fixed-timestep render interpolation.

pub mod atlas;
pub mod batch;
pub mod camera;
pub mod gizmos;
pub mod gpu;
pub mod sprite;
pub mod texture;
pub mod window;

pub use atlas::SpriteSheet;
pub use batch::RenderStats;
pub use camera::{Camera2D, ScalingMode};
pub use gizmos::Gizmos;
pub use gpu::GpuContext;
pub use sprite::{Sprite, SpriteRegion};
pub use texture::{AssetLoader, Texture};
pub use window::{WindowInfo, WindowPlugin};
