# Presentation: Making It Feel Like a Game

Chapter 4 finished the game; this chapter makes it *feel* like one. A score readout, a game
over banner, a blip when you eat, a buzz when you die, a body you can read at a glance. None
of it will change a single rule — and that "none" is enforced by architecture, not
discipline, which is the real subject of this chapter.

## The line down the middle of every game

Your crate is now two files, and the boundary between them is the most important line in the
project:

- **`src/game.rs`** — the simulation. State, rules, events. Runs on the tick clock. Knows
  nothing about pixels, speakers, or windows. *Could run on a server.*
- **`src/main.rs`** — the presentation. Sprites, text, sound, camera. Runs on the render
  clock (`add_frame_system`). Reads simulation state freely; **never writes it.**

If you've built backends and frontends, you already know this shape — the sim is the
service, the presentation is a client, and the events from chapter 4 are the domain-event
stream between them. The one-way rule is exactly as load-bearing as it is in that world, and
the failure smells the same: the day a rendering function sneaks a write into game state
("I'll just fix the score display by writing the score…"), your program grows bugs that
depend on *frame rate*. A game that behaves differently on a faster monitor is this exact
sin, discovered in production.

Fulcrum stacks the deck for you — tick systems and frame systems are registered differently,
so the boundary is visible in `main()` at a glance — but the discipline is yours to keep.
Everything you type this chapter is a frame system or setup for one, and every bit of it
answers to one test: *if I deleted this, would the game still be the same game?* The answer
must always be yes. Chapter 6 deletes all of it to prove the point.

## Step 1 — a window worth shipping

`Fulcrum::new(title)` accepted all defaults. The grown-up form takes a config — replace the
start of `main`:

```rust,ignore
    Fulcrum::with_config(FulcrumConfig {
        title: "My Snake".into(),
        window_size: (1152, 864), // 3x the 384x288 world; any size works, letterboxed
        clear_color: Color::rgb(0.05, 0.06, 0.05),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
    // ... unchanged from here
```

> **Toolbox — `FulcrumConfig`:** the app's startup knobs in one struct — window title and
> size, the color behind everything (`clear_color`), and, crucially for chapter 6, the
> `seed` that feeds `SimRng`. The `..Default::default()` idiom means you name only what you
> care about. `Fulcrum::new(title)` was always just this with one field set.

## Step 2 — sound: someone finally reads the events

Chapter 4's simulation announces `AppleEaten` and `RunEnded` into channels nobody read. Now
somebody reads them. First, two sounds into your assets directory:

```text
cp games/snake/assets/eat.wav games/snake/assets/die.wav games/my-snake/assets/
```

A resource to hold the handles, loaded in `setup` (add the `SoundLoader` parameter and the
insert alongside `Square`):

```rust,ignore
/// Sound handles, loaded once.
#[derive(Resource)]
struct Sounds {
    eat: Handle<Sound>,
    die: Handle<Sound>,
}
```

```rust,ignore
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,          // <-- new
    mut camera: ResMut<Camera2D>,
) {
    // ... camera and floor unchanged ...
    commands.insert_resource(Sounds {
        eat: sounds.load("eat.wav"),
        die: sounds.load("die.wav"),
    });
```

And the system that turns announcements into audio — register it with
`.add_frame_system(sound_effects)`:

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
        audio.play_with(
            &sound_assets,
            sounds.eat,
            PlayParams {
                volume: 0.6,
                // A little rise as the score climbs; pure presentation, so this is
                // allowed to be as cute as it likes.
                pitch: 1.0 + (score.0 % 8) as f32 * 0.03,
                ..Default::default()
            },
        );
    }
    for _ in ended.read() {
        audio.play(&sound_assets, sounds.die);
    }
}
```

(You'll need `AppleEaten`, `RunEnded`, and `Score` in the `use my_snake::game::{...}` list.)

> **Toolbox — `EventReader<T>`:** the subscribing end of chapter 4's channel. `.read()`
> yields each event this system hasn't seen yet, exactly once — each reader keeps its own
> cursor, so ten systems can consume the same announcements independently. Notice what this
> system is *not* doing: checking whether the score changed since last frame, comparing
> snake lengths, keeping a `previous_state` mirror. The simulation said what happened; this
> code only decides what it sounds like.
>
> **Toolbox — `Audio` + `Assets<Sound>` + `PlayParams`:** `SoundLoader` loads files into
> the `Assets<Sound>` store and hands back handles; `audio.play(...)` fires one;
> `play_with(...)` takes volume/pitch/pan. Fire-and-forget — no channels to manage for
> one-shot effects.

The pitch line is worth a beat: pure presentation gets to be *playful*. It can read anything
(here, the score) and do whatever feels good, because nothing downstream depends on it. The
strictness budget you spend on the simulation is what buys the looseness here. And when you
later want a particle burst on eating too ([Part II, chapter 11](ch11-particles.md) does
exactly this for Grove), it's another reader on the same channel — the sim, again, unchanged.

## Step 3 — the HUD: text is just another projection

The score and the banner are `Text` entities — spawned once in `setup`, then rewritten from
state every frame, the same one-way flow as the snake's sprites. Markers first:

```rust,ignore
/// Marks the score readout entity.
#[derive(Component)]
struct ScoreText;

