//! Step-4 (phase 3) acceptance: live texture reload. The example writes its own asset dir,
//! shows a sprite, then rewrites the PNG after one simulated second — the on-screen color changes with
//! no restart. Run: `cargo run -p fulcrum --example hot_reload`

use fulcrum::prelude::*;

fn write_png(dir: &std::path::Path, rgba: [u8; 4]) {
    // Hand-rolled 1x1 PNG via the image crate (dev-dependency).
    let mut img = image::RgbaImage::new(4, 4);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba(rgba);
    }
    img.save(dir.join("swatch.png")).unwrap();
}

fn asset_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("fulcrum-hot-reload-example");
    std::fs::create_dir_all(&dir).unwrap();
    write_png(&dir, [220, 40, 40, 255]); // start red
    dir
}

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    commands.spawn((
        Sprite::new(assets.load("swatch.png")).with_size(vec2(300.0, 300.0)),
        Transform2D::default(),
    ));
}

/// After ~3 seconds of wall time, rewrite the PNG green — the watcher does the rest.
fn rewrite_later(time: Res<Time>, mut done: Local<bool>) {
    if !*done && time.tick >= 60 {
        *done = true;
        write_png(&asset_dir(), [40, 220, 90, 255]);
        println!("rewrote swatch.png -> green; the sprite should change without a restart");
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "hot reload".into(),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(asset_dir()))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_frame_system(rewrite_later)
    .run();
}
