//! Phase-4 acceptance: record 1,000 ticks of the dungeon demo, then play the file back in a
//! fresh app — every embedded state hash must reproduce.

use dungeon::game::{self, GamePlugin};
use fulcrum::prelude::*;

fn script(input: &mut Input, tick: u32) {
    match tick {
        10 => input.push_key(Key::D, true),
        140 => {
            input.push_key(Key::D, false);
            input.push_key(Key::W, true);
        }
        200 => input.push_key(Key::Space, true),
        205 => input.push_key(Key::Space, false),
        280 => {
            input.push_key(Key::W, false);
            input.push_key(Key::S, true);
        }
        300 => input.push_key(Key::Space, true),
        305 => input.push_key(Key::Space, false),
        450 => input.push_key(Key::S, false),
        600 => input.push_key(Key::A, true),
        800 => input.push_key(Key::A, false),
        _ => {}
    }
}

fn build_app(record: bool) -> Fulcrum {
    let app = Fulcrum::with_config(FulcrumConfig {
        record_replays: record,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    game::register_components(app)
        .with_plugin(ScenePlugin)
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/level1.scene.ron").unwrap();
                spawner.load(level);
            },
        )
}

#[test]
fn record_1000_ticks_then_playback_matches_every_hash() {
    let dir = std::env::temp_dir().join("fulcrum-dungeon-replay");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dungeon.freplay");

    let mut app = build_app(true);
    app.run_startup();
    for tick in 0..1000 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
    }
    app.save_replay(&path).unwrap();

    let replay = Replay::load(&path).unwrap();
    assert_eq!(replay.ticks.len(), 1000);
    assert!(
        replay.state_hashes.len() >= 17,
        "expected a hash every 60 ticks, got {}",
        replay.state_hashes.len()
    );

    build_app(false)
        .run_replay(&path)
        .expect("dungeon playback must reproduce every embedded state hash");
}
