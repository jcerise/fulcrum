# What a Game Actually Is

This track assumes you can program — and assumes nothing else. If you've shipped web services
or CLIs but never a game, the next six chapters are yours. Together we'll build **Snake**,
completely, and you'll type every line: this is a code-along, not a tour. Snake isn't
impressive, but it's the smallest program that contains every idea that makes games
*different* from the software you already write, and by the end you'll have a playable,
polished, *tested* game in a crate you built from `cargo new`.

(If you've made games before, skip ahead to [Grove](ch01-window.md) — it covers this
material at four times the speed.)

A finished copy of everything we build lives in the repository at `games/snake`, and each
chapter ends with a **checkpoint** you can run and diff against. Use it when you're stuck;
type your own the rest of the time. Typing is the point — the questions that occur to you
mid-keystroke ("wait, why a *resource*?") are the actual curriculum.

## The one idea to hold onto

Nearly every program you've written shares one deep assumption: *nothing happens unless
something happens*. A web server sleeps until a request arrives. A CLI runs, prints, exits.
The OS parks your process, and your mental model — handlers, requests, callbacks — is built
around reacting.

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

Every game ever shipped is a decoration of those three lines. Each trip through the loop
draws one **frame**; do that 60 times a second and the pictures fuse into motion. That's all
"60 FPS" means. Keep the loop in your head — everything Fulcrum asks of you in this chapter
is a slot in it.

## Step 0 — create your project

Work happens inside your clone of the Fulcrum repository. The workspace's `Cargo.toml` lists
`games/*` as members, so a new crate in `games/` joins the build automatically — no manifest
surgery. From the repository root:

```text
cargo new games/my-snake
```

Replace the generated `games/my-snake/Cargo.toml` with:

```toml
[package]
name = "my-snake"
version = "0.1.0"
edition = "2024"

[dependencies]
fulcrum = { workspace = true }
# Manifest-only: Fulcrum's #[derive(Component)] macros generate paths that
# start with `bevy_ecs::`, so the crate name must be resolvable here.
bevy_ecs = { workspace = true }
```

You'll never write `bevy_ecs::` yourself — everything comes through `fulcrum::prelude` — but
the derive macros need that second line to compile. It's the one piece of ceremony in the
whole setup.

Finally, the game needs somewhere to keep images and sounds, and one image to start with: an
8×8 white square. Every rectangle in Snake will be a tinted, stretched copy of it — no art
pipeline between you and shipping:

```text
mkdir games/my-snake/assets
cp games/snake/assets/white.png games/my-snake/assets/
```

## Step 1 — a window

Replace `games/my-snake/src/main.rs` with the smallest possible Fulcrum program:

```rust,ignore
use fulcrum::prelude::*;

fn main() {
    Fulcrum::new("My Snake")
        .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
        .with_plugin(DefaultPlugins)
        .run();
}
```

Run it:

```text
cargo run -p my-snake
```

A dark, empty window, holding steady. Underwhelming — and already running the loop from the
top of this chapter, 60 times a second, doing nothing each time. Close it and look at the
four calls you just made, because they're the frame everything else hangs on:

> **Toolbox — `Fulcrum::new(title)`:** creates the app: an empty **world** (where all game
> state will live), an empty schedule of functions to run, and a window title. Nothing
> happens until `.run()`.
>
> **Toolbox — `.insert_resource(AssetServer::new(...))`:** a **resource** is a global
> singleton stored in the world, addressed by type. This one tells the engine where your
> `assets/` directory is. `concat!(env!("CARGO_MANIFEST_DIR"), "/assets")` resolves it at
> compile time relative to *your crate*, so the game runs from any working directory.
>
> **Toolbox — `.with_plugin(DefaultPlugins)`:** a **plugin** is a bundle of engine setup.
> `DefaultPlugins` installs everything a windowed game needs: the window itself, the GPU
> handshake, sprite rendering, input collection, audio, text. In chapter 6 we'll build the
> game *without* this line — that's what will make it testable.
>
> **Toolbox — `.run()`:** hands control to the engine, forever. This offends the instinct
> that *your* code should own `main`, but it's the same inversion as a web framework owning
> the accept loop while you write handlers: the loop is where all the platform misery lives
> (window events, GPU frame pacing, catching up missed ticks), and it's identical in every
> game. You write the parts that differ.

## Step 2 — something on the screen

Games put things on screen by **spawning entities** — we'll define entities properly in the
next chapter; for today, read "spawn" as "create a thing the renderer can see." Add a setup
function above `main`, and register it:

```rust,ignore
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let square = assets.load("white.png");
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(0.4, 0.9, 0.5))
            .with_size(Vec2::splat(24.0)),
        Transform2D::from_xy(-100.0, 0.0),
    ));
}
```

```rust,ignore
        .with_plugin(DefaultPlugins)
        .add_startup(setup)      // <-- new
        .run();
```

