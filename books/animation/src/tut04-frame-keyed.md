# Gameplay on Frames

This is the chapter the book's introduction promised: *"the sword connects on the exact tick
the attack clip shows its extension frame"* — as working code. The dummy returns, fights
back, and the whole thing becomes a game. Along the way you'll restructure the crate the way
every Fulcrum game is structured, because chapter 5 needs the simulation to stand alone.

## The idea, before the code

Look at the attack animation's four frames again (chapter 1's `hero.json`): windup, sweep,
**extension**, follow-through. In most engines, "did the attack hit?" is decided when the
button was pressed, or after a hand-tuned `0.18` seconds — and the animation is a loosely
correlated movie played over the top. When they drift, players say the game feels "floaty"
or "unfair" and can't say why.

Fulcrum's animation runs on the simulation clock, so there's a better option: **the
animation is the timing.** The blade is out on frame 2 of the clip; therefore frame 2 *is*
the hitbox window. If the artist retimes the windup from 120 ms to 200 ms, the attack
genuinely lands later — art and gameplay cannot drift, because they are the same data.

Two honest patterns exist in Fulcrum for sim/animation coupling, and this book wants you to
know you're choosing one:

- **Animation as decoration** (the dungeon game): gameplay keeps its own timers, *tells*
  the animator facts, never reads back. Sims stay runnable with animators entirely absent.
  Right choice when animation is garnish on mechanics that must stand alone.
- **Animation as gameplay** (the dojo, this chapter): frame indices and machine states are
  simulation facts that rules read. Richer feel, one obligation: the animation data must
  load headless — which Fulcrum's loaders do (clip timing needs no GPU; chapter 5 leans on
  this hard).

## Step 1 — give the simulation its own file

Same split as every Fulcrum game, same reason: chapter 5 deletes the window. Create
`games/my-dojo/src/lib.rs`:

```rust,ignore
pub mod game;
```

Create `src/game.rs` and move the *simulation* in — starting with the constants that name
this chapter's whole idea:

```rust,ignore
use fulcrum::prelude::*;

/// Arena size in world units (letterboxed by the camera; 16 px art at 1:1).
pub const ARENA_W: f32 = 320.0;
pub const ARENA_H: f32 = 180.0;

/// Hero walk speed, world units per second.
pub const HERO_SPEED: f32 = 90.0;
/// The attack clip's frame indices are gameplay constants: the blade is out on this frame.
pub const STRIKE_FRAME: usize = 2;
/// How far the extension frame reaches.
pub const STRIKE_RANGE: f32 = 26.0;
/// The dummy's hit clip swings back toward you on this frame.
pub const REBOUND_FRAME: usize = 1;
/// Stand closer than this during the rebound and you get bonked.
pub const REBOUND_RANGE: f32 = 16.0;
```

Then the nouns:

```rust,ignore
/// The player. Facing is simulation state (the strike checks it); the sprite flip that
/// *shows* it lives in the presentation.
#[derive(Component)]
pub struct Hero {
    pub facing_left: bool,
    /// Has the current swing already connected? One swing, one bonk.
    pub swing_connected: bool,
}

/// The training dummy.
#[derive(Component)]
pub struct Dummy {
    /// Has the current wobble already bonked the hero back?
    pub rebounded: bool,
}

/// Scorekeeping: hits you landed on the dummy, hits its rebound landed on you.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bonks {
    pub landed: u32,
    pub taken: u32,
}
```

Notice `facing_left` moved *into* the sim (chapter 2 flipped the sprite directly). The
strike needs to know which way the hero faces — the moment presentation-ish data gets read
by a rule, it's simulation state, and the sprite flip becomes a projection of it.

## Step 2 — the plugin and the spawn

