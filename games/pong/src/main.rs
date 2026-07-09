//! Pong: the Fulcrum phase-1 milestone game. `cargo run -p pong`
//!
//! W/S moves the left paddle; first to 5 wins; Enter restarts after a win. All visuals here are
//! built from one 1x1 white texture — text rendering arrives in phase 2, so the score is drawn
//! as 3x5 pixel-grid digits.

use fulcrum::prelude::*;
use pong::game::{self, Ball, COURT, GamePlugin, GameState, PADDLE_SIZE, Paddle, Score};

/// Handle to the shared 1x1 white texture all Pong sprites use.
#[derive(Resource)]
struct WhiteTexture(Handle<Texture>);

/// Tag for score-display cell sprites (rebuilt whenever the score changes).
#[derive(Component)]
struct ScoreDigit;

/// 3x5 digit bitmaps, one row per entry, 3 bits per row (MSB = left column).
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];
const CELL: f32 = 12.0;

/// Give the simulation's entities their looks, and add the static dressing (center line).
fn attach_visuals(
    mut commands: Commands,
    mut assets: AssetLoader,
    paddles: Query<Entity, With<Paddle>>,
    balls: Query<Entity, With<Ball>>,
) {
    let white = assets.load("white.png");
    for paddle in &paddles {
        commands
            .entity(paddle)
            .insert(Sprite::new(white).with_size(PADDLE_SIZE));
    }
    for ball in &balls {
        commands
            .entity(ball)
            .insert(Sprite::new(white).with_size(Vec2::splat(game::BALL_SIZE)));
    }
    // Dashed center line.
    let mut y = -COURT.y / 2.0 + 10.0;
    while y < COURT.y / 2.0 {
        commands.spawn((
            Sprite::new(white)
                .with_size(vec2(4.0, 16.0))
                .with_color(Color::rgba(1.0, 1.0, 1.0, 0.4)),
            Transform2D::from_xy(0.0, y),
        ));
        y += 32.0;
    }
    commands.insert_resource(WhiteTexture(white));
}

/// Rebuild the score sprites whenever the score or game state changes (cosmetic, frame-side).
fn update_score_display(
    mut commands: Commands,
    score: Res<Score>,
    state: Res<GameState>,
    white: Res<WhiteTexture>,
    old_cells: Query<Entity, With<ScoreDigit>>,
    mut cache: Local<Option<(Score, GameState)>>,
) {
    if *cache == Some((*score, *state)) {
        return;
    }
    *cache = Some((*score, *state));
    for cell in &old_cells {
        commands.entity(cell).despawn();
    }
    let color = match *state {
        GameState::Playing => Color::rgba(1.0, 1.0, 1.0, 0.8),
        GameState::GameOver => Color::rgb(1.0, 0.35, 0.35), // red = game over, Enter restarts
    };
    spawn_digit(
        &mut commands,
        white.0,
        score.player.min(9),
        vec2(-80.0, 240.0),
        color,
    );
    spawn_digit(
        &mut commands,
        white.0,
        score.ai.min(9),
        vec2(80.0, 240.0),
        color,
    );
}

/// Draw one 3x5 digit as a grid of square sprites; `origin` is the top-left cell center.
fn spawn_digit(
    commands: &mut Commands,
    white: Handle<Texture>,
    value: u32,
    origin: Vec2,
    color: Color,
) {
    let rows = DIGITS[value as usize];
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..3u32 {
            if bits >> (2 - col) & 1 == 1 {
                commands.spawn((
                    ScoreDigit,
                    Sprite::new(white)
                        .with_size(Vec2::splat(CELL - 2.0))
                        .with_color(color),
                    Transform2D::from_translation(
                        origin + vec2(col as f32 * CELL, -(row as f32) * CELL),
                    ),
                ));
            }
        }
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Pong".into(),
        window_size: (COURT.x as u32, COURT.y as u32),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(attach_visuals.after(game::spawn_entities))
    .add_frame_system(update_score_display)
    .run();
}
