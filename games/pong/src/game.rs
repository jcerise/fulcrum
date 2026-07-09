//! Pong simulation: paddles, ball, scoring. Pure logic — no sprites, no audio — so it runs
//! headless for determinism tests. All systems live in `FixedUpdate`, chained for a fully
//! deterministic order.

use fulcrum::prelude::*;

/// Court size in world units (matches the window).
pub const COURT: Vec2 = Vec2::new(800.0, 600.0);
/// Paddle size.
pub const PADDLE_SIZE: Vec2 = Vec2::new(12.0, 80.0);
/// Ball is a square of this side.
pub const BALL_SIZE: f32 = 12.0;
/// Paddle center distance from the court center.
pub const PADDLE_X: f32 = 370.0;
/// Player paddle speed, units/second.
pub const PADDLE_SPEED: f32 = 340.0;
/// AI paddle speed — a touch slower than the ball can climb, so it's beatable.
pub const AI_SPEED: f32 = 240.0;
/// Serve speed along X.
pub const SERVE_SPEED: f32 = 300.0;
/// Ball X speed cap after repeated paddle hits.
pub const MAX_BALL_SPEED: f32 = 700.0;
/// First to this score wins.
pub const WIN_SCORE: u32 = 5;

/// The human paddle (W/S).
#[derive(Component)]
pub struct Player;

/// The computer paddle.
#[derive(Component)]
pub struct Ai;

/// Any paddle.
#[derive(Component)]
pub struct Paddle;

/// The ball.
#[derive(Component)]
pub struct Ball;

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// Points per side.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score {
    /// Human player's points.
    pub player: u32,
    /// AI's points.
    pub ai: u32,
}

/// Playing or waiting for Enter after a win.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    /// Ball in play.
    #[default]
    Playing,
    /// Someone reached [`WIN_SCORE`]; Enter restarts.
    GameOver,
}

/// Installs the Pong simulation: entities, score, and the chained `FixedUpdate` systems.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Score::default());
        app.world_mut().insert_resource(GameState::default());
        app.add_systems(Startup, spawn_entities);
        app.add_systems(
            FixedUpdate,
            (
                control_player,
                control_ai,
                move_ball,
                collide_paddles,
                score_and_serve,
                restart_on_enter,
            )
                .chain(),
        );
    }
}

/// Spawn paddles and ball. Public so the binary can order its sprite-attachment after it.
pub fn spawn_entities(mut commands: Commands, mut rng: ResMut<SimRng>) {
    commands.spawn((Paddle, Player, Transform2D::from_xy(-PADDLE_X, 0.0)));
    commands.spawn((Paddle, Ai, Transform2D::from_xy(PADDLE_X, 0.0)));
    commands.spawn((
        Ball,
        Transform2D::default(),
        Velocity(serve(&mut rng, true)),
    ));
}

/// A fresh serve velocity, toward the given side, with an RNG vertical component.
fn serve(rng: &mut SimRng, toward_player: bool) -> Vec2 {
    let dir = if toward_player { -1.0 } else { 1.0 };
    vec2(dir * SERVE_SPEED, rng.range_f32(-140.0..140.0))
}

fn paddle_y_limit() -> f32 {
    COURT.y / 2.0 - PADDLE_SIZE.y / 2.0
}

fn control_player(
    mut paddles: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    let mut dir = 0.0;
    if input.pressed(Key::W) {
        dir += 1.0;
    }
    if input.pressed(Key::S) {
        dir -= 1.0;
    }
    for mut transform in &mut paddles {
        transform.translation.y = (transform.translation.y + dir * PADDLE_SPEED * time.fixed_delta)
            .clamp(-paddle_y_limit(), paddle_y_limit());
    }
}

fn control_ai(
    mut paddles: Query<&mut Transform2D, (With<Ai>, Without<Ball>)>,
    balls: Query<&Transform2D, With<Ball>>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    let Ok(ball) = balls.single() else { return };
    for mut paddle in &mut paddles {
        let gap = ball.translation.y - paddle.translation.y;
        let step = (AI_SPEED * time.fixed_delta).min(gap.abs());
        paddle.translation.y =
            (paddle.translation.y + gap.signum() * step).clamp(-paddle_y_limit(), paddle_y_limit());
    }
}

fn move_ball(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    let wall = COURT.y / 2.0 - BALL_SIZE / 2.0;
    for (mut transform, mut velocity) in &mut balls {
        transform.translation += velocity.0 * time.fixed_delta;
        if transform.translation.y >= wall {
            transform.translation.y = wall;
            velocity.0.y = -velocity.0.y.abs();
        } else if transform.translation.y <= -wall {
            transform.translation.y = -wall;
            velocity.0.y = velocity.0.y.abs();
        }
    }
}

fn collide_paddles(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    paddles: Query<&Transform2D, (With<Paddle>, Without<Ball>)>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    for (mut ball, mut velocity) in &mut balls {
        for paddle in &paddles {
            let delta = ball.translation - paddle.translation;
            let reach = (PADDLE_SIZE + Vec2::splat(BALL_SIZE)) / 2.0;
            let overlapping = delta.x.abs() <= reach.x && delta.y.abs() <= reach.y;
            let moving_into = velocity.0.x.signum() == -delta.x.signum();
            if overlapping && moving_into {
                // Reflect, speed up 5% (capped), and steer by where the paddle was struck.
                velocity.0.x = (-velocity.0.x * 1.05).clamp(-MAX_BALL_SPEED, MAX_BALL_SPEED);
                velocity.0.y += (delta.y / reach.y) * 160.0;
                // Push the ball just outside the paddle so it can't collide twice.
                ball.translation.x = paddle.translation.x + delta.x.signum() * (reach.x + 0.5);
            }
        }
    }
}

fn score_and_serve(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
    mut state: ResMut<GameState>,
    mut rng: ResMut<SimRng>,
) {
    if *state != GameState::Playing {
        return;
    }
    let out = COURT.x / 2.0 + BALL_SIZE;
    for (mut ball, mut velocity) in &mut balls {
        let x = ball.translation.x;
        if x.abs() <= out {
            continue;
        }
        let player_scored = x > 0.0;
        if player_scored {
            score.player += 1;
        } else {
            score.ai += 1;
        }
        ball.translation = Vec2::ZERO;
        if score.player >= WIN_SCORE || score.ai >= WIN_SCORE {
            *state = GameState::GameOver;
            velocity.0 = Vec2::ZERO;
        } else {
            // Loser receives the serve.
            velocity.0 = serve(&mut rng, player_scored);
        }
    }
}

fn restart_on_enter(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    mut paddles: Query<&mut Transform2D, (With<Paddle>, Without<Ball>)>,
    mut score: ResMut<Score>,
    mut state: ResMut<GameState>,
    mut rng: ResMut<SimRng>,
    input: Res<Input>,
) {
    if *state != GameState::GameOver || !input.just_pressed(Key::Enter) {
        return;
    }
    *score = Score::default();
    *state = GameState::Playing;
    for mut paddle in &mut paddles {
        paddle.translation.y = 0.0;
    }
    for (mut ball, mut velocity) in &mut balls {
        ball.translation = Vec2::ZERO;
        velocity.0 = serve(&mut rng, true);
    }
}