```rust,ignore
/// Installs the whole simulation. Requires `AnimPlugin` (part of `DefaultPlugins`;
/// headless tests add it explicitly) — the animation system *is* gameplay here.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Bonks::default());
        app.add_systems(Startup, spawn_dojo);
        app.add_systems(FixedUpdate, (control_hero, strike, rebound).chain());
    }
}

/// Load the state machines and spawn both fighters. `AnimatorLoader` works headless — clip
/// timing and regions are simulation data; only texture upload needs a GPU — so this exact
/// startup runs in the windowed game and in tests.
fn spawn_dojo(mut commands: Commands, mut animators: AnimatorLoader) {
    let hero_machine = animators
        .load("anim/hero.animsm.ron")
        .expect("hero machine loads");
    let dummy_machine = animators
        .load("anim/dummy.animsm.ron")
        .expect("dummy machine loads");

    commands.spawn((
        Hero {
            facing_left: false,
            swing_connected: false,
        },
        Sprite::from_sheet(Handle::INVALID, 0).with_z(2.0),
        Transform2D::from_xy(ARENA_W * 0.3, ARENA_H * 0.5),
        Animator::new(hero_machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
    commands.spawn((
        Dummy { rebounded: false },
        Sprite::from_sheet(Handle::INVALID, 0).with_z(1.0),
        Transform2D::from_xy(ARENA_W * 0.75, ARENA_H * 0.5),
        Animator::new(dummy_machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
}
```

The dummy runs a machine of its own — you copied `anim/dummy.animsm.ron` in chapter 1, and
it's four lines of interesting: one `hit` trigger, two states, `on_finish` home. A training
dummy is about the smallest state machine that earns its file.

`control_hero` is chapter 3's control system, moved here, with three upgrades you can make
yourself before checking against the shipped `games/dojo/src/game.rs`: it clamps the
position to the arena, it writes `hero.facing_left` instead of touching the sprite, and its
attack trigger stays gated on `free`. (The sprite-flipping moves to `main.rs` in step 4.)

## Step 3 — the strike and the rebound

The heart of the chapter — read the conditions in order, because together they *are* the
sentence this book promised:

```rust,ignore
/// The frame-keyed hitbox. Not "when Space was pressed", not "after 0.2 seconds" — the
/// strike connects on the exact tick the attack clip shows its extension frame.
#[allow(clippy::type_complexity)] // standard ECS system shape
pub fn strike(
    mut heroes: Query<(&Transform2D, &mut Hero, &Animator, &AnimationPlayer)>,
    mut dummies: Query<(&Transform2D, &mut Animator), (With<Dummy>, Without<Hero>)>,
    mut bonks: ResMut<Bonks>,
) {
    for (at, mut hero, animator, player) in &mut heroes {
        if animator.state() != "attack" {
            hero.swing_connected = false;
            continue;
        }
        if player.frame_index != STRIKE_FRAME || hero.swing_connected {
            continue;
        }
        for (dummy_at, mut dummy_animator) in &mut dummies {
            let delta = dummy_at.translation - at.translation;
            let in_front = if hero.facing_left {
                delta.x < 0.0
            } else {
                delta.x > 0.0
            };
            if in_front && delta.length() <= STRIKE_RANGE {
                dummy_animator.trigger("hit");
                hero.swing_connected = true;
                bonks.landed += 1;
            }
        }
    }
}
```

Walk the conditions:

- **`animator.state() != "attack"`** — outside the attack there is no hitbox; and passing
  through here is where the swing bookkeeping resets.
- **`player.frame_index != STRIKE_FRAME`** — the load-bearing line. `frame_index` is plain
  component data, advanced on the fixed clock; comparing it to a constant *is* the hitbox
  window. The window is exactly as long as the artist made frame 2 (70 ms → 4 ticks).
- **`hero.swing_connected`** — the window is 4 ticks long and this system runs every tick;
  without the flag, one swing lands four hits. Every frame-keyed mechanic needs an
  edge-guard like this, same instinct as `just_pressed` versus `pressed`.
- The hit itself is *a trigger on the dummy's machine* — the two machines talk through the
  same param interface gameplay uses. And `Without<Hero>` on the dummy query is how the
  two-animator system convinces the borrow checker the queries are disjoint.

The rebound is the same shape pointed the other way — the *dummy's* frames are the hitbox
now. Its hit clip tilts away (frame 0), swings back through center (frame 1: the rebound),
and settles (frame 2). Stand inside the arc on the return frame and you get bonked:

