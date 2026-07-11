//! Snake, windowed: everything you can see or hear. This binary is a *view* of the simulation
//! in `game.rs` — it projects the `Snake` resource into sprites each frame, dresses apples,
//! and turns events into sound. Delete this file and the game still runs headless in tests;
//! that asymmetry is the whole design.
//!
//! `cargo run -p snake` — WASD/arrows steer, Enter restarts.

use fulcrum::prelude::*;
use snake::game::{
    self, Apple, AppleEaten, CELL, GRID_H, GRID_W, GamePlugin, RunEnded, Score, Snake, SnakeState,
};

/// The 8x8 white square every rectangle in the game is a tinted copy of.
#[derive(Resource)]
struct Square(Handle<Texture>);

/// Sound handles, loaded once.
#[derive(Resource)]
struct Sounds {
    eat: Handle<Sound>,
    die: Handle<Sound>,
}

/// Marks the score readout entity.
#[derive(Component)]
struct ScoreText;

/// Marks the end-of-run banner entity.
#[derive(Component)]
struct Banner;

/// Marks a sprite that mirrors one snake body segment (see [`project_snake`]).
#[derive(Component)]
struct SegmentView;

fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut camera: ResMut<Camera2D>,
) {
    // Show exactly the grid: 384x288 world units, bars on any window shape.
    camera.scaling = ScalingMode::Letterbox {
        width: GRID_W as f32 * CELL,
        height: GRID_H as f32 * CELL,
    };
    camera.center = vec2(GRID_W as f32 * CELL / 2.0, GRID_H as f32 * CELL / 2.0);

    commands.insert_resource(Square(assets.load("white.png")));
    commands.insert_resource(Sounds {
        eat: sounds.load("eat.wav"),
        die: sounds.load("die.wav"),
    });

    // A checkerboard floor so motion is readable against something.
    let square = assets.load("white.png");
    for x in 0..GRID_W {
        for y in 0..GRID_H {
            let shade = if (x + y) % 2 == 0 { 0.10 } else { 0.12 };
            commands.spawn((
                Sprite::new(square)
                    .with_color(Color::rgb(shade, shade + 0.02, shade))
                    .with_size(Vec2::splat(CELL))
                    .with_z(-1.0),
                Transform2D::from_translation(game::cell_center((x, y))),
            ));
        }
    }

    commands.spawn((
        Text::new("Score: 0").with_size(8.0).with_z(10.0),
        Transform2D::from_xy(4.0, GRID_H as f32 * CELL - 10.0),
        ScoreText,
    ));
    commands.spawn((
        Text::new("")
            .with_size(16.0)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::from_xy(
            GRID_W as f32 * CELL / 2.0,
            GRID_H as f32 * CELL / 2.0 + 24.0,
        ),
        Banner,
    ));
}

/// Rebuild the snake's sprites from its state, every frame. Reconciliation, not simulation:
/// grow/shrink the entity pool to match the body length, then reposition everything.
fn project_snake(
    snake: Res<Snake>,
    mut segments: Query<(Entity, &mut Transform2D, &mut Sprite), With<SegmentView>>,
    square: Option<Res<Square>>,
    mut commands: Commands,
) {
    let Some(square) = square else { return };
    let mut views: Vec<_> = segments.iter_mut().collect();
    for _ in views.len()..snake.body.len() {
        commands.spawn((
            Sprite::new(square.0)
                .with_size(Vec2::splat(CELL - 2.0))
                .with_z(2.0),
            Transform2D::default(),
            SegmentView,
        ));
    }
    for (entity, _, _) in views.iter().skip(snake.body.len()) {
        commands.entity(*entity).try_despawn();
    }
    for (index, ((_, transform, sprite), cell)) in
        views.iter_mut().zip(snake.body.iter()).enumerate()
    {
        transform.translation = game::cell_center(*cell);
        // Head bright, body fading toward the tail: readable direction at a glance.
        let fade = 1.0 - (index as f32 / snake.body.len().max(1) as f32) * 0.45;
        sprite.color = if index == 0 {
            Color::rgb(0.55, 1.0, 0.45)
        } else {
            Color::rgb(0.2 * fade, 0.75 * fade, 0.25 * fade)
        };
    }
}

/// Give each new apple a sprite. The sim spawned the entity; the view dresses it.
fn dress_apples(
    undressed: Query<Entity, (With<Apple>, Without<Sprite>)>,
    square: Option<Res<Square>>,
    mut commands: Commands,
) {
    let Some(square) = square else { return };
    for apple in &undressed {
        commands.entity(apple).try_insert(
            Sprite::new(square.0)
                .with_color(Color::rgb(1.0, 0.35, 0.3))
                .with_size(Vec2::splat(CELL - 4.0))
                .with_z(1.0),
        );
    }
}

#[allow(clippy::type_complexity)] // standard ECS system shape
fn hud(
    score: Res<Score>,
    state: Res<SnakeState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<Banner>>,
    )>,
) {
    if let Ok(mut text) = texts.p0().single_mut() {
        text.value = format!("Score: {}", score.0);
    }
    if let Ok(mut banner) = texts.p1().single_mut() {
        banner.value = match *state {
            SnakeState::Playing => String::new(),
            SnakeState::GameOver => format!("GAME OVER\nScore: {}\nEnter to restart", score.0),
            SnakeState::Won => "YOU FILLED THE GRID?!\nEnter to go again".to_string(),
        };
    }
}

fn sound_effects(
    mut eaten: EventReader<AppleEaten>,
    mut ended: EventReader<RunEnded>,
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    sound_assets: Res<Assets<Sound>>,
    score: Res<Score>,
) {
    let Some(sounds) = sounds else { return };
    for _ in eaten.read() {
        audio.play_with(
            &sound_assets,
            sounds.eat,
            PlayParams {
                volume: 0.6,
                // A little rise as the score climbs; pure presentation, so this is allowed
                // to be as cute as it likes.
                pitch: 1.0 + (score.0 % 8) as f32 * 0.03,
                ..Default::default()
            },
        );
    }
    for _ in ended.read() {
        audio.play(&sound_assets, sounds.die);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Snake".into(),
        window_size: (1152, 864), // 3x the 384x288 world; any size works, letterboxed
        clear_color: Color::rgb(0.05, 0.06, 0.05),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_frame_system(project_snake)
    .add_frame_system(dress_apples)
    .add_frame_system(hud)
    .add_frame_system(sound_effects)
    .run();
}
