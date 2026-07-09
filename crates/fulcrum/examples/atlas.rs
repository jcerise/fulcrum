//! Step-2 (phase 2) acceptance: 1,000 sprites drawing distinct regions of one sprite sheet must
//! batch into a single draw call. Prints RenderStats once per second.
//! Run: `cargo run -p fulcrum --example atlas`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let texture = assets.load("crate.png"); // 16x16 -> 4x4 grid of 4px tiles
    let sheet = assets.add_sheet(SpriteSheet::from_grid(texture, vec2(4.0, 4.0), 4, 4));
    for i in 0..1000u32 {
        let x = (i % 40) as f32 * 28.0 - 550.0;
        let y = (i / 40) as f32 * 26.0 - 320.0;
        commands.spawn((
            Sprite::from_sheet(sheet, i % 16).with_size(vec2(24.0, 24.0)),
            Transform2D::from_xy(x, y),
        ));
    }
}

fn report(time: Res<Time>, stats: Res<RenderStats>, mut acc: Local<f32>) {
    *acc += time.frame_delta;
    if *acc >= 1.0 {
        *acc = 0.0;
        println!("sprites: {} | batches: {}", stats.sprites, stats.batches);
    }
}

fn main() {
    Fulcrum::new("atlas")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_frame_system(report)
        .run();
}
