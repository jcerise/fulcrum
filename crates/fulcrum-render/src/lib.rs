//! Fulcrum rendering: owns the winit window/event-loop runner, the wgpu device and surface, and
//! (as the engine grows) the sprite-batch renderer with fixed-timestep render interpolation.

pub mod batch;
pub mod camera;
pub mod gpu;
pub mod sprite;
pub mod texture;
pub mod window;

pub use batch::RenderStats;
pub use camera::{Camera2D, ScalingMode};
pub use gpu::GpuContext;
pub use sprite::Sprite;
pub use texture::{AssetLoader, Texture};
pub use window::{WindowInfo, WindowPlugin};
