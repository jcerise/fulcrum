//! Chapter 3: the simulation — input, fixed timestep, and a hero that walks.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let hero = assets.load("hero.png");
    commands.spawn((
        Sprite::new(hero).with_size(vec2(48.0, 48.0)),
        Transform2D::from_xy(0.0, 0.0),
        Player,
    ));
}

/// Runs 60 times per second, every run exactly `fixed_delta` apart — the simulation.
fn movement(mut players: Query<&mut Transform2D, With<Player>>, input: Res<Input>, time: Res<Time>) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) { dir.x -= 1.0 }
    if input.pressed(Key::D) { dir.x += 1.0 }
    if input.pressed(Key::S) { dir.y -= 1.0 }
    if input.pressed(Key::W) { dir.y += 1.0 }
    for mut transform in &mut players {
        transform.translation += dir.normalize_or_zero() * 150.0 * time.fixed_delta;
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Grove".into(),
        clear_color: Color::rgb(0.16, 0.24, 0.16),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(movement)
    .run();
}
