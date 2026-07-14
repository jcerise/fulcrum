//! Grove runs headless from its data files: same seed + same input = same world, and the game
//! is actually winnable by walking to gems.

use fulcrum::prelude::*;
use grove::game::{self, GamePlugin, Gems, GroveState, PlayerTag};

fn build() -> Fulcrum {
    let app = Fulcrum::new("grove test").insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    game::register_components(app)
        .with_plugin(ScenePlugin)
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/grove.scene.ron").unwrap();
                spawner.load(level);
            },
        )
}

fn run(seed: u64, ticks: u32) -> (u32, u32, u32) {
    let mut app = build();
    app.world_mut().resource_mut::<FulcrumConfig>().seed = seed; // documented: seed set pre-run
    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            // Wander the grove: right, then up, then left along the dirt path.
            match tick {
                0 => input.push_key(Key::D, true),
                160 => {
                    input.push_key(Key::D, false);
                    input.push_key(Key::W, true);
                }
                320 => {
                    input.push_key(Key::W, false);
                    input.push_key(Key::A, true);
                }
                _ => {}
            }
            input.sample(|s| s);
        }
        app.tick();
    }
    let gems = *app.world().resource::<Gems>();
    let world = app.world_mut();
    let x = world
        .query_filtered::<&Transform2D, With<PlayerTag>>()
        .iter(world)
        .next()
        .unwrap()
        .translation
        .x
        .to_bits();
    (gems.collected, gems.total, x)
}

#[test]
fn determinism_scripted_run_is_identical_and_playable() {
    let a = run(DEFAULT_SEED, 600);
    let b = run(DEFAULT_SEED, 600);
    assert_eq!(a, b, "same seed + same script must reproduce exactly");
    assert_eq!(a.1, 8, "the scene spawns 8 gems");
}

#[test]
fn walking_over_a_gem_collects_it() {
    let mut app = build();
    app.run_startup();
    // Head toward the gem at (210, 170) from spawn (72, 72): right then up.
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
                _ => {}
            }
            input.sample(|s| s);
        }
        app.tick();
        if app.world().resource::<Gems>().collected > 0 {
            return; // collected!
        }
    }
    let world = app.world_mut();
    let at = world
        .query_filtered::<&Transform2D, With<PlayerTag>>()
        .iter(world)
        .next()
        .unwrap()
        .translation;
    panic!(
        "no gem collected in 600 ticks; player ended at {at:?}, state {:?}",
        app.world().resource::<GroveState>()
    );
}
