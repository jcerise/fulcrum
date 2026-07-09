//! Keyboard and mouse input, sampled once per simulation tick.
//!
//! # Determinism contract
//!
//! Raw OS events accumulate in a pending buffer; the runner drains it into the readable state
//! exactly once per fixed tick, immediately before `FixedUpdate` runs. Every system in a tick
//! therefore sees identical input, and a recorded per-tick event stream reproduces a run
//! exactly (the basis of phase-4 replays). Headless harnesses drive the same path via
//! [`Input::push_key`] & co. followed by [`Input::sample`].

use bevy_ecs::prelude::Resource;
use glam::Vec2;
use rustc_hash::FxHashSet;

/// Keyboard keys, identified by **physical position** (scancode-based), so WASD works the same
/// on any keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(missing_docs, reason = "key names are self-describing")]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Up,
    Down,
    Left,
    Right,
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Shift,
    Ctrl,
    Alt,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(missing_docs, reason = "button names are self-describing")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Default)]
struct Pending {
    keys: Vec<(Key, bool)>,
    buttons: Vec<(MouseButton, bool)>,
    cursor: Option<Vec2>,
    scroll: f32,
}

/// Tick-sampled keyboard and mouse state. Read it from any `FixedUpdate` system via
/// `Res<Input>`.
#[derive(Resource, Default)]
pub struct Input {
    pressed: FxHashSet<Key>,
    just_pressed: FxHashSet<Key>,
    just_released: FxHashSet<Key>,
    mouse_pressed: FxHashSet<MouseButton>,
    mouse_just_pressed: FxHashSet<MouseButton>,
    mouse_just_released: FxHashSet<MouseButton>,
    mouse_screen: Vec2,
    mouse_world: Vec2,
    scroll_delta: f32,
    pending: Pending,
}

impl Input {
    /// Is the key currently held?
    pub fn pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    /// Did the key go down this tick?
    pub fn just_pressed(&self, key: Key) -> bool {
        self.just_pressed.contains(&key)
    }

    /// Did the key go up this tick?
    pub fn just_released(&self, key: Key) -> bool {
        self.just_released.contains(&key)
    }

    /// Is the mouse button currently held?
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// Did the mouse button go down this tick?
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.contains(&button)
    }

    /// Did the mouse button go up this tick?
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        self.mouse_just_released.contains(&button)
    }

    /// Cursor position in physical pixels, top-left origin, +Y down.
    pub fn mouse_screen(&self) -> Vec2 {
        self.mouse_screen
    }

    /// Cursor position in world units (window-center origin, +Y up).
    pub fn mouse_world(&self) -> Vec2 {
        self.mouse_world
    }

    /// Scroll wheel movement this tick, in lines (positive = away from the user).
    pub fn scroll_delta(&self) -> f32 {
        self.scroll_delta
    }

    // --- Event feeding: called by the window runner (or a headless test harness). ---

    /// Queue a key state change.
    pub fn push_key(&mut self, key: Key, pressed: bool) {
        self.pending.keys.push((key, pressed));
    }

    /// Queue a mouse button state change.
    pub fn push_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        self.pending.buttons.push((button, pressed));
    }

    /// Queue a cursor move (physical pixels, top-left origin).
    pub fn push_cursor(&mut self, screen: Vec2) {
        self.pending.cursor = Some(screen);
    }

    /// Queue scroll wheel movement (lines).
    pub fn push_scroll(&mut self, delta: f32) {
        self.pending.scroll += delta;
    }

    /// Drain queued events into the readable state for the next tick. Called by the runner
    /// once per fixed tick with the viewport size in physical pixels.
    pub fn sample(&mut self, viewport: Vec2) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();

        for (key, down) in self.pending.keys.drain(..) {
            if down {
                if self.pressed.insert(key) {
                    self.just_pressed.insert(key);
                }
            } else if self.pressed.remove(&key) {
                self.just_released.insert(key);
            }
        }
        for (button, down) in self.pending.buttons.drain(..) {
            if down {
                if self.mouse_pressed.insert(button) {
                    self.mouse_just_pressed.insert(button);
                }
            } else if self.mouse_pressed.remove(&button) {
                self.mouse_just_released.insert(button);
            }
        }
        if let Some(screen) = self.pending.cursor.take() {
            self.mouse_screen = screen;
        }
        // Phase-1 projection: origin at window center, +Y up, 1 unit = 1 pixel. A future
        // Camera2D replaces this mapping.
        self.mouse_world = Vec2::new(
            self.mouse_screen.x - viewport.x / 2.0,
            viewport.y / 2.0 - self.mouse_screen.y,
        );
        self.scroll_delta = self.pending.scroll;
        self.pending.scroll = 0.0;
    }
}
