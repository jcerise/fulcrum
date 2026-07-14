//! The Animation Book's payoff: animation-keyed gameplay, tested headless. Clips and state
//! machines load from the real asset files without a GPU, advance on the fixed clock, and
//! the exact frame a sword connects on is an assertable fact.

use dojo::game::{Bonks, Dummy, GamePlugin, Hero, STRIKE_FRAME};
use fulcrum::prelude::*;

/// The whole game, minus everything visible — including the real machines and clip timing,
/// loaded from `assets/` by the same startup system the binary runs.
fn build(seed: u64) -> Fulcrum {
    Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(AnimPlugin)
    .with_plugin(GamePlugin)
}

fn run_ticks(app: &mut Fulcrum, n: u32) {
    for _ in 0..n {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
    }
}

fn hold(app: &mut Fulcrum, key: Key, ticks: u32) {
    app.world_mut().resource_mut::<Input>().push_key(key, true);
    run_ticks(app, ticks);
    app.world_mut().resource_mut::<Input>().push_key(key, false);
    run_ticks(app, 1);
}

fn hero_state(app: &mut Fulcrum) -> (String, usize) {
    let world = app.world_mut();
    let (animator, player) = world
        .query_filtered::<(&Animator, &AnimationPlayer), With<Hero>>()
        .single(world)
        .expect("one hero");
    (animator.state().to_string(), player.frame_index)
}

fn hero_x(app: &mut Fulcrum) -> f32 {
    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, With<Hero>>()
        .single(world)
        .expect("one hero")
        .translation
        .x
}

fn bonks(app: &Fulcrum) -> Bonks {
    *app.world().resource::<Bonks>()
}

#[test]
fn the_strike_connects_on_the_extension_frame() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Walk into range (hero spawns at x=96, dummy at x=240, strike range 26).
    hold(&mut app, Key::D, 82);
    assert!(hero_x(&mut app) > 240.0 - 26.0 - 8.0, "walked into range");

    // Swing, then watch every tick: the bonk must land on the exact tick the attack clip
    // shows its extension frame — not when Space was pressed, ~11 ticks earlier.
    app.world_mut()
        .resource_mut::<Input>()
        .push_key(Key::Space, true);
    let mut connect = None;
    for tick in 0..40 {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
        app.world_mut()
            .resource_mut::<Input>()
            .push_key(Key::Space, false);
        if bonks(&app).landed == 1 && connect.is_none() {
            connect = Some((tick, hero_state(&mut app)));
        }
    }
    let (tick, (state, frame)) = connect.expect("the strike landed");
    assert_eq!(
        (state.as_str(), frame),
        ("attack", STRIKE_FRAME),
        "the bonk lands exactly while the extension frame shows"
    );
    // Windup (7 ticks) + sweep (4) must pass first; the connect tick is deep in the swing.
    assert!(
        tick >= 10,
        "landed on tick {tick}, before the blade was out"
    );
    assert_eq!(
        bonks(&app),
        Bonks {
            landed: 1,
            taken: 0
        }
    );
}

#[test]
fn a_whiff_is_a_whiff() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Swing from the spawn point, far out of range: nothing anywhere should count it.
    hold(&mut app, Key::Space, 2);
    run_ticks(&mut app, 40);
    assert_eq!(
        bonks(&app),
        Bonks {
            landed: 0,
            taken: 0
        }
    );
    assert_eq!(
        hero_state(&mut app).0,
        "idle",
        "attack finished and went home"
    );
}

#[test]
fn the_dummy_fights_back_at_point_blank() {
    let mut app = build(DEFAULT_SEED);
    app.run_startup();
    // Walk all the way to the dummy and swing from inside rebound range. The hero is
    // committed to the attack animation while the dummy's wobble swings back — bonk.
    hold(&mut app, Key::D, 96);
    hold(&mut app, Key::Space, 2);
    let mut was_hit = false;
    for _ in 0..60 {
        run_ticks(&mut app, 1);
        was_hit |= hero_state(&mut app).0 == "hit";
    }
    assert_eq!(
        bonks(&app),
        Bonks {
            landed: 1,
            taken: 1
        }
    );
    assert!(
        was_hit,
        "the rebound interrupted the hero into the hit state"
    );
    assert_eq!(
        hero_state(&mut app).0,
        "idle",
        "and the hit state went home too"
    );
}

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

        let world = app.world_mut();
        let mut anim_state: Vec<(String, usize, u32)> = world
            .query::<(&Animator, &AnimationPlayer)>()
            .iter(world)
            .map(|(a, p)| (a.state().to_string(), p.frame_index, p.tick_in_frame))
            .collect();
        anim_state.sort();
        let mut positions: Vec<(u32, u32)> = world
            .query_filtered::<&Transform2D, Or<(With<Hero>, With<Dummy>)>>()
            .iter(world)
            .map(|t| (t.translation.x.to_bits(), t.translation.y.to_bits()))
            .collect();
        positions.sort();
        (*world.resource::<Bonks>(), anim_state, positions)
    };
    assert_eq!(fingerprint(7), fingerprint(7), "same seed, same fight");
}
