//! Milestone acceptance: Pong headless, same seed + scripted input twice, identical outcomes.

use fulcrum::prelude::*;
use pong::game::{Ball, GamePlugin, Paddle, Score, Velocity};

/// Scripted input: hold W for a stretch, then S, with a tap in the middle — enough to move the
/// paddle and change rally outcomes.
fn script(input: &mut Input, tick: u32) {
    match tick {
        20 => input.push_key(Key::W, true),
        180 => input.push_key(Key::W, false),
        200 => input.push_key(Key::S, true),
        205 => {
            // Fast tap inside one tick window.
            input.push_key(Key::W, true);
            input.push_key(Key::W, false);
        }
        450 => input.push_key(Key::S, false),
        _ => {}
    }
}

#[derive(Debug, PartialEq)]
struct Outcome {
    score: (u32, u32),
    ball_bits: Vec<(u32, u32, u32, u32)>,
    paddle_bits: Vec<u32>,
}

fn run(seed: u64, ticks: u32) -> Outcome {
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
            input.sample(vec2(800.0, 600.0));
        }
        app.tick();
    }

    let score = *app.world().resource::<Score>();
    let world = app.world_mut();
    let ball_bits = world
        .query_filtered::<(&Transform2D, &Velocity), With<Ball>>()
        .iter(world)
        .map(|(t, v)| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                v.0.x.to_bits(),
                v.0.y.to_bits(),
            )
        })
        .collect();
    let paddle_bits = world
        .query_filtered::<&Transform2D, With<Paddle>>()
        .iter(world)
        .map(|t| t.translation.y.to_bits())
        .collect();
    Outcome {
        score: (score.player, score.ai),
        ball_bits,
        paddle_bits,
    }
}

#[test]
fn same_seed_scripted_600_ticks_identical() {
    let a = run(fulcrum::prelude::DEFAULT_SEED, 600);
    let b = run(fulcrum::prelude::DEFAULT_SEED, 600);
    assert_eq!(a, b, "same seed + same script must reproduce exactly");
    assert!(
        a.score.0 + a.score.1 > 0,
        "10 seconds of play should produce at least one point, got {:?}",
        a.score
    );
}

#[test]
fn game_actually_plays_out() {
    let outcome = run(7, 3600); // one minute
    let total = outcome.score.0 + outcome.score.1;
    assert!(
        total >= 3,
        "a minute of pong scores several points, got {outcome:?}"
    );
}
