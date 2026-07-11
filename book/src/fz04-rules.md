# Rules: Making It a Game

A toy becomes a game when something can go wrong. This chapter adds everything that can:
apples worth chasing, growth that turns your own body into the hazard, walls that end the
run, and the restart. It also faces the first question every game asks its programmer within
an hour: *where does randomness come from?* The answer is the most engineer-brained idea in
this whole track.

From here on we work in the real crate — the simulation now lives in
`games/snake/src/game.rs`, which this chapter walks through. Run it with the bare view:

```text
cargo run -p snake --example fz04_rules
```

## Collision on a grid is equality

The reason this track builds Snake and not a platformer: on a grid, every physical question
becomes trivial. The `step` system computes `next`, the cell the head is about to enter, and
then interrogates it:

```rust,ignore
// Rule 1: walls end the run. Two comparisons, not geometry.
let out_of_bounds = next.0 < 0 || next.0 >= GRID_W || next.1 < 0 || next.1 >= GRID_H;

// Rule 2: so does biting yourself...
let tail = *snake.body.back().expect("non-empty");
let bites_self = snake.occupies(next) && !(next == tail && snake.grow == 0);

if out_of_bounds || bites_self {
    *state = SnakeState::GameOver;
    ended.write(RunEnded { won: false });
    return;
}
```

Rule 2 hides a classic off-by-one that's worth savoring, because it's *rules* logic, not
code logic: moving into the cell your own tail currently occupies is usually **legal** — the
tail vacates it in the same step. Unless the snake just ate, in which case the tail stays
put this step, and that same move kills you. Players of every Snake clone have felt the
difference between getting this right and wrong without ever being able to say what it was.
Rules live in the corner cases; write them deliberately, and (chapter 6) test them.

Eating is the same equality check against the apple's cell — and growing is delightfully
lazy. We don't insert segments; we *owe* them:

```rust,ignore
snake.body.push_front(next);
if snake.grow > 0 {
    snake.grow -= 1;      // digest: skip one tail-pop, and the body gets longer
} else {
    snake.body.pop_back();
}
```

Eat an apple, `grow += 2`, and for the next two steps the tail simply doesn't retreat. No
special cases, no inserted cells. When a mechanic can be expressed as *briefly not doing
something you normally do*, take the deal.

## Randomness you can regress-test

The next apple must appear somewhere "random." Reach for your usual tool — some global
`rand()` seeded by the clock — and you've just built a program whose behavior can never be
reproduced: every bug report becomes "it happened once." Games solved this decades ago, the
same way you'd solve it for any system you wanted reproducible: **all gameplay randomness
flows from one seeded generator.** Fulcrum provides it as the `SimRng` resource:

```rust,ignore
fn free_cell(snake: &Snake, taken: &[Cell], rng: &mut SimRng) -> Cell {
    loop {
        let cell = (rng.range_i32(0..GRID_W), rng.range_i32(0..GRID_H));
        if !snake.occupies(cell) && !taken.contains(&cell) {
            return cell;
        }
    }
}
```

Same seed → same sequence of draws → the apples appear in the same places, in the same
order, every run. The player still can't predict them; the *test suite* can. "Random" was
never the requirement — *unpredictable to the player* was, and a seeded stream delivers that
while staying a pure function of its history. (The rejection-sampling loop is fine: the
board is mostly empty, so it nearly always exits on the first draw. And note the fussy
`taken` parameter — the head has already moved onto the eaten apple's cell this step, and
without it the new apple could legally spawn *under the snake's chin*. Corner cases,
again.)

This is also the moment to name the discipline `SimRng` belongs to, because it's bigger than
randomness. The simulation may consume only: its own state, this tick's `Input`, `SimRng`,
and the tick clock. Not the wall clock, not the frame rate, not a thread's whims. Keep that
contract and the whole game remains a deterministic function of `(seed, input history)` —
which is the property chapter 6 cashes in. Fulcrum can't stop you from calling
`SystemTime::now()` in a system; it can only promise you'll regret it.

## Modes, as an enum

Snake has phases — playing, dead, (theoretically) victorious — and things behave differently
in each. The idiom is almost embarrassingly plain: an enum in a resource, checked at the top
of systems.

```rust,ignore
#[derive(Resource, Default, PartialEq, Eq)]
enum SnakeState { #[default] Playing, GameOver, Won }

// in steer and step:
if *state != SnakeState::Playing { return; }
```

You may feel the pull toward something grander — a state-pattern hierarchy, transition
tables. Resist it at this scale. An enum plus early returns *is* a state machine, the
compiler exhaustively checks every `match` over it, and any system can read it. Grove's
dungeon-crawling sibling runs on the same idiom. When a game genuinely outgrows it (nested
menus, pause-inside-cutscene), you'll know, and you'll reach for real machinery then — the
mistake is paying for it now.

The restart system completes the loop of play: on Enter, when not `Playing`, put back
`Snake::default()`, reset the timer and score, despawn the apples, spawn a fresh one. Notice
what it does **not** touch: the sprites. It resets *state*; the projection from chapter 2
notices the three-segment snake and prunes the views on the next frame, unprompted. This is
where that architecture starts paying rent — resetting a game whose visuals are a projection
is trivial, while resetting one whose visuals are hand-managed is a bug factory ("the old
segments are still on screen after restart" is a bug you will simply never have).

## What the sim announces, without knowing who listens

One more piece rode along in the code above: `ended.write(RunEnded { won: false })` — and
its sibling `eaten.write(AppleEaten(next))`. These are **events**, the ECS's pub-sub
channel: the simulation announces *facts*, and any number of systems — this chapter: none!
— may subscribe. Writing to a channel nobody reads yet looks like over-engineering; it's
actually the seam the next chapter is built on. Sound, score popups, screen shake: all of it
will attach to these two lines without the simulation changing at all. The sim speaks;
whether anything is listening is not its business.

The game is now complete — playable, losable, restartable, honest. It's also mute, colorless
about its feelings, and doesn't show the score. Everything missing is *presentation*, and
presentation is a different kind of code with different rules. That split is next.
