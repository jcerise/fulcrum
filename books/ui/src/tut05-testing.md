# Proof: Testing the UI

"UI is untestable" is folklore from engines where the UI is a second program with its own
event loop. The Bazaar's UI is a projection of the sim, its clicks are commands, and its
layout loads from a file — so a test can grab any layer it wants: send the commands a
button would send, or go the whole way and *click the real layout with a synthetic
pointer*, headless, in milliseconds. This chapter writes both, and closes with the test
every Fulcrum game ends on.

Everything here goes in `games/my-bazaar/tests/gameplay.rs`; the shipped version is
`games/bazaar/tests/gameplay.rs`.

## Step 1 — the harness

```rust,ignore
use bazaar::game::{DAY_TICKS, DayClock, GamePlugin, GameState, PATIENCE, Queue, Takings, WARES};
use fulcrum::prelude::*;

/// The whole game, minus everything visible. `UiPlugin` stays: it owns the dispatcher
/// that turns `ui:click` commands back into `UiEvent`s, and it runs fine without a
/// window — no GPU means no textures, but the tree, ids, layout, and hit-testing are
/// all live. Most tests never load a layout at all: a click is just a command, and a
/// test can send the same commands the pointer would.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig { seed, ..Default::default() })
        .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
        .insert_resource(Assets::<Texture>::default())
        .insert_resource(Camera2D::default()) // Stretch: UI pixels == screen pixels
        .insert_resource(WindowInfo { width: 320, height: 180, scale_factor: 1.0 })
        .with_plugin(UiPlugin)
        .with_plugin(GamePlugin)
}

fn run_ticks(app: &mut Fulcrum, n: u32) {
    for _ in 0..n {
        app.world_mut().resource_mut::<Input>().sample(|screen| screen);
        app.tick();
    }
}
```

Three resources stand in for the window: a texture store (stays empty), a camera, and a
`WindowInfo`. The camera's default `Stretch` scaling makes UI pixels equal screen pixels —
the one configuration where a test can aim a cursor with no coordinate math.

