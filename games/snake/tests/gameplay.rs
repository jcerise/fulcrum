//! The From Zero track's payoff, in code: Snake is a real program, so test it like one.
//! No window, no GPU, no sleeps — the simulation steps by hand and the assertions read
//! plain state. See the book's "Proof: Testing Your Game" chapter.

use fulcrum::prelude::*;
use snake::game::{Apple, GRID_H, GRID_W, GamePlugin, OnCell, Score, Snake, SnakeState};

/// The whole game, minus everything visible. This is the same `GamePlugin` the binary runs.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin)
}

/// Press exactly one key for one tick (press + release straddling a sample).
fn tap(app: &mut Fulcrum, key: Key) {
    let mut input = app.world_mut().resource_mut::<Input>();
    input.push_key(key, true);
    input.sample(|s| s);
    app.tick();
    let mut input = app.world_mut().resource_mut::<Input>();
    input.push_key(key, false);
    input.sample(|s| s);
    app.tick();
}

/// Advance `n` ticks with no input held.
fn run_ticks(app: &mut Fulcrum, n: u32) {
    for _ in 0..n {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
    }
}

#[test]
fn driving_into_the_wall_ends_the_run() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Head starts at (12, 9) moving right; the wall is 12 cells (96 ticks) away.
    run_ticks(&mut app, 120);
    assert_eq!(*app.world().resource::<SnakeState>(), SnakeState::GameOver);
    // Enter brings a fresh snake back.
    tap(&mut app, Key::Enter);
    assert_eq!(*app.world().resource::<SnakeState>(), SnakeState::Playing);
    assert_eq!(app.world().resource::<Snake>().body.len(), 3);
}

#[test]
fn a_greedy_bot_eats_apples() {
    // The test *plays* the game: every tick, steer one axis at a time toward the apple.
    // Determinism makes this meaningful — this exact game happens every run.
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    for _ in 0..3600 {
        let (head, dir) = {
            let snake = app.world().resource::<Snake>();
            (snake.head(), snake.dir)
        };
        let apple = {
            let world = app.world_mut();
            world
                .query_filtered::<&OnCell, With<Apple>>()
                .iter(world)
                .next()
                .map(|on| on.0)
        };
        let key = apple.and_then(|apple| {
            // Prefer closing the x gap, then the y gap; never press the reverse direction.
            let candidates = [
                (apple.0 > head.0, Key::D, (1, 0)),
                (apple.0 < head.0, Key::A, (-1, 0)),
                (apple.1 > head.1, Key::W, (0, 1)),
                (apple.1 < head.1, Key::S, (0, -1)),
            ];
            candidates
                .into_iter()
                .find(|(wanted, _, d)| *wanted && (d.0 != -dir.0 || d.1 != -dir.1))
                .map(|(_, key, _)| key)
                // Apple dead behind us: a reversal is forbidden, so sidestep first —
                // perpendicular, toward the middle of the grid.
                .or(Some(match dir {
                    (x, 0) if x != 0 => {
                        if head.1 < GRID_H / 2 {
                            Key::W
                        } else {
                            Key::S
                        }
                    }
                    _ => {
                        if head.0 < GRID_W / 2 {
                            Key::D
                        } else {
                            Key::A
                        }
                    }
                }))
        });
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            if let Some(key) = key {
                input.push_key(key, true);
            }
            input.sample(|screen| screen);
        }
        app.tick();
        // Release whatever we pressed so the next press is an edge again.
        if let Some(key) = key {
            app.world_mut().resource_mut::<Input>().push_key(key, false);
        }
        if app.world().resource::<Score>().0 >= 5 {
            break;
        }
    }
    let score = app.world().resource::<Score>().0;
    assert!(score >= 5, "the bot should eat 5 apples; got {score}");
    // Two apples eaten = four segments grown.
    assert_eq!(
        app.world().resource::<Snake>().body.len() as u32,
        3 + score * 2 - app.world().resource::<Snake>().grow
    );
}

#[test]
fn determinism_same_seed_same_game() {
    // Determinism, the property every other test stands on: identical seed and input
    // stream => bit-identical outcome. Exact equality — no epsilons.
    let fingerprint = |seed: u64| {
        let mut app = build(seed);
        app.run_startup();
        run_ticks(&mut app, 90);
        let apple = {
            let world = app.world_mut();
            world
                .query_filtered::<&OnCell, With<Apple>>()
                .iter(world)
                .next()
                .map(|on| on.0)
        };
        let world = app.world();
        (
            world.resource::<Snake>().body.clone(),
            apple,
            *world.resource::<Score>(),
            *world.resource::<SnakeState>(),
        )
    };
    assert_eq!(fingerprint(7), fingerprint(7), "same seed, same game");
    assert_ne!(
        fingerprint(7).1,
        fingerprint(8).1,
        "different seeds place the apple differently"
    );
}
