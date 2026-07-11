# State: What a Game Knows

Ask a beginner what Snake *is* and they'll describe what it looks like: green squares chasing
an apple. Ask a game programmer and you'll get something else entirely:

```rust,ignore
struct Snake {
    body: VecDeque<Cell>,   // cells the snake occupies, head first
}
type Cell = (i32, i32);     // (column, row) on a 24 x 18 grid
```

That's the snake. All of it. Not the sprites, not the animation — a queue of grid cells. This
chapter is about the single most useful reframing in game programming: **the game is its
state; everything on screen is a projection of it.** If you've built UIs with React or Elm,
you already believe this — `view = f(state)` — and games are where that idea came from before
it had a name.

## Choosing the state well

Notice what the representation buys before a single rule is written. A grid game doesn't need
positions like `x: 187.4` — it needs *which cell*, so `(i32, i32)`, and collision later
becomes `==` instead of geometry. The body is ordered, head-first, and the game's core verb —
slither forward — will be literally `push_front(new_head); pop_back()`. A `VecDeque` isn't a
data structure we happened to pick; it's the mechanics of Snake, stated as a type. Time spent
making the state *shaped like the game* repays itself in every system you write afterward.
This is the same judgment you exercise designing a schema before writing queries, and it's
just as decisive here.

## Where state lives: the ECS, for people who know databases

Fulcrum stores all game state in one place — the **World** — organized as an ECS: entities,
components, systems. Every explanation of ECS for game people uses games; you're an engineer,
so here's the honest version: **an ECS is an in-memory relational database, tuned for
iteration.**

- An **entity** is a row id. Literally just an id — it *has* nothing, *is* nothing, until…
- **Components** are values attached to an entity — columns, except any entity can have any
  subset of them. `Transform2D` (where), `Sprite` (what it looks like), your own structs.
- A **system** is a function that runs on a schedule (chapter 1's `add_system`) and asks for
  data by shape. A **query** is exactly a `SELECT`: `Query<&mut Transform2D,
  With<Patroller>>` reads "every row that has both a `Transform2D` and a `Patroller`, give me
  the transform, writable."
- A **resource** is a singleton — configuration, the score, *the snake* — data with exactly
  one instance, addressed by type instead of by entity.

Why do games do this instead of `struct Player`, `struct Apple`, objects with methods? Because
games are the worst case for class hierarchies. Thirty years of `MovingEntity extends Entity`,
`Player extends MovingEntity`… and then the design asks for a ghost that moves like a player
but collides like a wall, and the hierarchy shatters. Game *designs* recombine behaviors too
freely for inheritance to keep up. ECS is composition taken to its logical end: an entity is
*only* the set of facts attached to it, and behavior comes from systems matching facts. When
this book later gives something a `Health` component to make it damageable — anything, a
door, a snowman — no class diagram needs renegotiating. Rule of thumb, and the only design
guidance you need for a while: **many similar things → entities with components; exactly-one
things → a resource.** Apples: entities. The snake, the score: resources.

## Projection: state on one side, pixels on the other

Run this chapter's program:

```text
cargo run -p snake --example fz02_state
```

A snake bent into an S, drawn from a hand-written `VecDeque` literal. Nothing moves — there
are no rules yet — but the architecture of the whole game is already on screen. The state is
inserted as a resource in `main`:

```rust,ignore
.insert_resource(Snake {
    body: VecDeque::from([(12, 9), (11, 9), (10, 9), (10, 8), (10, 7), (11, 7), (12, 7)]),
})
```

and one *frame* system — `add_frame_system`, the render clock, because drawing is
presentation — reconciles sprites against it:

```rust,ignore
fn project_snake(
    snake: Res<Snake>,
    mut segments: Query<(Entity, &mut Transform2D, &mut Sprite), With<SegmentView>>,
    square: Option<Res<Square>>,
    mut commands: Commands,
) {
    let Some(square) = square else { return };
    let mut views: Vec<_> = segments.iter_mut().collect();
    for _ in views.len()..snake.body.len() {
        commands.spawn((Sprite::new(square.0) /* … */, Transform2D::default(), SegmentView));
    }
    for (entity, _, _) in views.iter().skip(snake.body.len()) {
        commands.entity(*entity).try_despawn();
    }
    for (index, ((_, transform, sprite), cell)) in
        views.iter_mut().zip(snake.body.iter()).enumerate()
    {
        transform.translation = cell_center(*cell);
        sprite.color = if index == 0 { /* head, brighter */ } else { /* body */ };
    }
}
```

Too few sprite entities? Spawn more. Too many? Despawn the extras. Then position every sprite
from its cell. It's a tiny reconciler — React's diffing, twenty lines, by hand. The state
never knows the sprites exist; data flows one direction. When the snake grows in chapter 4,
this function will draw the new segment without being told, because it doesn't render
*changes*, it renders *state*.

Two mechanical notes on the ECS code itself, and then you have all of it:

- **`Commands` is a write-ahead queue, not a live handle.** `commands.spawn(...)` doesn't
  create the entity mid-loop; it records the intent, applied after the system finishes. This
  is how the ECS lets many systems run without you fighting the borrow checker over the whole
  world — the same reason databases batch writes in a transaction.
- **`Option<Res<Square>>`** — the texture handle might not exist yet on the very first frame
  (startup hasn't stashed it). Asking for an `Option` instead of the resource itself turns
  "crash on frame zero" into "skip a frame." You'll use this pattern constantly.

One more habit worth naming, because it was a *choice*: the grid math lives in one function
(`cell_center`), converting `(column, row)` to world coordinates exactly once, at the
projection boundary. The simulation will never think in pixels. When you someday want cells
to be 24 pixels, or the board centered differently, it's one line.

Reshape the `VecDeque` literal and rerun — the view follows without complaint. That obedience
is the point of the whole chapter. Next we make the state change over time, which means
finally taking input and the tick seriously.
