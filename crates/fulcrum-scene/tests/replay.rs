//! Replay acceptance: record → save → load → playback reproduces every embedded state hash;
//! tampering with a recorded input reports divergence at the right tick; header mismatches
//! warn before playback starts.

use bevy_ecs::prelude::ResMut;
use fulcrum_core::{
    Commands, Component, EventReader, Fulcrum, FulcrumConfig, Input, Key, Query, Replay,
    ReplayError, ReplayModSet, SimRng, Transform2D, replay, vec2,
};
use fulcrum_scene::{RegisterComponentExt, ScenePlugin};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Default, Clone)]
struct Walker {
    speed: f32,
}

fn spawn_walkers(mut commands: Commands, mut rng: ResMut<SimRng>) {
    for _ in 0..5 {
        commands.spawn((
            Transform2D::from_translation(vec2(
                rng.range_f32(-100.0..100.0),
                rng.range_f32(-100.0..100.0),
            )),
            Walker {
                speed: rng.range_f32(20.0..60.0),
            },
        ));
    }
}

/// RNG drift, plus input steering, plus a command that teleports everyone.
fn drive_walkers(
    mut walkers: Query<(&mut Transform2D, &Walker)>,
    mut rng: ResMut<SimRng>,
    input: bevy_ecs::prelude::Res<Input>,
    mut commands: EventReader<fulcrum_core::CommandEvent>,
) {
    let steer = if input.pressed(Key::D) { 1.0 } else { 0.0 };
    let teleport = commands.read().any(|c| c.name == "teleport");
    for (mut transform, walker) in &mut walkers {
        let jitter = vec2(rng.range_f32(-1.0..1.0), rng.range_f32(-1.0..1.0));
        transform.translation += (jitter + vec2(steer, 0.0)) * walker.speed * (1.0 / 60.0);
        if teleport {
            transform.translation = vec2(0.0, 0.0);
        }
    }
}

fn build_app(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        record_replays: true,
        ..Default::default()
    })
    .insert_resource(fulcrum_asset::AssetServer::default())
    .with_plugin(ScenePlugin)
    .register_component::<Walker>("Walker")
    .add_startup(spawn_walkers)
    .add_system(drive_walkers)
}

/// Drive `ticks` with scripted input and commands, recording throughout.
fn record(path: &std::path::Path, ticks: u32) {
    let mut app = build_app(7);
    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            match tick {
                100 => input.push_key(Key::D, true),
                400 => input.push_key(Key::D, false),
                _ => {}
            }
            input.sample(|s| s);
        }
        if tick == 700 {
            app.world_mut()
                .resource_mut::<fulcrum_core::CommandOutbox>()
                .send("teleport", "");
        }
        app.tick();
    }
    replay::save_replay(app.world_mut(), path).unwrap();
}

#[test]
fn playback_reproduces_every_state_hash() {
    let dir = std::env::temp_dir().join("fulcrum-replay-roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("run.freplay");
    record(&path, 1000);

    let loaded = Replay::load(&path).unwrap();
    assert_eq!(loaded.ticks.len(), 1000);
    // Hashes every 60 ticks (0, 60, ..., 960) plus the final post-run hash at tick 1000.
    assert_eq!(loaded.state_hashes.len(), 18);
    assert_eq!(loaded.ticks[100].input.keys, vec![(Key::D, true)]);
    assert!(
        loaded.ticks[700]
            .commands
            .iter()
            .any(|c| c.name == "teleport")
    );

    build_app(0) // seed comes from the header, not this config
        .run_replay(&path)
        .expect("playback must reproduce every embedded state hash");
}

#[test]
fn tampered_input_reports_divergence_at_the_right_tick() {
    let dir = std::env::temp_dir().join("fulcrum-replay-tamper");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("run.freplay");
    record(&path, 1000);

    let mut tampered = Replay::load(&path).unwrap();
    tampered.ticks[500].input.keys.push((Key::D, true)); // a press the real run never saw
    let tampered_path = dir.join("tampered.freplay");
    tampered.save(&tampered_path).unwrap();

    let err = build_app(0)
        .run_replay(&tampered_path)
        .expect_err("tampered input must diverge");
    match err {
        ReplayError::Divergence { tick, .. } => {
            assert_eq!(
                tick, 540,
                "first hash checkpoint after the tampered tick (500) is 540"
            );
        }
        other => panic!("expected divergence, got {other}"),
    }
}

#[test]
fn header_mismatches_warn_before_playback() {
    let dir = std::env::temp_dir().join("fulcrum-replay-warns");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("run.freplay");
    record(&path, 10);

    let replay = Replay::load(&path).unwrap();
    let mut app = build_app(0);
    // Pretend a mod is loaded that the recording run didn't have.
    app.world_mut().insert_resource(ReplayModSet(vec![(
        "extra_mod".to_string(),
        "1.0.0".to_string(),
    )]));
    let warnings = app.start_playback(replay);
    assert!(
        warnings.iter().any(|w| w.contains("mod set mismatch")),
        "expected a mod set warning, got {warnings:?}"
    );

    // Identical setup warns about nothing.
    let replay = Replay::load(&path).unwrap();
    assert_eq!(build_app(0).start_playback(replay), Vec::<String>::new());
}

#[test]
fn bad_magic_is_rejected() {
    let dir = std::env::temp_dir().join("fulcrum-replay-magic");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("not-a-replay.freplay");
    std::fs::write(&path, b"GREPLAY\x01junk").unwrap();
    assert!(matches!(Replay::load(&path), Err(ReplayError::BadMagic)));
}
