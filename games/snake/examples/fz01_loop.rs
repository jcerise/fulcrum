//! From Zero, chapter 1: a window with something alive in it.
//!
//! The square patrols left and right with nobody touching it — proof that a game is a loop,
//! not a callback. Every line here is explained in the chapter.

use fulcrum::prelude::*;

/// Our own marker: "this entity is the thing that patrols."
#[derive(Component)]
struct Patroller;

/// Runs once at startup: create the square.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let square = assets.load("white.png");
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(0.4, 0.9, 0.5))
            .with_size(Vec2::splat(24.0)),
        Transform2D::from_xy(-100.0, 0.0),
        Patroller,
    ));
}

/// Runs 60 times a second, forever: nudge the square.
fn patrol(mut squares: Query<&mut Transform2D, With<Patroller>>, time: Res<Time>) {
    for mut transform in &mut squares {
        // The simulation's own clock: 120 ticks (2 seconds) right, 120 left, repeat.
        let heading_right = time.tick % 240 < 120;
        let direction = if heading_right { 1.0 } else { -1.0 };
        // 100 units/second x the tick's fixed duration = the same speed on every machine.
        transform.translation.x += direction * 100.0 * time.fixed_delta;
    }
}

fn main() {
    Fulcrum::new("fz01: a window, alive")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system(patrol)
        .run();
}
