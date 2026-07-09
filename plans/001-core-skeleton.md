---
id: core-skeleton
title: "Phase 1: Core skeleton — window, ECS, renderer, fixed timestep (milestone: Pong)"
status: complete
priority: 1
created: 2026-07-08
steps_completed: 10
steps_total: 10
tags: [engine, core, rendering, ecs, determinism]
---

# Phase 1: Core skeleton (milestone: Pong)

## Summary

Bootstrap the Fulcrum workspace and build the minimal engine core: a windowed app driven by a deterministic fixed-timestep loop, a bevy_ecs wrapper hidden behind Fulcrum's own API, a wgpu sprite-batch renderer with render interpolation, keyboard/mouse input, synchronous asset loading, and a seeded RNG. The milestone is a playable Pong living in `games/pong` that uses only the public `fulcrum` prelude.

## Context

- The repo is **empty**. This plan creates the entire workspace structure.
- Locked architectural decisions (do not revisit):
  - **bevy_ecs** as the ECS, wrapped behind Fulcrum's own types/prelude. Users never write `use bevy_ecs::...`.
  - **wgpu + winit** for platform/rendering.
  - **Same-binary determinism** is a core promise: fixed timestep, input sampled at tick boundaries, seeded RNG resource, no wall-clock or iteration-order nondeterminism inside the simulation. Cross-platform bit-exactness is NOT promised; f32 math is fine.
  - **ECS-component-driven rendering** is the only blessed drawing path for game content (spawn entities with `Sprite` + `Transform2D`). Immediate-mode drawing exists later only as debug gizmos (phase 2).
- Full workspace crate map (later phases fill it in; create only the phase-1 crates now, but keep names consistent):
  - `crates/fulcrum` — facade, re-exports everything as `fulcrum::prelude`
  - `crates/fulcrum-core` — app builder, plugins, schedules, time, input, RNG (later: replay)
  - `crates/fulcrum-render` — wgpu backend, sprite batch (later: camera, text, tilemap, gizmos, particles)
  - `crates/fulcrum-asset` — assets, handles, loaders (later: VFS, hot reload)
  - Later phases: `fulcrum-audio`, `fulcrum-anim`, `fulcrum-scene`, `fulcrum-ui`, `fulcrum-mod`, `fulcrum-spatial`
