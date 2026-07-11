# Introduction

Fulcrum is an opinionated 2D game engine for Rust. It exists because making a small game
should feel small: open a window in six lines, put a sprite on screen in six more, and grow
from there without the engine ever asking you to care about render graphs, thread pools, or
lifetimes it could have handled for you.

Three opinions shape everything in it:

1. **ECS is the pattern.** Your game is entities (things), components (facts about things),
   and systems (functions over those facts). There is no scene-graph alternative, no "manager"
   classes — one idiomatic way.

2. **The simulation is deterministic.** Game logic runs on a fixed 60 Hz tick, input is
   sampled at tick boundaries, and randomness flows through a seeded generator. Same build +
   same inputs = bit-identical results, every run. That one promise buys you honest headless
   tests, replays, and — someday — lockstep multiplayer, and it costs you almost nothing if
   you follow a few rules you'll learn in [Chapter 3](ch03-simulation.md).

3. **Content is data.** Animations, entity definitions, levels, and UI live in files, not
   code — and every one of them hot-reloads while your game runs. By
   [Chapter 7](ch07-data-driven.md), adding a new monster to your game won't require touching
   Rust at all. (This is also the foundation for first-class modding.)

## How this book works

**Never built a game before?** This book has an on-ramp written just for you: [From Zero](
fz01-what-is-a-game.md) builds Snake in six deliberately slow chapters, assuming solid
programming experience but no game development at all — every concept motivated from first
principles, the *why* before the *how*. Start there, then come back here; Grove will read at
twice the speed for it.

For everyone else: we build one game, from nothing: **Grove** — a little top-down game about collecting gems in a
hedge garden while a fox hunts you. Every chapter adds one engine concept and one piece of the
game; Part I (chapters 1–10) builds Grove, and Part II (chapters 11–14) layers the power
features on top — particles, spatial queries and pathfinding, mods, and replays. All the code
is real: each chapter's complete program lives in the repository (`games/grove/examples/`,
and `games/snake/` for the From Zero track) and compiles in CI, so nothing on these pages
can silently rot.

```text
cargo run -p grove --example ch01_window   # each chapter, runnable
cargo run -p grove                         # the finished game
```

## Setup

Fulcrum is a Cargo workspace dependency (crates.io publication is planned once the API
settles). Games depend on the single facade crate and import one prelude:

```toml
[dependencies]
fulcrum = { path = "path/to/fulcrum/crates/fulcrum" }
# One quirk, explained in chapter 2:
bevy_ecs = { version = "0.19", default-features = true }
```

```rust,ignore
use fulcrum::prelude::*;
```

That line is the entire API surface. Let's open a window.
