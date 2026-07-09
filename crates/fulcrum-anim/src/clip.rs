//! [`AnimationClip`]: a sequence of sprite-sheet frames with per-frame durations in ticks.

use fulcrum_asset::Handle;
use fulcrum_render::SpriteSheet;

/// An animation: sheet regions to show, and how many simulation ticks each lasts.
///
/// Durations are in **ticks, not seconds** — animation is simulation state (gameplay can key
/// off frames), so it advances deterministically in `FixedUpdate`.
pub struct AnimationClip {
    /// The sheet the frames come from.
    pub sheet: Handle<SpriteSheet>,
    /// Region indices to display, in order.
    pub frames: Vec<u32>,
    /// Duration of each frame in simulation ticks (parallel to `frames`, min 1).
    pub frame_ticks: Vec<u32>,
    /// Loop forever, or stop on the last frame.
    pub looping: bool,
}

impl AnimationClip {
    /// A clip with uniform frame timing expressed as frames-per-second, converted to ticks for
    /// the given simulation `tick_rate`.
    pub fn from_fps(
        sheet: Handle<SpriteSheet>,
        frames: Vec<u32>,
        fps: f32,
        looping: bool,
        tick_rate: u32,
    ) -> Self {
        let ticks = ((tick_rate as f32 / fps).round() as u32).max(1);
        let frame_ticks = vec![ticks; frames.len()];
        Self {
            sheet,
            frames,
            frame_ticks,
            looping,
        }
    }
}
