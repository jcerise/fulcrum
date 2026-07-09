//! Step-6 batching stress test: 10,000 static sprites across 2 textures. Prints FPS and batch
//! counts once per second. Run: `cargo run -p fulcrum --example stress --release`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let textures = [assets.load("ship.png"), assets.load("crate.png")];
    // Deterministic pseudo-grid placement; no RNG needed.
    for i in 0..10_000u32 {
        let x = (i % 125) as f32 * 10.0 - 620.0;
        let y = (i / 125) as f32 * 8.0 - 320.0;
        commands.spawn((
            Sprite::new(textures[(i % 2) as usize]).with_size(vec2(8.0, 8.0)),
            Transform2D::from_xy(x, y),
        ));
    }
}

fn report(time: Res<Time>, stats: Res<RenderStats>, mut acc: Local<(f32, u32)>) {
    acc.0 += time.frame_delta;
    acc.1 += 1;
    if acc.0 >= 1.0 {
        println!(
            "fps: {:>3} | sprites: {} | batches: {}",
            acc.1, stats.sprites, stats.batches
        );
        *acc = (0.0, 0);
    }
}

fn main() {
    Fulcrum::new("stress: 10k sprites")
        .insert_resource(AssetServer::new("crates/fulcrum/examples/assets"))
        .with_plugin(WindowPlugin)
        .add_startup(setup)
        .add_frame_system(report)
        .run();
}
