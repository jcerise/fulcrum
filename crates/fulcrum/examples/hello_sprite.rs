//! The first-page-of-the-docs program: one sprite, moved with the arrow keys.
//! Run from the workspace root: `cargo run -p fulcrum --example hello_sprite`

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let ship = assets.load("ship.png");
    commands.spawn((
        Sprite::new(ship).with_size(vec2(48.0, 48.0)),
        Transform2D::default(),
        Player,
    ));
}

fn movement(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;
    if input.pressed(Key::Left) {
        direction.x -= 1.0
    }
    if input.pressed(Key::Right) {
        direction.x += 1.0
    }
    if input.pressed(Key::Down) {
        direction.y -= 1.0
    }
    if input.pressed(Key::Up) {
        direction.y += 1.0
    }
    for mut transform in &mut players {
        transform.translation += direction * 250.0 * time.fixed_delta;
    }
}

fn main() {
    Fulcrum::new("hello sprite")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system(movement)
        .run();
}
