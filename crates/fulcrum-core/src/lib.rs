//! Fulcrum engine core: the [`Fulcrum`] app builder, [`Plugin`] trait, schedules, and (as the
//! engine grows) time, input, and the deterministic simulation RNG.
//!
//! Windowless by design — rendering lives in `fulcrum-render`, and games consume everything
//! through the `fulcrum` facade crate's prelude. This crate also re-exports the ECS vocabulary
//! (from `bevy_ecs`) and math types (from `glam`) so that no other Fulcrum crate or game needs
//! to name those dependencies directly.

pub mod app;
pub mod input;
pub mod math;
pub mod plugin;
pub mod rng;
pub mod schedule;
pub mod time;
pub mod transform;

pub use app::{DEFAULT_SEED, Fulcrum, FulcrumConfig};
pub use input::{Input, Key, MouseButton};
pub use math::Rect;
pub use plugin::Plugin;
pub use rng::SimRng;
pub use schedule::{FixedUpdate, PreRender, Startup, Update};
pub use time::Time;
pub use transform::{PreviousTransform2D, Transform2D};

// ECS vocabulary, re-exported so games and engine crates never import `bevy_ecs`.
// Buffered events are called "messages" in bevy_ecs 0.19; Fulcrum keeps the classic
// event vocabulary.
pub use bevy_ecs::prelude::{
    Added, Bundle, Changed, Commands, Component, Entity, IntoScheduleConfigs, Local,
    Message as Event, MessageReader as EventReader, MessageWriter as EventWriter,
    Messages as Events, Or, ParamSet, Query, Res, ResMut, Resource, With, Without,
};
pub use bevy_ecs::schedule::ScheduleLabel;
pub use bevy_ecs::system::ScheduleSystem;
pub use bevy_ecs::world::World;

// Math vocabulary. 1 world unit = 1 pixel, +Y up.
pub use glam::{Vec2, vec2};

// The blessed hash map types: deterministic iteration order for identical insertion sequences,
// unlike std's RandomState maps. Sim systems must use these (see docs/determinism.md).
pub use rustc_hash::{FxHashMap, FxHashSet};

/// An RGBA color with `f32` components in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component.
    pub r: f32,
    /// Green component.
    pub g: f32,
    /// Blue component.
    pub b: f32,
    /// Alpha component (0.0 = fully transparent).
    pub a: f32,
}

impl Color {
    /// Opaque color from red/green/blue components.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Color from red/green/blue/alpha components.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque white.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Opaque red.
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    /// Opaque green.
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    /// Opaque blue.
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
}