```rust,ignore
#[allow(clippy::type_complexity)] // standard ECS system shape
pub fn rebound(
    mut dummies: Query<(&Transform2D, &mut Dummy, &Animator, &AnimationPlayer)>,
    mut heroes: Query<(&Transform2D, &mut Animator), (With<Hero>, Without<Dummy>)>,
    mut bonks: ResMut<Bonks>,
) {
    for (at, mut dummy, animator, player) in &mut dummies {
        if animator.state() != "hit" {
            dummy.rebounded = false;
            continue;
        }
        if player.frame_index != REBOUND_FRAME || dummy.rebounded {
            continue;
        }
        for (hero_at, mut hero_animator) in &mut heroes {
            if hero_at.translation.distance(at.translation) <= REBOUND_RANGE {
                hero_animator.trigger("hurt");
                dummy.rebounded = true;
                bonks.taken += 1;
            }
        }
    }
}
```

Now chapter 3's dormant details wake up. The `hurt` trigger finally has a sender — and the
hero is usually *mid-attack* when it arrives, which is exactly why `Any → hit` was declared
above `Any → attack`: interruptions beat intentions, by line order, and you tuned that file
two chapters before you needed it.

## Step 4 — the presentation half

`main.rs` shrinks to what a viewer needs: config, camera, a tatami floor, the sprite flip,
and a HUD. Two pieces are worth typing attentively; the rest, take from the shipped
`games/dojo/src/main.rs`.

The flip — facing is sim state, the flip is its projection, one line each way:

```rust,ignore
fn flip_faces(mut heroes: Query<(&Hero, &mut Sprite)>) {
    for (hero, mut sprite) in &mut heroes {
        if sprite.flip_x != hero.facing_left {
            sprite.flip_x = hero.facing_left;
        }
    }
}
```

And the HUD's second line — the best animation debugging tool in this book, four format
specifiers long:

```rust,ignore
        let line = |(animator, player): (&Animator, &AnimationPlayer)| {
            format!("{} #{}", animator.state(), player.frame_index)
        };
```

Register both with `add_frame_system`, wire `use my_dojo::game::{...}` and
`.with_plugin(GamePlugin)`, and run:

```text
cargo run -p my-dojo
```

Walk to the dummy. Swing while watching the state line: `attack #0`, `#1`, and the *instant*
`#2` appears, the dummy tilts and `landed` ticks up. Swing and stand your ground: the
dummy's `hit #1` arrives and suddenly you're the one flinching. Then swing from a step
further back and watch the same `#2` pass with no bonk — window, range, and facing, all
legible on screen.

## Checkpoint

Your crate should now match `games/dojo` — the shipped game, file for file
(`cargo run -p dojo` to compare feel). The state readout should make the following sentence
feel obvious in a way it didn't at the top of the chapter: *the animation isn't playing
over the game; the animation is the game's clock for these mechanics.*

## Exercises

1. **Retime the attack.** In `hero.json`, stretch the windup (frame 10) from 120 ms to
   300 ms and rerun. The attack now telegraphs — and lands later, with zero gameplay edits,
   because the timing *is* the data. Tune the four durations until the swing feels punchy
   again; you're doing game-feel work in an art file, which is the whole point.
2. **Sweep hits too.** Make the sweep frame (index 1) also count as a hit window, at a
   shorter range — `matches!(player.frame_index, 1 | 2)` plus a per-frame range. Decide
   what happens to `swing_connected` (can one swing now hit on both frames? should it?).
3. **A second dummy (harder).** Spawn another dummy nearer the wall and make one swing able
   to bonk both. The inner loop already handles it — but the rebound now has two potential
   senders, and the hero's `hurt` trigger can fire twice in a tick. Verify (HUD, or a
   `dbg!`) that the machine handles the double-trigger gracefully, and explain why
   (chapter 3's evaluation rules say).

One promise left from the introduction: proving all of this — the exact frames, the exact
ticks — in tests that run without a window. That's next, and it's short, because you've
already done the hard part.
