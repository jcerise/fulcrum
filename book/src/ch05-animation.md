# Animation

Static sprites read as placeholders; motion makes a game. Fulcrum's animation stack has three
layers, and you choose how far up to climb.

## Sprite sheets and Aseprite

Artists (and generators) pack frames into one image. Fulcrum's native interchange is the
[Aseprite](https://www.aseprite.org/) JSON export — the de-facto standard for pixel art:

```text
aseprite -b player.ase --sheet creatures.png --data creatures.json --format json-array --list-tags
```

One loader call turns that into a sprite sheet plus one **clip** per tag:

```rust,ignore
fn setup(mut commands: Commands, mut aseprite: AsepriteLoader) {
    let art = aseprite.load("creatures.json").expect("sheet loads");
    commands.spawn((
        Sprite::from_sheet(art.sheet, 6),                    // a frame, by index...
        Transform2D::from_xy(120.0, 0.0),
        AnimationPlayer::play(art.clips["gem"]),             // ...or a whole clip, by tag
    ));
}
```

`AnimationPlayer` owns the sprite's region from then on, advancing frames **in simulation
ticks**. That's deliberate: animation is game state. Frame durations are converted from the
file's milliseconds to ticks at load, and a non-looping clip reports `finished()` — so "the
attack lands on frame 3" is a deterministic fact your gameplay can rely on, not a render-side
coincidence.

## State machines: animation logic as data

Every 2D game eventually hand-rolls "idle when still, run when moving, attack overrides
both". In Fulcrum that's a file:

```text
StateMachine(
    initial: "idle",
    params: { "speed": Float(0.0) },
    states: {
        "idle": (clip: "creatures.json#player_idle"),
        "run":  (clip: "creatures.json#player_run"),
    },
    transitions: [
        (from: State("idle"), to: "run",  when: [Gt("speed", 5.0)]),
        (from: State("run"),  to: "idle", when: [Lt("speed", 5.0)]),
    ],
)
```

Gameplay code never mentions clips again — it just reports facts through parameters:

```rust,ignore
let machine = animators.load("anim/player.animsm.ron").expect("machine loads");
commands.spawn((/* sprite, transform */, Animator::new(machine),
                AnimationPlayer::play(art.clips["player_idle"])));

// in the movement system:
animator.set_float("speed", velocity.length());
```

Transitions evaluate every tick: `Any`-state transitions first, then the current state's, in
declaration order, first match wins. `Trigger` params (`animator.trigger("attack")`) last
exactly one tick — Grove's big sibling, the dungeon game, uses one for its sword swing, with
`on_finish: "idle"` sending the attack state home when its clip ends. Files validate on load
and report *every* problem at once, with names.

```text
cargo run -p grove --example ch05_animation
```

The hero now bobs at rest and hustles when you move — and you can tune the feel by editing
the `.animsm.ron`, no recompile (chapter 7 makes that live).

## Want all of it?

Animation is the subject of the first Fulcrum **deep dive**: a whole sub-book
(`books/animation/` in the repository, `mdbook build books/animation`) with a code-along
tutorial that builds a game where the sword connects on an exact animation frame — plus the
complete reference: every clip and player rule, the full Aseprite pipeline (including
one-shot tags via `repeat`), precise state-machine evaluation semantics, and a recipe
collection. This chapter is the trailer; that book is the movie.
