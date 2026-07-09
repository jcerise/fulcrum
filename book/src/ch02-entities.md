# Entities, Components, and Sprites

Fulcrum is an **ECS** engine. The mental model fits in three sentences:

- An **entity** is a thing in your game — an ID, nothing more.
- A **component** is a fact attached to an entity: its position, its looks, its health.
- A **system** is a function that runs over every entity with a particular set of facts.

There are no game-object classes and no inheritance. A "player" is just an entity that
happens to have a position, a sprite, and whatever facts make it player-ish. This sounds
austere; in practice it's liberating — any system that cares about "things with positions and
velocities" works on players, gems, and foxes alike without knowing what they are.

## Your first entity

```rust,ignore
use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let gem = assets.load("gem.png");
    commands.spawn((
        Sprite::new(gem).with_size(vec2(64.0, 64.0)),
        Transform2D::from_xy(0.0, 0.0),
    ));
}

fn main() {
    Fulcrum::with_config(/* as chapter 1 */)
        .insert_resource(AssetServer::new("assets"))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .run();
}
```

`cargo run -p grove --example ch02_sprite` — a gem, centered.

Walk through it:

- **`add_startup(setup)`** registers a system that runs once, before the first tick. Systems
  are plain functions; their parameters declare what they need, and the engine provides it.
- **`Commands`** queues world mutations — here, spawning one entity with two components.
- **`AssetLoader`** loads textures with one line. Paths are relative to the asset root
  (`AssetServer::new(...)` sets it; games usually point it at their own `assets/` directory).
  Loading is synchronous and deduplicated: load the same path twice, get the same cheap
  `Handle<Texture>` back. A missing file logs an error and renders as a magenta placeholder —
  asset problems never crash a game.
- **`Transform2D`** is where the entity is: translation, rotation, scale.
- **`Sprite`** is what it looks like: a texture (or a sheet region, later), a tint, an
  optional size override, flips, and a `z` draw order.

Spawn an entity with both and the engine draws it. Batched, sorted by `z`, interpolated —
none of which you had to ask for.

## Conventions worth memorizing

Fulcrum fixes its coordinate conventions so you never negotiate them again: **1 world unit =
1 pixel** (at camera zoom 1), **+Y is up**, the origin starts at the window center, and
rotation is radians, counter-clockwise. Sprites are `Nearest`-filtered by default — this is a
pixel-art-friendly engine.

## The `bevy_ecs` manifest quirk

Fulcrum's ECS is [`bevy_ecs`], wrapped completely — you never import it. But Rust derive
macros generate code that names their home crate, so any crate that writes
`#[derive(Component)]` needs `bevy_ecs` in its **`Cargo.toml`** (never in source). You'll hit
this in the next chapter when we define our first component; it's one manifest line.

[`bevy_ecs`]: https://crates.io/crates/bevy_ecs
