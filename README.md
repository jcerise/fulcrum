# Fulcrum

Fulcrum is an opinionated 2D game engine for Rust: the hard parts abstracted away, a small
prelude-driven API on top, and strong defaults so you spend your time making the game. It targets
everything from arcade games to RTS and simulation titles — sprites only, no 3D — with ECS as a
first-class pattern, data-driven content (animations, prefabs, UI) throughout, and modding as a
first-class feature.

## Locked architectural decisions

- **ECS:** [`bevy_ecs`](https://crates.io/crates/bevy_ecs), fully wrapped behind Fulcrum's own
  prelude — games never import `bevy_ecs` directly.
- **Platform/rendering:** `winit` + `wgpu`, sprite-batch renderer, ECS-component-driven drawing
  (immediate mode exists only for debug gizmos).
- **Determinism:** same-binary determinism is a core promise — fixed-timestep simulation with
  render interpolation, tick-sampled input, seeded RNG. Same build + same inputs = same result
  (enables replays and same-platform lockstep). See `docs/determinism.md`.
- **Audio:** `kira`. **Modding:** layered asset VFS + Lua (`mlua`).
- **Conventions:** 1 world unit = 1 pixel, +Y up; simulation mutates state only in `FixedUpdate`.

## Workspace layout

- `crates/fulcrum` — facade crate; `use fulcrum::prelude::*;` is the whole API.
- `crates/fulcrum-core` — app builder, plugins, schedules, time, input, RNG.
- `crates/fulcrum-render` — window, wgpu backend, sprite batching.
- `crates/fulcrum-asset` — asset handles, storage, loaders.
- `games/` — milestone games that dogfood the engine (`pong`, then asteroids, dungeon, RTS slice).

## Learning Fulcrum

**Start with [The Fulcrum Book](book/src/SUMMARY.md)** — it teaches the whole engine by
building a small game (`games/grove`) from an empty window up. Build the rendered version
with `mdbook build book` (output in `book/built/`). Every chapter's complete program compiles
in CI as an example: `cargo run -p grove --example ch01_window`.

## Status

Pre-alpha. Phases 1-3 complete (core, 2D essentials, data-driven layer); phase 4 (modding,
particles, pathfinding, replays) is planned. Build plans live in `plans/`; the milestone
games in `games/` double as integration tests.
