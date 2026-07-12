# Time and Input

The snake is data; now the data changes. This chapter earns Snake's two verbs — *move* and
*turn* — and both turn out to be about something deeper than they look: movement is about
owning your own clock, and turning is about the gap between what a player pressed and what
they meant. You'll build a bug on purpose, feel it, and then fix it properly.

For now the edges of the board will wrap — dying is a *rule*, and rules are the next
chapter's job. This chapter is pure mechanics.

## Step 1 — state for a moving snake

A moving snake knows more than a posed one: which way it's going, and which turns are
pending. Extend the resource, and add a second one for timing:

```rust,ignore
#[derive(Resource)]
struct Snake {
    body: VecDeque<Cell>,
    /// Direction of travel as a cell delta: (1, 0) is rightward.
    dir: Cell,
    /// Turns waiting to happen: at most two, applied one per movement step.
    queued: VecDeque<Cell>,
}

/// The snake's own clock: one movement step every `every` simulation ticks. The simulation
/// runs at 60 Hz; the snake doesn't have to.
#[derive(Resource)]
struct StepTimer {
    every: u32,
    countdown: u32,
}
```

Update `main` to match — the S-shape shrinks to a plain three-segment snake (it's about to
move; the pose doesn't matter anymore), and both resources get inserted:

```rust,ignore
        .insert_resource(Snake {
            body: VecDeque::from([(12, 9), (11, 9), (10, 9)]),
            dir: (1, 0),
            queued: VecDeque::new(),
        })
        .insert_resource(StepTimer {
            every: 8,
            countdown: 8,
        })
```

Ignore `queued` for a few minutes — it's the payoff of step 3.

## Step 2 — movement, on the snake's own clock

Chapter 1 established that the simulation ticks at 60 Hz. But Snake obviously doesn't move 60
cells a second — classic Snake *steps*, chunkily, several times a second, and speeds up as
you score. That's what `StepTimer` is for. Type the movement system:

```rust,ignore
/// Advance the world. Runs every tick; *does something* every `every` ticks.
fn step(mut snake: ResMut<Snake>, mut timer: ResMut<StepTimer>) {
    timer.countdown -= 1;
    if timer.countdown > 0 {
        return;
    }
    timer.countdown = timer.every;

    if let Some(turn) = snake.queued.pop_front() {
        snake.dir = turn;
    }
    let head = *snake.body.front().expect("non-empty");
    // Wrap at the edges — the `+ GRID` before `%` keeps negative numbers in range.
    let next = (
        (head.0 + snake.dir.0 + GRID_W) % GRID_W,
        (head.1 + snake.dir.1 + GRID_H) % GRID_H,
    );
    // Moving IS this: new head on, tail off. The middle never changes.
    snake.body.push_front(next);
    snake.body.pop_back();
}
```

Register it and run:

```rust,ignore
        .add_startup(setup)
        .add_system(step)        // <-- new; we'll revisit this line in step 4
        .add_frame_system(project_snake)
```

The snake slides right, exits stage right, re-enters stage left. Two things in that function
are idioms you'll reuse in every game you ever write:

**The countdown pattern.** The system runs every tick; it *does something* every eighth
tick. A countdown in a resource, decremented on the fixed clock, is how games express every
"every N" behavior — attack cooldowns, spawn waves, animation cadence. Two properties make
it the idiom rather than an accident:

- **The speed is data, not code.** `every: 8` sits in a resource, so the rule "eating makes
  the game faster" (next chapter) will be one assignment, and a test can crank it without
  touching any system.
- **It counts ticks, not seconds.** `every: 8` is *exactly* 8/60ths of a second, every time.
  The wall-clock version (`if now - last_step > 133ms`) drifts, jitters across machines, and
  quietly destroys the reproducibility we're saving up for chapter 6.

**Movement as queue surgery.** Push a head, pop a tail. No segment "follows" any other — the
middle of the snake never moves at all. This is the `VecDeque` from chapter 2 doing exactly
what it was chosen for; if you've ever implemented a queue, you had already implemented
Snake. And notice what you did *not* touch: `project_snake` is drawing the moving snake
without a single edit, because it renders state, not changes.

## Step 3 — input, the naive way (build the bug)

Your instinct from every other kind of program is `on_key_down(handler)`. Games don't do
callbacks, and the reason is the loop: a callback fires *between* the moments the simulation
cares about, so a handler could only stash a note for later anyway. Fulcrum embraces that:
the engine collects raw OS events, and once per tick — right before your systems run — it
folds them into an `Input` resource that systems simply *read*.

> **Toolbox — `Res<Input>`:** the tick's snapshot of the keyboard and mouse. Two calls
> matter today: `input.pressed(key)` — is it held down right now? — and
> `input.just_pressed(key)` — did it go down *this tick*? If you've done embedded work,
> that's **level versus edge**. Movement in most games uses `pressed` (hold to walk);
> turning in Snake must be `just_pressed`: a turn is an event, and a held key shouldn't
> queue forty turns. When something fires every tick that should fire once, you've mixed
> these up — it's the most common first input bug in any engine.
>
> There's a subtler guarantee hiding in "folded once per tick": every system in a tick sees
> the *same* input snapshot, and the sequence of snapshots is itself well-defined data. A
> recorded list of them literally *is* a recording of the game — which is how the replay
> system in [Part II](ch14-replays.md) works, and how our tests will fake a player in
> chapter 6.

Now type the *obvious* steering system — which is wrong, instructively:

```rust,ignore
fn steer(input: Res<Input>, mut snake: ResMut<Snake>) {
    if input.just_pressed(Key::W) { snake.dir = (0, 1); }
    if input.just_pressed(Key::S) { snake.dir = (0, -1); }
    if input.just_pressed(Key::A) { snake.dir = (-1, 0); }
    if input.just_pressed(Key::D) { snake.dir = (1, 0); }
}
```

Register it alongside `step` (`.add_system(steer)`), run, and play for thirty seconds.
Really do this — the next section lands differently once you've felt it. Try a fast
"up, then left" between two movement steps. You'll die to a bug you can't quite name.

Here's its name. The snake steps every 8 ticks; a decent player's fingers are faster. Tap
"up, left" between two steps: the first assignment sets `dir` up, the second immediately
overwrites it left — the up never happens. Worse: while moving right, that same quick pair
can leave `dir = (-1, 0)` — a 180° reversal straight into your own neck. Instant death, and
the player is *certain* the game ate their input. They're right. It did.

## Step 4 — the turn buffer: input is intent, not command

The fix is the `queued` field from step 1: a queue of *intents*, drained one per movement
step (that's the `pop_front` already sitting in `step`). Replace `steer` entirely:

```rust,ignore
/// Read intent. The reversal check compares against the direction that will be in effect
/// *when this turn applies*, not the current one.
fn steer(input: Res<Input>, mut snake: ResMut<Snake>) {
    let presses = [
        (Key::W, (0, 1)),
        (Key::Up, (0, 1)),
        (Key::S, (0, -1)),
        (Key::Down, (0, -1)),
        (Key::A, (-1, 0)),
        (Key::Left, (-1, 0)),
        (Key::D, (1, 0)),
        (Key::Right, (1, 0)),
    ];
    for (key, dir) in presses {
        if !input.just_pressed(key) {
            continue;
        }
        let against = *snake.queued.back().unwrap_or(&snake.dir);
        let reversal = dir.0 == -against.0 && dir.1 == -against.1;
        if !reversal && dir != against && snake.queued.len() < 2 {
            snake.queued.push_back(dir);
        }
    }
}
```

Read the `against` line twice; it's the whole trick. A new turn is validated not against the
direction the snake is moving *now*, but against the last turn *already in the queue* — the
direction that will be true when this turn actually applies. "Up, then left" while moving
right: up queues (legal vs. right), left queues (legal vs. *up*). Both happen, one step
apart, exactly as the player meant. The cap of two keeps a panicking player from wedging five
turns of stale intent into the future.

This is the pattern beneath a lot of "game feel": **input is intent, arriving on the
player's schedule; the simulation consumes it on its own.** Fighting-game combo buffers,
jump-a-few-frames-early forgiveness in platformers — same idea, different costumes. Players
never see it. They just report that the controls feel *right*.

One more thing before you run it. Two systems now write `Snake`, and it matters which goes
first. Replace the two `add_system` lines with one:

```rust,ignore
        .add_system((steer, step).chain())
```

> **Toolbox — `.chain()`:** registering a tuple of systems with `.chain()` fixes their
> order: `steer` then `step`, every tick. Intent is read, then the world advances. Without
> it, the engine is free to order them however it likes — fine for independent systems,
> quietly wrong for these two. Being explicit costs six characters; being implicit costs an
> afternoon someday.

## Checkpoint

```text
cargo run -p my-snake
```

WASD or arrows. The snake should turn crisply, take a fast double-turn correctly (tap two
directions quickly — both apply, one step apart), refuse to reverse into itself, and wrap at
the edges. The reference is `games/snake/examples/fz03_move.rs`
(`cargo run -p snake --example fz03_move`).

New vocabulary this chapter:

| Tool | What it's for |
| --- | --- |
| `Res<Input>` | this tick's keyboard/mouse snapshot |
| `pressed` vs `just_pressed` | level (held now) vs edge (went down this tick) |
| `Key::W`, `Key::Up`, … | key identifiers, in the prelude |
| `(a, b).chain()` | run these systems in exactly this order |
| the countdown pattern | "every N ticks" behavior, as data on the fixed clock |

## Exercises

1. **Sprint.** While Space is *held*, make the snake step every 4 ticks instead of 8. This is
   deliberately a `pressed` (level) problem right after a chapter full of `just_pressed`
   (edge) reasoning — if you reach for the wrong one, you'll feel it immediately. Decide
   whether sprint belongs in `steer` or `step`, and defend the choice.
2. **Feel the buffer.** Change the queue cap from 2 to 0 (turns overwrite `snake.dir`
   directly) and play for a minute — try a quick up-then-left. Then set the cap to 5 and whip
   the keys around. Name what's wrong with each before restoring 2: one loses intent, the
   other honors *stale* intent. Game feel lives in exactly this kind of number.
3. **Order matters (harder).** Swap the chain to `(step, steer)` and describe precisely what
   changed — not "it feels worse": trace a keypress through the tick. (Answer shape: a turn
   pressed on the same tick as a movement step now applies one step later.) Then consider:
   why didn't the compiler, or the engine, catch this? What *could* catch it? Hold that
   thought until chapter 6.

The snake moves, turns crisply, and wraps politely off the edges. It's a toy, not a game —
nothing can go wrong, and nothing is at stake. Next chapter we add the apple, the growth, and
the dying. That's where the game lives.
