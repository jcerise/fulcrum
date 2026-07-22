# Rules: Making It a Game

A toy becomes a game when something can go wrong. This chapter adds everything that can:
apples worth chasing, growth that turns your own body into the hazard, walls that end the
run, and the restart. It also faces the first question every game asks its programmer within
an hour — *where does randomness come from?* — and answers it with the most engineer-brained
idea in this whole track.

It's the longest chapter, because it's the one where you build the actual game. Take it in
order; every step ends with code that compiles.

## Step 1 — give the simulation its own file

So far everything lives in `main.rs`. Before the rules move in, split the crate the way every
Fulcrum game is split — because the *next two chapters* depend on it: chapter 5 will grow the
presentation without touching a rule, and chapter 6 will run the rules without any
presentation at all.

Create `games/my-snake/src/lib.rs`:

```rust,ignore
//! Snake: library form, so tests can drive the same simulation the binary ships.

pub mod game;
```

Create `games/my-snake/src/game.rs`, and move the *simulation* pieces into it from
`main.rs` — the constants, `Cell`, `cell_center`, `Snake`, `StepTimer`, `steer`, and `step`
— everything except `setup`, `project_snake`, `SegmentView`, `Square`, and `main`, which are
presentation and stay put. Mark the moved items `pub` as you go. The two files will shortly
look like this chapter's listings, so don't fuss over the seams; the point is the *sorting
test* you just performed on every item: **does this decide what happens (simulation), or
what it looks like (presentation)?** You'll apply that test to every line you write from now
on.

While you're in `game.rs`, the moved types grow into their final forms — the snake learns to
grow, and gets two helpers the rules will lean on:

```rust,ignore
use std::collections::VecDeque;

use fulcrum::prelude::*;

pub type Cell = (i32, i32);

pub const GRID_W: i32 = 24;
pub const GRID_H: i32 = 18;
pub const CELL: f32 = 16.0;

pub fn cell_center(cell: Cell) -> Vec2 {
    vec2(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}

#[derive(Resource)]
pub struct Snake {
    pub body: VecDeque<Cell>,
    pub dir: Cell,
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

#[derive(Resource)]
pub struct StepTimer {
    pub every: u32,
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
```

The `Default` impls replace the literals that were in `main` — the starting snake is now
something the game can *ask for*, which is what makes the restart in step 6 a one-liner.

## Step 2 — the new nouns

Below that, type the state the rules will need. Each one is a decision you can now make
yourself with chapter 2's rule of thumb — exactly-one things are resources, many-things are
entities:

```rust,ignore
/// Apples eaten this round.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score(pub u32);

/// The round's phase. An enum resource is Fulcrum's idiom for "what mode is the game in".
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnakeState {
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

/// Announcement: an apple was eaten at this cell.
#[derive(Event)]
pub struct AppleEaten(pub Cell);

/// Announcement: the run ended (hit a wall, hit yourself, or won).
#[derive(Event)]
pub struct RunEnded {
    /// True if the grid was filled rather than a collision.
    pub won: bool,
}
```

Two of these deserve their design arguments up front:

**`SnakeState`, the mode enum.** Snake has phases — playing, dead, (theoretically)
victorious — and things behave differently in each. The idiom is almost embarrassingly
plain: an enum in a resource, checked at the top of systems with an early return. You may
feel the pull toward something grander — a state-pattern hierarchy, transition tables.
Resist it at this scale. An enum plus early returns *is* a state machine, the compiler
exhaustively checks every `match` over it, and any system can read it. When a game genuinely
outgrows this (nested menus, pause-inside-cutscene), you'll know, and you'll reach for real
machinery then; the mistake is paying for it now.

**The two `Event` types.** These are the ECS's pub-sub channel: the simulation will
*announce facts* — an apple was eaten, the run ended — and any number of systems may
subscribe. Nobody subscribes yet, in this whole chapter! Writing to a channel nobody reads
looks like over-engineering; it's actually the seam chapter 5 is built on. Sound, score
popups, screen shake: all of it will attach to these announcements without the simulation
changing at all. The sim speaks; whether anything is listening is not its business.

## Step 3 — the plugin: your game as an installable unit

