//! State machine acceptance: tick-by-tick transitions, trigger semantics, validation errors.

use fulcrum_anim::{AnimPlugin, AnimationClip, AnimationPlayer, Animator, StateMachineAsset};
use fulcrum_asset::{Assets, Handle};
use fulcrum_core::Fulcrum;
use fulcrum_render::Sprite;

const MACHINE: &str = r#"StateMachine(
    initial: "idle",
    params: { "speed": Float(0.0), "attack": Trigger },
    states: {
        "idle":   (clip: "p#idle"),
        "run":    (clip: "p#run"),
        "attack": (clip: "p#attack", on_finish: "idle"),
    },
    transitions: [
        (from: State("idle"), to: "run",    when: [Gt("speed", 0.1)]),
        (from: State("run"),  to: "idle",   when: [Lt("speed", 0.1)]),
        (from: Any,           to: "attack", when: [Triggered("attack")]),
    ],
)"#;

/// Build the machine with per-tag test clips: idle/run loop, attack is 2 ticks, non-looping.
fn setup() -> (
    Fulcrum,
    bevy_ecs::entity::Entity,
    Vec<Handle<AnimationClip>>,
) {
    let mut app = Fulcrum::new("test").with_plugin(AnimPlugin);
    let mut handles = Vec::new();
    {
        let world = app.world_mut();
        let mut clips = world.resource_mut::<Assets<AnimationClip>>();
        for looping in [true, true, false] {
            handles.push(clips.insert(AnimationClip {
                sheet: Handle::INVALID,
                frames: vec![0, 1],
                frame_ticks: vec![1, 1],
                looping,
            }));
        }
    }
    let clip_for = {
        let handles = handles.clone();
        move |clip_ref: &str| -> Result<Handle<AnimationClip>, String> {
            match clip_ref {
                "p#idle" => Ok(handles[0]),
                "p#run" => Ok(handles[1]),
                "p#attack" => Ok(handles[2]),
                other => Err(format!("unknown {other}")),
            }
        }
    };
    let machine = fulcrum_anim::state_machine::test_build(MACHINE, clip_for).unwrap();
    let machine = app
        .world_mut()
        .resource_mut::<Assets<StateMachineAsset>>()
        .insert(machine);
    let entity = app
        .world_mut()
        .spawn((
            Sprite::from_sheet(Handle::INVALID, 0),
            Animator::new(machine),
            AnimationPlayer::play(Handle::INVALID),
        ))
        .id();
    app.run_startup();
    (app, entity, handles)
}

fn state(app: &Fulcrum, entity: bevy_ecs::entity::Entity) -> String {
    app.world()
        .entity(entity)
        .get::<Animator>()
        .unwrap()
        .state()
        .to_string()
}

fn active_clip(app: &Fulcrum, entity: bevy_ecs::entity::Entity) -> Handle<AnimationClip> {
    app.world()
        .entity(entity)
        .get::<AnimationPlayer>()
        .unwrap()
        .clip
}

#[test]
fn drives_idle_run_attack_and_back() {
    let (mut app, entity, clips) = setup();
    app.tick();
    assert_eq!(state(&app, entity), "idle");
    assert_eq!(active_clip(&app, entity), clips[0]);

    // Speed up: idle -> run.
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Animator>()
        .unwrap()
        .set_float("speed", 1.0);
    app.tick();
    assert_eq!(state(&app, entity), "run");
    assert_eq!(active_clip(&app, entity), clips[1]);

    // Trigger attack from Any.
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Animator>()
        .unwrap()
        .trigger("attack");
    app.tick();
    assert_eq!(state(&app, entity), "attack");

    // Attack is 2 ticks, non-looping; when finished, on_finish returns to idle (speed still
    // high, so the next tick it runs again -- assert the intermediate hop).
    app.tick(); // attack advances to last frame
    app.tick(); // finished() -> on_finish fires... unless speed sends idle->run first from idle
    let hop = state(&app, entity);
    assert!(
        hop == "idle" || hop == "run",
        "returned via on_finish, got {hop}"
    );

    // Slow down: back to idle and stays.
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Animator>()
        .unwrap()
        .set_float("speed", 0.0);
    app.tick();
    app.tick();
    assert_eq!(state(&app, entity), "idle");
}

#[test]
fn triggers_last_one_tick_and_fire_once() {
    let (mut app, entity, _clips) = setup();
    app.tick(); // idle
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Animator>()
        .unwrap()
        .trigger("attack");
    app.tick();
    assert_eq!(state(&app, entity), "attack");
    // The trigger is consumed: after attack finishes we return to idle and STAY (no re-fire).
    for _ in 0..4 {
        app.tick();
    }
    assert_eq!(state(&app, entity), "idle");
}

#[test]
fn validation_reports_every_problem_at_once() {
    let bad = r#"StateMachine(
        initial: "nope",
        params: {},
        states: { "idle": (clip: "p#idle", on_finish: "gone") },
        transitions: [
            (from: State("ghost"), to: "idle", when: [Gt("speed", 0.5)]),
            (from: Any, to: "missing", when: []),
        ],
    )"#;
    let Err(error) = fulcrum_anim::state_machine::test_build(bad, |_| Ok(Handle::INVALID)) else {
        panic!("invalid machine validated");
    };
    let message = error.to_string();
    for expected in ["nope", "gone", "ghost", "missing", "speed"] {
        assert!(
            message.contains(expected),
            "missing `{expected}` in: {message}"
        );
    }
}
