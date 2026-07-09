//! Step-3 (phase 2) acceptance: all four gizmo primitives tracking a moving entity, drawn above
//! sprites. Run: `cargo run -p fulcrum --example gizmos`

use fulcrum::prelude::*;

#[derive(Component)]
struct Orbiter;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let krate = assets.load("crate.png");
    let ship = assets.load("ship.png");
    // A big crate behind everything, so "above sprites" is visible.
    commands.spawn((
        Sprite::new(krate).with_size(vec2(300.0, 300.0)),
        Transform2D::default(),
    ));
    commands.spawn((
        Sprite::new(ship).with_size(vec2(48.0, 48.0)).with_z(1.0),
        Transform2D::from_xy(150.0, 0.0),
        Orbiter,
    ));
}

fn orbit(mut orbiters: Query<&mut Transform2D, With<Orbiter>>, time: Res<Time>) {
    let angle = time.tick as f32 * 0.02;
    for mut transform in &mut orbiters {
        transform.translation = vec2(angle.cos(), angle.sin()) * 150.0;
    }
}

/// Cosmetic overlay: draw all four primitives around the orbiter each frame.
fn overlay(mut gizmos: ResMut<Gizmos>, orbiters: Query<&Transform2D, With<Orbiter>>) {
    for transform in &orbiters {
        let p = transform.translation;
        gizmos.circle(p, 40.0, Color::GREEN);
        gizmos.rect(
            Rect::from_center_size(p, vec2(56.0, 56.0)),
            Color::rgb(1.0, 1.0, 0.0),
        );
        gizmos.line(Vec2::ZERO, p, Color::rgba(1.0, 0.3, 0.3, 0.9));
        gizmos.point(p, Color::rgb(0.3, 0.7, 1.0));
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "gizmos".into(),
        gizmos_enabled: true, // force on so the demo works in release builds too
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(orbit)
    .add_frame_system(overlay)
    .run();
}