Run it again: a green square, just left of center. Three new tools:

> **Toolbox — `add_startup(fn)`:** registers a function to run **once**, before the first
> trip through the loop — the moral equivalent of your service's init code. The parameters
> of the function are its dependency injection: you declare what you need from the world
> (`Commands`, `AssetLoader`, later queries and resources) and the engine hands them to you.
> Every function you register works this way.
>
> **Toolbox — `Commands` + `spawn`:** `commands.spawn((A, B, C))` creates an entity carrying
> those pieces of data. Here: a `Sprite` (what to draw — which texture, what tint, what
> size) and a `Transform2D` (where — position, rotation, scale). The renderer draws every
> entity that has both. That's not special-cased for squares; it's how *everything* gets
> drawn, in Snake and in every Fulcrum game.
>
> **Toolbox — `AssetLoader`:** loads files from the `assets/` directory you registered in
> step 1 and returns a `Handle` — a cheap, copyable ticket for the texture. Loading happens
> once; handles get passed around.

## Step 3 — motion, and the two clocks

Now the part that makes it a *game* program and not a drawing program. Add a marker so we can
find our square, put it on the spawn, and write a function that moves it:

```rust,ignore
#[derive(Component)]
struct Patroller;
```

```rust,ignore
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(0.4, 0.9, 0.5))
            .with_size(Vec2::splat(24.0)),
        Transform2D::from_xy(-100.0, 0.0),
        Patroller,               // <-- new
    ));
```

```rust,ignore
fn patrol(mut squares: Query<&mut Transform2D, With<Patroller>>, time: Res<Time>) {
    for mut transform in &mut squares {
        let heading_right = time.tick % 240 < 120;
        let direction = if heading_right { 1.0 } else { -1.0 };
        transform.translation.x += direction * 100.0 * time.fixed_delta;
    }
}
```

```rust,ignore
        .add_startup(setup)
        .add_system(patrol)      // <-- new
        .run();
```

Run it. The square patrols: right for two seconds, left for two seconds, forever, hands off
the keyboard. You have a simulation.

> **Toolbox — `add_system(fn)`:** registers a function on the **simulation clock**: called
> every tick, 60 times a second, for the life of the program. This is the "advance the
> world" slot of the loop, and it's where all of Snake's actual game logic will live.
>
> **Toolbox — `Query<&mut Transform2D, With<Patroller>>`:** "give me the transform,
> writable, of every entity that has both a `Transform2D` and a `Patroller`." Note what the
> function *doesn't* have: a reference to the square we spawned. Systems don't hold objects;
> they ask the world for data by shape, every tick. Chapter 2 is entirely about why.
>
> **Toolbox — `Res<Time>`:** read-only access to the `Time` resource. Two fields matter
> today: `time.tick`, the number of simulation ticks since startup, and `time.fixed_delta`,
> the duration of one tick in seconds.

The two `time` fields deserve more than a bullet, because they encode the first genuinely
non-obvious design decision in game programming — one Fulcrum makes *for* you. The question:
how big is "one small slice of time" in the loop?

The obvious answer is "however long the last frame took" — measure the delta, multiply your
speeds by it, and everything moves at the same real-world rate on any machine. Most tutorials
teach this. It's also a trap that has shipped a thousand bugs: physics that explodes when a
laggy frame produces one huge step, gameplay that differs subtly between a 60 Hz office
monitor and a 144 Hz gaming one, and — worst — behavior that can never be reproduced twice,
because the sequence of deltas is different on every run. If you've fought a bug that only
happens in production under load, you know this genre of pain.

The engineering fix is the same one you'd reach for in a distributed system: **make time
discrete.** Fulcrum runs two clocks:

- The **simulation clock** ticks at exactly 60 Hz. Each **tick** advances the world by
  precisely 1/60th of a second — never more, never less. If the machine hiccups, the engine
  runs *more ticks* to catch up; each tick is still the same size. `add_system` functions
  live here.
- The **render clock** is the messy real-world one: draw a frame whenever the display wants
  one. Rendering *reads* the simulation and never writes it.

Read `patrol` again with that in mind:

- `100.0 * time.fixed_delta` means **100 units per second**, expressed honestly: a
  per-second speed times the fixed size of one tick. Never move things by bare constants per
  tick; per-second units multiplied by `fixed_delta` is the idiom.
- One free gift to notice: the square glides smoothly even though the simulation nudges it in
  60 discrete steps a second. When the render clock falls between two ticks, the engine draws
  positions *interpolated* between them. You get that precisely because the two clocks are
  separate.

### Reading `time.tick % 240 < 120`

The turnaround line deserves its own dissection, because its numbers look arbitrary until
you see that there's really only one decision inside them.

Start with `time.tick`: it's just a counter. 0 on the first tick, then 1, 2, 3… forever,
incrementing 60 times a second and never resetting — "time since startup, measured in 60ths
of a second." After one second it's 60; after a minute, 3600.

