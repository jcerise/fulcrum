//! Step-6 interpolation check: the simulation runs at only 10 Hz, but the orbiting ship should
//! move smoothly at monitor refresh thanks to render interpolation.
//! Run: `cargo run -p fulcrum --example interp`

use fulcrum::prelude::*;

#[derive(Component)]
struct Orbiter;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let ship = assets.load("ship.png");
    commands.spawn((
        Sprite::new(ship).with_size(vec2(48.0, 48.0)),
        Transform2D::from_xy(200.0, 0.0),
        Orbiter,
    ));
}

/// Runs 10 times per second; moves the ship a big step around a circle each tick.
fn orbit(mut orbiters: Query<&mut Transform2D, With<Orbiter>>, time: Res<Time>) {
    let angle = time.tick as f32 * 0.4;
    for mut transform in &mut orbiters {
        transform.translation = vec2(angle.cos(), angle.sin()) * 200.0;
        transform.rotation = angle + std::f32::consts::FRAC_PI_2;
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "interp: 10 Hz sim, smooth render".into(),
        tick_rate: 10,
        ..Default::default()
    })
    .insert_resource(AssetServer::new("crates/fulcrum/examples/assets"))
    .with_plugin(WindowPlugin)
    .add_startup(setup)
    .add_system(orbit)
    .run();
}
