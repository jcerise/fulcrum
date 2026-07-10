---
id: power-features
title: "Phase 4: Power features — modding (VFS + Lua), particles, spatial/pathfinding, replays (milestone: RTS slice)"
status: in-progress
priority: 4
created: 2026-07-08
steps_completed: 1
steps_total: 9
tags: [engine, modding, lua, vfs, particles, pathfinding, determinism, replay]
---

# Phase 4: Power features (milestone: RTS slice with replay)

## Summary

Deliver Fulcrum's differentiators: first-class modding (a layered virtual filesystem plus sandboxed, deterministic Lua scripting), a particle system, spatial queries and pathfinding helpers for crowd-scale games, and the payoff of the determinism investment — input recording and replay files with a CI-enforced determinism harness. The milestone is an RTS slice in `games/rts-slice`: select and command units, pathfinding, combat, moddable unit definitions, and a replay that plays back bit-identically.

## Context

- **Prerequisites:** plans 001–003 complete. Phase 3's data-driven layer (registry, prefabs, RON everywhere, tick-snapshotted input+UI events) is what makes this phase tractable.
- New crates: `crates/fulcrum-mod` (Lua runtime + mod loader), `crates/fulcrum-spatial` (grid index, A*, flow fields). VFS goes into `crates/fulcrum-asset`; particles into `crates/fulcrum-render`; replay into `crates/fulcrum-core`.
- New workspace deps: `mlua` (feature `lua54`, vendored), `bincode` or `postcard` (replay serialization — pick `postcard`), `crc32fast` or `xxhash-rust` (state hashing — pick `xxhash-rust`, deterministic seedless mode).
- Determinism stance for modding: **Lua runs inside the fixed tick and must be deterministic** — same rules as Rust sim systems. The API surface enforces this (no `os.time`, no `math.random` — replaced with engine equivalents).
- Security stance: mods are *sandboxed by capability omission* (no io/os/ffi in the Lua env), but this is anti-footgun, not a hard security boundary — document honestly.

## Steps

### Step 1: Layered virtual filesystem and mod manifests

**Files:** `crates/fulcrum-asset/src/vfs.rs`, `crates/fulcrum-asset/src/lib.rs` (route `read_bytes` through VFS), `crates/fulcrum-mod/src/manifest.rs`

**Requires review:** true

Architectural: every asset read in the engine changes route.

- `Vfs` resource: ordered stack of mounts, later mounts shadow earlier ones:

```rust
pub struct Vfs { mounts: Vec<Mount> }           // Mount { name, root: PathBuf }
impl Vfs {
    pub fn mount(&mut self, name: &str, root: PathBuf);   // pushed on top
    pub fn read(&self, path: &AssetPath) -> Result<Vec<u8>, VfsError>;    // top-down first hit
    pub fn list(&self, dir: &str, ext: &str) -> Vec<AssetPath>;           // union, shadowed dedup, SORTED (determinism)
    pub fn source_of(&self, path: &AssetPath) -> Option<&str>;            // which mount wins (debug/inspector)
}
```

- Base game `assets/` is mount 0, installed automatically. The phase-1 `read_bytes` seam (and therefore *every* loader: textures, RON, Aseprite, UI, sounds) now reads through `Vfs` — no per-loader changes should be needed beyond the seam.
- Hot-reload watcher (phase 3) watches all mounts.
- Mod manifest `mod.ron` at each mod root:

```ron
Mod(
    id: "more_slimes",              // [a-z0-9_], unique
    name: "More Slimes!",
    version: "0.1.0",
    engine_version: "0.1",          // compat check, warn on mismatch
    load_after: ["some_other_mod"], // optional ordering constraints
    scripts: ["scripts/init.lua"],  // entry points, run in listed order
)
```

- Parse + validate manifests here; loading/ordering logic is step 4.

**Acceptance criteria:**
- [ ] With a mod mounting `sprites/slime.png`, the mod's file wins; unmounting restores the base file
- [ ] `list("prefabs", "ron")` unions base+mod dirs, dedups shadowed paths, returns sorted order (unit tests)
- [ ] All existing games/tests pass unchanged with VFS routing in place (pure refactor for non-modded runs)

---

### Step 2: Lua runtime — sandboxed, deterministic, tick-scheduled

**Files:** `crates/fulcrum-mod/Cargo.toml`, `crates/fulcrum-mod/src/lib.rs`, `crates/fulcrum-mod/src/runtime.rs`, `crates/fulcrum-mod/src/sandbox.rs`

**Requires review:** true

