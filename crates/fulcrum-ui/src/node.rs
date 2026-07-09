//! UI tree building blocks: [`UiNode`] (layout) and [`UiRect`] (computed placement).
//!
//! UI lives in **UI space**: top-left origin, +Y down, in virtual pixels (the camera's letterbox
//! virtual resolution, or window pixels otherwise). It renders above the world and ignores the
//! camera transform.

use bevy_ecs::prelude::Component;
use fulcrum_core::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Which point of the parent a node pins to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[expect(missing_docs, reason = "anchor names are self-describing")]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// The anchor as a (0..1, 0..1) fraction of the parent rect (top-left origin).
    pub fn fraction(self) -> Vec2 {
        match self {
            Anchor::TopLeft => Vec2::new(0.0, 0.0),
            Anchor::TopCenter => Vec2::new(0.5, 0.0),
            Anchor::TopRight => Vec2::new(1.0, 0.0),
            Anchor::CenterLeft => Vec2::new(0.0, 0.5),
            Anchor::Center => Vec2::new(0.5, 0.5),
            Anchor::CenterRight => Vec2::new(1.0, 0.5),
            Anchor::BottomLeft => Vec2::new(0.0, 1.0),
            Anchor::BottomCenter => Vec2::new(0.5, 1.0),
            Anchor::BottomRight => Vec2::new(1.0, 1.0),
        }
    }
}

/// How a node's size is determined.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum UiSize {
    /// Explicit size in UI pixels.
    Px(Vec2),
    /// Fill the parent.
    Fill,
    /// Fit content: stacked children plus the widget's intrinsic size (label text, etc.).
    #[default]
    Fit,
}

/// Automatic child placement.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum StackDir {
    /// Children place themselves via their own anchors.
    #[default]
    None,
    /// Children stack top-to-bottom with this gap.
    Vertical(f32),
    /// Children stack left-to-right with this gap.
    Horizontal(f32),
}

/// A UI tree node's layout parameters.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNode {
    /// Which point of the parent to pin to.
    pub anchor: Anchor,
    /// Which point of self sits on the anchor, `(0..1, 0..1)`.
    pub pivot: Vec2,
    /// Pixel offset from the anchor (+Y down).
    pub offset: Vec2,
    /// Size policy.
    pub size: UiSize,
    /// Automatic child placement.
    pub stack: StackDir,
    /// Hidden nodes (and their subtrees) neither draw nor hit-test.
    pub visible: bool,
}

impl Default for UiNode {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopLeft,
            pivot: Vec2::ZERO,
            offset: Vec2::ZERO,
            size: UiSize::Fit,
            stack: StackDir::None,
            visible: true,
        }
    }
}

/// Computed placement (UI pixels, top-left origin) plus depth-first draw order.
#[derive(Component, Clone, Copy, Debug)]
pub struct UiRect {
    /// The node's screen rect in UI pixels (`min` = top-left).
    pub rect: Rect,
    /// Pre-order tree index: later = drawn on top = hit-tested first.
    pub order: u32,
}

/// Stable name for [`UiQuery`](crate::UiQuery) lookups and button click events.
#[derive(Component, Clone, Debug)]
pub struct UiId(pub String);

/// Marks the root of a tree spawned from a layout file (drives hot reload).
#[derive(Component, Clone, Debug)]
pub struct UiRootPath(pub String);
