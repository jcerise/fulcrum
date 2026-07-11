# Time and Input

The snake is data; now the data changes. This chapter earns Snake's two verbs — *move* and
*turn* — and both turn out to be about something deeper than they look: movement is about
owning your own clock, and turning is about the gap between what a player pressed and what
they meant.

```text
cargo run -p snake --example fz03_move
```

WASD or arrows to steer. The edges wrap for now — dying is a *rule*, and rules are the next
chapter's job. This chapter is pure mechanics.

## The snake's own clock

Chapter 1 established the simulation ticks at 60 Hz. But Snake obviously doesn't move 60
cells a second — classic Snake *steps*, chunkily, several times a second, and speeds up as
you score. So the game keeps its own subdivided clock, as plain data:

```rust,ignore
struct StepTimer {
    every: u32,      // ticks between movement steps: 8 = 7.5 steps/second
    countdown: u32,  // ticks until the next one
}

fn step(mut snake: ResMut<Snake>, mut timer: ResMut<StepTimer>) {
    timer.countdown -= 1;
    if timer.countdown > 0 {
        return;                      // a tick passed; nothing visible happened
    }
    timer.countdown = timer.every;
    // ... one movement step ...
}
```

The system runs every tick; it *does something* every eighth tick. This pattern — a countdown
in a resource, decremented on the fixed clock — is how games express every "every N"
behavior: attack cooldowns, spawn waves, animation cadence. Two properties are worth making
explicit, because they're why the pattern is idiomatic rather than incidental:

- **The speed is data, not code.** `every: 8` sits in a resource, so the rule "eating makes
  the game faster" (next chapter) is one assignment, and a test can crank it without touching
  the systems.
- **It counts ticks, not seconds.** `every: 8` is *exactly* 8/60ths of a second, every time.
  The equivalent wall-clock version (`if now - last_step > 133ms`) drifts, jitters across
  machines, and quietly destroys the reproducibility we're saving up for chapter 6.

The movement itself is the `VecDeque` doing what it was chosen for:

```rust,ignore
let head = *snake.body.front().expect("non-empty");
let next = (
    (head.0 + snake.dir.0 + GRID_W) % GRID_W,   // wrap; the +GRID keeps negatives in range
    (head.1 + snake.dir.1 + GRID_H) % GRID_H,
);
snake.body.push_front(next);
snake.body.pop_back();
```

Push a head, pop a tail. No segment "follows" any other — the middle of the snake never
moves at all. If you've ever implemented a queue, you have already implemented Snake.

## Input: polling, not callbacks

Your instinct from every other kind of program is `on_key_down(handler)`. Games don't do
callbacks, and the reason is the loop: a callback fires *between* the moments the simulation
cares about, so the handler can only stash a note for later anyway. Fulcrum embraces that:
the engine collects raw OS events, and once per tick — right before your systems run — it
folds them into an `Input` resource that systems simply *read*:

```rust,ignore
if input.pressed(Key::W) { ... }        // is it held down right now?
if input.just_pressed(Key::W) { ... }   // did it go down THIS tick?
```

That distinction — **level versus edge**, if you've done embedded work — matters immediately.
Movement in Grove uses `pressed` (hold to walk). Turning in Snake must be `just_pressed`: a
turn is an event, and a held key shouldn't queue forty turns. Getting level/edge wrong is the
most common first input bug in any engine; when something fires every frame that should fire
once, this is why.

There's a subtler guarantee hiding in "folded once per tick": every system in a tick sees the
*same* input snapshot, and the snapshot sequence is itself well-defined data. A recorded list
of per-tick snapshots literally *is* a recording of the game — which is how the replay system
in [Part II](ch14-replays.md) works, and how our tests will fake a player in chapter 6.
Design decisions this small are load-bearing.

## The turn buffer: input is intent, not command

Here's the moment this chapter has been building toward, and it's a genuine game-design
lesson wearing a data-structure costume. The naive implementation of turning:

```rust,ignore
if input.just_pressed(Key::W) { snake.dir = (0, 1); }
```

Play with that for thirty seconds and you'll die to a bug you can't quite name. The snake
steps every 8 ticks; a decent player's fingers are faster. Tap "up, then left" between two
steps: the first assignment sets `dir` up, the second immediately overwrites it left — the
up never happens. Worse: moving right, tap "up, left" — `dir` ends up `(-1, 0)`, a 180°
reversal into your own neck. Instant death, and the player is *certain* the game ate their
input. They're right. It did.

The fix is a queue of *intents*, drained one per movement step:

```rust,ignore
fn steer(input: Res<Input>, mut snake: ResMut<Snake>) {
    for (key, dir) in PRESSES {
        if !input.just_pressed(key) { continue; }
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
apart, exactly as the player meant. The cap of two keeps a panicking player from wedging
five turns of stale intent into the future.

This is the pattern beneath a lot of "game feel": **input is intent, arriving on the render
clock's schedule; the simulation consumes it on its own.** Fighting-game combo buffers,
jump-a-few-frames-early forgiveness in platformers — same idea, different costumes. Players
never see it. They just report that the controls feel *right*.

Finally, order. Two systems now write `Snake`, and it matters which goes first:

```rust,ignore
.add_system((steer, step).chain())
```

`chain()` says: in this order, every tick. Intent is read, then the world advances. Being
explicit costs six characters; being implicit costs an afternoon someday.

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
nothing can go wrong, and nothing is at stake. Next chapter we add the apple, the growth,
and the dying. That's where the game lives.
