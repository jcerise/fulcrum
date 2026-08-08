# Quickstart

Five minutes, four steps, one file: a window with a square you steer with the arrow keys,
stopped at the walls. No art, no config files, no ECS theory — just enough to have something
running that you can start poking at.

This assumes you write Rust already and can read a `Query` signature without an explanation.
If you want the *why* behind everything here, start at [Chapter 1](ch01-window.md) instead;
if you've never built a game before, start at [From Zero](fz01-what-is-a-game.md).

The finished program is in the repo — run it now if you just want to see it:

```text
cargo run -p grove --example quickstart
```

## Step 0 — a crate

```text
cargo new my-game
cd my-game
mkdir assets
```

`Cargo.toml`:

```toml
[dependencies]
fulcrum = { path = "../fulcrum/crates/fulcrum" }
# Manifest-only. Fulcrum's #[derive(Component)] expands to `bevy_ecs::` paths, so the
# crate name has to resolve here. You will never write `bevy_ecs::` yourself.
bevy_ecs = "0.19"

# Fulcrum leans on wgpu and image decoding; unoptimized dependencies make debug builds
# painful to play. Games in this workspace inherit this — a standalone crate needs its own.
[profile.dev.package."*"]
opt-level = 2
```

We need exactly one image: an 8×8 solid white square. Every colored rectangle in this program is a
tinted, stretched copy of it, which is the standard trick for getting shapes on screen without
an art pipeline. Copy the one the sample games use:

```text
cp ../fulcrum/games/grove/assets/white.png assets/
```

> **Fulcrum-specific:** there are no primitive-shape draw calls. The renderer draws *textured
> quads*, full stop — `Gizmos` (`line`/`rect`/`circle`) exists, but it's for debug overlays
> and every call is a no-op in release builds by default. A white texture plus `with_color`
> and `with_size` is the idiomatic way to draw a rectangle you mean to ship.

## Step 1 — the window

`src/main.rs`, in full:

```rust,ignore
use fulcrum::prelude::*;

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Fulcrum Quickstart".into(),
        window_size: (800, 600),
        clear_color: Color::rgb(0.08, 0.09, 0.12),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .run();
}
```

```text
cargo run
```

A dark 800×600 window, vsync'd, closing cleanly. Four things happened:

- **`use fulcrum::prelude::*;`** is the entire public API surface. There is no second import
  you're missing.
- **`FulcrumConfig`** is the one place engine-wide knobs live — window, clear color, tick rate,
  RNG seed, hot reload, gizmos. `Fulcrum::new("title")` is the shorthand when the defaults
  suffice. It's also inserted as a resource, so any system can read it back via
  `Res<FulcrumConfig>`.
- **`AssetServer::new(...)`** points the engine at your `assets/` directory. The
  `concat!(env!("CARGO_MANIFEST_DIR"), "/assets")` idiom resolves the path at compile time
  relative to *your crate*, so the binary works from any working directory.
- **`DefaultPlugins`** is the bundle that puts a game on screen: window, renderer, assets,
  input, audio, animation, scenes, UI, and the debug inspector in debug builds. A *plugin* is
  just a unit of setup — you'll write your own the moment `main.rs` gets crowded.

`run()` takes over the thread and never returns. That inversion is deliberate: the loop is
where all the platform misery lives (window events, GPU frame pacing, catching up missed
ticks), and it's identical in every game.

## Step 2 — a square

Add a startup system above `main` and register it:

```rust,ignore
const PLAYER_SIZE: f32 = 40.0;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let white = assets.load("white.png");
    commands.spawn((
        Sprite::new(white)
            .with_color(Color::rgb(0.9, 0.7, 0.3))
            .with_size(Vec2::splat(PLAYER_SIZE)),
        Transform2D::from_xy(0.0, 0.0),
    ));
}
```

```rust,ignore
    .with_plugin(DefaultPlugins)
    .add_startup(setup)          // <-- new
    .run();
```

An amber square, dead center. What's going on:

- Systems are plain functions; their **parameters are the dependency injection**. Declare what
  you need from the world — `Commands`, `AssetLoader`, queries, resources — and the engine
  hands it to you. This is true of every function you register, no traits to implement.
- `commands.spawn((A, B, C))` creates an **entity** carrying those **components**. The
  renderer draws every entity that has both a `Sprite` and a `Transform2D`. Nothing is
  special-cased for the player; that pairing *is* how drawing works.
- `AssetLoader::load` returns a cheap, copyable `Handle` — loading happens once, handles get
  passed around.

> **Fulcrum-specific — the coordinate system:** world space is **+Y up, origin at the center
> of the screen, 1 unit = 1 pixel** at the default zoom. There's no camera entity to spawn;
> `Camera2D` is a resource defaulting to `center: (0,0), zoom: 1.0`. So the window covers
> x ∈ [-400, 400], y ∈ [-300, 300], and `Transform2D::from_xy(0.0, 0.0)` is the middle.
> Sprites are centered on their translation by default (`anchor: (0.5, 0.5)`).

## Step 3 — arrow keys

A marker component so systems can find the square, and a system that moves it:

```rust,ignore
const PLAYER_SPEED: f32 = 300.0; // units per second

#[derive(Component)]
struct Player;
```

```rust,ignore
    commands.spawn((
        Sprite::new(white)
            .with_color(Color::rgb(0.9, 0.7, 0.3))
            .with_size(Vec2::splat(PLAYER_SIZE)),
        Transform2D::from_xy(0.0, 0.0),
        Player,                  // <-- new
    ));
```

