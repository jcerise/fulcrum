# Proof: Testing Animation

The dojo's mechanics live on animation frames, so testing the dojo *means* testing
animation: that machines enter the right states on the right ticks, that the strike lands
exactly when the extension frame shows, that the whole fight is a deterministic function of
its inputs. Most engines can't write these tests at all — animation happens on the render
clock, and there's no render clock in CI. You've spent four chapters on the architecture
that makes them ordinary.

Everything here goes in `games/my-dojo/tests/gameplay.rs`; the shipped version is
`games/dojo/tests/gameplay.rs`.

## Step 1 — a headless dojo

```rust,ignore
use fulcrum::prelude::*;
use my_dojo::game::{Bonks, Dummy, GamePlugin, Hero, STRIKE_FRAME};

/// The whole game, minus everything visible — including the real machines and clip timing,
/// loaded from `assets/` by the same startup system the binary runs.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
    .with_plugin(AnimPlugin)
    .with_plugin(GamePlugin)
}
```

Two lines deserve a pause:

> **Toolbox — `AnimPlugin`, explicitly:** windowed, `DefaultPlugins` installed it for you.
> Headless there are no default plugins, and the dojo's sim genuinely requires the
> animation system — its clip storage and its two `FixedUpdate` systems (drive machines,
> advance players). One line declares that dependency honestly.
>
> **Why this works at all:** `spawn_dojo` calls `AnimatorLoader::load` — the same call, the
> same RON files, the same Aseprite JSONs as the windowed game. Clip *timing* and sheet
> *regions* are simulation data and load without a GPU; only texture upload needs one, and
> headless it's skipped. So the tests below run the real machines with the real frame
> durations — not stubs — on a CI box with no display. When the artist retimes the attack,
> these tests feel it, which is exactly what "animation is gameplay" should mean.

Add the input helpers you know from the other Fulcrum test suites (`run_ticks`, `hold`),
plus one animation-flavored one:

```rust,ignore
fn hero_state(app: &mut Fulcrum) -> (String, usize) {
    let world = app.world_mut();
    let (animator, player) = world
        .query_filtered::<(&Animator, &AnimationPlayer), With<Hero>>()
        .single(world)
        .expect("one hero");
    (animator.state().to_string(), player.frame_index)
}
```

The HUD from chapter 4, as a function: machine state and current frame are component data,
so a test reads them the way it reads a position.

## Step 2 — the headline test

```rust,ignore
#[test]
fn the_strike_connects_on_the_extension_frame() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Walk into range (hero spawns at x=96, dummy at x=240, strike range 26).
    hold(&mut app, Key::D, 82);

    // Swing, then watch every tick: the bonk must land on the exact tick the attack clip
    // shows its extension frame — not when Space was pressed, ~11 ticks earlier.
    app.world_mut().resource_mut::<Input>().push_key(Key::Space, true);
    let mut connect = None;
    for tick in 0..40 {
        app.world_mut().resource_mut::<Input>().sample(|screen| screen);
        app.tick();
        app.world_mut().resource_mut::<Input>().push_key(Key::Space, false);
        if bonks(&app).landed == 1 && connect.is_none() {
            connect = Some((tick, hero_state(&mut app)));
        }
    }
    let (tick, (state, frame)) = connect.expect("the strike landed");
    assert_eq!((state.as_str(), frame), ("attack", STRIKE_FRAME));
    assert!(tick >= 10, "landed on tick {tick}, before the blade was out");
}
```

Read the two assertions as one sentence: *when the hit registered, the hero was in the
attack state showing the strike frame, and enough ticks had passed for the windup and sweep
to have actually played.* That second assertion is the anti-drift tripwire — if someone
"optimizes" the hit to fire on the input press, `tick >= 10` fails and names the crime.

## Step 3 — the negative spaces

A rule is only as tested as its edges. Two short ones:

