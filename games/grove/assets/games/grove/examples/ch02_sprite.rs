//! Chapter 2: entities, components, and a sprite on screen.

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let gem = assets.load("gem.png");
    commands.spawn((
        Sprite::new(gem).with_size(vec2(64.0, 64.0)),
        Transform2D::from_xy(0.0, 0.0),
    ));
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
    .run();
}