- Pin dependency versions in the workspace `[workspace.dependencies]`. Use the latest stable of each at implementation time (bevy_ecs 0.16.x-era APIs are assumed below — adapt trait names like `IntoScheduleConfigs` to whatever the pinned version uses, keeping Fulcrum's public signatures stable).
- Key deps: `bevy_ecs`, `wgpu`, `winit`, `glam` (math, re-exported), `image` (PNG decode), `rand_pcg` + `rand_core` (RNG), `bytemuck`, `rustc-hash`, `thiserror`, `log` + `env_logger`.

## Steps

### Step 1: Scaffold the cargo workspace and CI

**Files:** `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `README.md`, `crates/fulcrum/Cargo.toml`, `crates/fulcrum/src/lib.rs`, `crates/fulcrum-core/Cargo.toml`, `crates/fulcrum-core/src/lib.rs`, `crates/fulcrum-render/Cargo.toml`, `crates/fulcrum-render/src/lib.rs`, `crates/fulcrum-asset/Cargo.toml`, `crates/fulcrum-asset/src/lib.rs`, `games/pong/Cargo.toml`, `games/pong/src/main.rs`, `.github/workflows/ci.yml`

**Requires review:** false

- `git init` the repo first (it is not yet a git repository).
- Root `Cargo.toml`: virtual workspace with `members = ["crates/*", "games/*"]`, `resolver = "2"`. Define all shared versions under `[workspace.dependencies]` (bevy_ecs, wgpu, winit, glam, image, bytemuck, rand_pcg, rand_core, rustc-hash, thiserror, log, env_logger). Set `[profile.dev.package."*"] opt-level = 2` so wgpu/image are usable in debug builds.
- Each crate starts as a compiling stub (`lib.rs` with a doc comment); `games/pong` is a binary printing "fulcrum pong" for now.
- `rust-toolchain.toml`: pin current stable channel.
- CI workflow: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo build --workspace`. Linux runner is sufficient; install `libwayland-dev libxkbcommon-dev` etc. for winit.
- README: one-paragraph engine pitch + the locked decisions list.

**Acceptance criteria:**
- [ ] `cargo build --workspace` succeeds on a clean checkout
- [ ] `cargo run -p pong` prints the stub line
- [ ] CI workflow file runs fmt/clippy/test/build

---

### Step 2: App builder, Plugin trait, and schedules in fulcrum-core

**Files:** `crates/fulcrum-core/src/lib.rs`, `crates/fulcrum-core/src/app.rs`, `crates/fulcrum-core/src/plugin.rs`, `crates/fulcrum-core/src/schedule.rs`

**Requires review:** true

This is the engine's front door; the API shape set here propagates everywhere.

- Wrap `bevy_ecs::world::World` + a set of named schedules inside a `Fulcrum` struct (working name `App` internally; the facade re-exports it as `Fulcrum`).
- Schedules (as `ScheduleLabel` newtypes): `Startup` (once), `FixedUpdate` (the deterministic simulation tick — the default target of `add_system`), `Update` (once per rendered frame, cosmetic/non-sim work), plus internal `PreRender` (extraction).
- Public API:

```rust
pub struct Fulcrum { /* world, schedules, config, runner */ }

pub struct FulcrumConfig {
    pub title: String,
    pub window_size: (u32, u32),   // default (1280, 720)
    pub tick_rate: u32,            // default 60 (Hz)
    pub seed: u64,                 // default 0xF0CU5 constant; SimRng seed
    pub clear_color: Color,
}

impl Fulcrum {
    pub fn new(title: impl Into<String>) -> Self;
    pub fn with_config(config: FulcrumConfig) -> Self;
    pub fn with_plugin(self, plugin: impl Plugin) -> Self;
    pub fn add_startup<M>(self, systems: impl IntoScheduleConfigs<M>) -> Self;
    pub fn add_system<M>(self, systems: impl IntoScheduleConfigs<M>) -> Self;        // -> FixedUpdate
    pub fn add_frame_system<M>(self, systems: impl IntoScheduleConfigs<M>) -> Self;  // -> Update
    pub fn insert_resource<R: Resource>(self, r: R) -> Self;
    pub fn run(self);  // hands off to the runner installed by the render/window plugin
}

pub trait Plugin {
    fn build(&self, app: &mut Fulcrum);
}
```

- Re-export the bevy_ecs vocabulary users need (`Commands`, `Query`, `Res`, `ResMut`, `Component`, `Resource`, `Entity`, `With`, `Without`, `EventReader`, `EventWriter`, derive macros) from `fulcrum_core` so downstream crates and games depend only on Fulcrum crates.
- Re-export `glam` types (`Vec2`, `vec2`, …) and define `Color` (rgba f32, `Color::rgb`, `Color::WHITE` etc.) here.
- `run()` uses a swappable runner function (`fn(Fulcrum)`) so fulcrum-core stays windowless/testable; the window plugin (step 3) installs the real winit runner. Default runner executes Startup then N ticks for headless tests.

**Acceptance criteria:**
- [ ] A headless unit test builds a `Fulcrum`, adds a startup + fixed system, runs 5 ticks, and observes component changes via the world
- [ ] No `bevy_ecs` import is needed by the test — only `fulcrum_core` paths
- [ ] Plugins can add systems and resources in `build`

---

### Step 3: winit window and deterministic fixed-timestep loop

**Files:** `crates/fulcrum-core/src/time.rs`, `crates/fulcrum-render/src/window.rs`, `crates/fulcrum-render/src/lib.rs`

**Requires review:** false

- `Time` resource in fulcrum-core:

```rust
pub struct Time {
    pub fixed_delta: f32,     // 1.0 / tick_rate — the ONLY dt sim systems may use
    pub tick: u64,            // fixed ticks elapsed
    pub frame_delta: f32,     // wall dt for Update/cosmetic systems
    pub alpha: f32,           // interpolation factor for rendering, [0,1)
}
```

- `WindowPlugin` (in fulcrum-render, since it owns the event loop + surface lifecycle) installs a winit `ApplicationHandler` runner implementing the canonical accumulator loop:
  - each `about_to_wait`/redraw: `accumulator += frame_dt.min(0.25)`; while `accumulator >= fixed_delta` { snapshot previous transforms (step 6), sample input (step 7), run `FixedUpdate`, `tick += 1` }; set `alpha = accumulator / fixed_delta`; run `Update`, then `PreRender` + render.
  - The 0.25 s clamp prevents spiral-of-death; document that long stalls slow the sim rather than exploding it.
- Window close → clean exit. Resize events update a `WindowInfo` resource `{ width, height, scale_factor }`.
- Determinism rule established here and documented in `fulcrum-core/src/lib.rs` docs: **`FixedUpdate` systems must not read wall-clock time, `frame_delta`, or `alpha`** — only `fixed_delta` and `tick`.

**Acceptance criteria:**
- [ ] `cargo run -p pong` (still a stub calling `Fulcrum::new(...).run()`) opens a window that closes cleanly
- [ ] With a test system counting ticks, tick count after ~1s of wall time ≈ tick_rate (manual check)
- [ ] Headless runner still works for tests (no winit dependency in fulcrum-core)

---

### Step 4: wgpu bootstrap — surface, device, clear, present

**Files:** `crates/fulcrum-render/src/gpu.rs`, `crates/fulcrum-render/src/lib.rs`

**Requires review:** false

- `GpuContext { instance, surface, adapter, device, queue, surface_config }` created inside `WindowPlugin` once the window exists; stored as a (non-send if required by platform) resource.
- Per frame: acquire surface texture, begin a render pass clearing to `FulcrumConfig::clear_color`, submit, present. Handle `SurfaceError::Lost/Outdated` by reconfiguring, `OutOfMemory` by exiting with an error log.
- Resize reconfigures the surface.
- Choose `PresentMode::AutoVsync` default.

**Acceptance criteria:**
- [ ] Window shows the configured clear color at any size, resizes without panicking
- [ ] No validation errors with `RUST_LOG=wgpu=warn`

---

### Step 5: Asset system — handles, storage, texture loading

**Files:** `crates/fulcrum-asset/src/lib.rs`, `crates/fulcrum-asset/src/handle.rs`, `crates/fulcrum-asset/src/assets.rs`, `crates/fulcrum-render/src/texture.rs`

**Requires review:** false

- Phase 1 keeps loading **synchronous** (load-on-call from disk); async/streaming is out of scope for the whole project until proven needed. Hot reload comes in phase 3; the layered VFS in phase 4 — design the loader entry point as a single `fn read_bytes(path: &AssetPath) -> Result<Vec<u8>>` so those can slot in behind it.
- Types:

```rust
pub struct Handle<T> { id: u32, _marker: PhantomData<fn() -> T> } // Copy, Eq, Hash
pub struct Assets<T> { /* slab of T, path -> id map for dedup */ }
impl<T> Assets<T> {
    pub fn get(&self, h: Handle<T>) -> Option<&T>;
    pub fn insert(&mut self, value: T) -> Handle<T>;
}
pub struct AssetServer { root: PathBuf /* default "assets/" next to the executable/CWD */ }
```

- `AssetServer::load_texture(&mut Assets<Texture>, &GpuContext, path) -> Handle<Texture>`: decode with `image`, upload as `wgpu::Texture` (RGBA8, `Nearest` filtering default — this is a pixel-art-friendly engine), keep `{texture, view, size}` in `Texture`. Dedup by path.
- Wire a convenience so games write `assets.load("ship.png")` — an `Assets` system-param-style wrapper resource combining server + texture storage + gpu access is acceptable; keep the public call to one line.
- Missing file: log error and return a built-in 2×2 magenta placeholder texture handle (never panic in asset loading).

**Acceptance criteria:**
- [ ] Loading the same path twice returns the same handle
- [ ] Missing file yields placeholder + error log, no panic
- [ ] A test loads a PNG fixture and verifies dimensions

---

### Step 6: Sprite component, Transform2D, batch renderer with interpolation

**Files:** `crates/fulcrum-render/src/sprite.rs`, `crates/fulcrum-render/src/batch.rs`, `crates/fulcrum-render/src/shader.wgsl`, `crates/fulcrum-core/src/transform.rs`

**Requires review:** true

The core drawing path — API here is what every game touches.

- Components (Transform2D lives in fulcrum-core; Sprite in fulcrum-render):

```rust
#[derive(Component, Clone, Copy)]
pub struct Transform2D { pub translation: Vec2, pub rotation: f32, pub scale: Vec2 }
impl Transform2D { pub fn from_xy(x: f32, y: f32) -> Self; } // rotation 0, scale ONE

#[derive(Component, Clone, Copy)] // engine-managed; auto-inserted, users don't touch it
pub struct PreviousTransform2D(pub Transform2D);

#[derive(Component, Clone)]
pub struct Sprite {
    pub texture: Handle<Texture>,
    pub color: Color,                 // tint, default WHITE
    pub custom_size: Option<Vec2>,    // None = texture size in pixels
    pub anchor: Vec2,                 // (0.5, 0.5) = centered, default
    pub flip_x: bool, pub flip_y: bool,
    pub z: f32,                       // draw order, higher = in front
}
impl Sprite { pub fn new(texture: Handle<Texture>) -> Self; }
```

- Interpolation: at the start of every fixed tick (installed by the runner in step 3, before `FixedUpdate`), copy `Transform2D` → `PreviousTransform2D` for all sprite entities (auto-insert `PreviousTransform2D` for new entities). At render, draw at `prev.lerp(current, alpha)` (lerp translation and scale; shortest-arc lerp rotation).
- Batcher: collect all `(Sprite, Transform2D, PreviousTransform2D)` in `PreRender`, sort by `(z, texture_id)`, build one vertex buffer of quads, issue one draw call per contiguous texture run. Single WGSL shader: textured quad × tint color. Orthographic projection: world units = pixels, origin at window center, +Y up, sized from `WindowInfo` (a proper `Camera2D` arrives in phase 2 — bake the projection math into one function so the camera can replace it).
- World coordinates doc comment: 1 world unit = 1 pixel at default zoom, +Y up. This is a locked convention.

**Acceptance criteria:**
- [ ] An example scene with 3 overlapping sprites renders with correct z-order, tint, flip, and anchor
- [ ] 10,000 static sprites (2 textures) render at 60 fps in release with ≤ 2 draw calls per texture-contiguous run (log batch count)
- [ ] A sprite moved only in `FixedUpdate` at 10 Hz tick rate visibly interpolates smoothly at monitor refresh

---

### Step 7: Input — keyboard and mouse, sampled at tick boundaries

**Files:** `crates/fulcrum-core/src/input.rs`, `crates/fulcrum-render/src/window.rs` (event feeding)

**Requires review:** false

- Determinism contract: winit events accumulate into a **pending buffer**; the runner drains it into the `Input` resource exactly once per fixed tick, immediately before `FixedUpdate`. Sim systems therefore see identical input for a whole tick, and a recorded per-tick input stream (phase 4 replay) reproduces the run.
- API:

```rust
pub struct Input {
    // keyboard
    pub fn pressed(&self, key: Key) -> bool;
    pub fn just_pressed(&self, key: Key) -> bool;
    pub fn just_released(&self, key: Key) -> bool;
    // mouse
    pub fn mouse_pressed(&self, b: MouseButton) -> bool;
    pub fn mouse_just_pressed(&self, b: MouseButton) -> bool;
    pub fn mouse_screen(&self) -> Vec2;   // pixels, top-left origin
    pub fn mouse_world(&self) -> Vec2;    // via current projection
    pub fn scroll_delta(&self) -> f32;
}
pub enum Key { A..Z, Digit0..9, Arrow*, Space, Enter, Escape, Shift, Ctrl, Alt, Tab, /* physical-key based */ }
```

- `Key` maps from winit **physical** key codes (layout-independent — right for games; document this).
- `just_*` semantics are per-tick (cleared when the next tick samples).

**Acceptance criteria:**
- [ ] Holding a key reports `pressed` every tick; `just_pressed` exactly one tick
- [ ] Events arriving between ticks are not lost (press+release within one frame still yields one `just_pressed` and one `just_released` on subsequent ticks)
- [ ] `mouse_world` matches a sprite drawn at that world position (manual check)

---

### Step 8: Deterministic RNG resource and determinism ground rules

**Files:** `crates/fulcrum-core/src/rng.rs`, `docs/determinism.md`

**Requires review:** false

- `SimRng` resource wrapping `rand_pcg::Pcg32`, seeded from `FulcrumConfig::seed`:

```rust
pub struct SimRng(Pcg32);
impl SimRng {
    pub fn u32(&mut self) -> u32;
    pub fn range_f32(&mut self, r: Range<f32>) -> f32;
    pub fn range_i32(&mut self, r: Range<i32>) -> i32;
    pub fn chance(&mut self, p: f32) -> bool;
    pub fn fork(&mut self) -> SimRng;  // child stream for subsystems
}
```

- `docs/determinism.md` — the written contract, kept current from now on:
  1. Sim state changes only in `FixedUpdate`, using `fixed_delta`/`tick`, never wall time.
  2. All sim randomness through `SimRng` (never `rand::thread_rng`, never hashing addresses).
  3. No iteration over `std::collections::HashMap`/`HashSet` in sim systems — use re-exported `FxHashMap` (deterministic order for identical insertion sequences) or sort before iterating. bevy_ecs query iteration order is stable for identical spawn/despawn sequences — which same-binary + same-input guarantees.
  4. Input reaches the sim only via the tick-sampled `Input` resource.
  5. Rendering, audio, and cosmetic `Update` systems may be nondeterministic freely.
- Re-export `rustc_hash::{FxHashMap, FxHashSet}` from the prelude as the blessed map types.

**Acceptance criteria:**
- [ ] Test: two `Fulcrum` instances with the same seed running 1,000 headless ticks of a system doing RNG-driven spawns/moves end with identical component state (hash positions)
- [ ] `docs/determinism.md` exists and states the five rules

---

### Step 9: Facade crate and prelude

**Files:** `crates/fulcrum/src/lib.rs`, `crates/fulcrum/src/prelude.rs`, `crates/fulcrum/examples/hello_sprite.rs`

**Requires review:** false

- `fulcrum` depends on all phase-1 crates and defines `DefaultPlugins` (window+render, asset, input — everything needed to get a game on screen).
- `fulcrum::prelude` re-exports: `Fulcrum`, `FulcrumConfig`, `DefaultPlugins`, `Plugin`, ECS vocabulary, `Transform2D`, `Sprite`, `Color`, `Handle`, `Assets`, `Texture`, `Time`, `Input`, `Key`, `MouseButton`, `SimRng`, `Vec2`/`vec2`, `FxHashMap`/`FxHashSet`.
- `hello_sprite` example: ≤ 30 lines, spawns one sprite that moves with arrow keys — the "first page of the docs" program. Every public item it touches must come from `use fulcrum::prelude::*;`.
- Run `cargo doc` and ensure the prelude page reads as a coherent API index; add crate-level doc comments with a minimal example on `Fulcrum`.

**Acceptance criteria:**
- [ ] `cargo run -p fulcrum --example hello_sprite` works using only `fulcrum::prelude::*`
- [ ] `cargo doc -p fulcrum` builds without warnings; `Fulcrum` has a compiling doctest

---

### Step 10: Milestone — Pong in games/pong

**Files:** `games/pong/src/main.rs`, `games/pong/assets/` (generated or checked-in 1×1 white texture)

**Requires review:** false

Pong exercises every phase-1 feature and doubles as living documentation.

- Entities: two paddles (`Sprite` with `custom_size`, white texture), ball, center line (row of small sprites), score displays.
- Systems (all `FixedUpdate`): player paddle from W/S, right paddle simple AI tracking ball (or Up/Down for 2P — pick AI), ball movement + wall bounce, paddle collision as AABB test with angle influenced by hit offset, scoring + ball reset with serve direction from `SimRng`.
- Score display without text (text arrives in phase 2): render each digit as a 3×5 grid of small square sprites from a `const [[u8; 15]; 10]` bitmap table. Encapsulate in a `spawn_digit`/`update_score` helper.
- Target ≤ ~250 lines; if the game code fights the engine anywhere, fix the engine API (that's the point of the milestone), noting the change.

**Acceptance criteria:**
- [ ] `cargo run -p pong` is a playable Pong: paddle control, bouncing, scoring to 5, ball interpolates smoothly
- [ ] Uses only `fulcrum::prelude`; zero direct wgpu/winit/bevy_ecs imports
- [ ] Same seed + scripted input (a small test harness feeding synthetic input for 600 ticks headless) produces identical final score/positions across two runs

## Testing

- **Headless determinism tests** (steps 2, 8, 10): fulcrum-core's default runner executes N ticks without a window; assert identical world state across same-seed runs. These become the seed of the phase-4 determinism harness — keep them in `crates/fulcrum-core/tests/determinism.rs`.
- **Unit tests**: asset dedup/placeholder (step 5), input edge semantics (step 7) via directly feeding the pending buffer, digit-bitmap helper (step 10).
- **Manual/visual checks**: interpolation smoothness, batching counts (log), resize behavior. Record what was checked in the PR/commit description.
- CI runs all headless tests; GPU-dependent code paths are exercised only locally for now (no GPU in CI).

## Notes

- **Runner indirection** (step 2) is what keeps fulcrum-core windowless and the determinism tests cheap — don't collapse it "for simplicity."
- bevy_ecs API churn: pin the version; wrap rather than re-export anything whose name is likely to change (schedule config traits especially). Fulcrum's public signatures are the stability boundary.
- Alternatives already rejected (do not reopen): miniquad backend, hecs, immediate-mode game drawing, async asset loading in phase 1.
- Text rendering, camera, and audio intentionally absent — phase 2 (`plans/002-2d-essentials.md`).
