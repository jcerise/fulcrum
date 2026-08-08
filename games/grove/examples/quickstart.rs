//! Quickstart: a window, a square, arrow-key movement, and walls.
//!
//! The finished program from the book's Quickstart chapter, in one file.
//! `cargo run -p grove --example quickstart`

use fulcrum::prelude::*;

/// Playfield size in world units. The window is the same size, so 1 unit = 1 pixel and the
/// arena edges land exactly on the window edges.
const ARENA: Vec2 = Vec2::new(800.0, 600.0);
/// Side length of the player square.
const PLAYER_SIZE: f32 = 40.0;
/// Player speed in units per second.
const PLAYER_SPEED: f32 = 300.0;

/// Marker component: "this entity is the thing the arrow keys drive."
#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let white = assets.load("white.png");
    commands.spawn((
        Sprite::new(white)
            .with_color(Color::rgb(0.9, 0.7, 0.3))
            .with_size(Vec2::splat(PLAYER_SIZE)),
        Transform2D::from_xy(0.0, 0.0),
        Player,
    ));
}

/// Simulation system: runs on the fixed 60 Hz tick, not per frame.
fn move_player(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::Left) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::Right) {
        dir.x += 1.0;
    }
    if input.pressed(Key::Down) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::Up) {
        dir.y += 1.0;
    }

    // The sprite is centered on its translation, so the reachable area shrinks by half a
    // square on every side.
    let limit = (ARENA - Vec2::splat(PLAYER_SIZE)) / 2.0;
    for mut transform in &mut players {
        let step = dir.normalize_or_zero() * PLAYER_SPEED * time.fixed_delta;
        transform.translation = (transform.translation + step).clamp(-limit, limit);
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Fulcrum Quickstart".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: Color::rgb(0.08, 0.09, 0.12),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(move_player)
    .run();
}
