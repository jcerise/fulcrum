//! The [`Time`] resource: fixed-timestep simulation time and per-frame render timing.

use bevy_ecs::prelude::Resource;

/// Engine time. Inserted automatically when the app is built.
///
/// # Determinism contract
///
/// Systems in [`FixedUpdate`](crate::FixedUpdate) may only use [`fixed_delta`](Self::fixed_delta)
/// and [`tick`](Self::tick). Reading [`frame_delta`](Self::frame_delta) or
/// [`alpha`](Self::alpha) from a simulation system breaks determinism — those fields exist for
/// cosmetic [`Update`](crate::Update) systems and the renderer.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Time {
    /// Duration of one simulation tick in seconds: `1.0 / tick_rate`. The ONLY delta-time
    /// simulation systems may use.
    pub fixed_delta: f32,
    /// Number of completed simulation ticks since startup.
    pub tick: u64,
    /// Wall-clock duration of the last rendered frame, in seconds. Cosmetic systems only.
    pub frame_delta: f32,
    /// Interpolation factor in `[0, 1)`: how far the render moment sits between the last two
    /// simulation ticks. Used by the renderer; cosmetic systems only.
    pub alpha: f32,
}

impl Time {
    /// Time state for a simulation running at `tick_rate` Hz.
    pub fn new(tick_rate: u32) -> Self {
        Self {
            fixed_delta: 1.0 / tick_rate as f32,
            tick: 0,
            frame_delta: 0.0,
            alpha: 0.0,
        }
    }
}