`% 240` folds that ever-growing counter into a repeating cycle. `tick % 240` can only ever
be 0 through 239, and as `tick` climbs it sweeps through that range and wraps: 0, 1, … 239,
0, 1, … Read it as "where am I within the current 240-tick (four-second) cycle?" This is
*the* standard trick for making anything periodic out of a clock that only counts up.

`< 120` splits the cycle into two halves and asks "am I in the first one?"

```text
tick % 240:   0 ............ 119 | 120 ............ 239 |  (wraps, repeats)
< 120:        true  (first half) | false (second half)  |
direction:    ──► right, 2 sec   | ◄── left, 2 sec      |
```

So the two literals aren't independent — the line contains exactly one decision: **how long
should one leg of the patrol last?** The answer is 120 ticks (2 seconds × 60 ticks/sec), and
everything else is derived from it: the full cycle is one leg right plus one leg left,
`2 × 120 = 240`, and the split point is one leg, `120`. Written the way you'd write it in
production code, the derivation is visible and there are no magic numbers left:

```rust,ignore
const TICKS_PER_SECOND: u64 = 60;
const LEG: u64 = 2 * TICKS_PER_SECOND; // how long to travel one direction

let heading_right = time.tick % (2 * LEG) < LEG;
```

Two more things the line teaches. First, it picks a **velocity**, not a position: when the
boolean flips, the square doesn't jump anywhere — the next line just starts accumulating
`fixed_delta`-sized nudges in the other direction from wherever it is. The position you see
is the running sum of every nudge so far. Second, the decision consults *simulation* time —
counting ticks, never the wall clock. 240 ticks is exactly four seconds on every machine,
every run; a game that reads the wall clock inside its logic has already given up
reproducibility, and reproducibility is the thesis of this whole track — it's what will let
us test Snake in chapter 6 like the pure function it is.

Keep the general recipe; you'll use it constantly: `period = seconds × 60`; `tick % period`
is your position in the cycle; compare against fractions of `period` to carve the cycle into
phases. Chapter 3 packages the same idea as a countdown timer.

## Checkpoint

Your `main.rs` should now match the repository's reference for this chapter,
`games/snake/examples/fz01_loop.rs`. Run the reference and compare:

```text
cargo run -p snake --example fz01_loop
```

Same patrolling square? You're done. (The reference names its window differently and orders
items differently — diff for *meaning*, not bytes.)

## What you didn't write

Everything you just didn't type — the window, the GPU handshake, the loop, tick catch-up,
interpolation, input plumbing, image decoding — is the engine, and `DefaultPlugins` installed
all of it. An engine is not a framework for games so much as *the parts of every game that
are the same game*. Fulcrum's particular opinions (fixed 60 Hz ticks, state in one world,
determinism) exist so the parts you *do* write stay small and testable. You'll feel each
opinion earn its keep as Snake grows.

Your Fulcrum vocabulary after one chapter:

| Tool | What it's for |
| --- | --- |
| `Fulcrum::new(title)` / `.run()` | build the app; surrender to the loop |
| `DefaultPlugins` | window, renderer, input, audio — the whole engine |
| `insert_resource` / `Res<T>` | global singletons in the world, by type |
| `add_startup(fn)` | run once, before the first tick |
| `add_system(fn)` | run every simulation tick (60 Hz) |
| `Commands::spawn` | create an entity from pieces of data |
| `Sprite` + `Transform2D` | what to draw + where; the renderer draws the pair |
| `Query<...>` | ask the world for data by shape |
| `time.tick` / `time.fixed_delta` | simulation time: count of ticks / size of one |

## Exercises

Every chapter ends with a few of these, done in *your* crate. The verification step is always
the same: run it and watch. No solutions are provided, on purpose; being briefly stuck is the
lesson.

1. **More squares, same system.** In `setup`, spawn two more `Patroller` squares at different
   `y` positions (and, if you like, different colors). Don't touch `patrol` — then explain to
   yourself why you didn't have to. That query was never about *the* square.
2. **Change the rhythm.** Make the patrol turn around every second instead of every two.
   Conceptually one number changes — the leg length — but it appears in the code as *two*
   literals. Before you edit, predict what each wrong single edit would do: what does
   `% 120 < 120` make the square do? What about `% 240 < 60`? Then make the right change,
   and check at least one of your predictions by running it — a lopsided patrol is worth
   seeing once.
3. **Accelerate (harder).** Make the square's speed grow over time — say, 40 units/second
   plus 10 more for every elapsed second — while keeping the two-second turnaround. Everything
   you need is already in the system: `time.tick` for "how long has it been," `fixed_delta`
   for "how big is this step." If your square teleports or crawls, you've mixed the two up —
   which is exactly the mistake this chapter exists to inoculate against.

Next: the square becomes a snake — which turns out to mean almost nothing on screen and
everything in how we think about *state*.
