# Proof: Testing Your Game

Games have a reputation as untestable — QA armies, "playtest it again," bugs that only
happen sometimes. That reputation comes from engines that let wall clocks, frame rates, and
unseeded randomness leak into game logic, making every run unrepeatable. You've spent five
chapters not doing that, on purpose. This chapter is where the discipline pays out: you'll
write three tests, and one of them *plays the game*.

## Step 1 — a headless game, and hands to play it

Create `games/my-snake/tests/gameplay.rs`. The first function builds the *entire game* —
same `GamePlugin`, same rules, same RNG — with the presentation simply absent:

```rust,ignore
use fulcrum::prelude::*;
use my_snake::game::{Apple, GRID_H, GRID_W, GamePlugin, OnCell, Score, Snake, SnakeState};

/// The whole game, minus everything visible. This is the same `GamePlugin` the binary runs.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin)
}
```

Compare it to `main`: no `DefaultPlugins`, no `.run()`. That compiles and runs — on a CI box
with no GPU, no display, no audio device — because of chapter 5's line: the simulation never
learned that pixels exist. And there's the `seed` field `FulcrumConfig` has been carrying
since chapter 5, finally set by hand: it feeds `SimRng`, so a test chooses its apples.

Where a real player's keystrokes arrive through the window, a test pushes them directly and
steps time itself. Two helpers you'll use in every test:

```rust,ignore
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
```

> **Toolbox — `app.tick()`:** advance the simulation by exactly one tick — 1/60th of a
> second — running the same `FixedUpdate` systems the windowed game runs. This is chapter
> 1's loop with *you* holding the crank. Its startup sibling is `app.run_startup()`, which
> fires the `Startup` systems (like `spawn_first_apple`) once.
>
> **Toolbox — `Input::push_key` / `sample`:** `push_key(key, down)` queues a raw key event,
> exactly as the window would; `sample(...)` folds queued events into the tick's snapshot,
> exactly as the engine does before each tick. Chapter 3 promised the input snapshot
> sequence was well-defined data; this is that promise, being spent. Note `tap` samples the
> *release* too — otherwise the key stays held and the next `just_pressed` never fires.
>
> **Toolbox — `world()` / `world_mut()`:** direct access to the world outside any system.
> `world.resource::<T>()` reads state; `world_mut().resource_mut::<T>()` writes it. Systems
> ask politely via parameters; tests are allowed to just reach in.

Look at what's *missing*: no `sleep`, no "wait for the game to settle," no timeouts. A
thousand ticks of Snake — sixteen seconds of gameplay — runs in about a millisecond, so
tests can play whole games, and the flakiness that haunts UI testing (its root cause is
always *real time*) has nothing to grab onto.

## Step 2 — test the corner where the rules live

The first test aims exactly where chapter 4 said rules concentrate:

```rust,ignore
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
```

Run it now — `cargo test -p my-snake` — and watch it pass in milliseconds. The assertions
read plain resources — `SnakeState`, `Snake` — because the game's truth *is* its state
(chapter 2, cashing out one more time). No screen-scraping, no image diffing: the test asks
the world what's true the same way the rules do. (`DEFAULT_SEED` comes from the prelude; any
`u64` works.)

## Step 3 — the bot: a test that plays

The second test is the fun one. It plays Snake — a greedy little bot that reads the apple's
cell each tick and presses a key toward it:

```rust,ignore
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
    // Each apple eaten = two segments grown (minus any still owed).
    assert_eq!(
        app.world().resource::<Snake>().body.len() as u32,
        3 + score * 2 - app.world().resource::<Snake>().grow
    );
}
```

(One new tool rode along: `world.query_filtered::<&OnCell, With<Apple>>()` is a chapter-2
query built *outside* a system — same `SELECT`, test-side.)

