//! The UI Book's payoff: a game played entirely through its UI, tested headless. The real
//! layout file loads without a GPU, clicks travel as replayable `ui:click` commands, and
//! the full pointer path — hit-test, press, release, dispatch — runs without a window.

use bazaar::game::{DAY_TICKS, DayClock, GamePlugin, GameState, PATIENCE, Queue, Takings, WARES};
use fulcrum::prelude::*;

/// The whole game, minus everything visible. `UiPlugin` stays: it owns the dispatcher
/// that turns `ui:click` commands back into `UiEvent`s, and it runs fine without a
/// window — no GPU means no textures, but the tree, ids, layout, and hit-testing are
/// all live. Most tests never load a layout at all: a click is just a command, and a
/// test can send the same commands the pointer would.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .insert_resource(Assets::<Texture>::default())
    .insert_resource(Camera2D::default()) // Stretch: UI pixels == screen pixels
    .insert_resource(WindowInfo {
        width: 320,
        height: 180,
        scale_factor: 1.0,
    })
    .with_plugin(UiPlugin)
    .with_plugin(GamePlugin)
}

/// The sim plus the *real* shipped layout, loaded headless.
fn build_with_ui(seed: u64) -> Fulcrum {
    build(seed).add_startup(load_hud)
}

fn load_hud(mut ui: UiLoader) {
    ui.load("ui/hud.ui.ron").expect("hud loads");
}

fn run_ticks(app: &mut Fulcrum, n: u32) {
    for _ in 0..n {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
    }
}

/// What a button press becomes: the same command `interact_ui` sends on release. The
/// event is guaranteed readable "for that tick and the next", so run both before
/// asserting anything.
fn click(app: &mut Fulcrum, id: &str) {
    app.world_mut()
        .resource_mut::<CommandOutbox>()
        .send("ui:click", id);
    run_ticks(app, 2);
}

fn front_wants(app: &Fulcrum) -> usize {
    app.world()
        .resource::<Queue>()
        .0
        .first()
        .expect("someone is waiting")
        .wants
}

fn takings(app: &Fulcrum) -> Takings {
    *app.world().resource::<Takings>()
}

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

#[test]
fn the_wrong_ware_sells_nothing() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    run_ticks(&mut app, 91);
    let wants = front_wants(&app);
    click(&mut app, WARES[(wants + 1) % WARES.len()].id);
    assert_eq!(takings(&app).served, 0);
    assert_eq!(takings(&app).gold, 0);
    assert_eq!(
        app.world().resource::<Queue>().0.len(),
        1,
        "the customer keeps waiting"
    );
}

#[test]
fn patience_runs_out() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Long enough for the first arrival plus their entire patience, serving no one.
    run_ticks(&mut app, 91 + PATIENCE);
    assert!(takings(&app).lost >= 1, "somebody gave up");
    assert_eq!(takings(&app).served, 0);
}

#[test]
fn pause_freezes_the_day() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    run_ticks(&mut app, 10);
    app.world_mut()
        .resource_mut::<Input>()
        .push_key(Key::Escape, true);
    run_ticks(&mut app, 1);
    assert_eq!(*app.world().resource::<GameState>(), GameState::Paused);
    let frozen = app.world().resource::<DayClock>().0;
    run_ticks(&mut app, 30);
    assert_eq!(
        app.world().resource::<DayClock>().0,
        frozen,
        "a paused day doesn't tick"
    );
    // The Resume button is just another command.
    click(&mut app, "resume");
    assert_eq!(*app.world().resource::<GameState>(), GameState::Trading);
    run_ticks(&mut app, 1);
    assert!(app.world().resource::<DayClock>().0 < frozen);
}

#[test]
fn sundown_ends_the_day_and_next_day_resets_it() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    run_ticks(&mut app, DAY_TICKS);
    assert_eq!(*app.world().resource::<GameState>(), GameState::DayOver);
    let closed = app.world().resource::<DayClock>().0;
    assert_eq!(closed, 0);
    click(&mut app, "next_day");
    assert_eq!(*app.world().resource::<GameState>(), GameState::Trading);
    assert_eq!(takings(&app), Takings::default());
    let clock = app.world().resource::<DayClock>().0;
    assert!(
        clock >= DAY_TICKS - 2,
        "the clock reset (a fresh day is already ticking): {clock}"
    );
}

/// The full pointer path, headless: cursor over the real button's laid-out rect, press,
/// release — `interact_ui` emits the command, the dispatcher turns it into a `UiEvent`,
/// and the sim makes the sale. No window anywhere.
#[test]
fn a_pointer_click_on_the_real_layout_serves_a_customer() {
    let mut app = build_with_ui(DEFAULT_SEED);
    app.run_startup();
    run_ticks(&mut app, 91);
    let frame = |app: &mut Fulcrum| app.world_mut().run_schedule(Update);
    frame(&mut app); // layout: every UiRect is now placed

    let wants = front_wants(&app);
    let target = WARES[wants].id;
    let center = {
        let world = app.world_mut();
        let mut buttons = world.query::<(&UiRect, &UiId)>();
        buttons
            .iter(world)
            .find(|(_, id)| id.0 == target)
            .expect("serve button is laid out")
            .0
            .rect
            .center()
    };

    let point = |app: &mut Fulcrum, press: Option<bool>| {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_cursor(center);
        if let Some(down) = press {
            input.push_mouse_button(MouseButton::Left, down);
        }
        input.sample(|screen| screen);
    };
    point(&mut app, None);
    frame(&mut app); // hover
    point(&mut app, Some(true));
    frame(&mut app); // press
    point(&mut app, Some(false));
    frame(&mut app); // release: the `ui:click` command is queued
    assert!(
        app.world().resource::<UiFocus>().pointer_over_ui,
        "the pointer is over the shop"
    );
    run_ticks(&mut app, 2); // command -> UiEvent -> sale
    assert_eq!(takings(&app).served, 1);
}

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
        app.world_mut()
            .resource_mut::<Input>()
            .push_mouse_button(MouseButton::Left, true);
        run_ticks(&mut app, 120);

        let world = app.world();
        let queue: Vec<(usize, u32)> = world
            .resource::<Queue>()
            .0
            .iter()
            .map(|c| (c.wants, c.patience))
            .collect();
        (
            *world.resource::<Takings>(),
            queue,
            world.resource::<DayClock>().0,
            *world.resource::<GameState>(),
        )
    };
    assert_eq!(fingerprint(7), fingerprint(7), "same seed, same market day");
}
