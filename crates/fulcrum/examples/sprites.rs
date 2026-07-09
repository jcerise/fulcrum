//! Step-6 acceptance scene: overlapping sprites exercising z-order, tint, flip, anchor, and
//! custom size. Run from the workspace root: `cargo run -p fulcrum --example sprites`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let ship = assets.load("ship.png");
    let krate = assets.load("crate.png");

    // Background crate, scaled up, drawn behind everything (z = 0).
    commands.spawn((
        Sprite::new(krate).with_size(vec2(160.0, 160.0)),
        Transform2D::from_xy(0.0, 0.0),
    ));
    // Red-tinted ship, z = 1, overlapping the crate, flipped vertically (nose down).
    commands.spawn((
        {
            let mut s = Sprite::new(ship)
                .with_color(Color::rgb(1.0, 0.3, 0.3))
                .with_z(1.0);
            s.flip_y = true;
            s.custom_size = Some(vec2(96.0, 96.0));
            s
        },
        Transform2D::from_xy(-40.0, 20.0),
    ));
    // White ship in front of everything (z = 2), anchored bottom-left so its corner sits at the
    // window center; rotated 45 degrees.
    commands.spawn((
        {
            let mut s = Sprite::new(ship).with_z(2.0);
            s.anchor = vec2(0.0, 0.0);
            s.custom_size = Some(vec2(96.0, 96.0));
            s
        },
        Transform2D {
            translation: vec2(0.0, 0.0),
            rotation: std::f32::consts::FRAC_PI_4,
            scale: Vec2::ONE,
        },
    ));
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "sprites".into(),
        clear_color: Color::rgb(0.1, 0.1, 0.15),
        ..Default::default()
    })
    .insert_resource(AssetServer::new("crates/fulcrum/examples/assets"))
    .with_plugin(WindowPlugin)
    .add_startup(setup)
    .run();
}
