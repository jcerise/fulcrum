# Engine Architecture

For the curious and the contributing. Games only ever depend on the facade:

```text
fulcrum            facade: DefaultPlugins + the prelude
├── fulcrum-core   app builder, schedules, Time, Input, SimRng, Transform2D, math
├── fulcrum-render window/wgpu, sprite batcher, camera, text, tilemaps, gizmos, hot-reload pump
├── fulcrum-asset  Handle<T>/Assets<T>, AssetServer (the single disk seam), file watcher
├── fulcrum-audio  kira-backed Sound/Audio
├── fulcrum-anim   clips, Aseprite import, animation state machines
├── fulcrum-scene  ComponentRegistry, prefabs, scenes, *Def resolvers
└── fulcrum-ui     retained game UI + the egui debug inspector
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
  `AssetServer::read_bytes` — hot reload hooks it today; the planned mod VFS (phase 4)
  layers over the same seam.
- **Sim data vs. cosmetic dressing.** Anything gameplay reads (tilemap tiles, animation
  state) loads deterministically inside the simulation; GPU-only work (textures, glyph
  atlases) attaches from frame-side systems. That split is why headless tests and windowed
  play behave identically.
- **Determinism as a tested contract** — `docs/determinism.md` states the rules; every
  milestone game carries a same-seed bit-identical test, headed for a CI-gated replay
  harness in phase 4.

The four games in `games/` double as the engine's integration tests, in ascending order of
ambition: `pong` (phase 1: core loop), `asteroids` (phase 2: sheets/audio/text),
`dungeon` (phase 3: fully data-driven), and `grove` (this book). Phase 4 — modding via a
layered VFS + sandboxed Lua, particles, spatial queries and pathfinding, and replay files —
is specced in `plans/004-power-features.md`.