```rust,ignore
fn move_player(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::Left) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::Right) {
        dir.x += 1.0;
    }
    if input.pressed(Key::Down) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::Up) {
        dir.y += 1.0;
    }

    for mut transform in &mut players {
        transform.translation += dir.normalize_or_zero() * PLAYER_SPEED * time.fixed_delta;
    }
}
```

```rust,ignore
    .add_startup(setup)
    .add_system(move_player)     // <-- new
    .run();
```

Arrow keys move the square, diagonals at the same speed as the axes (that's
`normalize_or_zero`, which also spares you a divide-by-zero when no key is held).

This is the step with the most Fulcrum in it:

> **Fulcrum-specific — two clocks, two schedules.** `add_system` puts a function in
> **`FixedUpdate`**: the simulation, running at exactly `tick_rate` (60 by default) steps per
> second regardless of framerate, with input sampled at tick boundaries. `add_frame_system`
> puts it in **`Update`**: once per rendered frame, for cosmetics only. `Time` carries both —
> use `time.fixed_delta` (a constant 1/60) in simulation systems and `time.frame_delta` in
> frame systems. Mixing them up is the classic Fulcrum beginner bug: `frame_delta` in a
> `FixedUpdate` system destroys determinism and makes your game speed depend on the monitor.
>
> Motion still looks smooth at 144 Hz because the renderer **interpolates sprite positions
> between the last two ticks** (`time.alpha`). You get fixed-step logic and smooth rendering
> without doing anything.
>
> Keeping the simulation on that fixed tick, with all randomness flowing through `SimRng`, is
> what buys you headless tests, replays, and eventually lockstep networking. See
> [Chapter 3](ch03-simulation.md) and [Chapter 10](ch10-determinism.md).

`input.pressed(Key)` is held-this-tick; `just_pressed` / `just_released` are the edges. Mouse
position is available in both screen and world space.

## Step 4 — walls

The square happily wanders off screen. Clamp it:

```rust,ignore
/// Playfield size in world units — same as the window, so the edges line up.
const ARENA: Vec2 = Vec2::new(800.0, 600.0);
```

```rust,ignore
    // The sprite is centered on its translation, so the reachable area shrinks by half a
    // square on every side.
    let limit = (ARENA - Vec2::splat(PLAYER_SIZE)) / 2.0;
    for mut transform in &mut players {
        let step = dir.normalize_or_zero() * PLAYER_SPEED * time.fixed_delta;
        transform.translation = (transform.translation + step).clamp(-limit, limit);
    }
```

Done — a square that slides along the walls instead of escaping. `Vec2` is
[`glam::Vec2`](https://docs.rs/glam), re-exported, so `clamp`, `length`, `normalize_or_zero`,
`extend`, and friends are all there.

> **Fulcrum-specific — why a constant and not the window size?** `WindowInfo` exists and holds
> the live window dimensions, but reading it from a `FixedUpdate` system makes your simulation
> depend on renderer state: the game would play differently when resized, and it wouldn't run
> at all headless (where there is no window). The convention is that the simulation owns its
> own playfield constant and the window is configured to match. That's the split the whole
> engine is built around, and it's what lets a Fulcrum game be tested without a GPU.

## The whole thing

```rust,ignore
use fulcrum::prelude::*;

const ARENA: Vec2 = Vec2::new(800.0, 600.0);
const PLAYER_SIZE: f32 = 40.0;
const PLAYER_SPEED: f32 = 300.0;

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let white = assets.load("white.png");
    commands.spawn((
        Sprite::new(white)
            .with_color(Color::rgb(0.9, 0.7, 0.3))
            .with_size(Vec2::splat(PLAYER_SIZE)),
        Transform2D::from_xy(0.0, 0.0),
        Player,
    ));
}

fn move_player(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::Left) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::Right) {
        dir.x += 1.0;
    }
    if input.pressed(Key::Down) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::Up) {
        dir.y += 1.0;
    }

    let limit = (ARENA - Vec2::splat(PLAYER_SIZE)) / 2.0;
    for mut transform in &mut players {
        let step = dir.normalize_or_zero() * PLAYER_SPEED * time.fixed_delta;
        transform.translation = (transform.translation + step).clamp(-limit, limit);
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Fulcrum Quickstart".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: Color::rgb(0.08, 0.09, 0.12),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(move_player)
    .run();
}
```

## Things to try from here

Each of these is a few lines on top of what you have:

- **Press `F12`** (debug builds) for the inspector: live entity list, editable component
  values, frame/tick stats, loaded assets. It comes free with `DefaultPlugins`.
- **Spawn a second square** with a different color and no `Player` marker, then write a system
  that pushes it away from the player — you'll have written collision.
- **Move the camera:** `Res<Camera2D>` → `ResMut`, set `center` to follow the player, or bump
  `zoom`.
- **Add text:** `commands.spawn((Text::new("hi"), Transform2D::from_xy(0.0, 250.0)))`.
- **Make the square shrink over time** in an `add_frame_system` using `time.frame_delta`, to
  feel the difference between the two schedules.

## Where to go next

- The [Cheat Sheet](appendix-cheatsheet.md) is the whole API on one page — components,
  resources, system parameters, data file formats. Bookmark it.
- [Building Grove](ch01-window.md) is the main tutorial: the same ground at a proper pace,
  then tilemaps, animation, events, audio, prefabs and scenes, UI, and testing.
- [Power Features](ch11-particles.md) covers particles, spatial queries and pathfinding, Lua
  modding, and replays.
- [Engine Architecture](appendix-architecture.md) if you'd rather read how it works before
  using it.
- `games/_template/` in the repo is this program plus a determinism test and the standalone
  manifest, ready to `cp -r` out and rename.
