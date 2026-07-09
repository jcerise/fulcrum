//! Input semantics tests: tick-boundary sampling, just-pressed edges, world-space mouse.

use fulcrum_core::{Input, Key, MouseButton, vec2};

const VIEWPORT: fulcrum_core::Vec2 = fulcrum_core::Vec2::new(800.0, 600.0);

#[test]
fn held_key_is_pressed_every_tick_but_just_pressed_once() {
    let mut input = Input::default();
    input.push_key(Key::W, true);

    input.sample(VIEWPORT); // tick 1: the press arrives
    assert!(input.pressed(Key::W));
    assert!(input.just_pressed(Key::W));

    input.sample(VIEWPORT); // tick 2: still held, no new events
    assert!(input.pressed(Key::W));
    assert!(!input.just_pressed(Key::W));

    input.push_key(Key::W, false);
    input.sample(VIEWPORT); // tick 3: released
    assert!(!input.pressed(Key::W));
    assert!(input.just_released(Key::W));

    input.sample(VIEWPORT); // tick 4: edge flags cleared
    assert!(!input.just_released(Key::W));
}

#[test]
fn press_and_release_between_ticks_is_not_lost() {
    let mut input = Input::default();
    // Both events arrive between two tick samples (e.g. a very fast tap).
    input.push_key(Key::Space, true);
    input.push_key(Key::Space, false);

    input.sample(VIEWPORT);
    assert!(input.just_pressed(Key::Space), "press edge preserved");
    assert!(input.just_released(Key::Space), "release edge preserved");
    assert!(!input.pressed(Key::Space), "key is up after the tap");
}

#[test]
fn mouse_buttons_and_scroll_sample_per_tick() {
    let mut input = Input::default();
    input.push_mouse_button(MouseButton::Left, true);
    input.push_scroll(2.0);
    input.push_scroll(1.0);

    input.sample(VIEWPORT);
    assert!(input.mouse_pressed(MouseButton::Left));
    assert!(input.mouse_just_pressed(MouseButton::Left));
    assert_eq!(
        input.scroll_delta(),
        3.0,
        "scroll accumulates within a tick"
    );

    input.sample(VIEWPORT);
    assert!(!input.mouse_just_pressed(MouseButton::Left));
    assert_eq!(input.scroll_delta(), 0.0, "scroll resets next tick");
}

#[test]
fn mouse_world_uses_center_origin_y_up() {
    let mut input = Input::default();
    // Top-left corner of an 800x600 window.
    input.push_cursor(vec2(0.0, 0.0));
    input.sample(VIEWPORT);
    assert_eq!(input.mouse_world(), vec2(-400.0, 300.0));

    // Window center.
    input.push_cursor(vec2(400.0, 300.0));
    input.sample(VIEWPORT);
    assert_eq!(input.mouse_world(), vec2(0.0, 0.0));
    assert_eq!(input.mouse_screen(), vec2(400.0, 300.0));
}
