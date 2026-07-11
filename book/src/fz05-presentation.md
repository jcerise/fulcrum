# Presentation: Making It Feel Like a Game

Chapter 4 finished the game; this chapter makes it *feel* like one. A score readout, a game
over banner, a blip when you eat, a buzz when you die, a head you can find at a glance. None
of it changes a single rule — and that "none" is enforced by architecture, not discipline,
which is the real subject of this chapter.

This is the shipped game now, view and all:

```text
cargo run -p snake
```

## The line down the middle of every game

Snake is now two files, and the boundary between them is the most important line in the
project:

- **`src/game.rs`** — the simulation. State, rules, events. Runs on the tick clock. Knows
  nothing about pixels, speakers, or windows. *Could run on a server.*
- **`src/main.rs`** — the presentation. Sprites, text, sound, camera. Runs on the render
  clock (`add_frame_system`). Reads simulation state freely; **never writes it.**

If you've built backends and frontends, you already know this shape — the sim is the
service, the presentation is a client, and events are the domain-event stream between them.
The one-way rule is exactly as load-bearing as it is in that world, and the failure smells
the same: the day a rendering function sneaks a write into game state ("I'll just fix the
score display by writing the score…"), your program grows bugs that depend on *frame rate*.
A game that behaves differently on a faster monitor is this exact sin, discovered in
production.

Fulcrum stacks the deck for you — tick systems and frame systems are registered differently,
so the boundary is visible in `main()` at a glance — but the discipline is yours to keep.
The reward comes in chapter 6, when the entire presentation is deleted in one line and the
game still runs.

## Reacting to events: sound

Chapter 4's simulation announces `AppleEaten` and `RunEnded` into channels nobody read. Now
somebody reads them:

```rust,ignore
fn sound_effects(
    mut eaten: EventReader<AppleEaten>,
    mut ended: EventReader<RunEnded>,
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    sound_assets: Res<Assets<Sound>>,
    score: Res<Score>,
) {
    let Some(sounds) = sounds else { return };
    for _ in eaten.read() {
        audio.play_with(&sound_assets, sounds.eat, PlayParams {
            volume: 0.6,
            pitch: 1.0 + (score.0 % 8) as f32 * 0.03,   // a little rise as you streak
            ..Default::default()
        });
    }
    for _ in ended.read() {
        audio.play(&sound_assets, sounds.die);
    }
}
```

Notice what this system is *not* doing: checking whether the score changed since last frame,
comparing snake lengths, keeping a `previous_state` mirror. Event readers hand each system
the events it hasn't seen yet, exactly once. The simulation said what happened; this code
only decides what it sounds like. When you later want a particle burst on eating too
([Part II, chapter 11](ch11-particles.md) does exactly this for Grove), it's another reader
on the same channel — the sim, again, unchanged.

The pitch line is worth a beat: pure presentation gets to be *playful*. It can read anything
(here, the score) and do whatever feels good, because nothing downstream depends on it.
The strictness budget you spend on the simulation is what buys the looseness here.

## Text is just another projection

The score and the banner are `Text` entities — spawned once, then a frame system writes
their `value` from state, the same one-way flow as the snake's sprites:

```rust,ignore
fn hud(score: Res<Score>, state: Res<SnakeState>, mut texts: /* queries */) {
    // every frame, unconditionally:
    text.value = format!("Score: {}", score.0);
    banner.value = match *state {
        SnakeState::Playing => String::new(),
        SnakeState::GameOver => format!("GAME OVER\nScore: {}\nEnter to restart", score.0),
        SnakeState::Won => "YOU FILLED THE GRID?!\nEnter to go again".to_string(),
    };
}
```

Formatting a string 60 times a second when it changes once a run offends the optimizer in
you. Let it go — this is nanoseconds, and the alternative (change detection, dirty flags,
subscriptions) is real complexity spent to avoid fake cost. *Render everything from state,
every frame* is the presentation-side twin of chapter 2's projection, and it means the HUD
can never be stale, desynced, or forgotten after a restart. There is no "update the UI" step
to miss, because there is no step at all. (When something is genuinely expensive, engines
optimize *that thing* — this book's UI module retains and diffs for you — but the mental
model stays "view of state.")

The last touches are all one-liners in the projection you already own: the head drawn
brighter than the body, the body fading toward the tail so direction reads at a glance, a
checkerboard floor so motion is visible against something. Cheap tricks, chosen over pretty
ones on purpose — with a single white texture and tints, there's no art pipeline between
you and shipping. **Readability is the floor for game feel**: before juice, before polish,
a player must be able to see what's true. (When you want the pretty version, Grove's
chapters cover sprites, sheets, and animation; the architecture you'd hang them on is
exactly what you have now.)

## Exercises

All presentation-side — the rules don't change, and (chapter 6 will make this concrete) no
test should notice any of these.

1. **Best-score display.** Track the best score across restarts and show it next to the
   current one. Store it in its own little resource, updated by a frame system whenever
   `Score` exceeds it. Notice two things: it survives Enter-to-restart *for free* (restart
   resets sim state, and this isn't sim state), and you've just drawn a meaningful line —
   is a high score gameplay or presentation? There's a real argument for each; take one side
   and be able to say why.
2. **Eyes.** Give the head two tiny white squares, offset perpendicular to `snake.dir`. The
   view is allowed to read anything in the world — direction included — and twenty minutes of
   fiddling with offsets here teaches more about "readability is the floor" than any
   paragraph. (Where do the eyes go? More entities to reconcile, or draw them relative to the
   head's view each frame? Both work; feel the trade.)
3. **Death flash (harder).** When `RunEnded` fires, tint the whole floor red and fade back
   over half a second. You'll need three small things: a marker component on the floor tiles
   (they're spawned bare today), a frame system reading `RunEnded`, and somewhere to keep the
   fade countdown — try `Local<f32>`, frame-side state for frame-side effects, ticked down by
   `time.frame_delta`. That's the whole presentation toolkit in one effect: markers,
   events, local state, the render clock.

## What you have

A complete game, and — more valuable — a complete *shape* for every game after it:

```text
simulation (tick clock)                presentation (frame clock)
  state: Snake, Score, SnakeState  ──read──►  projections: segments, apples, HUD
  rules: steer, step, restart      ──events─► reactions: sound, (someday) particles
        ▲ Input, SimRng only                        never writes back
```

One thing is left to prove: that the left half really can stand alone. Next chapter we
delete the window and let the machines play.
