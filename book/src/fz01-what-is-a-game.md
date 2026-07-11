# What a Game Actually Is

This track assumes you can program — and assumes nothing else. If you've shipped web services
or CLIs but never a game, the next six chapters are yours. We'll build Snake, completely: not
because Snake is impressive, but because it's the smallest program that contains every idea
that makes games *different* from the software you already write. We'll go slowly and explain
why everything is the way it is. (If you've made games before, skip ahead to
[Grove](ch01-window.md) — it covers this material at four times the speed.)

## The program you're used to writing is asleep by default

Nearly every program you've written shares one deep assumption: *nothing happens unless
something happens*. A web server sleeps until a request arrives. A CLI runs, prints, exits. A
GUI app idles until a click. The operating system parks your process, and your mental model —
handlers, requests, callbacks — is built around reacting.

A game breaks that assumption in the first second. Watch any game with your hands off the
controls: enemies patrol, water shimmers, the clock ticks down. **A game is a simulation that
runs whether or not you touch it.** Input doesn't *drive* the program, it *steers* a thing
that is already moving. So a game can't be a pile of event handlers. It's a loop:

```text
loop {
    read what the player did since last time
    advance the world by one small slice of time
    draw the world
}
```

Every game ever shipped is a decoration of those three lines. Each trip through the loop draws
one **frame** — one complete picture. Do that 60 times a second and the pictures fuse into
motion, exactly like film. That's all "60 FPS" means: the loop completed 60 times.

## The two clocks

Here's the first genuinely non-obvious design decision, and it's one Fulcrum makes for you.
How big is "one small slice of time"?

The obvious answer: however long the last frame took. Fast machine, small slices; slow
machine, big slices; multiply all your speeds by the measured delta and everything moves at
the same real-world rate. Most tutorials teach this. It's also a trap that has shipped a
thousand bugs: physics that explodes when a laggy frame produces one huge step (the bullet is
*past* the wall before anyone checks), gameplay that differs subtly between a 60 Hz office
monitor and a 144 Hz gaming one, and — worst — behavior that can never be reproduced twice,
because the sequence of deltas is different on every run. If you've fought a bug that only
happens in production under load, you know this genre of pain.

The engineering fix is the same one you'd reach for in any distributed system: **make time
discrete.** Fulcrum runs two clocks:

- The **simulation clock** ticks at exactly 60 Hz. Each **tick** advances the world by
  precisely 1/60th of a second — never more, never less. Game logic lives here. If the
  machine hiccups, the engine runs *more ticks* to catch up; each tick is still the same size.
- The **render clock** is the messy real-world one: draw a frame whenever the display wants
  one. Rendering *reads* the simulation and never writes it.

The payoff is enormous and mostly invisible: the same inputs produce the same game, every run,
on every machine — which is what will let us *test* Snake headlessly in the final chapter, the
way you'd test any other pure function. Hold that thought; it's the thesis of this whole
track.

## Six lines, explained honestly

Enough theory. Run this:

```text
cargo run -p snake --example fz01_loop
```

A green square patrols left and right, untouched. Here is the whole program, and — because
this track promised the *why* of everything — an honest account of each piece:

```rust,ignore
use fulcrum::prelude::*;

#[derive(Component)]
struct Patroller;

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let square = assets.load("white.png");
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(0.4, 0.9, 0.5))
            .with_size(Vec2::splat(24.0)),
        Transform2D::from_xy(-100.0, 0.0),
        Patroller,
    ));
}

fn patrol(mut squares: Query<&mut Transform2D, With<Patroller>>, time: Res<Time>) {
    for mut transform in &mut squares {
        let heading_right = time.tick % 240 < 120;
        let direction = if heading_right { 1.0 } else { -1.0 };
        transform.translation.x += direction * 100.0 * time.fixed_delta;
    }
}

fn main() {
    Fulcrum::new("fz01: a window, alive")
        .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system(patrol)
        .run();
}
```

- **`main` builds an app and surrenders to it.** `.run()` never returns — the engine owns the
  loop. This offends the instinct that *your* code should be in charge, but it's the same
  inversion as a web framework owning the accept loop while you write handlers: the loop is
  where all the platform misery lives (window events, GPU frame pacing, catching up missed
  ticks), and it's identical in every game. You write the parts that differ.
- **`add_startup(setup)`** registers a function to run once, before the first tick — the
  moral equivalent of your service's init code.
- **`add_system(patrol)`** registers a function on the simulation clock: called every tick,
  60 times a second, forever. This is the "advance the world" slot of the loop, and it's
  where Snake will actually live.
- **`patrol` moves at `100.0 * time.fixed_delta`** — 100 units per second, times the fixed
  size of one tick. Speeds in per-second units, multiplied by the tick length, stay honest
  no matter what the render clock is doing.
- **`time.tick`** is the simulation counting its own heartbeats: 120 ticks is exactly two
  seconds, on every machine, every run. When the square needs to "decide" something over
  time, it consults *simulation* time — never the wall clock. (A game that reads the wall
  clock inside its logic has already given up reproducibility. More on that in chapter 4.)
- The `Component`, `spawn`, `Query` machinery is the next chapter's whole subject; squint
  past it for now. One detail worth noticing today: the square glides smoothly even though
  the simulation moves it in 60 discrete nudges a second — when the render clock falls
  between two ticks, the engine draws positions *interpolated* between them. You get that
  for free precisely because the two clocks are separate.

## What "engine" means, one paragraph

Everything you just didn't write — the window, the GPU handshake, the loop, tick catch-up,
interpolation, input plumbing, image decoding — is the engine. `DefaultPlugins` installs all
of it. An engine is not a framework for games so much as *the parts of every game that are the
same game*. Fulcrum's particular opinions (fixed 60 Hz ticks, ECS, determinism) exist so the
parts you *do* write stay small and testable. You'll feel each opinion earn its keep as Snake
grows.

## Exercises

Every chapter in this track ends with a few of these. They modify the chapter's example
(`games/snake/examples/fz01_loop.rs` here) — the verification step is always the same:
run it and watch. No solutions are provided, on purpose; being briefly stuck is the lesson.

1. **More squares, same system.** In `setup`, spawn two more `Patroller` squares at different
   `y` positions (and, if you like, different colors). Don't touch `patrol` — then explain to
   yourself why you didn't have to. That query was never about *the* square.
2. **Change the rhythm.** Make the patrol turn around every second instead of every two. One
   number changes — but predict *which occurrences of it* before you edit, and check your
   understanding of `time.tick % 240 < 120` against what happens.
3. **Accelerate (harder).** Make the square's speed grow over time — say, 40 units/second
   plus 10 more for every elapsed second — while keeping the two-second turnaround. Everything
   you need is already in the system: `time.tick` for "how long has it been," `fixed_delta`
   for "how big is this step." If your square teleports or crawls, you've mixed the two up —
   which is exactly the mistake this chapter exists to inoculate against.

Next: the square becomes a snake — which turns out to mean almost nothing on screen and
everything in how we think about *state*.