/// Marks the end-of-run banner entity.
#[derive(Component)]
struct Banner;
```

At the bottom of `setup`:

```rust,ignore
    commands.spawn((
        Text::new("Score: 0").with_size(8.0).with_z(10.0),
        Transform2D::from_xy(4.0, GRID_H as f32 * CELL - 10.0),
        ScoreText,
    ));
    commands.spawn((
        Text::new("")
            .with_size(16.0)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::from_xy(
            GRID_W as f32 * CELL / 2.0,
            GRID_H as f32 * CELL / 2.0 + 24.0,
        ),
        Banner,
    ));
```

And the frame system (register with `.add_frame_system(hud)`, and add `SnakeState` to the
imports):

```rust,ignore
#[allow(clippy::type_complexity)] // standard ECS system shape
fn hud(
    score: Res<Score>,
    state: Res<SnakeState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<Banner>>,
    )>,
) {
    if let Ok(mut text) = texts.p0().single_mut() {
        text.value = format!("Score: {}", score.0);
    }
    if let Ok(mut banner) = texts.p1().single_mut() {
        banner.value = match *state {
            SnakeState::Playing => String::new(),
            SnakeState::GameOver => format!("GAME OVER\nScore: {}\nEnter to restart", score.0),
            SnakeState::Won => "YOU FILLED THE GRID?!\nEnter to go again".to_string(),
        };
    }
}
```

> **Toolbox — `Text`:** a component like `Sprite` — pair it with a `Transform2D` and the
> engine draws it. Setting `.value` is the whole API for updating it.
>
> **Toolbox — `ParamSet`:** both queries here ask for `&mut Text`, and the ECS won't hand
> one system two potentially-overlapping mutable views of the same component (the borrow
> checker's rules, applied to the world). `ParamSet` is the standard fix: it proves you'll
> only use one at a time (`p0()`, then `p1()`). When the compiler rejects a system with two
> similar queries, this is almost always the tool.

Formatting a string 60 times a second when it changes once a run offends the optimizer in
you. Let it go — this is nanoseconds, and the alternative (change detection, dirty flags,
subscriptions) is real complexity spent to avoid fake cost. *Render everything from state,
every frame* is the presentation-side twin of chapter 2's projection, and it means the HUD
can never be stale, desynced, or forgotten after a restart. There is no "update the UI" step
to miss, because there is no step at all. (When something is genuinely expensive, engines
optimize *that thing* — Fulcrum's UI module retains and diffs for you — but the mental model
stays "view of state.")

## Step 4 — readability, the floor of game feel

Last, three one-liners' worth of polish in the projection you already own. In
`project_snake`, replace the color assignment so the head is bright and the body fades
toward the tail — direction readable at a glance:

```rust,ignore
        // Head bright, body fading toward the tail: readable direction at a glance.
        let fade = 1.0 - (index as f32 / snake.body.len().max(1) as f32) * 0.45;
        sprite.color = if index == 0 {
            Color::rgb(0.55, 1.0, 0.45)
        } else {
            Color::rgb(0.2 * fade, 0.75 * fade, 0.25 * fade)
        };
```

Cheap tricks, chosen over pretty ones on purpose — with a single white texture and tints,
there's no art pipeline between you and shipping. **Readability is the floor for game
feel**: before juice, before polish, a player must be able to see what's true. (When you
want the pretty version, Grove's chapters cover sprites, sheets, and animation; the
architecture you'd hang them on is exactly what you have now.)

## Checkpoint

```text
cargo run -p my-snake
```

Score in the corner, rising blips as you streak, a proper game-over screen, a snake whose
direction you can read peripherally. Your `main.rs` should now match
`games/snake/src/main.rs` — the shipped game (`cargo run -p snake`) — top to bottom.

What you have is more valuable than one game; it's a complete *shape* for every game after
it:

```text
simulation (tick clock)                presentation (frame clock)
  state: Snake, Score, SnakeState  ──read──►  projections: segments, apples, HUD
  rules: steer, step, restart      ──events─► reactions: sound, (someday) particles
        ▲ Input, SimRng only                        never writes back
```

New vocabulary this chapter:

| Tool | What it's for |
| --- | --- |
| `FulcrumConfig` / `with_config` | window, clear color, and (soon) the RNG seed |
| `EventReader<T>` | subscribe to sim announcements; each reader has its own cursor |
| `SoundLoader`, `Audio`, `Assets<Sound>`, `PlayParams` | load and fire one-shot sounds |
| `Text` (+ `with_size`, `with_align`) | drawable text, updated by assignment |
| `ParamSet` | one system, two conflicting queries, used one at a time |

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

One thing is left to prove: that the left half of that diagram really can stand alone. Next
chapter we delete the window and let the machines play.
