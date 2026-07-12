# State: What a Game Knows

Ask a beginner what Snake *is* and they'll describe what it looks like: green squares chasing
an apple. Ask a game programmer and you'll get a data structure. This chapter is about the
single most useful reframing in game programming — **the game is its state; everything on
screen is a projection of it** — and by the end of it your window will show a snake drawn
entirely from data, with not one line of code that "moves a sprite."

If you've built UIs with React or Elm, you already believe this: `view = f(state)`. Games are
where that idea came from, before it had a name.

We're rebuilding `main.rs` this chapter. The patrolling square was chapter 1's hello-world;
delete `Patroller`, `patrol`, and the spawn in `setup` — keep the skeleton (`main`, the empty
`setup`, the imports) and we'll grow Snake in its place.

## Step 1 — the snake, as data

At the top of `main.rs`, type the entire snake:

```rust,ignore
use std::collections::VecDeque;

use fulcrum::prelude::*;

const GRID_W: i32 = 24;
const GRID_H: i32 = 18;
const CELL: f32 = 16.0;

/// A grid square, `(column, row)` from the bottom-left.
type Cell = (i32, i32);

/// The entire snake: an ordered list of cells, head first.
#[derive(Resource)]
struct Snake {
    body: VecDeque<Cell>,
}
```

That's the snake. All of it. Not the sprites, not the animation — a queue of grid cells. And
before a single rule exists, notice what this *particular* shape buys:

- **`(i32, i32)`, not `x: 187.4`.** Snake is a grid game; it needs *which cell*, not
  geometry. Every collision question in chapter 4 — walls, apples, biting yourself — will be
  an `==` comparison.
- **A `VecDeque`, head first.** The game's core verb — slither forward — will literally be
  `push_front(new_head); pop_back()`. This isn't a container we happened to pick; it's the
  mechanics of Snake, stated as a type.

Time spent making state *shaped like the game* repays itself in every function you write
afterward. It's the same judgment you exercise designing a schema before writing queries, and
it's just as decisive here.

Add the one helper that touches both worlds — grid coordinates in, world coordinates out:

```rust,ignore
/// Grid coordinates -> world coordinates (cell centers; the grid's corner is the origin).
fn cell_center(cell: Cell) -> Vec2 {
    vec2(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}
```

This is a *choice* worth naming: the conversion from cells to pixels happens in exactly one
function, at the drawing boundary. The simulation will never think in pixels. When you
someday want 24-pixel cells or a differently-centered board, it's one line.

## Where state lives: the ECS, for people who know databases

You've now used `#[derive(Resource)]` and `#[derive(Component)]` (chapter 1's `Patroller`)
without a definition. Here it is, in your language rather than gamer language. Fulcrum stores
all game state in one place — the **World** — organized as an ECS: entities, components,
systems. Honestly stated: **an ECS is an in-memory relational database, tuned for
iteration.**

- An **entity** is a row id. Literally just an id — it *has* nothing, *is* nothing, until…
- **Components** are values attached to an entity — columns, except any entity can have any
  subset of them. `Transform2D` (where), `Sprite` (what it looks like), your own structs
  (`Patroller`). `commands.spawn((A, B, C))` from chapter 1 was an `INSERT`.
- A **system** — every function you've registered with `add_startup` or `add_system` — asks
  for data by shape. A **query** is a `SELECT`: `Query<&mut Transform2D, With<Patroller>>`
  reads "every row that has both a `Transform2D` and a `Patroller`; give me the transform,
  writable."
- A **resource** is a singleton — configuration, the score, *the snake* — data with exactly
  one instance, addressed by type instead of by entity.

Why do games do this instead of `struct Player`, `struct Apple`, objects with methods?
Because games are the worst case for class hierarchies. Thirty years of `MovingEntity extends
Entity`, `Player extends MovingEntity`… and then the design asks for a ghost that moves like
a player but collides like a wall, and the hierarchy shatters. Game *designs* recombine
behaviors too freely for inheritance to keep up. ECS is composition taken to its logical end:
an entity is *only* the set of facts attached to it, and behavior comes from systems matching
facts.

Rule of thumb — the only design guidance you need for a while: **many similar things →
entities with components; exactly-one things → a resource.** Apples: entities. The snake, the
score: resources. You'll apply this rule a half-dozen times before the track ends.

## Step 2 — a board to see it on

Rewrite `setup` to build the stage: a camera that frames the grid, and a checkerboard floor
so motion (next chapter) reads against something. Two new component/resource types first:

```rust,ignore
/// Marks sprites that mirror one body segment each.
#[derive(Component)]
struct SegmentView;

/// The one texture in the whole game: an 8x8 white square, tinted per use.
#[derive(Resource)]
struct Square(Handle<Texture>);
```

```rust,ignore
fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    // Show exactly the 384x288 grid regardless of window size (bars fill the rest).
    camera.scaling = ScalingMode::Letterbox {
        width: GRID_W as f32 * CELL,
        height: GRID_H as f32 * CELL,
    };
    camera.center = vec2(GRID_W as f32 * CELL / 2.0, GRID_H as f32 * CELL / 2.0);

    let square = assets.load("white.png");
    // A checkerboard floor, one tinted square per grid cell.
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
    // One apple, hard-coded for now (chapter 4 makes it appear by the rules).
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(1.0, 0.35, 0.3))
            .with_size(Vec2::splat(CELL - 4.0))
            .with_z(1.0),
        Transform2D::from_translation(cell_center((18, 12))),
    ));
    commands.insert_resource(Square(square));
}
```

