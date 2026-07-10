# Replays: the Payoff

Chapter 10 made a promise: follow five rules and replays come for free. Time to collect. A
Fulcrum replay is not a video and not a savegame — it's the *inputs*, and determinism does the
rest. Six hundred ticks of Grove is a few kilobytes.

## What a replay is

Player intent enters the simulation through exactly two channels, and the engine records both
per tick:

1. **The sampled `Input` delta** — key and button transitions, cursor position (including its
   world-space mapping, because the camera is cosmetic and can't be reconstructed).
2. **`CommandEvent`s** — the named-order channel: `world.resource_mut::<CommandOutbox>()
   .send("move", payload)`. UI clicks travel as `ui:click` commands automatically; Lua mods
   send them with `fulcrum.emit_command`.

Why commands, when input alone would replay? Because *derived intent* is fragile — "right
click at (312, 89) while these units were selected" only means something if every cosmetic bit
of selection state replays too. A `move` command carries its meaning with it. This is the
lockstep-networking shape (record the orders, not the mouse), and the RTS slice is built on
it: selection is local cosmetic state, and only commands reach the sim. During playback,
locally re-derived commands are discarded in favor of the recorded stream, so a system that
deterministically re-emits an order never doubles it.

## Recording and playing back

```rust,ignore
Fulcrum::with_config(FulcrumConfig { record_replays: true, ..Default::default() })
// ...play...
app.save_replay("run.freplay")?;        // or save_replay(world, path) from a system
```

`record_replays: true` records from tick 0 (about 6 MB per *hour* — leave it on in
development). Playback:

```rust,ignore
build_the_same_app().run_replay("run.freplay")?;    // headless, to completion
// or, windowed: app.start_playback(replay); app.run();
```

The file's header carries the seed (playback reseeds `SimRng`), the tick rate, the game id,
and the loaded mod set — mismatches warn before a single tick runs, because a replay is only
valid against the world that recorded it.

## Divergence: the debugging superpower

While recording, the engine embeds a **state hash** every 60 ticks — a fingerprint of
`Time::tick`, the `SimRng` stream position, and every *registered* component, in canonical
order. Playback recomputes and compares each one:

```text
Err(Divergence { tick: 540, expected: 0x..., actual: 0x... })
```

That error is the whole reason to love this system. A determinism bug — a frame-side write, a
stray `HashMap` iteration, wall-clock creep — stops being "the runs differ somewhere" and
becomes "state went wrong between ticks 480 and 540." The engine's CI runs exactly this as a
gate: every milestone game records a scripted run and plays it back, release mode, every
commit. (Only *registered* components are fingerprinted — register anything whose divergence
you want caught. One more reason chapter 7 mattered.)

The chapter's example records the same scripted walk the headless tests use, saves it, and
plays it back in a fresh app:

```text
cargo run -p grove --example ch14_replay
recorded 600 ticks (1/8 gems) -> /tmp/grove.freplay
playback reproduced the run — every state hash matched
```

Try breaking it: open the example and make the player speed depend on
`std::time::SystemTime`. The playback now names the tick where physics forked. That's rule
one, enforced by a file.

## Where Grove ends and your game begins

Now you really have touched everything: the app, ECS, the two schedules, sprites, worlds,
animation, audio, prefabs and scenes, UI, tools, determinism — and the power tier: particles,
spatial queries, pathfinding, mods, and replays. The games in the repository are the same
ideas at increasing scale, ending with `games/rts-slice`: two hundred units, Lua-scripted
waves, a sample mod, and battles that replay hash-clean from a file.

Go make something. The fox is waiting.
