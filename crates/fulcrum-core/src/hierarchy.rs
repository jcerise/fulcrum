//! Minimal parent/child links, used by prefab children and (later) UI trees.
//!
//! Deliberately **no transform propagation**: prefab children get world transforms composed
//! once at spawn. Most 2D games don't need live hierarchies; revisit if a real game hurts
//! without it.

use bevy_ecs::prelude::{Component, Entity};

/// This entity's parent.
#[derive(Component, Debug, Clone, Copy)]
pub struct Parent(pub Entity);

/// This entity's children, in spawn order.
#[derive(Component, Debug, Clone, Default)]
pub struct Children(pub Vec<Entity>);
