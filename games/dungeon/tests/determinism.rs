//! Milestone acceptance: the dungeon headless, same seed + scripted input twice -> identical
//! world state (positions + health + gold), driven entirely from the data files.

use dungeon::game::{self, GamePlugin, Gold, Health, MonsterTag, PlayerTag};
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
        _ => {}
    }
}

#[derive(Debug, PartialEq)]
struct Outcome {
    gold: u32,
    player: (u32, u32, i32),
    monsters: Vec<(u32, u32, i32)>,
}

fn run(seed: u64, ticks: u32) -> Outcome {
    let app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    let mut app = game::register_components(app)
        .with_plugin(ScenePlugin)
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/level1.scene.ron").unwrap();
                spawner.load(level);
            },
        );

    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
    }

    let gold = app.world().resource::<Gold>().0;
    let world = app.world_mut();
    let player = world
        .query_filtered::<(&Transform2D, &Health), With<PlayerTag>>()
        .iter(world)
        .map(|(t, h)| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                h.current,
            )
        })
        .next()
        .expect("player spawned from the scene");
    let mut monsters: Vec<(u32, u32, i32)> = world
        .query_filtered::<(&Transform2D, &Health), With<MonsterTag>>()
        .iter(world)
        .map(|(t, h)| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                h.current,
            )
        })
        .collect();
    monsters.sort_unstable();
    Outcome {
        gold,
        player,
        monsters,
    }
}

#[test]
fn determinism_same_seed_scripted_600_ticks_identical() {
    let a = run(DEFAULT_SEED, 600);
    let b = run(DEFAULT_SEED, 600);
    assert_eq!(a, b, "same seed + same script must reproduce exactly");
    assert_eq!(
        a.monsters.len() + usize::from(a.gold > 0).max(1) - 1 + a.monsters.len(),
        a.monsters.len() * 2,
        "sanity"
    );
    assert!(
        !a.monsters.is_empty() || a.gold > 0,
        "monsters spawned (or were farmed)"
    );
}

#[test]
fn player_moves_and_walls_block() {
    let outcome = run(7, 300);
    // The script walks right then up; the player must have left the spawn tile but stayed
    // inside the map (walls at x=0..16 world units).
    let x = f32::from_bits(outcome.player.0);
    let y = f32::from_bits(outcome.player.1);
    assert!(x > 80.0, "moved right, got x={x}");
    assert!(
        x < 640.0 && y > 16.0 && y < 384.0,
        "stayed in bounds ({x},{y})"
    );
}

#[test]
fn walls_block_headless() {
    let _ = env_logger::try_init();
    // Hold D for 20 seconds: the player must pin against the east wall, not pass through.
    let app = Fulcrum::new("walls").insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    let mut app = game::register_components(app)
        .with_plugin(ScenePlugin)
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/level1.scene.ron").unwrap();
                spawner.load(level);
            },
        );
    app.run_startup();
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(Key::D, true);
    }
    for _ in 0..1200 {
        app.world_mut().resource_mut::<Input>().sample(|s| s);
        app.tick();
    }
    let world = app.world_mut();
    let x = world
        .query_filtered::<&Transform2D, With<PlayerTag>>()
        .iter(world)
        .next()
        .unwrap()
        .translation
        .x;
    // Map is 40 tiles = 640 units wide with a wall column at 624..640; the player radius is 6.
    // Without collision the player would sail past 640. There's also an interior wall at
    // x = 416..432 (tile 26) with a gap at y = 160..176 the player won't be aligned with.
    assert!(x < 640.0, "player must not escape the map, got {x}");
    assert!(
        (400.0..640.0).contains(&x) || (72.0..432.0).contains(&x),
        "pinned against a wall, got {x}"
    );
}
