//! Step-8 (phase 3) acceptance: the F12 egui debug overlay — performance stats, entity
//! inspector with editable registered components, and asset list. Starts open here for
//! demonstration. Run: `cargo run -p fulcrum --example inspector`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader, mut debug: ResMut<DebugUi>) {
    debug.open = true; // normally toggled with F12
    let ship = assets.load("ship.png");
    let krate = assets.load("crate.png");
    commands.spawn((
        Name("player ship".into()),
        Sprite::new(ship).with_size(vec2(64.0, 64.0)),
        Transform2D::from_xy(150.0, 0.0),
    ));
    commands.spawn((
        Name("crate".into()),
        Sprite::new(krate).with_size(vec2(96.0, 96.0)),
        Transform2D::from_xy(-150.0, 40.0),
    ));
}

fn spin(mut sprites: Query<&mut Transform2D>, time: Res<Time>) {
    for mut transform in &mut sprites {
        transform.rotation = time.tick as f32 * 0.01;
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "inspector".into(),
        clear_color: Color::rgb(0.1, 0.1, 0.14),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(spin)
    .run();
}
