//! The loader works without a GPU: sheet regions and clip timing are simulation data, so a
//! headless app (tests, servers) can load real Aseprite exports and `.animsm.ron` machines
//! and key gameplay off animation frames deterministically. Only texture upload needs a GPU,
//! and headless it is simply skipped (the fixture's `hero.png` doesn't even exist).

use bevy_ecs::prelude::{Commands, Entity, Resource};
use fulcrum_anim::{AnimPlugin, AnimationPlayer, Animator, AnimatorLoader};
use fulcrum_asset::{AssetServer, Handle};
use fulcrum_core::Fulcrum;
use fulcrum_render::Sprite;

#[derive(Resource)]
struct Loaded(Handle<fulcrum_anim::StateMachineAsset>);

fn build() -> (Fulcrum, Entity) {
    let mut app = Fulcrum::new("headless")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures"
        )))
        .with_plugin(AnimPlugin)
        .add_startup(|mut commands: Commands, mut animators: AnimatorLoader| {
            let machine = animators
                .load("hero.animsm.ron")
                .expect("machines load headless");
            commands.insert_resource(Loaded(machine));
        });
    app.run_startup();
    let machine = app.world().resource::<Loaded>().0;
    let entity = app
        .world_mut()
        .spawn((
            Sprite::from_sheet(Handle::INVALID, 0),
            Animator::new(machine),
            AnimationPlayer::play(Handle::INVALID),
        ))
        .id();
    (app, entity)
}

fn state(app: &Fulcrum, entity: Entity) -> String {
    app.world()
        .entity(entity)
        .get::<Animator>()
        .unwrap()
        .state()
        .to_string()
}

#[test]
fn real_assets_drive_machines_without_a_gpu() {
    let (mut app, entity) = build();
    app.tick();
    assert_eq!(state(&app, entity), "idle");

    // Fire the swing and record the state on every subsequent tick. The swing clip is two
    // 50 ms frames = two 3-tick frames at 60 Hz, played once (`"repeat": "1"`), so the
    // timeline is exact: the swing state lasts exactly the clip's 6 ticks (finished() sets
    // as the last frame's duration elapses; on_finish fires on the next tick's drive).
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Animator>()
        .unwrap()
        .trigger("swing");
    let mut timeline = Vec::new();
    for _ in 0..8 {
        app.tick();
        timeline.push(state(&app, entity));
    }
    let swings = timeline.iter().filter(|s| *s == "swing").count();
    assert_eq!(
        (swings, timeline.last().map(String::as_str)),
        (6, Some("idle")),
        "expected exactly the clip's 6 ticks of swing, then idle; got {timeline:?}"
    );

    // And the frame the player shows at any tick is a plain, assertable fact.
    let player = app.world().entity(entity).get::<AnimationPlayer>().unwrap();
    assert_eq!(player.frame_index, 0, "back on idle's first frame");
}

#[test]
fn determinism_headless_runs_are_identical() {
    let run = || {
        let (mut app, entity) = build();
        let mut trace = Vec::new();
        for tick in 0..40 {
            if tick == 5 {
                app.world_mut()
                    .entity_mut(entity)
                    .get_mut::<Animator>()
                    .unwrap()
                    .trigger("swing");
            }
            app.tick();
            let player = app.world().entity(entity).get::<AnimationPlayer>().unwrap();
            trace.push((
                state(&app, entity),
                player.frame_index,
                player.tick_in_frame,
            ));
        }
        trace
    };
    assert_eq!(run(), run(), "same input, same animation, every tick");
}
