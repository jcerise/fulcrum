//! Milestone acceptance: Asteroids headless, same seed + scripted input twice, identical state.

use asteroids::game::{Asteroid, GamePlugin, Lives, Score, Ship, Velocity};
use fulcrum::prelude::*;

/// Thrust in bursts, rotate continuously, fire constantly — chaotic enough to hit rocks.
fn script(input: &mut Input, tick: u32) {
    match tick {
        5 => {
            input.push_key(Key::Space, true);
            input.push_key(Key::A, true);
        }
        100 => input.push_key(Key::W, true),
        220 => input.push_key(Key::W, false),
        260 => {
            input.push_key(Key::A, false);
            input.push_key(Key::D, true);
        }
        400 => input.push_key(Key::W, true),
        _ => {}
    }
}

#[derive(Debug, PartialEq)]
struct Outcome {
    score: u32,
    lives: u32,
    ship: (u32, u32, u32),
    rocks: Vec<(u32, u32)>,
}

fn run(seed: u64, ticks: u32) -> Outcome {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
    }

    let score = app.world().resource::<Score>().0;
    let lives = app.world().resource::<Lives>().0;
    let world = app.world_mut();
    let ship = world
        .query_filtered::<(&Transform2D, &Velocity), With<Ship>>()
        .iter(world)
        .map(|(t, v)| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                v.0.x.to_bits(),
            )
        })
        .next()
        .unwrap();
    let mut rocks: Vec<(u32, u32)> = world
        .query_filtered::<&Transform2D, With<Asteroid>>()
        .iter(world)
        .map(|t| (t.translation.x.to_bits(), t.translation.y.to_bits()))
        .collect();
    rocks.sort_unstable();
    Outcome {
        score,
        lives,
        ship,
        rocks,
    }
}

#[test]
fn determinism_same_seed_scripted_600_ticks_identical() {
    let a = run(DEFAULT_SEED, 600);
    let b = run(DEFAULT_SEED, 600);
    assert_eq!(a, b, "same seed + same script must reproduce exactly");
    assert!(!a.rocks.is_empty(), "waves spawned rocks");
}

#[test]
fn firing_spree_scores_points() {
    // A minute of spinning fire: statistically certain to hit something, and the seed is fixed
    // so the result is reproducible.
    let outcome = run(1234, 3600);
    assert!(
        outcome.score > 0,
        "a minute of fire should score, got {outcome:?}"
    );
}
