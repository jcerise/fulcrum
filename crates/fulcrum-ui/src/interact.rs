//! Pointer interaction: hover/press states, click events, and the "pointer over UI" flag games
//! check before treating a click as world input.

use bevy_ecs::prelude::{Local, Or, Query, Res, ResMut, Resource, With};
use fulcrum_core::{
    CommandEvent, CommandOutbox, EventReader, EventWriter, Input, MouseButton, Vec2,
};
use fulcrum_render::{Camera2D, WindowInfo};

use crate::node::{UiId, UiRect};
use crate::widgets::{ButtonState, UiButton, UiImage, UiPanel};

/// A UI interaction the simulation can react to. Clicks travel as `ui:click` commands (so
/// replays capture them) and are turned back into `UiEvent`s at the start of each tick:
/// readable from `FixedUpdate` for that tick and the next.
#[derive(fulcrum_core::Event, Clone, Debug)]
pub enum UiEvent {
    /// A button with this [`UiId`] was clicked (pressed and released on it).
    Clicked(String),
}

/// Whether the pointer is currently over any visible UI element — check this before treating a
/// click as aimed at the world.
#[derive(Resource, Default, Clone, Copy)]
pub struct UiFocus {
    /// True when the cursor is over a panel, button, or image.
    pub pointer_over_ui: bool,
}

/// `Update` system: hit-test the pointer, drive button states, emit clicks on release.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // ECS systems legitimately take many resources
pub(crate) fn interact_ui(
    mut buttons: Query<(&UiRect, &mut ButtonState, Option<&UiId>), With<UiButton>>,
    blockers: Query<&UiRect, Or<(With<UiPanel>, With<UiButton>, With<UiImage>)>>,
    camera: Option<Res<Camera2D>>,
    window: Option<Res<WindowInfo>>,
    input: Option<Res<Input>>,
    mut focus: ResMut<UiFocus>,
    mut outbox: ResMut<CommandOutbox>,
    mut was_down: Local<bool>,
    mut pressed_id: Local<Option<String>>,
) {
    let (Some(camera), Some(window), Some(input)) = (camera, window, input) else {
        return;
    };
    let window_size = Vec2::new(window.width as f32, window.height as f32);
    let pointer = camera.screen_to_ui(input.mouse_screen(), window_size);

    focus.pointer_over_ui = blockers.iter().any(|placed| placed.rect.contains(pointer));

    // Frame-level edge detection (Input state only changes per tick; frames in between see the
    // same value, so compare against last frame).
    let down = input.mouse_pressed(MouseButton::Left);
    let pressed_edge = down && !*was_down;
    let released_edge = !down && *was_down;
    *was_down = down;

    // Top-most button under the pointer (immutable pass), then drive states (mutable pass).
    let mut top_order: Option<u32> = None;
    for (placed, ..) in buttons.iter() {
        if placed.rect.contains(pointer) && top_order.is_none_or(|order| placed.order > order) {
            top_order = Some(placed.order);
        }
    }

    for (placed, mut state, id) in &mut buttons {
        let is_top = top_order == Some(placed.order) && placed.rect.contains(pointer);
        state.hovered = is_top;
        if pressed_edge && is_top {
            state.pressed = true;
            *pressed_id = id.map(|i| i.0.clone());
        }
        if released_edge {
            if state.pressed
                && is_top
                && let Some(id) = id
                && pressed_id.as_deref() == Some(id.0.as_str())
            {
                outbox.send("ui:click", id.0.clone());
            }
            state.pressed = false;
        }
    }
    if released_edge {
        *pressed_id = None;
    }
}

/// `FixedUpdate` system (registered first by `UiPlugin`): turns `ui:click` commands — live or
/// replayed — back into [`UiEvent`]s for game systems.
pub(crate) fn dispatch_ui_commands(
    mut commands: EventReader<CommandEvent>,
    mut events: EventWriter<UiEvent>,
) {
    for command in commands.read() {
        if command.name == "ui:click" {
            events.write(UiEvent::Clicked(command.payload.clone()));
        }
    }
}
