//! Step-1 (phase 2) acceptance scene for `Camera2D` scaling modes.
//!
//! `MODE=stretch|fixedheight|letterbox|integer WIN=1280x720 cargo run -p fulcrum --example camera`
//! Arrow keys pan, Q/E zoom, R rotates — all cosmetic `Update`-side camera motion.

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let krate = assets.load("crate.png");
    let ship = assets.load("ship.png");
    // A crate every 64 units marks the world grid; the ship marks the origin.
    for gx in -8..=8 {
        for gy in -5..=5 {
            commands.spawn((
                Sprite::new(krate).with_size(vec2(32.0, 32.0)),
                Transform2D::from_xy(gx as f32 * 64.0, gy as f32 * 64.0),
            ));
        }
    }
    commands.spawn((
        Sprite::new(ship).with_size(vec2(48.0, 48.0)).with_z(1.0),
        Transform2D::default(),
    ));
}

fn drive_camera(mut camera: ResMut<Camera2D>, input: Res<Input>, time: Res<Time>) {
    let dt = time.frame_delta;
    let mut pan = Vec2::ZERO;
    if input.pressed(Key::Left) {
        pan.x -= 1.0
    }
    if input.pressed(Key::Right) {
        pan.x += 1.0
    }
    if input.pressed(Key::Down) {
        pan.y -= 1.0
    }
    if input.pressed(Key::Up) {
        pan.y += 1.0
    }
    let zoom = camera.zoom;
    camera.center += pan * 300.0 * dt / zoom;
    if input.pressed(Key::Q) {
        camera.zoom *= 1.0 - dt
    }
    if input.pressed(Key::E) {
        camera.zoom *= 1.0 + dt
    }
    if input.pressed(Key::R) {
        camera.rotation += dt
    }
}

fn main() {
    let scaling = match std::env::var("MODE").as_deref() {
        Ok("fixedheight") => ScalingMode::FixedHeight(360.0),
        Ok("letterbox") => ScalingMode::Letterbox {
            width: 640.0,
            height: 360.0,
        },
        Ok("integer") => ScalingMode::IntegerScale {
            width: 320.0,
            height: 180.0,
        },
        _ => ScalingMode::Stretch,
    };
    let win = std::env::var("WIN").unwrap_or_default();
    let (w, h) = win
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1280, 720));

    Fulcrum::with_config(FulcrumConfig {
        title: "camera".into(),
        window_size: (w, h),
        clear_color: Color::rgb(0.16, 0.17, 0.24),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/assets"
    )))
    .with_plugin(DefaultPlugins)
    .insert_resource(Camera2D {
        scaling,
        ..Default::default()
    })
    .add_startup(setup)
    .add_frame_system(drive_camera)
    .run();
}
