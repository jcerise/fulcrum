//! Determinism gate: run the simulation headless, same seed + scripted input twice, and
//! require bit-identical outcomes. Keep this passing as your game grows — it is the cheapest
//! way to catch frame-side state leaking into the simulation.

use fulcrum::prelude::*;
use my_game::game::{GamePlugin, Player};

/// Scripted input: a diagonal run, a direction change, and a fast tap inside one tick window.
fn script(input: &mut Input, tick: u32) {
    match tick {
        10 => {
            input.push_key(Key::W, true);
            input.push_key(Key::D, true);
        }
        120 => input.push_key(Key::D, false),
        150 => input.push_key(Key::S, true),
        155 => {
            // Fast tap inside one tick window.
            input.push_key(Key::A, true);
            input.push_key(Key::A, false);
        }
        240 => {
            input.push_key(Key::W, false);
            input.push_key(Key::S, false);
        }
        _ => {}
    }
}

/// Run the sim for `ticks` and return the player transform as exact bits.
fn run(seed: u64, ticks: u32) -> Vec<(u32, u32)> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (800, 600),
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

    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, With<Player>>()
        .iter(world)
        .map(|t| (t.translation.x.to_bits(), t.translation.y.to_bits()))
        .collect()
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 300);
    let b = run(42, 300);
    assert!(!a.is_empty(), "simulation spawned no player");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}
