//! From Zero, chapter 3: time and input. The snake moves on its own clock, you steer it, and
//! the edges wrap (rules that can kill you arrive in chapter 4). Note the input *buffer* —
//! tap two turns quickly and both happen, one per step.

use std::collections::VecDeque;

use fulcrum::prelude::*;

const GRID_W: i32 = 24;
const GRID_H: i32 = 18;
const CELL: f32 = 16.0;
type Cell = (i32, i32);

#[derive(Resource)]
struct Snake {
    body: VecDeque<Cell>,
    /// Direction of travel as a cell delta: (1, 0) is rightward.
    dir: Cell,
    /// Turns waiting to happen: at most two, applied one per movement step.
    queued: VecDeque<Cell>,
}

/// The snake's own clock: one movement step every `every` simulation ticks. The simulation
/// runs at 60 Hz; the snake doesn't have to.
#[derive(Resource)]
struct StepTimer {
    every: u32,
    countdown: u32,
}

#[derive(Component)]
struct SegmentView;
#[derive(Resource)]
struct Square(Handle<Texture>);

fn cell_center(cell: Cell) -> Vec2 {
    vec2(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}

/// Read intent. `just_pressed` is an edge — true only on the tick the key went down — so a
/// held key doesn't spam the queue. The reversal check compares against the direction that
/// will be in effect *when this turn applies*, not the current one: that's what makes a fast
/// "up, then left" do exactly what the player meant.
fn steer(input: Res<Input>, mut snake: ResMut<Snake>) {
    let presses = [
        (Key::W, (0, 1)),
        (Key::Up, (0, 1)),
        (Key::S, (0, -1)),
        (Key::Down, (0, -1)),
        (Key::A, (-1, 0)),
        (Key::Left, (-1, 0)),
        (Key::D, (1, 0)),
        (Key::Right, (1, 0)),
    ];
    for (key, dir) in presses {
        if !input.just_pressed(key) {
            continue;
        }
        let against = *snake.queued.back().unwrap_or(&snake.dir);
        let reversal = dir.0 == -against.0 && dir.1 == -against.1;
        if !reversal && dir != against && snake.queued.len() < 2 {
            snake.queued.push_back(dir);
        }
    }
}

/// Advance the world. Runs every tick; *does something* every `every` ticks.
fn step(mut snake: ResMut<Snake>, mut timer: ResMut<StepTimer>) {
    timer.countdown -= 1;
    if timer.countdown > 0 {
        return;
    }
    timer.countdown = timer.every;

    if let Some(turn) = snake.queued.pop_front() {
        snake.dir = turn;
    }
    let head = *snake.body.front().expect("non-empty");
    // Wrap at the edges — the `+ GRID` before `%` keeps negative numbers in range.
    let next = (
        (head.0 + snake.dir.0 + GRID_W) % GRID_W,
        (head.1 + snake.dir.1 + GRID_H) % GRID_H,
    );
    // Moving IS this: new head on, tail off. The middle never changes.
    snake.body.push_front(next);
    snake.body.pop_back();
}

// --- everything below is chapter 2's view, unchanged -----------------------------------

fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: GRID_W as f32 * CELL,
        height: GRID_H as f32 * CELL,
    };
    camera.center = vec2(GRID_W as f32 * CELL / 2.0, GRID_H as f32 * CELL / 2.0);
    let square = assets.load("white.png");
    for x in 0..GRID_W {
        for y in 0..GRID_H {
            let shade = if (x + y) % 2 == 0 { 0.10 } else { 0.12 };
            commands.spawn((
                Sprite::new(square)
                    .with_color(Color::rgb(shade, shade + 0.02, shade))
                    .with_size(Vec2::splat(CELL))
                    .with_z(-1.0),
                Transform2D::from_translation(cell_center((x, y))),
            ));
        }
    }
    commands.insert_resource(Square(square));
}

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
        transform.translation = cell_center(*cell);
        sprite.color = if index == 0 {
            Color::rgb(0.55, 1.0, 0.45)
        } else {
            Color::rgb(0.2, 0.65, 0.25)
        };
    }
}

fn main() {
    Fulcrum::new("fz03: time and input")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .insert_resource(Snake {
            body: VecDeque::from([(12, 9), (11, 9), (10, 9)]),
            dir: (1, 0),
            queued: VecDeque::new(),
        })
        .insert_resource(StepTimer {
            every: 8,
            countdown: 8,
        })
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system((steer, step).chain())
        .add_frame_system(project_snake)
        .run();
}
