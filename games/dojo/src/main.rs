//! Dojo, windowed: everything you can see. The simulation in `game.rs` runs headless; this
//! binary adds the floor, the sprite flip, and a HUD that shows the animation state machine
//! live — watch the `hero:` line while you play and the machine stops being abstract.
//!
//! `cargo run -p dojo` — WASD/arrows move, Space swings. Don't stand next to the dummy
//! after you hit it.

use dojo::game::{ARENA_H, ARENA_W, Bonks, Dummy, GamePlugin, Hero};
use fulcrum::prelude::*;

/// Marks the scoreline text entity.
#[derive(Component)]
struct ScoreText;

/// Marks the live state-machine readout entity.
#[derive(Component)]
struct StateText;

fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA_W,
        height: ARENA_H,
    };
    camera.center = vec2(ARENA_W / 2.0, ARENA_H / 2.0);

    // A tatami floor: broad woven-mat rectangles, alternating weave direction by tint.
    let white = assets.load("white.png");
    let (mat_w, mat_h) = (40.0, 20.0);
    let (cols, rows) = ((ARENA_W / mat_w) as i32, (ARENA_H / mat_h) as i32);
    for x in 0..cols {
        for y in 0..rows {
            let weave = if (x + y) % 2 == 0 { 0.0 } else { 0.02 };
            commands.spawn((
                Sprite::new(white)
                    .with_color(Color::rgb(0.32 + weave, 0.27 + weave, 0.16))
                    .with_size(vec2(mat_w - 1.0, mat_h - 1.0))
                    .with_z(-1.0),
                Transform2D::from_xy(
                    x as f32 * mat_w + mat_w / 2.0,
                    y as f32 * mat_h + mat_h / 2.0,
                ),
            ));
        }
    }

    commands.spawn((
        Text::new("").with_size(8.0).with_z(10.0),
        Transform2D::from_xy(4.0, ARENA_H - 10.0),
        ScoreText,
    ));
    commands.spawn((
        Text::new("").with_size(8.0).with_z(10.0),
        Transform2D::from_xy(4.0, 8.0),
        StateText,
    ));
}

/// Facing is simulation state; the flip that shows it is presentation. One line each way.
fn flip_faces(mut heroes: Query<(&Hero, &mut Sprite)>) {
    for (hero, mut sprite) in &mut heroes {
        if sprite.flip_x != hero.facing_left {
            sprite.flip_x = hero.facing_left;
        }
    }
}

/// The HUD's second line is the whole animation system, live: current state and frame for
/// both machines. Watch it while you play — this is the book's best debugging tool.
#[allow(clippy::type_complexity)] // standard ECS system shape
fn hud(
    bonks: Res<Bonks>,
    heroes: Query<(&Animator, &AnimationPlayer), With<Hero>>,
    dummies: Query<(&Animator, &AnimationPlayer), With<Dummy>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<StateText>>,
    )>,
) {
    if let Ok(mut text) = texts.p0().single_mut() {
        text.value = format!("landed: {}   taken: {}", bonks.landed, bonks.taken);
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        let line = |(animator, player): (&Animator, &AnimationPlayer)| {
            format!("{} #{}", animator.state(), player.frame_index)
        };
        let hero = heroes.single().map(line).unwrap_or_default();
        let dummy = dummies.single().map(line).unwrap_or_default();
        text.value = format!("hero: {hero}   dummy: {dummy}");
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Dojo — wasd to move, space to swing".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.09, 0.08, 0.07),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_frame_system(flip_faces)
    .add_frame_system(hud)
    .run();
}
