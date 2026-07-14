# Clips and Players

The bottom layer of the system: a clip is timing data, a player is a component that spends
it. Everything here lives in `crates/fulcrum-anim` (`clip.rs`, `player.rs`).

## `AnimationClip`

```rust,ignore
pub struct AnimationClip {
    pub sheet: Handle<SpriteSheet>,  // where the frames live
    pub frames: Vec<u32>,            // region indices to display, in order
    pub frame_ticks: Vec<u32>,       // duration of each frame in ticks (parallel; min 1)
    pub looping: bool,               // loop forever, or stop on the last frame
}
```

- **Durations are simulation ticks, never seconds.** The Aseprite importer converts from
  the file's milliseconds at load (`round(ms / 1000 × tick_rate)`, minimum 1); if you build
  clips in code, you provide ticks directly. This is the design decision the whole system
  stands on: tick durations make animation deterministic, frame-exact, and testable, at the
  cost of quantizing to 16.7 ms steps at 60 Hz. Fulcrum takes that trade everywhere.
- `frames` are indices into the sheet's region table, in *display order* — the same region
  may appear multiple times (pingpong tags do this).
- A clip is an asset: store it in `Assets<AnimationClip>`, share it via handles. Every
  entity playing it keeps its own position in it (that's the player's job).

### Building clips in code

Most clips come from the Aseprite importer, but nothing stops you:

```rust,ignore
let clip = AnimationClip::from_fps(sheet, vec![0, 1, 2, 3], 10.0, true, config.tick_rate);
let handle = clips.insert(clip);   // clips: ResMut<Assets<AnimationClip>>
```

`from_fps` gives every frame the same duration (10 fps → 6 ticks each at 60 Hz). For
non-uniform timing, construct the struct directly — `frame_ticks` is yours.

## `AnimationPlayer`

```rust,ignore
#[derive(Component)]
pub struct AnimationPlayer {
    pub clip: Handle<AnimationClip>,
    pub playing: bool,        // paused players still show their current frame
    pub tick_in_frame: u32,   // ticks spent on the current frame so far
    pub frame_index: usize,   // index into the clip's frame list
    // finished: private — read via finished()
}
```

| Call | Meaning |
| --- | --- |
| `AnimationPlayer::play(clip)` | start `clip` from its first frame |
| `player.restart(clip)` | switch to `clip` from the start — **no-op if already active** |
| `player.finished()` | has a non-looping clip fully elapsed? |
| `player.playing = false` | pause on the current frame (`true` resumes) |
| `player.frame_index` | which frame shows right now — ordinary readable state |

`play` vs `restart` is the everyday sharp edge: calling `play` every tick pins a clip to
frame zero; calling `restart` every tick is free. Rule of thumb: continuous conditions
("still moving") use `restart`; events ("swing!") use `play`.

## Advance semantics, exactly

One engine system (`FixedUpdate`) advances every `(AnimationPlayer, Sprite)` pair, then
writes the current frame's region into the sprite. The precise rules, for when you're
keying gameplay to them:

1. A playing player's `tick_in_frame` increments once per tick. When it reaches the
   current frame's duration: reset to 0 and advance `frame_index`.
2. Past the last frame: looping clips wrap to frame 0; non-looping clips stop
   (`playing = false`) and set `finished`.
3. **A state or clip lasts exactly the sum of its frame ticks.** A 6-tick clip entered on
   tick N shows its frames during ticks N..N+5; `finished()` reads true from the end of
   tick N+5; a machine's `on_finish` fires on tick N+6. No off-by-one padding anywhere.
4. `frame_index` is clamped to the clip's length when read for display — this matters only
   when hot reload shrinks a clip under a live player; you'll never observe it otherwise.
5. The player **owns `Sprite::region`** while attached. Writes you make there are
   overwritten next tick. To hand-pose an entity, pause the player (or remove it).

Determinism note: because advance runs in `FixedUpdate` on tick durations, two runs with
the same inputs show the same frame on the same tick, headless or windowed, any machine.
The engine test `crates/fulcrum-anim/tests/headless_load.rs` pins this with exact per-tick
timelines; the dojo's `determinism_same_seed_same_dojo` fingerprints `(state, frame_index,
tick_in_frame)` across a whole scripted fight.

## What deliberately doesn't exist

- **No playback speed multiplier.** Retime the data (edit durations, or generate a second
  clip) rather than scaling time at runtime — a multiplier would break the "N ticks,
  exactly" arithmetic that frame-keyed gameplay relies on.
- **No blending or transitions.** Pixel-art frame animation cuts, it doesn't crossfade.
- **No animation events baked into clips.** Gameplay reads `frame_index` (see the dojo's
  `strike`) instead of clips calling gameplay — one direction of data flow, on purpose.