The resources, events, and systems now need registering — but `main.rs` shouldn't have to
list the simulation's internals line by line (and in chapter 6, tests will need to install
the same list *without* `main.rs`). Fulcrum's unit of packaging is the same one
`DefaultPlugins` uses. Still in `games/my-snake/src/game.rs`, below the event types from
step 2, add:

```rust,ignore
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
```

> **Toolbox — `Plugin`:** a bundle of registrations with a `build` method — "here's
> everything my feature needs installed." `main` will say `.with_plugin(GamePlugin)` and
> mean *the entire game*. Plugins are how Fulcrum code ships: the engine's defaults, this
> sim, and (in Part II) even mods all arrive through the same door.
>
> **Toolbox — schedules (`Startup`, `FixedUpdate`):** inside a plugin you name the schedule
> explicitly: `add_systems(Startup, ...)` is `add_startup`, and `add_systems(FixedUpdate,
> ...)` is `add_system` — FixedUpdate is the 60 Hz simulation clock's proper name. Same
> machinery, fully spelled out.
>
> **Toolbox — `register_event::<T>()`:** events need declaring once so the engine can
> manage their queues (delivery, and cleanup after everyone's read them). Forgetting this
> line is a startup panic, not a mystery — but it's always *this* line.

> **Sidebar — what `GamePlugin` actually does, and why it exists.** `build` runs exactly
> once, at the moment something calls `.with_plugin(GamePlugin)`, and its body is the same
> kind of setup calls `main` has been making since chapter 1 — just relocated. Read it top
> to bottom:
>
> - **The four `insert_resource` lines** place the simulation's state into the world at its
>   starting values — the `Default` impls you wrote in steps 1 and 2. From this moment on,
>   any system can ask for `Res<Snake>`, `ResMut<Score>`, and the rest, and get *this* data.
> - **The two `register_event` lines** open the announcement channels from step 2, so the
>   engine has queues ready before anything writes or reads.
> - **`add_systems(Startup, spawn_first_apple)`** runs one system, once, before the first
>   tick: the opening apple. (You'll write `spawn_first_apple` in step 4.)
> - **`add_systems(FixedUpdate, (steer, step, restart).chain())`** is the game's heartbeat:
>   every simulation tick, these three systems run in exactly this order — read the player's
>   intent, advance the world, then offer the restart.
>
> Nothing in that list is new machinery; what's new is that it's *packaged*. And the reason
> is that this list — resources, events, systems, order — effectively **is** the game, and
> two different callers are about to need it verbatim. In step 7, `main.rs` will install it
> under a real window; in chapter 6, the tests will install it with no window at all. Both
> write `.with_plugin(GamePlugin)`, and both are guaranteed the *identical* simulation.
> Without the plugin, those ten registrations would live in two files and drift apart the
> first time you added a resource and forgot one copy — the kind of bug where the game works
> but the tests quietly test something else. The plugin turns "the entire simulation" into a
> single named value you can hand to anyone, and it deliberately installs nothing visible:
> that restraint is what makes chapter 6's headless tests possible.

`steer` needs one addition to match the plugin — the mode gate from step 2 (a dead snake
shouldn't steer). Add the parameter and the guard:

```rust,ignore
pub fn steer(input: Res<Input>, mut snake: ResMut<Snake>, state: Res<SnakeState>) {
    if *state != SnakeState::Playing {
        return;
    }
    // ... the rest exactly as you wrote it in chapter 3 ...
```

## Step 4 — randomness you can regress-test

The first apple must appear somewhere "random." Reach for your usual tool — a global `rand()`
seeded by the clock — and you've just built a program whose behavior can never be reproduced:
every bug report becomes "it happened once." Games solved this decades ago, the same way
you'd solve it for any system you wanted reproducible: **all gameplay randomness flows from
one seeded generator.** Fulcrum provides it as a resource, `SimRng`:

```rust,ignore
fn spawn_first_apple(mut commands: Commands, snake: Res<Snake>, mut rng: ResMut<SimRng>) {
    let cell = free_cell(&snake, &[], &mut rng);
    commands.spawn((
        Apple,
        OnCell(cell),
        Transform2D::from_translation(cell_center(cell)),
    ));
}

/// Pick a random unoccupied cell. Rejection sampling is fine here: the board is mostly
/// empty, and because `SimRng` is seeded, "random" is still reproducible.
fn free_cell(snake: &Snake, taken: &[Cell], rng: &mut SimRng) -> Cell {
    loop {
        let cell = (rng.range_i32(0..GRID_W), rng.range_i32(0..GRID_H));
        if !snake.occupies(cell) && !taken.contains(&cell) {
            return cell;
        }
    }
}
```

> **Toolbox — `ResMut<SimRng>`:** the simulation's random number generator, seeded from
> `FulcrumConfig` (chapter 6 sets the seed by hand; until then it's a default). Same seed →
> same sequence of draws → the apples appear in the same places, in the same order, every
> run. The player still can't predict them; the *test suite* can. "Random" was never the
> requirement — *unpredictable to the player* was, and a seeded stream delivers that while
> staying a pure function of its history.

This is the moment to name the discipline `SimRng` belongs to, because it's bigger than
randomness. **The simulation may consume only: its own state, this tick's `Input`, `SimRng`,
and the tick clock.** Not the wall clock, not the frame rate, not a thread's whims. Keep that
contract and the whole game remains a deterministic function of `(seed, input history)` —
the property chapter 6 cashes in. Fulcrum can't stop you from calling `SystemTime::now()`
in a system; it can only promise you'll regret it.

(Two details in `free_cell` reward a second look. The rejection-sampling `loop` is fine
because the board is mostly empty, so it nearly always exits on the first draw. And the
fussy-looking `taken` parameter exists because, later this chapter, the head will already
have moved onto the eaten apple's cell when the next apple spawns — without it, the new
apple could legally appear *under the snake's chin*. Corner cases. They're coming.)

## Step 5 — the rules, all of them

Now replace chapter 3's `step` with the real one. This is the heart of the game — read the
comments as you type; every rule is one small block:

```rust,ignore
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
```

The mechanics you already own — the countdown, the turn dequeue, push-front. What's new is
worth savoring rule by rule:

- **Collision on a grid is equality.** The wall check is two comparisons per axis; the apple
  check is `on_cell.0 != next`. This is the payoff of chapter 2's `(i32, i32)` decision, and
  it's why this track builds Snake and not a platformer: every physical question is trivial,
  so all your attention goes to the *rules*.
- **Rule 2 hides a classic corner case** — and it's *rules* logic, not code logic. Moving
  into the cell your own tail occupies is usually **legal**: the tail vacates it in the same
  step. Unless the snake just ate — then the tail stays put this step, and the same move
  kills you. Players of every Snake clone have felt the difference between getting this
  right and wrong without being able to say what it was. Rules live in the corner cases;
  write them deliberately, and (chapter 6) test them.
- **Growing is delightfully lazy.** We don't insert segments; we *owe* them. Eat an apple,
  `grow += 2`, and for the next two steps the tail simply doesn't retreat. No special cases,
  no inserted cells. When a mechanic can be expressed as *briefly not doing something you
  normally do*, take the deal.
- **The speedup is one assignment** — because chapter 3 put the speed in data. Promised,
  delivered.
- **`EventWriter<T>`** is the announcing end of the channel from step 2: `ended.write(...)`
  costs one line and takes no position on who's listening.

## Step 6 — the restart

One more system completes the loop of play:

```rust,ignore
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
```

Notice what it does **not** touch: the sprites. It resets *state*; chapter 2's projection
notices the three-segment snake and prunes the views on the next frame, unprompted. This is
where that architecture starts paying rent — resetting a game whose visuals are a projection
is trivial, while resetting one whose visuals are hand-managed is a bug factory ("the old
segments are still on screen after restart" is a bug you will simply never have).

## Step 7 — point `main.rs` at the library

Back in `main.rs`: the simulation types now come from your library crate, the resources and
systems you moved are gone, and `GamePlugin` installs the game. The top becomes:

```rust,ignore
use fulcrum::prelude::*;
use my_snake::game::{self, Apple, CELL, GRID_H, GRID_W, GamePlugin, Snake};
```

Two things to notice about that import. First, `my_snake` with an underscore — Cargo maps
the crate name `my-snake` to that identifier. Second, `cell_center` is deliberately *not*
in the list, even though you moved it to `game.rs` in step 1: the `self` imports the `game`
module itself, and the two call sites left in this file — one in `setup`, one in
`project_snake` — change to the qualified form `game::cell_center(...)`. (Why qualify
instead of import? Taste, mostly: `game::cell_center` reads as "the *sim's* coordinate
convention, consulted by the view" — the crate boundary kept visible at the call site.)
In `main`, drop the `insert_resource(Snake {...})` and `insert_resource(StepTimer {...})`
blocks and the `add_system` line, and add the plugin:

```rust,ignore
    Fulcrum::new("My Snake")
        .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
        .with_plugin(DefaultPlugins)
        .with_plugin(GamePlugin)
        .add_startup(setup)
        .add_frame_system(project_snake)
        .add_frame_system(dress_apples)   // <-- new, below
        .run();
```

Two presentation-side edits finish the wiring. First, delete the hard-coded apple from
`setup` — the sim spawns real ones now. Second, the sim spawns them *bare*: an `Apple`, an
`OnCell`, a `Transform2D` — position is gameplay, but no `Sprite`, because how it looks is
none of the sim's business. The view dresses them:

```rust,ignore
/// The sim spawns apple *entities*; the view gives each one a sprite.
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
```

That `Without<Sprite>` filter is doing quiet, tidy work: the query matches only apples that
haven't been dressed yet, so the system is naturally idempotent — new apples get sprites
once, dressed ones are never touched again. (And if the compiler is still complaining about
`cell_center`, that's the qualified-call change from the top of this step:
`game::cell_center(...)`.)

## Checkpoint

```text
cargo run -p my-snake
```

Play it. Chase apples, grow, speed up, die at a wall, die in your own coils, press Enter, go
again. It's a *game* — someone can want something and fail to get it.

Your `game.rs` should now match `games/snake/src/game.rs` — not "the reference for this
chapter" but the actual simulation the shipped game runs; from here on your crate and the
real one are structurally identical. The windowed reference for this chapter's bare view is
`games/snake/examples/fz04_rules.rs` (`cargo run -p snake --example fz04_rules`).

New vocabulary this chapter:

| Tool | What it's for |
| --- | --- |
| `Plugin` / `with_plugin` | package registrations; install a whole feature at once |
| `add_systems(Startup, ...)` / `(FixedUpdate, ...)` | the schedule names behind `add_startup` / `add_system` |
| `#[derive(Event)]` + `register_event::<T>()` | declare a pub-sub announcement type |
| `EventWriter<T>` | the announcing end: `writer.write(event)` |
| `ResMut<SimRng>` | seeded, reproducible gameplay randomness |
| `Without<T>` in a query | match entities that *lack* a component |
| enum resource + early return | the right-sized state machine for game modes |

## Exercises

From here on you're editing your real simulation, `src/game.rs` — the rules are whatever you
make them say.

1. **Golden apples.** Give each spawned apple a 1-in-5 chance (`rng.chance(0.2)`) of being
   golden: worth 3 growth instead of 2. You'll want a marker component on the entity — and
   note that the view doesn't know about it yet, so your golden apple will look ordinary
   until you also touch `dress_apples`. That mild annoyance *is* the sim/presentation split,
   felt from the inside; chapter 5 gives it words.
2. **Break the tail rule on purpose.** Delete `&& snake.grow == 0` from the self-bite check,
   then construct the play situation where the difference shows (you'll need to have just
   eaten, with your head one step behind your tail — a tight spiral). This is practice for a
   permanent skill: verifying a corner case by *reaching* it, not by reading the code and
   nodding.
3. **Wrap mode (harder).** Add a `WrapMode(bool)` resource; when true, edges wrap (chapter
   3's modulo) instead of killing. Two things to get right beyond the arithmetic: the
   self-bite rule must still apply after wrapping, and you should be able to say why the win
   condition doesn't care either way. Then ask the chapter-4 question of your own feature:
   what's *its* corner case? (Hint: a wrapped head entering the column the tail is about to
   vacate.)

The game is now complete — playable, losable, restartable, honest. It's also mute, colorless
about its feelings, and doesn't show the score. Everything missing is *presentation*, and
presentation is a different kind of code with different rules. That split is next.
