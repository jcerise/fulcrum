//! Chapter 14: record a run of Grove, play the file back, verify every state hash. Headless —
//! run it anywhere:
//!
//! ```text
//! cargo run -p grove --example ch14_replay
//! ```

use fulcrum::prelude::*;
use grove::game::{self, GamePlugin, Gems};

fn build(record: bool) -> Fulcrum {
    let app = Fulcrum::with_config(FulcrumConfig {
        title: "grove".into(),
        record_replays: record,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    game::register_components(app)
        .with_plugin(ScenePlugin) // registry + prefabs + the replay state hasher
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/grove.scene.ron").unwrap();
                spawner.load(level);
            },
        )
}

fn main() {
    env_logger::init();
    let path = std::env::temp_dir().join("grove.freplay");

    // Record: the gem run the headless tests use — right, up, right to (210, 170).
    let mut app = build(true);
    app.run_startup();
    for tick in 0..600u32 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            match tick {
                0 => input.push_key(Key::D, true),
                80 => {
                    input.push_key(Key::D, false);
                    input.push_key(Key::W, true);
                }
                140 => input.push_key(Key::W, false),
                150 => input.push_key(Key::D, true),
                300 => input.push_key(Key::D, false),
                _ => {}
            }
            input.sample(|s| s);
        }
        app.tick();
    }
    let recorded = *app.world().resource::<Gems>();
    app.save_replay(&path).expect("replay saves");
    println!(
        "recorded 600 ticks ({}/{} gems) -> {}",
        recorded.collected,
        recorded.total,
        path.display()
    );

    // Play back in a fresh app: every embedded state hash is checked on the fly.
    match build(false).run_replay(&path) {
        Ok(()) => println!("playback reproduced the run — every state hash matched"),
        Err(error) => println!("divergence! {error}"),
    }
}