> **Toolbox — `ResMut<Camera2D>`:** the camera is a resource (there's exactly one — the rule
> of thumb again), and `ResMut` is `Res`'s writable sibling. `ScalingMode::Letterbox` says
> "always show exactly this rectangle of the world, bars on whatever doesn't fit," which
> frees the game from ever caring about window size. Set-and-forget.
>
> **Toolbox — `.with_z(...)`:** draw order. The floor at `-1.0` sits under the apple at
> `1.0`; segments will go at `2.0`. Explicit z beats "hope the spawn order works out."
>
> **Toolbox — `commands.insert_resource(...)`:** the same operation as `.insert_resource`
> on the app builder, but from inside a system — here stashing the texture handle so other
> systems can use it without reloading.

Note that 432 floor sprites raised nobody's pulse. Entities are row ids, not objects — games
routinely run tens of thousands.

## Step 3 — put a snake in the world

In `main`, insert the snake as a hand-written literal — an S-shape, so it's unmistakably
*our* data on screen:

```rust,ignore
fn main() {
    Fulcrum::new("My Snake")
        .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
        // The snake, as pure data. Reshape it and rerun — the view just follows.
        .insert_resource(Snake {
            body: VecDeque::from([
                (12, 9),
                (11, 9),
                (10, 9),
                (10, 8),
                (10, 7),
                (11, 7),
                (12, 7),
            ]),
        })
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_frame_system(project_snake)   // written in the next step
        .run();
}
```

## Step 4 — the projection

Now the function this chapter exists for. It runs every frame and has one job: **make the
sprites agree with the state.**

```rust,ignore
/// The projection: every frame, make the sprites agree with the state. Spawn views if the
/// body grew, despawn extras if it shrank, then reposition all of them. State -> pixels,
/// one direction only.
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
            Color::rgb(0.55, 1.0, 0.45) // the head, brighter
        } else {
            Color::rgb(0.2, 0.65, 0.25)
        };
    }
}
```

Read it top to bottom: too few sprite entities? Spawn more. Too many? Despawn the extras.
Then position and tint every sprite from its cell. It's a tiny reconciler — React's diffing,
twenty lines, by hand. The state never learns the sprites exist; data flows one direction.
When the snake grows in chapter 4, this function will draw the new segment without being
told, because it doesn't render *changes*, it renders *state*.

> **Toolbox — `add_frame_system(fn)`:** the render clock's sibling of `add_system`. Frame
> systems run when a frame is drawn — 60 times a second, 144, whatever the display wants —
> and by Fulcrum convention they *read* simulation state and never write it. Drawing is
> presentation, so `project_snake` lives here. The tick/frame split is the engine's most
> important line, and you now have code on both sides of it.
>
> **Toolbox — `Commands` is a write-ahead queue, not a live handle.** `commands.spawn(...)`
> doesn't create the entity mid-loop; it records the intent, applied after the system
> finishes. This is how the ECS lets many systems run without you fighting the borrow
> checker over the whole world — the same reason databases batch writes in a transaction.
>
> **Toolbox — `Option<Res<Square>>`:** the texture handle might not exist yet on the very
> first frame (startup hasn't stashed it). Asking for an `Option` instead of the resource
> itself turns "crash on frame zero" into "skip a frame." You'll use this pattern
> constantly.

## Checkpoint

```text
cargo run -p my-snake
```

A snake bent into an S, head glowing, apple waiting, checkerboard underneath. Nothing moves —
there are no rules yet — but the architecture of the whole game is already on screen: state
on one side, a projection on the other, and an arrow between them that points one way.

Prove it to yourself before moving on: change one cell in the `VecDeque` literal and rerun.
You edited *data*; the picture followed. The reference for this chapter is
`games/snake/examples/fz02_state.rs` (`cargo run -p snake --example fz02_state`).

New vocabulary this chapter:

| Tool | What it's for |
| --- | --- |
| `#[derive(Resource)]` / `#[derive(Component)]` | declare your own state types to the world |
| entity / component / resource | row id / facts attached to it / typed singleton |
| `ResMut<T>` | writable resource access |
| `Camera2D` + `ScalingMode::Letterbox` | frame a fixed world rectangle in any window |
| `add_frame_system(fn)` | run on the render clock; read state, draw things |
| `Option<Res<T>>` | tolerate a resource that doesn't exist yet |
| `try_despawn` / `Commands` queueing | structural changes, applied after the system runs |

## Exercises

1. **Draw your initials.** Replace the body literal with cells spelling a letter or two. Ten
   seconds of work — but notice what you're doing while you do it: *authoring game state
   directly*, with no tooling between you and the world. Level editors are this exercise with
   a nicer pen.
2. **A second apple.** Add another hard-coded apple in `setup`. Then answer, from the rule of
   thumb in this chapter: why are apples spawned entities while the snake is a resource? What
   would it cost to have a third, a tenth, a hundredth apple in each design?
3. **Gradient body (harder).** Make the body fade toward the tail instead of one flat green —
   everything you need is the segment `index` and `snake.body.len()`, both already in
   `project_snake`'s loop. The shipped game does this (`games/snake/src/main.rs`); write yours
   first, then compare. If your version recomputes colors every frame and that feels
   wasteful — good instinct, wrong battlefield. Reread the projection step.

The view's obedience to the data is the point of the whole chapter. Next we make the state
change over time — which means finally taking input and the tick seriously.
