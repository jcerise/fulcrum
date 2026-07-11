//! The Snake simulation, exactly as built across the book's From Zero chapters.
//!
//! Everything in this file follows one discipline: it is *pure state transition*. No sprites,
//! no sounds, no window — those live in `main.rs` and react to what happens here. That split
//! is what makes the tests in `tests/` possible, and it's the single most important habit the
//! From Zero track teaches.

use std::collections::VecDeque;

use fulcrum::prelude::*;

/// One square of the play field, addressed as `(column, row)` from the bottom-left.
/// A plain tuple on purpose: grid games don't need vector math, they need equality.
pub type Cell = (i32, i32);

/// Grid dimensions. 24 x 18 cells of 16 pixels = a 384 x 288 world, letterboxed by the camera.
pub const GRID_W: i32 = 24;
/// See [`GRID_W`].
pub const GRID_H: i32 = 18;
/// Size of one cell in world units (= pixels at 1x zoom).
pub const CELL: f32 = 16.0;

/// The center of a cell in world space. The grid's bottom-left corner sits at the world
/// origin, so cell `(0, 0)` centers at `(8, 8)`.
pub fn cell_center(cell: Cell) -> Vec2 {
    vec2(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}

/// The snake itself: a queue of cells, head at the front. This is *all* of the snake — the
/// sprites you see are a projection of this data, rebuilt by the presentation every frame.
#[derive(Resource)]
pub struct Snake {
    /// Body cells, head first. `VecDeque` because the game is literally "push a new head,
    /// pop the tail".
    pub body: VecDeque<Cell>,
    /// Direction of travel, as a cell delta: `(1, 0)` is rightward.
    pub dir: Cell,
    /// Buffered turns (see [`steer`]): at most two, applied one per movement step.
    pub queued: VecDeque<Cell>,
    /// Segments still owed from eating: while positive, the tail doesn't shrink.
    pub grow: u32,
}

impl Default for Snake {
    fn default() -> Self {
        // Three segments in the middle of the field, heading right.
        let head = (GRID_W / 2, GRID_H / 2);
        Self {
            body: VecDeque::from([head, (head.0 - 1, head.1), (head.0 - 2, head.1)]),
            dir: (1, 0),
            queued: VecDeque::new(),
            grow: 0,
        }
    }
}

impl Snake {
    /// The head cell.
    pub fn head(&self) -> Cell {
        *self.body.front().expect("a snake always has a body")
    }

    /// Is `cell` occupied by any segment?
    pub fn occupies(&self, cell: Cell) -> bool {
        self.body.contains(&cell)
    }
}

/// The movement clock: the snake steps once every `every` simulation ticks. Speed lives in
/// data, not in code, so eating apples can raise it.
#[derive(Resource)]
pub struct StepTimer {
    /// Ticks between movement steps (60 Hz ticks; 8 = 7.5 steps/second).
    pub every: u32,
    /// Ticks remaining until the next step.
    pub countdown: u32,
}

impl Default for StepTimer {
    fn default() -> Self {
        Self {
            every: 8,
            countdown: 8,
        }
    }
}

/// Apples eaten this round.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score(pub u32);

/// The round's phase. An enum resource is Fulcrum's idiom for "what mode is the game in".
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnakeState {
    /// The snake moves and rules apply.
    #[default]
    Playing,
    /// The snake hit something; Enter restarts.
    GameOver,
    /// The snake filled the entire grid. (Yes, really. Good luck.)
    Won,
}

/// Marks the apple entity; its cell rides along in [`OnCell`].
#[derive(Component)]
pub struct Apple;

/// Which grid cell an entity sits on (the sim's source of truth for collisions).
#[derive(Component)]
pub struct OnCell(pub Cell);

/// Announcement: an apple was eaten at this cell. The sim writes it; sound and score displays
/// react to it. The sim never knows those exist.
#[derive(Event)]
pub struct AppleEaten(pub Cell);

/// Announcement: the run ended (hit a wall, hit yourself, or won).
#[derive(Event)]
pub struct RunEnded {
    /// True if the grid was filled rather than a collision.
    pub won: bool,
}

/// Installs the whole simulation. Note what it does *not* install: anything visible.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Snake::default());
        app.world_mut().insert_resource(StepTimer::default());
        app.world_mut().insert_resource(Score::default());
        app.world_mut().insert_resource(SnakeState::default());
        app.register_event::<AppleEaten>();
        app.register_event::<RunEnded>();
        app.add_systems(Startup, spawn_first_apple);
        // One chain, explicit order: read intent, advance the world, allow restarts.
        app.add_systems(FixedUpdate, (steer, step, restart).chain());
    }
}

