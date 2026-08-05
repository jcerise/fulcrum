# Fulcrum

Fulcrum is an opinionated 2D game engine for Rust: the hard parts abstracted away, a small
prelude-driven API on top, and strong defaults so you spend your time making the game. It targets
everything from arcade games to RTS and simulation titles — sprites only, no 3D — with ECS as a
first-class pattern, data-driven content (animations, prefabs, scenes, UI) throughout, and
modding as a first-class feature.

## Locked architectural decisions

- **ECS:** [`bevy_ecs`](https://crates.io/crates/bevy_ecs), fully wrapped behind Fulcrum's own
  prelude — games never import `bevy_ecs` directly (it appears in game manifests only so
  `#[derive(Component)]` resolves).
- **Platform/rendering:** `winit` + `wgpu`, sprite-batch renderer, ECS-component-driven drawing
  (immediate mode exists only for debug gizmos).
- **Determinism:** same-binary determinism is a core promise — fixed-timestep simulation with
  render interpolation, tick-sampled input, seeded RNG. Same build + same inputs = same result,
  and replays prove it: input recordings play back bit-identically, with state hashing to catch
  divergence, enforced by a CI gate. See `docs/determinism.md`.
- **Audio:** `kira`, cosmetic by contract (playback never feeds back into the simulation).
- **Modding:** layered asset VFS + sandboxed, deterministic Lua (`mlua`) that runs inside the
  fixed tick under the same rules as Rust systems.
- **Conventions:** 1 world unit = 1 pixel, +Y up; simulation mutates state only in `FixedUpdate`.

## What the engine does today

- **Rendering** — sprite batching, sprite sheets/atlases, `Camera2D` with pixel-art-friendly
  scaling modes, text (fontdue), tilemaps, a data-driven particle system, debug gizmos.
- **Animation** — frame clips with tick-based durations (animation is simulation state, so
  gameplay can key off frames deterministically), Aseprite import, and data-driven animation
  state machines.
- **Assets** — typed handles, synchronous loading through one seam, hot reload with debounced
  file watching, and the layered VFS that mods mount onto.
- **Data-driven structure** — a component registry (string names ↔ typed components), prefabs
  and scenes in RON, world save/load, and state hashing.
- **Game UI** — retained-mode node tree with anchors/pivots/stacking, panel/label/button/image
  widgets, RON layout files with hot reload, and pointer interaction surfaced as replayable
  events the simulation reads. In debug builds, F12 opens an egui world inspector.
- **Audio** — WAV/OGG/MP3/FLAC one-shots with volume/pitch/pan; runs silently on machines with
  no audio device instead of failing.
- **Modding** — mod manifests, the layered VFS for asset overrides, and a sandboxed
  deterministic Lua runtime with Lua↔ECS bindings (sandboxed by capability omission — an
  anti-footgun, not a hard security boundary).
- **Spatial & pathfinding** — a deterministic uniform-grid index for crowd-scale queries, grid
  A*, path simplification, and flow fields.
- **Replays** — input recording to compact replay files, bit-identical playback, and the
  headless determinism test harness every milestone game uses.

## Workspace layout

- `crates/fulcrum` — facade crate; `use fulcrum::prelude::*;` is the whole API.
- `crates/fulcrum-core` — app builder, plugins, schedules, time, input, RNG, transforms,
  hierarchy, replay.
- `crates/fulcrum-render` — window, wgpu backend, sprite batching, camera, text, tilemaps,
  particles, gizmos.
- `crates/fulcrum-asset` — asset handles, storage, loaders, hot reload, layered VFS.
- `crates/fulcrum-anim` — animation clips, Aseprite import, state machines.
- `crates/fulcrum-audio` — sounds and playback via kira.
- `crates/fulcrum-scene` — component registry, prefabs, scenes, world save/load, state hashing.
- `crates/fulcrum-ui` — retained-mode game UI and the egui debug overlay.
- `crates/fulcrum-mod` — mod manifests, Lua runtime, Lua↔ECS bindings.
- `crates/fulcrum-spatial` — spatial grid, A*, flow fields.
- `games/` — the games that dogfood the engine, doubling as integration tests:
  - Milestones: `pong`, `asteroids`, `dungeon`, `rts-slice` (one per engine phase).
  - Tutorial games: `snake` (From Zero track), `grove` (The Fulcrum Book), `dojo`
    (the Animation Book), `bazaar` (the UI Book).
  - `_template` — a standalone scaffold for starting your own game (see below).
- `book/`, `books/` — The Fulcrum Book and the deep-dive books.
- `docs/` — design documents (`determinism.md`).
- `plans/` — the phase build plans (all four complete).
- `tools/` — asset-generation scripts.

## Learning Fulcrum

**Start with [The Fulcrum Book](book/src/SUMMARY.md)** — it teaches the whole engine by
building a small game (`games/grove`) from an empty window up. Build the rendered version
with `mdbook build book` (output in `book/built/`). Every chapter's complete program compiles
in CI as an example: `cargo run -p grove --example ch01_window`. New to games entirely? The
book's **From Zero** track builds Snake from first principles, code-along style.

**Deep dives** (`books/`) take one system each and cover it completely, tutorial first:

- [Animation](books/animation/src/SUMMARY.md) — clips, the Aseprite pipeline, state
  machines, and frame-keyed gameplay, building `games/dojo` along the way.
- [UI](books/ui/src/SUMMARY.md) — layout files, widgets, the click path across the sim
  boundary, nine-slice skins and themes, building `games/bazaar` along the way.

## Starting your own game

Copy [`games/_template`](games/_template/README.md) out of the repo — it is a standalone
package (excluded from this workspace) carrying everything an external game needs: the
`fulcrum` dependency wiring, the toolchain pin, the debug-build profile block, the
simulation/visuals split, and a headless determinism test. Its README walks through the
copy-out steps, including using Fulcrum as a git dependency (no crates.io publish needed).

## Status

Pre-alpha. All four build phases are complete — core, 2D essentials, the data-driven layer,
and power features (modding, particles, spatial/pathfinding, replays) — each proven by its
milestone game. The API is unstable and the crates are not yet published; depend on the repo
directly via path or git (see `games/_template`).
