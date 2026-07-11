# Proof: Testing Your Game

Games have a reputation as untestable — QA armies, "playtest it again," bugs that only
happen sometimes. That reputation comes from engines that let wall clocks, frame rates, and
unseeded randomness leak into game logic, making every run unrepeatable. You've spent five
chapters not doing that, on purpose. This chapter is where the discipline pays out: Snake is
about to be tested like the deterministic function it is. Everything here is real code from
`games/snake/tests/gameplay.rs`:

```text
cargo test -p snake
```

## A headless game

The test builds the *entire game* — same `GamePlugin`, same rules, same RNG — with the
presentation simply absent:

```rust,ignore
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig { seed, ..Default::default() })
        .with_plugin(GamePlugin)   // and nothing else: no window, no DefaultPlugins
}
```

That compiles and runs — on a CI box with no GPU, no display, no audio device — because of
chapter 5's line: the simulation never learned that pixels exist. And where a real player's
keystrokes arrive through the window, a test pushes them by hand and steps time itself:

```rust,ignore
let mut input = app.world_mut().resource_mut::<Input>();
input.push_key(Key::Enter, true);
input.sample(|s| s);   // fold pending input, exactly as the engine does each tick
app.tick();            // advance the simulation by exactly 1/60th of a second
```

Look at what's *missing* from that snippet: no `sleep`, no "wait for the game to settle," no
timeouts. `app.tick()` is the loop from chapter 1 with you holding the crank. A thousand
ticks of Snake — sixteen seconds of gameplay — runs in about a millisecond, so tests can
play whole games, and the flakiness that haunts UI testing (its root cause is always *real
time*) has nothing to grab onto.

## Test the corner where the rules live

The first test aims exactly where chapter 4 said rules concentrate — the corner case:

```rust,ignore
#[test]
fn driving_into_the_wall_ends_the_run() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    run_ticks(&mut app, 120);          // head starts 12 cells from the wall, undriven
    assert_eq!(*app.world().resource::<SnakeState>(), SnakeState::GameOver);
    tap(&mut app, Key::Enter);         // and the restart actually restarts
    assert_eq!(*app.world().resource::<SnakeState>(), SnakeState::Playing);
    assert_eq!(app.world().resource::<Snake>().body.len(), 3);
}
```

The assertions read plain resources — `SnakeState`, `Snake` — because the game's truth *is*
its state (chapter 2, cashing out one more time). No screen-scraping, no image diffing: the
test asks the world what's true the same way the rules do.

## The bot: a test that plays

The second test is the fun one. It plays Snake — a greedy little bot that reads the apple's
cell each tick and presses a key toward it:

```rust,ignore
let key = apple.and_then(|apple| {
    let candidates = [
        (apple.0 > head.0, Key::D, (1, 0)),
        (apple.0 < head.0, Key::A, (-1, 0)),
        (apple.1 > head.1, Key::W, (0, 1)),
        (apple.1 < head.1, Key::S, (0, -1)),
    ];
    candidates.into_iter()
        .find(|(wanted, _, d)| *wanted && (d.0 != -dir.0 || d.1 != -dir.1))
        .map(|(_, key, _)| key)
        .or(Some(/* apple dead behind us: sidestep perpendicular first */))
});
// press it, tick, release, repeat — until Score reaches 5
```

Twenty lines of policy, and the assertion is the one that matters most in this whole track:
**the game is winnable by playing it.** Not "the collision function returns true" — apples
get eaten, by input, through every rule at once. It even asserts the arithmetic of growth
(`3 + score * 2` segments) as an invariant along the way. And because the apples come from
`SimRng`, this exact game — same apples, same path, same score — happens every single run,
on your machine and CI's. A flaky end-to-end test of a randomized game would be worthless;
a deterministic one is a regression trap for every rule you wrote. (The bot's sidestep
fallback exists because its first version drove straight into a wall whenever the apple
spawned directly behind it — the test's own corner case, found the honest way.)

## The keystone: same seed, same game

The last test asserts determinism itself:

```rust,ignore
assert_eq!(fingerprint(7), fingerprint(7), "same seed, same game");
```

where `fingerprint` plays 90 ticks and returns the exact body cells, apple cell, score, and
state. Exact equality — on a game with randomness in it — no epsilons, no "approximately."
It looks almost too silly to keep. Keep it. It's not testing Snake; it's testing that Snake
*stayed a pure function*, and it's the tripwire that catches the day someone (you) reaches
for the wall clock or an unseeded random in a simulation system. When this test fails, no
other test's verdict means anything. Engines in this family treat that property as
infrastructure: Fulcrum's CI plays scripted runs of every game in the repository, twice,
release mode, every commit, and fails the build on a single divergent bit.

## Where you are, and where the rest of the book goes

Count what you actually learned by shipping this toy: the loop and the two clocks; state as
truth and views as projections; ECS as a queryable world; intent buffering; seeded
randomness; rules-in-corner-cases; the sim/presentation split; events; and games as testable,
deterministic functions. That is not beginner cargo — that's the working mental model of the
whole discipline, and every game you build from here is these ideas with more nouns.

The rest of the book assumes exactly this model and moves fast. [Grove](ch01-window.md)
(chapters 1–10) rebuilds it at full speed with real content — sprite sheets, animation,
tilemaps, data-driven entities, UI — and [Part II](ch11-particles.md) adds the power tools:
particles, pathfinding, mods, and replays, where the determinism you just proved becomes a
shareable file that reproduces a whole game from its inputs.

You've made a game. The next one's yours.
