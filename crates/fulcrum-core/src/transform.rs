//! [`Transform2D`]: position, rotation, and scale in 2D world space.
//!
//! Conventions (locked): 1 world unit = 1 pixel at default zoom, +Y up, origin at the window
//! center, rotation in radians counter-clockwise.

use bevy_ecs::prelude::{Component, Entity, Without};
use bevy_ecs::world::World;
use glam::Vec2;

/// Where an entity is, how it's rotated, and how it's scaled.
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Transform2D {
    /// World position in pixels.
    pub translation: Vec2,
    /// Rotation in radians, counter-clockwise.
    pub rotation: f32,
    /// Scale multiplier per axis (1.0 = native size).
    pub scale: Vec2,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    /// No translation, no rotation, scale 1.
    pub const IDENTITY: Self = Self {
        translation: Vec2::ZERO,
        rotation: 0.0,
        scale: Vec2::ONE,
    };

    /// A transform at `(x, y)` with no rotation and scale 1.
    pub fn from_xy(x: f32, y: f32) -> Self {
        Self {
            translation: Vec2::new(x, y),
            ..Self::IDENTITY
        }
    }

    /// A transform at `translation` with no rotation and scale 1.
    pub fn from_translation(translation: Vec2) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    /// Interpolate between two transforms: linear for translation and scale, shortest-arc for
    /// rotation. Used by the renderer with [`Time::alpha`](crate::Time).
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let mut delta = other.rotation - self.rotation;
        // Wrap the rotation difference into (-PI, PI] so interpolation takes the short way.
        delta =
            (delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        Self {
            translation: self.translation.lerp(other.translation, t),
            rotation: self.rotation + delta * t,
            scale: self.scale.lerp(other.scale, t),
        }
    }
}

/// Engine-managed snapshot of an entity's [`Transform2D`] at the start of the current simulation
/// tick. Auto-inserted for every entity with a `Transform2D`; the renderer interpolates between
/// this and the current transform. Games never touch it.
#[derive(Component, Clone, Copy, Debug)]
pub struct PreviousTransform2D(pub Transform2D);

/// Copy every entity's `Transform2D` into its `PreviousTransform2D`, inserting the snapshot
/// component where missing. Called by [`Fulcrum::tick`](crate::Fulcrum::tick) before the
/// simulation schedule runs.
pub(crate) fn snapshot_previous_transforms(world: &mut World) {
    let mut existing = world.query::<(&Transform2D, &mut PreviousTransform2D)>();
    for (current, mut previous) in existing.iter_mut(world) {
        previous.0 = *current;
    }

    let mut missing =
        world.query_filtered::<(Entity, &Transform2D), Without<PreviousTransform2D>>();
    let to_insert: Vec<(Entity, Transform2D)> = missing
        .iter(world)
        .map(|(entity, transform)| (entity, *transform))
        .collect();
    for (entity, transform) in to_insert {
        world
            .entity_mut(entity)
            .insert(PreviousTransform2D(transform));
    }
}
