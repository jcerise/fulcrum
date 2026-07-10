# Engine Architecture

For the curious and the contributing. Games only ever depend on the facade:

```text
fulcrum            facade: DefaultPlugins + the prelude
├── fulcrum-core   app builder, schedules, Time, Input, SimRng, Transform2D, math, replays
├── fulcrum-render window/wgpu, sprite batcher, camera, text, tilemaps, particles, gizmos
├── fulcrum-asset  Handle<T>/Assets<T>, AssetServer over a layered VFS, file watcher
├── fulcrum-audio  kira-backed Sound/Audio
├── fulcrum-anim   clips, Aseprite import, animation state machines
├── fulcrum-scene  ComponentRegistry, prefabs, scenes, *Def resolvers, the replay state hash
├── fulcrum-ui     retained game UI + the egui debug inspector
├── fulcrum-spatial uniform-grid index, NavGrid/A*, flow fields
└── fulcrum-mod    mod discovery/mounting, sandboxed Lua runtime, the fulcrum.* API
```

Decisions you're building on (and where they're enforced):

- **bevy_ecs, wrapped.** The ECS is `bevy_ecs`, re-exported through the prelude so its churn
  stays behind Fulcrum's API. `FixedUpdate` runs a single-threaded executor — parallel
  execution of ambiguously-ordered systems would silently break determinism.
- **wgpu + winit,** with wgpu's version pinned to what `egui-wgpu` supports — engines
  version-match their ecosystem.
- **One draw path.** Sprites, text glyphs, tilemap chunks, and UI all feed one batcher
  (world-space and screen-space stages); gizmos are the only immediate-mode API.
- **Synchronous, seam-guarded assets.** Every disk read goes through
  `AssetServer::read_bytes`, which resolves against a stack of mounts — the base game at the
  bottom, mods layered above in load order. Hot reload and modding are the same seam.
- **Sim data vs. cosmetic dressing.** Anything gameplay reads (tilemap tiles, animation
  state) loads deterministically inside the simulation; GPU-only work (textures, glyph
  atlases) attaches from frame-side systems. That split is why headless tests and windowed
  play behave identically.
- **Determinism as a tested contract** — `docs/determinism.md` states the rules; every
  milestone game carries a same-seed bit-identical test *and* a record→playback replay
  round-trip, and CI runs them all in release as a dedicated gate. A state hash embedded
  every 60 ticks turns "the runs differ somewhere" into "state diverged at tick N."
- **Two recorded channels.** Replays capture the tick-sampled input delta and the
  `CommandOutbox` stream — the lockstep-shaped design that would carry networking later.
  Cosmetic state (selection, camera, particles) is deliberately unrecorded and re-derived.
- **Lua inside the tick.** Mod scripts run in an exclusive `FixedUpdate` stage against the
  component registry, sandboxed (no io, sim-clock time, per-mod deterministic RNG, an
  instruction budget) so mods inherit the determinism contract instead of threatening it.

The five games in `games/` double as the engine's integration tests, in ascending order of
ambition: `pong` (phase 1: core loop), `asteroids` (phase 2: sheets/audio/text), `dungeon`
(phase 3: fully data-driven), `grove` (this book), and `rts-slice` (phase 4: 200 units on
flow fields, Lua-scripted waves, a sample mod, and battles that replay hash-clean).