fn spawn_first_apple(mut commands: Commands, snake: Res<Snake>, mut rng: ResMut<SimRng>) {
    let cell = free_cell(&snake, &[], &mut rng);
    commands.spawn((
        Apple,
        OnCell(cell),
        Transform2D::from_translation(cell_center(cell)),
    ));
}

/// Pick a random unoccupied cell. Rejection sampling is fine here: the board is mostly empty,
/// and because `SimRng` is seeded, "random" is still reproducible.
fn free_cell(snake: &Snake, taken: &[Cell], rng: &mut SimRng) -> Cell {
    loop {
        let cell = (rng.range_i32(0..GRID_W), rng.range_i32(0..GRID_H));
        if !snake.occupies(cell) && !taken.contains(&cell) {
            return cell;
        }
    }
}

/// Turn intent into *buffered* turns. Two subtleties every Snake clone gets wrong at first:
/// buffering (a fast "up, left" between steps must not lose the second press) and reversal
/// (turning 180° would mean instant death, so it's rejected against the direction that will
/// actually be in effect when the turn applies).
pub fn steer(input: Res<Input>, mut snake: ResMut<Snake>, state: Res<SnakeState>) {
    if *state != SnakeState::Playing {
        return;
    }
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

/// The heart of the game: every `StepTimer::every` ticks, move one cell and apply every rule.
#[allow(clippy::too_many_arguments)] // standard ECS system shape
pub fn step(
    mut snake: ResMut<Snake>,
    mut timer: ResMut<StepTimer>,
    mut state: ResMut<SnakeState>,
    mut score: ResMut<Score>,
    apples: Query<(Entity, &OnCell), With<Apple>>,
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut eaten: EventWriter<AppleEaten>,
    mut ended: EventWriter<RunEnded>,
) {
    if *state != SnakeState::Playing {
        return;
    }
    timer.countdown -= 1;
    if timer.countdown > 0 {
        return;
    }
    timer.countdown = timer.every;

    if let Some(turn) = snake.queued.pop_front() {
        snake.dir = turn;
    }
    let head = snake.head();
    let next = (head.0 + snake.dir.0, head.1 + snake.dir.1);

    // Rule 1: walls end the run. On a grid this is two comparisons, not geometry.
    let out_of_bounds = next.0 < 0 || next.0 >= GRID_W || next.1 < 0 || next.1 >= GRID_H;
    // Rule 2: so does biting yourself. The tail cell is exempt *unless* we're growing:
    // if the tail moves away this same step, the head may enter its old cell.
    let tail = *snake.body.back().expect("non-empty");
    let bites_self = snake.occupies(next) && !(next == tail && snake.grow == 0);
    if out_of_bounds || bites_self {
        *state = SnakeState::GameOver;
        ended.write(RunEnded { won: false });
        return;
    }

    // Move: push the new head; pop the tail unless a meal is still being digested.
    snake.body.push_front(next);
    if snake.grow > 0 {
        snake.grow -= 1;
    } else {
        snake.body.pop_back();
    }

    // Rule 3: apples. Grid collision is equality.
    for (entity, on_cell) in &apples {
        if on_cell.0 != next {
            continue;
        }
        commands.entity(entity).despawn();
        snake.grow += 2;
        score.0 += 1;
        eaten.write(AppleEaten(next));
        // Speed up gently: every 3 apples, one tick faster, floor at 4.
        timer.every = 8u32.saturating_sub(score.0 / 3).max(4);
        // Rule 4: winning means there's nowhere left to put an apple.
        if snake.body.len() as i32 + snake.grow as i32 >= GRID_W * GRID_H {
            *state = SnakeState::Won;
            ended.write(RunEnded { won: true });
            return;
        }
        let cell = free_cell(&snake, &[next], &mut rng);
        commands.spawn((
            Apple,
            OnCell(cell),
            Transform2D::from_translation(cell_center(cell)),
        ));
    }
}

/// Enter, after a run ends, resets everything. Note it resets *state*, not entities the
/// presentation owns — those disappear on their own because they mirror this state.
#[allow(clippy::too_many_arguments)] // standard ECS system shape
fn restart(
    input: Res<Input>,
    mut state: ResMut<SnakeState>,
    mut snake: ResMut<Snake>,
    mut timer: ResMut<StepTimer>,
    mut score: ResMut<Score>,
    apples: Query<Entity, With<Apple>>,
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
) {
    if *state == SnakeState::Playing || !input.just_pressed(Key::Enter) {
        return;
    }
    *snake = Snake::default();
    *timer = StepTimer::default();
    *score = Score::default();
    *state = SnakeState::Playing;
    for apple in &apples {
        commands.entity(apple).despawn();
    }
    let cell = free_cell(&snake, &[], &mut rng);
    commands.spawn((
        Apple,
        OnCell(cell),
        Transform2D::from_translation(cell_center(cell)),
    ));
}
