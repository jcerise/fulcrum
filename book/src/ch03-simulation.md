# The Simulation: Input and Time

Let's make the gem a hero and walk them around:

```rust,ignore
#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let hero = assets.load("hero.png");
    commands.spawn((
        Sprite::new(hero).with_size(vec2(48.0, 48.0)),
        Transform2D::from_xy(0.0, 0.0),
        Player,
    ));
}

fn movement(mut players: Query<&mut Transform2D, With<Player>>, input: Res<Input>, time: Res<Time>) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) { dir.x -= 1.0 }
    if input.pressed(Key::D) { dir.x += 1.0 }
    if input.pressed(Key::S) { dir.y -= 1.0 }
    if input.pressed(Key::W) { dir.y += 1.0 }
    for mut transform in &mut players {
        transform.translation += dir.normalize_or_zero() * 150.0 * time.fixed_delta;
    }
}

// in main():
//     .add_startup(setup)
//     .add_system(movement)
```

`cargo run -p grove --example ch03_movement`. Three new ideas, and one big one.

## Queries and marker components

`Player` is a **marker**: a component with no data whose only job is to make entities
findable. `Query<&mut Transform2D, With<Player>>` reads as "every entity that has a
`Transform2D` and is a `Player`, give me mutable access to its transform." Queries are how
systems see the world; filters like `With`/`Without` narrow them without fetching data.

(This is where the manifest quirk from chapter 2 bites: `#[derive(Component)]` needs
`bevy_ecs = { workspace = true }` in your game's `Cargo.toml`.)

## Resources

Not everything is an entity. Global, singular state — the input, the clock, your score —
lives in **resources**, accessed with `Res<T>` (shared) or `ResMut<T>` (exclusive).
`insert_resource` adds your own.

## The big one: two schedules

Notice the method was `add_system`, and that we multiplied by `time.fixed_delta` rather than
a frame delta. Fulcrum runs your code in two distinct schedules:

- **`FixedUpdate`** — *the simulation*. Added with `add_system`. Runs at exactly `tick_rate`
  ticks per second (default 60), no matter the frame rate. All game state changes belong
  here. The only clocks you may use are `time.fixed_delta` (constant) and `time.tick` (a
  counter).

- **`Update`** — *presentation*. Added with `add_frame_system`. Runs once per rendered frame,
  uses `time.frame_delta`, and must never touch simulation state. Camera glide, visual
  flourishes, UI text — cosmetic work.

Why so strict? Because a fixed-step simulation whose inputs are sampled once per tick is
**deterministic**: same build, same seed, same inputs — bit-identical results, every time. You
get slow-motion-proof physics, headless tests that drive your real game (chapter 10), and
recorded replays, all from one discipline. The renderer hides the seams by interpolating
entity transforms between ticks, so 60 Hz simulation still looks butter-smooth on a 144 Hz
monitor.

`Input` is part of the same contract: the engine buffers raw OS events and delivers them to
the simulation once per tick — `pressed` (held), `just_pressed` / `just_released` (edges,
never lost even for taps faster than a tick). Keys are physical positions, so WASD survives
any keyboard layout. `input.mouse_world()` gives you the cursor in world coordinates, already
mapped through the camera.

One more habit to build now: when two simulation systems touch the same data, order them
explicitly — `add_system((movement, collide).chain())` runs them left to right. You'll see
this in every game in the repository.
