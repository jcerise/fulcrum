# Determinism and Testing

Grove is done — playable, data-driven, tooled. The last chapter is the payoff of the
discipline you've been practicing since chapter 3: proving the game works **without running a
window**.

## The five rules

Fulcrum promises *same-binary determinism*: one build, one seed, one input stream —
bit-identical simulation, every run. The engine enforces most of it structurally; your side
is five rules (the full contract lives in `docs/determinism.md`):

1. Simulation state changes only in `FixedUpdate`, timed only by `fixed_delta` and `tick`.
2. All simulation randomness comes from the `SimRng` resource — Grove's fox wanders with it.
3. Don't iterate `std` hash maps in the sim (their order is random per process); use the
   re-exported `FxHashMap`, or sort first.
4. Input reaches the sim only through the tick-sampled `Input`.
5. Presentation (frame systems, rendering, audio) may be as nondeterministic as it likes —
   and never writes sim state.

Follow those and replays, lockstep networking, and the tests below come for free.

## Structuring for testability

The finished Grove splits in two:

- **`game.rs`** — a `GamePlugin` with the components, resources, and `FixedUpdate` systems.
  No sprites, no sounds. It doesn't know pixels exist.
- **`main.rs`** — `DefaultPlugins`, scene/UI loading, camera glide, sprite flipping, the
  chime. All frame-side.

The reward: a test builds the app with `ScenePlugin + GamePlugin` and **no window at all**,
loads the same `grove.scene.ron` the real game uses, scripts the keyboard, and steps ticks by
hand:

```rust,ignore
let mut app = game::register_components(app)
    .with_plugin(ScenePlugin)
    .with_plugin(GamePlugin)
    .add_startup(load_the_scene);
app.run_startup();
for tick in 0..600 {
    let mut input = app.world_mut().resource_mut::<Input>();
    if tick == 0 { input.push_key(Key::D, true); }   // script the player
    input.sample(|s| s);
    app.tick();
}
assert!(app.world().resource::<Gems>().collected > 0);
```

Grove's real tests (in `games/grove/tests/`) assert two things: that a scripted walk
*actually collects a gem* — the game is provably playable, in CI, with no GPU — and that two
same-seed runs produce bit-identical state (`f32::to_bits` comparisons, no epsilons).

When a determinism test fails, something violated the rules — a frame-side write to sim
state, a stray `HashMap` iteration — and the fix is always to move the offending code to the
right side of the line. The discipline pays for itself the first time a bug report comes with
a replay file instead of a paragraph.

## Grove is built — now for the power tools

You've now touched every core component: the app and plugins, ECS, the two schedules, input,
sprites, cameras, tilemaps, animation and state machines, audio, events, prefabs, scenes, hot
reload, UI, gizmos, the inspector, and the determinism contract. You could stop here and ship
a game.

But the discipline you just proved out — a deterministic sim fed only by data — is exactly
what the engine's power features are built on. Part II adds them one at a time: particles
(chapter 11), spatial queries and pathfinding for the fox and for armies (chapter 12), mods
via the VFS and sandboxed Lua (chapter 13), and finally replays (chapter 14) — the direct
payoff of everything in this chapter.
