# Recipes

The patterns real games ask for, each with the shortest honest implementation. All of them
compose from pieces the tutorial built; file references point at working code in the
repository.

## A one-shot that returns home

Attack, hit-flinch, death-and-despawn — anything that plays once and hands control back:

```ron
"attack": (clip: "hero.json#attack", on_finish: "idle"),
```

Two requirements, one classic failure: the tag needs `"repeat": "1"` in the export (a
looping clip never finishes, so `on_finish` never fires — the single most common machine
bug), and the machine needs somewhere to go home to.

## Interruptions that beat intentions

A hit reaction must cut through an in-progress attack; a death must cut through everything.
Declaration order among `Any` transitions is the whole mechanism:

```ron
(from: Any, to: "dead",   when: [Is("alive", false)]),
(from: Any, to: "hit",    when: [Triggered("hurt")]),
(from: Any, to: "attack", when: [Triggered("attack")]),
```

Highest-priority interruption first. Working example: `games/dojo/assets/anim/hero.animsm.ron`,
proven by the dojo's `the_dummy_fights_back_at_point_blank` test.

## Locking controls during committed states

The gameplay side of the same coin — the machine owns "what's showing," gameplay decides
"what's legal":

```rust,ignore
let free = matches!(animator.state(), "idle" | "run");
if free && input.just_pressed(Key::Space) {
    animator.trigger("attack");
}
```

Gate movement and new intents on `free`. Do gate the trigger too: an `Any → attack` rule
happily fires out of the `hit` state otherwise. (`games/dojo/src/game.rs`, `control_hero`.)

## Frame-keyed hitboxes

The dojo's signature move — the active window of an attack *is* a frame of its clip:

```rust,ignore
if animator.state() == "attack"
    && player.frame_index == STRIKE_FRAME
    && !hero.swing_connected      // edge-guard: one swing, one hit
{ /* apply the hit */ }
```

Three parts, all load-bearing: the state check scopes the frame index to the right clip;
the frame check is the window (exactly as long as the artist made that frame); the
edge-guard flag resets when the state exits. Full pattern including the reset:
`games/dojo/src/game.rs`, `strike` and `rebound` — the latter shows the same recipe on an
NPC's animation against the player.

## Facing and flipping

One sheet, both directions. Facing is simulation state (rules read it); the flip is its
one-line projection:

```rust,ignore
// sim:      hero.facing_left = velocity.x < 0.0;
// frame:    sprite.flip_x = hero.facing_left;
```

Keep the flip out of the sim only if nothing simulates on it — the moment a rule cares
(the dojo's `in_front` check), promote facing to a component field and project it.

## Sound (or particles) on a frame

Footsteps on the contact frames, a whoosh on the swing frame. No baked-in animation events
— read the frames, presentation-side, with an edge-guard `Local`:

```rust,ignore
fn footsteps(
    players: Query<(&Animator, &AnimationPlayer), With<Hero>>,
    mut last: Local<Option<usize>>,
    /* audio params */
) {
    for (animator, player) in &players {
        let stepping = animator.state() == "run" && matches!(player.frame_index, 0 | 3);
        if stepping && *last != Some(player.frame_index) { /* play footstep */ }
        *last = if stepping { Some(player.frame_index) } else { None };
    }
}
```

Same shape as the dojo's `strike`, but running on the frame clock and allowed to be sloppy —
a missed footstep under lag is cosmetic; a missed hitbox wouldn't be. That asymmetry is why
gameplay frame-keys live in `FixedUpdate` and cosmetic ones can live in frame systems.

## Combos: buffering the next swing

`Any → attack` won't re-enter the active state (by design), so a combo is an explicit
chain:

```ron
"attack":  (clip: "hero.json#attack",  on_finish: "idle"),
"attack2": (clip: "hero.json#attack2", on_finish: "idle"),
...
(from: State("attack"), to: "attack2", when: [Triggered("attack")]),
```

Press attack mid-swing and the trigger routes to the follow-up instead of vanishing —
remember triggers last one tick, so this reads as "pressed during the current swing's
final tick window." For a lenient buffer, keep the intent in gameplay (a small tick
countdown) and re-trigger while it's live.

## Desynchronizing a crowd

Twenty torches flickering in unison scream "video game." Stagger the players at spawn:

```rust,ignore
let mut player = AnimationPlayer::play(clip);
player.frame_index = rng.range_i32(0..frames) as usize;   // SimRng if sim-side!
```

Use `SimRng` when the entities are simulation-spawned (determinism), or any hash of the
entity's position for pure decoration.

## Pausing everything (or one thing)

`player.playing = false` freezes a single entity on its current frame (the machine keeps
evaluating — set params to hold it still, or gate your param-writing systems). For a whole
game pause, gate the simulation the way the engine intends: stop ticking gameplay systems;
animation halts with them because it *is* one of them. There is deliberately no global
animation timescale — see "What deliberately doesn't exist" in the clips chapter.

## Placeholder art without an artist

Every animation in this book was drawn by a Python script. `tools/gen_dojo_art.py` is
~250 lines that draw frames with PIL, pack a sheet, and emit the Aseprite-format JSON with
tags, durations, and `repeat` — fork it, change the palette and poses, and you have a
legally-yours animated character before lunch. The format contract it targets is three
fields; see the Aseprite chapter.
