//! Template binary: attaches visuals to the simulation and runs the app. `cargo run`
//!
//! WASD/arrows move the square. Everything here is cosmetic — the simulation lives in
//! `game.rs` and must stay renderer-free so the determinism test can drive it headless.

use fulcrum::prelude::*;
use my_game::game::{self, GamePlugin, PLAYER_SIZE, Player};

/// Give the simulation's entities their looks (runs after [`game::spawn_entities`]).
fn attach_visuals(
    mut commands: Commands,
    mut assets: AssetLoader,
    players: Query<Entity, With<Player>>,
) {
    let white = assets.load("white.png");
    for player in &players {
        commands
            .entity(player)
            .insert(Sprite::new(white).with_size(Vec2::splat(PLAYER_SIZE)));
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "My Game".into(),
        window_size: (game::ARENA.x as u32, game::ARENA.y as u32),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(attach_visuals.after(game::spawn_entities))
    .run();
}