```rust,ignore
#[test]
fn a_whiff_is_a_whiff() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Swing from the spawn point, far out of range: nothing anywhere should count it.
    hold(&mut app, Key::Space, 2);
    run_ticks(&mut app, 40);
    assert_eq!(bonks(&app), Bonks { landed: 0, taken: 0 });
    assert_eq!(hero_state(&mut app).0, "idle", "attack finished and went home");
}
```

The whiff test quietly checks half the machine: the attack *ran* (controls were locked —
you could assert that too) and `on_finish` sent it home with nothing counted. And the
rebound gets the test its comedy deserves:

```rust,ignore
#[test]
fn the_dummy_fights_back_at_point_blank() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    hold(&mut app, Key::D, 96);        // all the way to the dummy
    hold(&mut app, Key::Space, 2);     // swing from inside rebound range
    let mut was_hit = false;
    for _ in 0..60 {
        run_ticks(&mut app, 1);
        was_hit |= hero_state(&mut app).0 == "hit";
    }
    assert_eq!(bonks(&app), Bonks { landed: 1, taken: 1 });
    assert!(was_hit, "the rebound interrupted the hero into the hit state");
    assert_eq!(hero_state(&mut app).0, "idle", "and the hit state went home too");
}
```

`was_hit` asserts the *interruption*: the hero was mid-attack when the rebound arrived, and
the machine's `Any → hit` outranked everything — the line order you set in chapter 3,
finally under test.

## Step 4 — the keystone

Every Fulcrum game closes with this test, and the dojo's version carries extra weight
because *animation state itself* is in the fingerprint:

```rust,ignore
#[test]
fn determinism_same_seed_same_dojo() {
    let fingerprint = |seed: u64| {
        let mut app = build(seed);
        app.run_startup();
        hold(&mut app, Key::D, 90);
        hold(&mut app, Key::Space, 2);
        run_ticks(&mut app, 30);
        hold(&mut app, Key::A, 20);
        hold(&mut app, Key::Space, 2);
        run_ticks(&mut app, 40);
        // ...collect: Bonks, every (state, frame_index, tick_in_frame), every position
        // (f32s compared as to_bits) — see the shipped test for the plumbing.
    };
    assert_eq!(fingerprint(7), fingerprint(7), "same seed, same fight");
}
```

`tick_in_frame` in the fingerprint is the strictest claim in the book: not just "the same
states happened" but *every animation was on the same frame, the same number of ticks into
it, after the same scripted fight*. That's what "animation is simulation state" cashes out
to. If anyone ever moves animation advance onto the render clock, or keys a duration off
wall time, this test fails the build.

```text
cargo test -p my-dojo
```

Four tests, milliseconds each. (The engine crate proves the layer below — that machines
load from real files and produce exact per-tick timelines with no GPU — in
`crates/fulcrum-anim/tests/headless_load.rs`, if you want to see the same idea one level
down.)

## Exercises

1. **Test the lock.** The whiff test's comment claims controls lock during the attack.
   Assert it: hold D *while* swinging and prove the hero's x doesn't change until the
   attack state ends.
2. **Test the retime.** Chapter 4's exercise 1 stretched the windup to 300 ms. Predict the
   new minimum connect tick (300 ms ≈ 18 ticks, plus the sweep), adjust the headline test's
   `tick >= 10` bound to match, and confirm by running. A designer just changed game
   balance in a JSON file and your test suite priced it — sit with how unusual that is.
3. **Mutation test the guard (harder).** Comment out `hero.swing_connected = true;` in
   `strike` and run the suite. If everything still passes, the multi-hit bug has no
   tripwire — write the test that fails: one swing at point-blank must land exactly one
   bonk despite a 4-tick window. (Check the shipped suite before assuming it has this one
   covered.)

## Where you are

You've built the pipeline end to end: an export with tagged, timed frames; clips that
advance on the simulation clock; a data-driven state machine per fighter; gameplay keyed to
exact frames; and a test suite that would catch a millisecond of drift. The reference
chapters that follow are the same system laid out flat — every field, every rule, every
recipe — for the day your own game needs one more trick than the dojo did.
