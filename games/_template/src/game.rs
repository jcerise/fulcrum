//! Template simulation: one player square steered with WASD/arrows. Pure logic — no sprites,
//! no audio — so it runs headless for determinism tests. All systems live in `FixedUpdate`.
//!
//! Replace this with your game's simulation, keeping the split: everything that affects
//! outcomes ticks here; everything cosmetic stays frame-side in `main.rs`.

use fulcrum::prelude::*;

/// Arena size in world units (matches the window).
pub const ARENA: Vec2 = Vec2::new(800.0, 600.0);
/// Player square side length.
pub const PLAYER_SIZE: f32 = 32.0;
/// Player speed, units/second.
pub const PLAYER_SPEED: f32 = 260.0;

/// The player-controlled square.
#[derive(Component)]
pub struct Player;

/// Installs the simulation: entities and the `FixedUpdate` systems.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.add_systems(Startup, spawn_entities);
        app.add_systems(FixedUpdate, control_player);
    }
}

/// Spawn the simulation entities (logic only; the binary attaches sprites in a later
/// startup system).
pub fn spawn_entities(mut commands: Commands) {
    commands.spawn((Player, Transform2D::from_xy(0.0, 0.0)));
}

/// Move the player from held keys, clamped to the arena.
fn control_player(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::W) || input.pressed(Key::Up) {
        dir.y += 1.0;
    }
    if input.pressed(Key::S) || input.pressed(Key::Down) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::A) || input.pressed(Key::Left) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::D) || input.pressed(Key::Right) {
        dir.x += 1.0;
    }
    let limit = (ARENA - Vec2::splat(PLAYER_SIZE)) / 2.0;
    for mut transform in &mut players {
        let next =
            transform.translation + dir.normalize_or_zero() * PLAYER_SPEED * time.fixed_delta;
        transform.translation = next.clamp(-limit, limit);
    }
}
