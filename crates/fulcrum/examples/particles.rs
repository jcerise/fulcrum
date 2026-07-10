//! Step-5 (phase 4) acceptance: 100 emitters of additive sparks, one-shot cleanup, hot
//! reload. Prints live particle counts + fps. Run:
//! `cargo run -p fulcrum --example particles --release`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut effects: EffectLoader) {
    let spark = effects.load("fx/spark.fx.ron").expect("effect loads");
    for i in 0..100 {
        let x = (i % 10) as f32 * 110.0 - 500.0;
        let y = (i / 10) as f32 * 65.0 - 300.0;
        commands.spawn((ParticleEmitter::new(spark), Transform2D::from_xy(x, y)));
    }
}

fn report(
    time: Res<Time>,
    emitters: Query<&ParticleEmitter>,
    mut acc: Local<f32>,
    mut frames: Local<u32>,
) {
    *acc += time.frame_delta;
    *frames += 1;
    if *acc >= 1.0 {
        let live: usize = emitters.iter().map(|e| e.live()).sum();
        println!(
            "fps: {:>4} | emitters: {} | live particles: {}",
            *frames,
            emitters.iter().len(),
            live
        );
        *acc = 0.0;
        *frames = 0;
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "particles".into(),
        clear_color: Color::rgb(0.05, 0.05, 0.09),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_frame_system(report)
    .run();
}
