//! Fulcrum's built-in schedules.
//!
//! Systems are grouped into a small, fixed set of schedules:
//!
//! - [`Startup`] runs exactly once, before the first tick.
//! - [`FixedUpdate`] is the deterministic simulation tick. All game-state mutation belongs here.
//!   Runs at [`FulcrumConfig::tick_rate`](crate::FulcrumConfig) regardless of frame rate.
//! - [`Update`] runs once per rendered frame, for cosmetic, non-simulation work (camera follow,
//!   visual effects, debug overlays). Systems here must never mutate simulation state.
//! - [`PreRender`] is internal: renderer extraction runs here after `Update`.

use bevy_ecs::schedule::ScheduleLabel;

/// Runs exactly once, before the first simulation tick. Spawn your initial entities here.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Startup;

/// The deterministic, fixed-timestep simulation schedule — the default target of
/// [`Fulcrum::add_system`](crate::Fulcrum::add_system).
///
/// Systems here may only use [`Time::fixed_delta`](crate::Time) and tick counts for timing —
/// never wall-clock time. See `docs/determinism.md`.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedUpdate;

/// Runs once per rendered frame, for cosmetic work only. Added via
/// [`Fulcrum::add_frame_system`](crate::Fulcrum::add_frame_system).
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Update;

/// Internal: renderer extraction. Runs after [`Update`], immediately before drawing.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreRender;
