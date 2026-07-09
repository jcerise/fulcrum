//! Headless clip playback tests: deterministic tick-by-tick frame advancement.

use fulcrum_anim::{AnimPlugin, AnimationClip, AnimationPlayer};
use fulcrum_asset::{Assets, Handle};
use fulcrum_core::Fulcrum;
use fulcrum_render::Sprite;

fn app_with_clip(frame_ticks: Vec<u32>, looping: bool) -> (Fulcrum, Handle<AnimationClip>) {
    let mut app = Fulcrum::new("test").with_plugin(AnimPlugin);
    let clip = app
        .world_mut()
        .resource_mut::<Assets<AnimationClip>>()
        .insert(AnimationClip {
            sheet: Handle::INVALID,
            frames: (0..frame_ticks.len() as u32).collect(),
            frame_ticks,
            looping,
        });
    (app, clip)
}

fn spawn_player(app: &mut Fulcrum, clip: Handle<AnimationClip>) -> bevy_ecs::entity::Entity {
    app.world_mut()
        .spawn((
            Sprite::from_sheet(Handle::INVALID, 0),
            AnimationPlayer::play(clip),
        ))
        .id()
}

#[test]
fn frames_advance_on_expected_ticks() {
    let (mut app, clip) = app_with_clip(vec![3, 2, 4], true);
    let entity = spawn_player(&mut app, clip);
    app.run_startup();

    // Expected frame per tick: durations 3,2,4 -> frame 0 for ticks 1..=2 (advance happens at
    // the 3rd tick), etc. Track (tick, expected frame after that tick).
    let expected = [
        (1, 0),
        (2, 0),
        (3, 1), // 3 ticks on frame 0
        (4, 1),
        (5, 2), // 2 ticks on frame 1
        (6, 2),
        (7, 2),
        (8, 2),
        (9, 0), // 4 ticks on frame 2, then loop
    ];
    for (tick, frame) in expected {
        app.tick();
        let player = app.world().entity(entity).get::<AnimationPlayer>().unwrap();
        assert_eq!(player.frame_index, frame, "after tick {tick}");
        // The sprite shows the current frame's region.
        let sprite = app.world().entity(entity).get::<Sprite>().unwrap();
        assert_eq!(sprite.region.unwrap().index, frame as u32);
    }
}

#[test]
fn non_looping_clip_clamps_and_finishes() {
    let (mut app, clip) = app_with_clip(vec![1, 1], false);
    let entity = spawn_player(&mut app, clip);
    app.run_startup();

    for _ in 0..10 {
        app.tick();
    }
    let player = app.world().entity(entity).get::<AnimationPlayer>().unwrap();
    assert_eq!(player.frame_index, 1, "clamped to last frame");
    assert!(player.finished());
    assert!(!player.playing);
}

#[test]
fn restart_with_same_clip_does_not_stutter() {
    let (mut app, clip) = app_with_clip(vec![5, 5], true);
    let entity = spawn_player(&mut app, clip);
    app.run_startup();
    for _ in 0..6 {
        app.tick(); // now on frame 1
    }
    let mut player = app
        .world_mut()
        .entity_mut(entity)
        .into_mut::<AnimationPlayer>()
        .unwrap();
    player.restart(clip);
    assert_eq!(player.frame_index, 1, "same clip -> no reset");
}

#[test]
fn hot_reload_shrinking_a_clip_clamps_live_players() {
    let (mut app, clip) = app_with_clip(vec![1, 1, 1, 1, 1], true);
    let entity = spawn_player(&mut app, clip);
    app.run_startup();
    for _ in 0..3 {
        app.tick(); // frame_index now 3
    }
    // Replace with a 2-frame clip behind the same handle (what hot reload does).
    app.world_mut()
        .resource_mut::<Assets<AnimationClip>>()
        .replace(
            clip,
            AnimationClip {
                sheet: Handle::INVALID,
                frames: vec![0, 1],
                frame_ticks: vec![1, 1],
                looping: true,
            },
        );
    app.tick(); // must clamp, not panic
    let sprite = app.world().entity(entity).get::<Sprite>().unwrap();
    assert!(
        sprite.region.unwrap().index <= 1,
        "clamped to the shorter clip"
    );
}