Sandbox scope and determinism constraints are the decisions here.

- One shared `mlua::Lua` instance (resource, main thread). Environment construction (`sandbox.rs`):
  - **Removed:** `io`, `os` (except `os.clock` replaced by a stub returning sim time), `package`/`require` (replaced, below), `load`/`loadstring` on strings from data, `dofile`, `collectgarbage` (stubbed), `print` (redirected to engine log, prefixed with mod id).
  - **Replaced:** `math.random`/`math.randomseed` → engine functions backed by a **per-mod `SimRng` fork** (deterministic, mod-order-independent); `require("mod_id.module")` → custom loader resolving through the VFS to the owning mod's `scripts/` dir, with cycle detection.
  - Instruction-count hook (`Lua::set_hook`, every N=10⁶ instructions) aborts a script exceeding a per-tick budget with a clear error naming the mod — protects against accidental infinite loops, not malice.
- Script lifecycle: mods register callbacks rather than owning a loop:

```lua
fulcrum.on_init(function() ... end)               -- after world setup, in load order
fulcrum.on_tick(function(tick) ... end)           -- inside FixedUpdate, in load order
fulcrum.on_event("unit_died", function(e) ... end) -- game/engine-emitted sim events
```

- Rust side: `LuaRuntime` resource with `run_entry(mod_id, path)`, and a `FixedUpdate` system invoking `on_tick` callbacks in **mod load order** (deterministic). Script errors: log with mod id + traceback, disable that callback after 3 consecutive errors (don't kill the game), surface in the debug inspector.

**Acceptance criteria:**
- [ ] `os.execute`/`io.open`/`require "socket"` are nil/fail inside scripts (sandbox test)
- [ ] Two runs of a script using `math.random` in `on_tick` produce identical sequences (same seed)
- [ ] An infinite `while true do end` in `on_tick` aborts with a per-tick budget error naming the mod; the game keeps running
- [ ] `require` resolves a second file within the same mod via VFS; cycles error cleanly

---

### Step 3: Lua ↔ ECS bindings

**Files:** `crates/fulcrum-mod/src/bindings.rs`, `crates/fulcrum-mod/src/lua_api.md` (modder-facing API doc)

**Requires review:** true

The modder API surface — the contract that must stay stable.

- Everything flows through the phase-3 `ComponentRegistry` (components as Lua tables ↔ RON values ↔ typed components). No per-component glue code.

```lua
-- entities & components
local e = fulcrum.spawn_prefab("prefabs/slime.prefab.ron", { x = 100, y = 50 })
fulcrum.despawn(e)
local hp = fulcrum.get(e, "Health")          -- table copy, nil if absent
fulcrum.set(e, "Health", { max = 20, current = 20 })
fulcrum.insert(e, "Burning", { ticks_left = 120 })

-- queries (component names -> iterator of {entity, comps...})
for e, pos, hp in fulcrum.query("Transform2D", "Health") do ... end

-- world / engine
fulcrum.input.pressed("Space")               -- tick-sampled Input (read-only)
fulcrum.emit("unit_died", { unit = e })      -- sim events, also received by Rust systems + other mods
fulcrum.audio.play("sounds/boom.ogg")        -- cosmetic, allowed
fulcrum.log("hello")
fulcrum.tick()                                -- current sim tick
```

- Implementation notes:
  - `get`/`set`/`insert`/`spawn`/`despawn` from Lua **queue** into a command buffer applied at defined points inside the tick (after each mod's `on_tick` returns), keeping &mut World access single-sited and ordering deterministic. `get` and `query` read a world snapshot borrow (exclusive system wraps the whole Lua tick phase — simplest sound approach; optimize later only if profiled).
  - `query` materializes matching entities in query iteration order (deterministic per phase-1 rules).
  - Entity values in Lua are opaque light userdata wrapping `Entity` (index+generation); stale-entity access returns nil + one-time warning rather than erroring.
  - `fulcrum.emit`: sim events are (name: String, RON-able table) pairs; Rust games subscribe via `EventReader<ModEvent>`; engine emits a small built-in set (documented in `lua_api.md`).
- `lua_api.md` is written alongside the code and is the canonical modder doc — every function with signature, determinism notes, and a runnable example. This file seeds the modding chapter of the book.

**Acceptance criteria:**
- [ ] A Lua script spawns 10 slimes from a prefab, buffs their Health via `set`, and a Rust system observes the changes that same tick
- [ ] `query` over 1,000 entities from Lua completes within 1 ms in release (log; sets a baseline)
- [ ] Stale entity `get` returns nil without panic; despawn-then-set in one tick is a no-op with a warning
- [ ] Headless determinism test: a Lua mod driving random spawns passes the same-seed identical-state check

---

### Step 4: Mod loader — discovery, ordering, lifecycle

**Files:** `crates/fulcrum-mod/src/loader.rs`, `crates/fulcrum-mod/src/lib.rs` (`ModPlugin`)

**Requires review:** false

- `ModPlugin` (opt-in, not in `DefaultPlugins`; games enable modding deliberately): scans `mods/` next to the game (each subdir with a `mod.ron`), plus `FulcrumConfig::extra_mod_dirs`.
- Load order: topological sort on `load_after` constraints, ties broken by mod id (lexicographic — deterministic); cycle = hard error listing the cycle. The resolved order is logged and drives: VFS mount order (later = higher priority), script `on_init`/`on_tick` order, per-mod RNG fork order (fork by sorted mod id at startup so adding a mod doesn't shift others' streams).
- Lifecycle: mount all VFS roots → parse manifests → run `on_init` scripts in order → normal play. Runtime enable/disable is out of scope (restart to change mods; document).
- `ModRegistry` resource: loaded mods (id, version, path, enabled) — feeds a "Mods" tab in the debug inspector (list, load order, VFS `source_of` lookup for a path).
- Replay-relevant: the loaded mod set + versions becomes part of replay metadata (step 7 records it; playback warns on mismatch).

**Acceptance criteria:**
- [ ] Two test mods with a `load_after` constraint load in constrained order; scripts run in that order (observable via log sequence)
- [ ] A data-only mod (no scripts, just an override PNG + prefab) works with zero Lua involvement
- [ ] Bad manifest (dup id, cycle, missing entry script) fails at startup with an actionable message
- [ ] Determinism: same mod set → identical run; adding an unrelated mod doesn't change another mod's RNG stream

---

### Step 5: Particle system

**Files:** `crates/fulcrum-render/src/particles.rs`, particle asset examples in `games/rts-slice/assets/fx/`

**Requires review:** false

Particles are **cosmetic**: simulated on the `Update` schedule with `frame_delta`, excluded from determinism and replay state (replays reproduce them approximately via the same sim events — fine).

- `ParticleEffectAsset` (RON, hot-reloadable):

```ron
ParticleEffect(
    texture: "fx/spark.png",           // or sheet region
    spawn_rate: 50.0,                  // per second; or Burst(count)
    lifetime: (0.3, 0.8),              // min..max seconds
    initial_speed: (20.0, 60.0), spread_deg: 360.0, direction_deg: 90.0,
    gravity: (0.0, -100.0),
    size: ( start: 4.0, end: 0.0 ), color: ( start: "#FFD080FF", end: "#FF400000" ),
    rotation_speed: (-3.0, 3.0),
)
```

- `#[derive(Component)] ParticleEmitter { effect: Handle<ParticleEffectAsset>, active: bool, one_shot: bool }` on an entity with `Transform2D`; emitter follows the (interpolated) entity.
- CPU sim into per-emitter `Vec<Particle>`; rendered as a dedicated instanced quad pass (additive and alpha blend modes per effect); pool capacity per emitter (default 1024) with oldest-recycled overflow.
- Cosmetic RNG: a non-sim `Pcg32` seeded from a frame counter — explicitly NOT `SimRng` (document why in the module docs: particles must never consume sim randomness).
- `Commands` helper: `spawn_effect_at(effect, pos)` for one-shot explosions (auto-despawn when all particles dead).

**Acceptance criteria:**
- [ ] 100 emitters × 200 live particles ≥ 60 fps release (log particle count + frame ms)
- [ ] One-shot explosion effect auto-cleans (entity count returns to baseline in a test)
- [ ] Editing the effect RON hot-reloads mid-emission without panic
- [ ] Determinism harness still passes with particles active (proves sim isolation)

---

### Step 6: Spatial index — uniform grid for range queries

**Files:** `crates/fulcrum-spatial/Cargo.toml`, `crates/fulcrum-spatial/src/lib.rs`, `crates/fulcrum-spatial/src/grid.rs`

**Requires review:** false

- `SpatialGrid` resource: uniform grid (cell size configured at plugin setup, default 64.0) rebuilt each tick from entities tagged `#[derive(Component)] SpatialIndexed` (opt-in tag — RTS units yes, bullets maybe, UI never):
  - Rebuild-per-tick (clear + reinsert) rather than incremental update — simpler, cache-friendly, fast enough for tens of thousands of entities; runs first in `FixedUpdate`.

```rust
impl SpatialGrid {
    pub fn query_circle(&self, center: Vec2, radius: f32) -> impl Iterator<Item = Entity> + '_;
    pub fn query_rect(&self, rect: Rect) -> impl Iterator<Item = Entity> + '_;
    pub fn nearest(&self, from: Vec2, max_radius: f32, filter: impl Fn(Entity) -> bool) -> Option<Entity>;
}
```

- **Determinism:** results are returned in a defined order (entities inserted per-cell in query-iteration order; queries visit cells in fixed row-major order) — document that callers may rely on it, and `nearest` ties break by entity index. This ordering guarantee is the reason to hand-roll rather than depend on an external crate.
- Exposed to Lua: `fulcrum.query_circle(x, y, r)` returning an ordered entity array.

**Acceptance criteria:**
- [ ] Property test vs brute force: 1,000 random entities, 100 random circle/rect queries — identical sets
- [ ] Ordered-result guarantee unit-tested (same world → same result *sequence*)
- [ ] 10,000 indexed entities: rebuild + 500 queries/tick within 1 ms release (log)

---

### Step 7: Pathfinding — grid A* and flow fields

**Files:** `crates/fulcrum-spatial/src/nav.rs`, `crates/fulcrum-spatial/src/astar.rs`, `crates/fulcrum-spatial/src/flowfield.rs`

**Requires review:** false

- `NavGrid` resource: walkability + cost per cell; constructed from a tilemap layer (`NavGrid::from_tilemap(&TilemapAsset, layer, |tile| -> Option<cost>)`) or built programmatically; games mutate it as buildings rise/fall (`set_walkable`, `set_cost`).
- A*: `astar(&NavGrid, from: (u32,u32), to: (u32,u32)) -> Option<Vec<(u32,u32)>>` — binary heap, octile heuristic, 8-directional with no-corner-cutting rule; deterministic tie-breaking (f, then h, then cell index). Path smoothing helper `simplify_path` (line-of-sight walkability raycast).
- Flow fields for crowds: `FlowField::compute(&NavGrid, goal_cells: &[(u32,u32)]) -> FlowField` (integration field via Dijkstra + per-cell best-neighbor direction); `sample(world_pos) -> Vec2`. One field serves any number of units heading to the same goal — the RTS move-command primitive. Budget note: computation is synchronous within the tick; a 512×512 field must compute < 4 ms release (measure; if exceeded, shrink scope rather than adding async).
- Both are plain deterministic functions over deterministic inputs — no exemptions needed.

**Acceptance criteria:**
- [ ] A* unit tests: straight line, obstacle detour, no-path → None, corner-cutting forbidden, deterministic tie-break (fixed expected path snapshots)
- [ ] Flow field: all walkable cells' directions descend the integration field; unreachable cells flagged
- [ ] 512×512 flow field computes < 4 ms release (asserted with a generous CI-safe bound, logged precisely)

---

### Step 8: Replay system and determinism harness

**Files:** `crates/fulcrum-core/src/replay.rs`, `crates/fulcrum-core/src/state_hash.rs`, `crates/fulcrum-core/tests/determinism.rs` (promote), `.github/workflows/ci.yml` (harness job), `docs/determinism.md` (update)

**Requires review:** true

The payoff step — public format + CI gate.

- Recording: the runner (phase 1) already snapshots input + UI events per tick. `ReplayRecorder` resource captures per tick: the `Input` delta (pressed/released keys+buttons, mouse pos if changed) + UI events + mod-emitted *player-command* events (games/Lua mark events as commands via `fulcrum.emit_command` / `EventWriter<CommandEvent>` — the RTS uses this for unit orders so replays are command-level, not just raw input).
- File format (`.freplay`, postcard + header): `{ magic, format_version, engine_version, game_id, seed, tick_rate, mod_set: [(id, version)], ticks: [...] , state_hashes: [(tick, u64)] every 60 ticks }`.
- Playback: `Fulcrum::run_replay(path)` — swaps the input-sampling stage to feed recorded data, seeds `SimRng` from the header, runs headless or windowed. Each embedded state hash is checked on the fly; first divergence → error with tick number (the debugging entry point for determinism bugs).
- `state_hash(world)`: xxhash over registered components of all entities in a canonical order (sorted entity index; component name order fixed) — reuses the registry's `extract`. Document what's covered (registered sim components) and not (unregistered/cosmetic).
- API: `replay.start_recording()`, `stop_and_save(path)`, auto-record-always ring buffer option (`FulcrumConfig::record_replays`).
- CI determinism harness job: run pong, asteroids, dungeon, rts-slice scripted headless runs twice + record/playback round-trip; any hash divergence fails CI. This is the regression gate that keeps the determinism promise honest from here on.

**Acceptance criteria:**
- [ ] Record 1,000 ticks of the dungeon demo → playback reproduces every embedded state hash
- [ ] Tampering with one recorded input mid-file → playback reports divergence at the right tick
- [ ] Replay with a missing/mismatched mod set warns before starting
- [ ] CI job runs the 4-game harness; intentionally adding a `thread_rng` call in a test branch makes it fail (verified once, then reverted)

---

### Step 9: Milestone — RTS slice in games/rts-slice

**Files:** `games/rts-slice/Cargo.toml`, `games/rts-slice/src/*.rs`, `games/rts-slice/assets/**`, `games/rts-slice/mods/sample_mod/**`

**Requires review:** false

The thesis demo: an RTS slice exercising every phase-4 feature at once.

- Map: tilemap with obstacles → `NavGrid`; camera pan (edge/WASD) + zoom.
- Units: worker + soldier prefabs; **unit stats defined in RON data files** (`units/*.unit.ron` — speed, hp, damage, range) loaded via VFS `list` (so mods can add unit types without touching game code).
- Selection: click + drag-box (screen→world via camera; `query_rect` for the box); selection is *local/cosmetic state* — only issued **commands** enter the sim (the lockstep-shaped architecture: sim consumes `CommandEvent`s exclusively).
- Move command: right-click → flow field to target; group of units follows it with simple local avoidance (`query_circle` separation steering).
- Combat: soldiers auto-acquire nearest enemy in range (`nearest`), attack on cooldown (ticks), particle blood/spark + audio on hit, corpse fade.
- Two teams: player + scripted attacker waves (waves driven by a **Lua script in the base game** — dogfooding the scripting API as a game-logic tool, not just for mods).
- `mods/sample_mod/`: adds a third unit type (new `.unit.ron` + sprite + a Lua on-death explosion effect) — proving data + script modding end to end; README documents making it.
- Replay: R starts/stops recording; `--replay file.freplay` plays back; the CI harness runs a 2,000-tick scripted battle and verifies record/playback hashes.
- Scale target: 200 units pathing + fighting at 60 fps release with tick ≤ 8 ms (log tick ms; this validates the spatial/flow-field budgets).

**Acceptance criteria:**
- [ ] Select/move/fight loop works; 200-unit battle holds 60 fps, tick ≤ 8 ms release
- [ ] Deleting `mods/sample_mod` removes the third unit type cleanly; re-adding restores it (no code change)
- [ ] A 2,000-tick battle replay plays back with all state hashes matching, windowed and headless
- [ ] Only `fulcrum::prelude` (+ `fulcrum::mod_api` for command/event types) imports; unit definitions contain zero Rust

## Testing

- Sandbox tests (step 2) are non-negotiable: assert each removed capability is absent and each replacement is deterministic.
- Property tests: spatial grid vs brute force (step 6); A*/flow-field snapshot tests (step 7).
- The CI determinism harness (step 8) becomes a permanent workspace-wide gate — every future engine PR runs it. Budget ~2 min CI time; keep scripted runs headless.
- Perf numbers (Lua query ms, grid rebuild ms, flow field ms, RTS tick ms) logged by the code itself and recorded in PR descriptions — these are the engine's living perf baselines.

## Notes

- **Lua over WASM** (locked in brainstorm): modder familiarity and zero-ABI-friction beat sandbox strength; the phase-3 registry means bindings are generic, not per-type. WASM could be added later as a *second* scripting backend behind the same command-buffer API if a game needs untrusted-mod hard sandboxing.
- **Command-level replay** (step 8) rather than raw-input-only: slightly more game cooperation required (`emit_command`), but it's exactly the architecture lockstep networking needs later — networking itself stays out of scope, but this phase leaves the door aligned.
- **Exclusive-system Lua tick**: Lua's world access single-threads that stage. Accepted: mod scripts are not the perf path; the escape hatch (snapshot + parallel apply) is a contained optimization if profiling ever demands it.
- Per-mod RNG forks keyed by sorted mod id (step 4) mean one mod's dice rolls never depend on another mod's presence — subtle but critical for replay stability across mod-set changes.
- Runtime mod enable/disable and mod dependency *version* solving are explicitly out of scope (restart to change mods; `load_after` + engine_version warning only).