Thirty lines of policy, and the assertion is the one that matters most in this whole track:
**the game is winnable by playing it.** Not "the collision function returns true" — apples
get eaten, by input, through `steer`'s buffer and `step`'s rules at once. It even asserts
the arithmetic of growth (`3 + score * 2`, less anything still owed) as an invariant along
the way. And because the apples come from `SimRng`, this exact game — same apples, same
path, same score — happens every single run, on your machine and CI's. A flaky end-to-end
test of a randomized game would be worthless; a deterministic one is a regression trap for
every rule you wrote.

(A confession, because it's instructive: the bot's sidestep fallback exists because its
first version drove straight into a wall whenever the apple spawned directly behind it — the
test's own corner case, found the honest way.)

## Step 4 — the keystone: same seed, same game

The last test asserts determinism itself:

```rust,ignore
#[test]
fn same_seed_same_game() {
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
```

Exact equality — on a game with randomness in it — no epsilons, no "approximately." It looks
almost too silly to keep. Keep it. It's not testing Snake; it's testing that Snake *stayed a
pure function*, and it's the tripwire that catches the day someone (you) reaches for the
wall clock or an unseeded random in a simulation system. When this test fails, no other
test's verdict means anything. Engines in this family treat that property as infrastructure:
Fulcrum's CI plays scripted runs of every game in the repository, twice, release mode, every
commit, and fails the build on a single divergent bit.

## Checkpoint

```text
cargo test -p my-snake
```

Three passing tests, a few milliseconds each. The reference is
`games/snake/tests/gameplay.rs`, and this time the comparison is the whole point: the
shipped game's test suite is *exactly what you just wrote*.

New vocabulary this chapter:

| Tool | What it's for |
| --- | --- |
| `FulcrumConfig { seed, .. }` | choose the `SimRng` stream; same seed, same game |
| `app.run_startup()` / `app.tick()` | fire startup once; advance the sim one tick, by hand |
| `Input::push_key` + `sample` | inject input exactly as the window would |
| `world()` / `world_mut()` | read and write state directly, test-side |
| `world.query_filtered::<..>()` | run a query outside any system |

## Exercises

The best ones in the book, because this chapter's skill compounds: every test you add makes
every future change cheaper.

1. **Test the turn buffer.** Chapter 3's proudest code has no test. Write one: within a
   single 8-tick step window, tap up then left; assert the snake's path shows both turns,
   one step apart. You'll need `tap`-like input plumbing with your own timing — build a small
   helper; test helpers are load-bearing code here, same as anywhere.
2. **Test the tail corner case.** Chapter 4's exercise had you *reach* the
   tail-cell situation by playing; now script it. Two assertions: entering the tail's cell
   while not growing is survival, entering it just after eating is death. If you can steer a
   test snake into a tight spiral on demand, you have fully internalized this track's input
   and time model — this is the hardest exercise here, and the most worth doing.
3. **Mutation testing, by hand (harder).** Comment out the reversal check in `steer` and run
   the suite. It passes. That should bother you — a rule with no test isn't guaranteed, it's
   lucky (the bot never presses a reversing key, so nothing notices). Write the test that
   fails: press the exact reverse of the current direction, assert the snake does *not* die
   into its own neck. Then restore the check, and carry the habit with you: when you're
   unsure a test suite earns its keep, break the code on purpose and see who complains.

## Where you are, and where the rest of the book goes

Count what you actually built and learned by shipping this toy: the loop and the two clocks;
state as truth and views as projections; ECS as a queryable world; intent buffering; seeded
randomness; rules-in-corner-cases; the sim/presentation split; events; plugins; and games as
testable, deterministic functions. That is not beginner cargo — that's the working mental
model of the whole discipline, and every game you build from here is these ideas with more
nouns.

The rest of the book assumes exactly this model and moves fast. [Grove](ch01-window.md)
(chapters 1–10) rebuilds it at full speed with real content — sprite sheets, animation,
tilemaps, data-driven entities, UI — and [Part II](ch11-particles.md) adds the power tools:
particles, pathfinding, mods, and replays, where the determinism you just proved becomes a
shareable file that reproduces a whole game from its inputs.

You've made a game — your own crate, from `cargo new` to a green test suite. The next one's
yours too.