> **Toolbox — headless UI loading:** `UiLoader` works without a GPU. Panel and image
> textures are skipped with a logged warning (there's nothing to upload to), so panels
> fall back to flat color and `Image` nodes become bare layout — but every node spawns,
> every id resolves, and layout solves real rectangles. Your skin is cosmetic; your
> *structure* is testable. This split is exactly why chapter 4 kept the theme out of the
> sim.

## Step 2 — a click is a command

```rust,ignore
/// What a button press becomes: the same command `interact_ui` sends on release. The
/// event is guaranteed readable "for that tick and the next", so run both before
/// asserting anything.
fn click(app: &mut Fulcrum, id: &str) {
    app.world_mut().resource_mut::<CommandOutbox>().send("ui:click", id);
    run_ticks(app, 2);
}
```

That helper *is* chapter 3's diagram, run backward: instead of pointer → command → event,
the test injects the command and lets the engine do the rest. First behavior test:

```rust,ignore
#[test]
fn the_right_ware_makes_a_sale() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // First arrival lands 90 ticks in.
    run_ticks(&mut app, 91);
    let wants = front_wants(&app);
    click(&mut app, WARES[wants].id);
    assert_eq!(takings(&app).served, 1);
    assert_eq!(takings(&app).gold, WARES[wants].price);
    assert!(app.world().resource::<Queue>().0.is_empty());
}
```

(`front_wants` and `takings` are three-line world-peeking helpers — see the shipped test.)
Note what this test *doesn't* know: no coordinates, no layout, no pointer. It tests the
contract between UI and sim — "a `serve_figs` click while figs are wanted sells figs" —
and it would keep passing if you redesigned the entire counter. Write its evil twin
(`the_wrong_ware_sells_nothing`: click `(wants + 1) % 3`, assert nothing moved), and the
pause pair from the shipped file: Esc via `push_key`, assert the clock freezes, `click`
resume, assert it thaws.

## Step 3 — the pointer path, end to end

One test should walk the *whole* road, layout file included — because ids can typo, panels
can cover buttons, and a moved button is a gameplay change in this game:

```rust,ignore
/// The full pointer path, headless: cursor over the real button's laid-out rect, press,
/// release — `interact_ui` emits the command, the dispatcher turns it into a `UiEvent`,
/// and the sim makes the sale. No window anywhere.
#[test]
fn a_pointer_click_on_the_real_layout_serves_a_customer() {
    let mut app = build(DEFAULT_SEED).add_startup(load_hud);
    app.run_startup();
    run_ticks(&mut app, 91);
    let frame = |app: &mut Fulcrum| app.world_mut().run_schedule(Update);
    frame(&mut app); // layout: every UiRect is now placed

    let wants = front_wants(&app);
    // ...find the serve button's on-screen rectangle by id:
    // Query<(&UiRect, &UiId)>, match id.0, take rect.center() — see the shipped test.
```

Then the click choreography, three frames like a real mouse:

```rust,ignore
    point(&mut app, None);        // move: hover
    frame(&mut app);
    point(&mut app, Some(true));  // press
    frame(&mut app);
    point(&mut app, Some(false)); // release: the `ui:click` command is queued
    frame(&mut app);
    assert!(app.world().resource::<UiFocus>().pointer_over_ui);
    run_ticks(&mut app, 2);       // command -> UiEvent -> sale
    assert_eq!(takings(&app).served, 1);
}
```

> **Toolbox — driving frames by hand:** sim tests call `app.tick()`; UI tests also need the
> frame schedule, and `world.run_schedule(Update)` is exactly what the window runner does
> once per rendered frame. Layout, interaction, and hot reload all live there. The
> choreography needs *separate* frames for press and release because `interact_ui` detects
> edges by comparing against the previous frame — mash press and release into one frame
> and nobody sees an edge. Which is not a test artifact: it's the actual timing contract
> your players' mice live under.
>
> **Toolbox — `UiRect`:** the layout system's output — one placed rectangle per visible
> node, in UI pixels, with a draw-order index. It's a component like any other, which is
> why a test can read button geometry with a query. Hidden nodes have no `UiRect` at all:
> "is the pause menu really gone?" is `is_none()`, not a screenshot diff.

## Step 4 — the keystone

Every Fulcrum game's test suite ends the same way, and now the claim covers menus:

```rust,ignore
#[test]
fn determinism_same_seed_same_bazaar() {
    let fingerprint = |seed: u64| {
        let mut app = build(seed);
        app.run_startup();
        // Three rounds of: wait for the market, then serve whoever's in front.
        for _ in 0..3 {
            run_ticks(&mut app, 300);
            let front = app.world().resource::<Queue>().0.first().map(|c| c.wants);
            if let Some(wants) = front {
                click(&mut app, WARES[wants].id);
            }
        }
        // A street call (world click, no UI over the pointer) hurries the next arrival.
        app.world_mut().resource_mut::<Input>().push_mouse_button(MouseButton::Left, true);
        run_ticks(&mut app, 120);
        // ...collect: takings, the queue's (wants, patience) pairs, the clock, the state —
        // see the shipped test for the plumbing.
    };
    assert_eq!(fingerprint(7), fingerprint(7), "same seed, same market day");
}
```

The script clicks buttons, clicks the street, and serves customers — every input class the
game has — and two runs from seed 7 produce identical worlds. That's the sentence from the
introduction, kept: *the UI is inside the deterministic boundary, because clicks are
commands.*

```text
cargo test -p my-bazaar
```

Seven tests, well under a second, no window ever opened.

## Where you are

Five chapters ago the screen was empty. Now: a layout language you can hot-edit, a HUD
driven honestly from sim state, buttons whose clicks replay, a skin system built from four
component fields and one idempotent system, and a test suite that clicks real buttons in
real layouts without a GPU. More usefully, you have the *shape* of Fulcrum UI in your
hands — data in files, projection every frame, commands across the boundary — which is the
shape everything in the reference section assumes.

## Exercises

1. **The last second.** Prove the patience boundary exactly: a customer with `1` tick left
   can still be served on that tick (chapter 3 ordered `serve_clicks` before `patience` —
   this test is why). You'll need `run_ticks` counts derived from `PATIENCE`, and you'll
   learn precisely when arrivals happen while you're at it.
2. **The street is deaf during pause.** Test that a world click while paused hurries
   nothing: pause, `push_mouse_button`, run, assert `NextArrival` unmoved. Then check the
   subtler half — does your test *prove* the guard, or would it pass anyway because no
   click reached the sim? Make the same click work while trading, in the same test, so the
   negative half can't lie to you.
3. **Mutation test the guard (harder).** Comment out the `*state != GameState::Trading`
   guard in `serve_clicks`, run the suite, and see which test fails. If the answer is
   "none", you've found a hole — write the test that catches it (serve during `DayOver`
   should sell nothing), then restore the guard and watch it pass. A guard nobody can
   catch missing is a guard you don't have.

That's the tutorial. The reference section starts with the layout engine's exact semantics
— every rule the panels have been obeying while you weren't looking.
