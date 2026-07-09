//! Widget components: what a UI node looks like.

use bevy_ecs::prelude::Component;
use fulcrum_asset::Handle;
use fulcrum_core::Color;
use fulcrum_render::{HAlign, Texture};
use serde::{Deserialize, Serialize};

/// A colored (optionally textured / nine-sliced) box.
#[derive(Component, Clone, Debug)]
pub struct UiPanel {
    /// Fill color (or tint over the image).
    pub color: Color,
    /// Optional background texture.
    pub image: Option<Handle<Texture>>,
    /// Nine-slice margins in texture pixels: `[left, top, right, bottom]`. Requires `image`.
    pub nine_slice: Option<[f32; 4]>,
}

impl Default for UiPanel {
    fn default() -> Self {
        Self {
            color: Color::rgba(0.0, 0.0, 0.0, 0.6),
            image: None,
            nine_slice: None,
        }
    }
}

/// A line (or lines) of text in the built-in pixel font.
#[derive(Component, Clone, Debug)]
pub struct UiLabel {
    /// The text to show.
    pub text: String,
    /// Font size in UI pixels.
    pub size: f32,
    /// Text color.
    pub color: Color,
    /// Alignment within the node's rect.
    pub h_align: HAlign,
}

impl Default for UiLabel {
    fn default() -> Self {
        Self {
            text: String::new(),
            size: 16.0,
            color: Color::WHITE,
            h_align: HAlign::Left,
        }
    }
}

/// Per-state colors for a button's background.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ButtonStyle {
    /// At rest.
    pub normal: Color,
    /// Cursor over.
    pub hover: Color,
    /// Held down.
    pub pressed: Color,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            normal: Color::rgba(0.25, 0.27, 0.35, 1.0),
            hover: Color::rgba(0.35, 0.38, 0.48, 1.0),
            pressed: Color::rgba(0.18, 0.2, 0.26, 1.0),
        }
    }
}

/// A clickable button: colored box + centered label. Clicks emit
/// [`UiEvent::Clicked`](crate::UiEvent) with the node's [`UiId`](crate::UiId).
#[derive(Component, Clone, Debug, Default)]
pub struct UiButton {
    /// Button caption.
    pub text: String,
    /// Caption size in UI pixels.
    pub text_size: f32,
    /// State colors.
    pub style: ButtonStyle,
}

/// Interaction state, driven by the pointer each frame.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ButtonState {
    /// Cursor is over the button.
    pub hovered: bool,
    /// Pressed and not yet released.
    pub pressed: bool,
}

/// A plain image.
#[derive(Component, Clone, Debug)]
pub struct UiImage {
    /// The texture to draw.
    pub image: Handle<Texture>,
    /// Tint.
    pub color: Color,
}
