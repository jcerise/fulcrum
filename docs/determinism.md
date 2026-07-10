# The Fulcrum determinism contract

Fulcrum promises **same-binary determinism**: the same build of a game, given the same seed and
the same per-tick inputs, produces bit-identical simulation state on every run. This is what
makes replays, headless regression tests, and (eventually) same-platform lockstep multiplayer
possible.

Cross-platform bit-exactness is **not** promised — `f32` math may differ across
architectures/compilers. A replay file is valid for the binary that recorded it.

## The five rules

Simulation code (everything in `FixedUpdate`) must follow these. Cosmetic code (`Update`
systems, rendering, audio) is exempt.

1. **Simulation state changes only in `FixedUpdate`**, timed only by `Time::fixed_delta` and
   `Time::tick`. Never read wall-clock time, `Time::frame_delta`, or `Time::alpha` in a
   simulation system.

2. **All simulation randomness comes from `SimRng`** (or a `SimRng::fork`). Never
   `rand::thread_rng()`, hashing of pointers/addresses, or time-based seeds.

3. **No iteration over `std::collections::HashMap`/`HashSet` in simulation systems.** Their
   `RandomState` hasher randomizes iteration order per process. Use the re-exported
   `FxHashMap`/`FxHashSet` (deterministic order for identical insertion sequences), or sort
   before iterating. ECS query iteration order is stable for identical spawn/despawn sequences —
   which rules 1–5 guarantee.

4. **Input reaches the simulation only through the tick-sampled `Input` resource.** The runner
   drains OS events into it exactly once per tick, so a recorded per-tick input stream
   reproduces a run exactly.

5. **Rendering, audio, and `Update`-schedule systems may be freely nondeterministic** — and must
   never write simulation state. If a cosmetic system needs randomness, it must not consume
   `SimRng` rolls.

## System ordering

`FixedUpdate` always runs on the single-threaded executor, so system execution order is the
schedule's topological order — deterministic for identical system registration. Still order
systems that write the same data explicitly (`.chain()`, `.before()`, `.after()`): the
topological order of ambiguous systems is an implementation detail that can shift when systems
are added or removed. A good default for game logic: one `.chain()`ed tuple.

## Replays

The payoff for the five rules: a `.freplay` file (header + per-tick records, postcard-encoded)
reproduces a run exactly on the same binary.

- **What's recorded per tick:** the sampled `Input` delta (including `mouse_world`, since the
  camera mapping is cosmetic and can't be reconstructed) and every `CommandEvent` drained from
  the `CommandOutbox`. Commands are the lockstep-shaped channel — UI clicks travel as
  `ui:click` commands, Lua mods use `fulcrum.emit_command`, and games send orders with
  `CommandOutbox::send`. During playback, locally re-derived commands are discarded and the
  recorded stream is injected, so input-derived commands never double.
- **Recording:** set `FulcrumConfig::record_replays` (records from tick 0; roughly 6 MB/hour at
  60 Hz) or call `ReplayRecorder::start_recording()` before the first tick. Save with
  `Fulcrum::save_replay(path)` or `fulcrum_core::replay::save_replay(&mut world, path)` from an
  exclusive system.
- **Playback:** `Fulcrum::run_replay(path)` (headless, to completion) or
  `Fulcrum::start_playback(replay)` before `run()` (windowed). The header reseeds `SimRng`;
  mismatched game/engine/tick-rate/mod-set headers warn first.
- **Divergence checks:** while recording, a state hash is embedded every 60 ticks (plus a final
  one on save); playback recomputes and compares each, and the first mismatch errors with its
  tick number — your entry point for hunting determinism bugs. The hash (installed by
  `ScenePlugin`) covers `Time::tick`, the `SimRng` stream position, and every **registered**
  component in canonical order. Unregistered components are invisible to it: register anything
  whose divergence you want caught.
- **Caveat:** `UiFocus::pointer_over_ui` is cosmetic state; if simulation logic reads it, route
  the decision through a command instead or replays won't capture it.

## Enforcement

`crates/fulcrum-core/tests/determinism.rs` runs seeded simulations twice and asserts
bit-identical world state, and every milestone game ships its own scripted determinism and
replay round-trip tests. CI runs them all in release as a dedicated `determinism` job — a hash
divergence anywhere fails the build. (Verified to catch real nondeterminism: a wall-clock
dependence temporarily added to the dungeon's movement failed the harness at the first hash
checkpoint after it took effect.)
