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

Within `FixedUpdate`, systems without explicit ordering may run in parallel/nondeterministic
order. If two systems write the same data, order them explicitly (`.chain()`, `.before()`,
`.after()`). A good default for game logic: one `.chain()`ed tuple of systems.

## Enforcement

`crates/fulcrum-core/tests/determinism.rs` runs seeded simulations twice and asserts
bit-identical world state. Every phase adds its features to this harness; phase 4 promotes it to
a CI gate covering every milestone game plus replay round-trips.
