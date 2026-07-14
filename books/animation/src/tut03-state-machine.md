# The State Machine

Chapter 2 ended with a diagnosis: clip-flow logic is data-shaped and doesn't belong braided
through gameplay. This chapter performs the extraction. The behavior stays identical — walk,
run, swing — but by the end, your control system never mentions a clip again, and the rules
live in a file you can tune without recompiling.

## Step 1 — read the machine you already copied

You already have the file — `assets/anim/hero.animsm.ron` came along in chapter 1's copy.
Open it:

```ron
StateMachine(
    initial: "idle",
    params: { "speed": Float(0.0), "attack": Trigger, "hurt": Trigger },
    states: {
        "idle":   (clip: "hero.json#idle"),
        "run":    (clip: "hero.json#run"),
        "attack": (clip: "hero.json#attack", on_finish: "idle"),
        "hit":    (clip: "hero.json#hit", on_finish: "idle"),
    },
    transitions: [
        // Declaration order is priority order: getting bonked beats swinging.
        (from: Any,           to: "hit",    when: [Triggered("hurt")]),
        (from: Any,           to: "attack", when: [Triggered("attack")]),
        (from: State("idle"), to: "run",    when: [Gt("speed", 1.0)]),
        (from: State("run"),  to: "idle",   when: [Lt("speed", 1.0)]),
    ],
)
```

Read it against chapter 2's rule list — every rule you hand-rolled is one line here:

- **`states`** name the machine's vocabulary, each bound to a clip by `"file.json#tag"`
  reference. Note `hit` is already here: the state you added painfully in chapter 2's
  exercise 3 costs one line, plus one transition.
- **`on_finish`** is "a finished attack hands back to idle" — the `finished()` polling you
  wrote, as a field.
- **`params`** are the machine's entire input surface: a float, and two **triggers** —
  one-tick pulses, made for "this just happened" facts like *attack pressed*.
- **`transitions`** are the rules. `from: State(..)` rules apply in one state;
  `from: Any` rules apply from every state. Conditions in a `when` list are AND-ed;
  evaluation is: `Any` rules first, then the current state's, in declaration order, first
  match wins, checked every tick. So `hit` beating `attack` isn't a comment — it's the
  line order.

One deliberate absence: nothing here says *when* speed changes or *what makes* `hurt` fire.
The machine doesn't know about keyboards or dummies. It maps facts to appearances; producing
the facts remains gameplay's job.

## Step 2 — swap it in

Three edits to `main.rs`. First, `setup` loads one machine instead of stashing three clips —
`Art` and its insert are deleted entirely:

```rust,ignore
fn setup(mut commands: Commands, mut animators: AnimatorLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);

    let machine = animators
        .load("anim/hero.animsm.ron")
        .expect("hero machine loads");
    commands.spawn((
        HeroTag,
        Sprite::from_sheet(Handle::INVALID, 0).with_z(1.0),
        Transform2D::from_xy(160.0, 90.0),
        Animator::new(machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
}
```

> **Toolbox — `AnimatorLoader`:** loads and *validates* a `.animsm.ron`, loading every
> referenced Aseprite file on demand. Validation is eager and complete: misname a state or
> reference a missing tag and the error lists **every** problem in the file at once, by
> name — not just the first. (Try it: break two things, read the error, fix them back.)
>
> **Toolbox — `Animator`:** the component that runs a machine on an entity. It drives the
> entity's `AnimationPlayer` — which is why both spawn with `Handle::INVALID`: the machine
> enters its initial state on the first tick and installs the right clip itself. You never
> hand a clip to this entity again.

Second, the control system — compare it to chapter 2's, rule by deleted rule:

```rust,ignore
fn control(
    mut heroes: Query<(&mut Transform2D, &mut Animator, &mut Sprite), With<HeroTag>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut animator, mut sprite)) = heroes.single_mut() else {
        return;
    };
    let free = matches!(animator.state(), "idle" | "run");

    let mut dir = Vec2::ZERO;
    if free {
        if input.pressed(Key::A) {
            dir.x -= 1.0;
        }
        if input.pressed(Key::D) {
            dir.x += 1.0;
        }
        if input.pressed(Key::S) {
            dir.y -= 1.0;
        }
        if input.pressed(Key::W) {
            dir.y += 1.0;
        }
    }
    let velocity = if dir == Vec2::ZERO {
        Vec2::ZERO
    } else {
        dir.normalize() * 90.0
    };
    transform.translation += velocity * time.fixed_delta;
    if velocity.x != 0.0 {
        sprite.flip_x = velocity.x < 0.0;
    }

    animator.set_float("speed", velocity.length());
    if free && input.just_pressed(Key::Space) {
        animator.trigger("attack");
    }
}
```

What's left is *gameplay*: move, face, and two lines of reporting. `set_float("speed", ...)`
states a fact every tick; `trigger("attack")` states a one-tick fact. Which clips those
facts imply is no longer this function's business.

Third — the interesting line — `let free = matches!(animator.state(), "idle" | "run")`.
Gameplay *reads* one thing back from the machine: which state it's in, by name. "The hero
is committed to an animation" (mid-swing, mid-flinch) is genuinely a gameplay condition,
and the machine is its source of truth. This read-back is the start of a two-way contract
that chapter 4 turns into an actual game. (First tick subtlety: the machine enters its
initial state on tick one, so `state()` is briefly `""` — which `matches!` treats as
locked. One tick, harmless, worth knowing.)

## Step 3 — tune without compiling

The payoff ritual. Run the game, then edit the RON and rerun (no `cargo build` in between —
the file loads at startup):

1. Change `Gt("speed", 1.0)` to `Gt("speed", 80.0)` — now only near-full speed reads as
   running; diagonal walk-drift stays idle.
2. Delete the `hit` state and its `Any` transition, rerun, and read the validation error
   aloud. Nothing else breaks: gameplay never mentioned `hit`. Restore it.
3. Swap the two `Any` lines and ask: what changed? (Nothing observable yet — no gameplay
   fires `hurt`. Chapter 4 makes the order matter, and it'll already be right.)

This file is where the game-feel conversation with a designer happens now. That's the whole
argument for logic-as-data, and you just had it without recompiling once.

## Checkpoint

Your program should match `games/dojo/examples/an03_machine.rs`:

```text
cargo run -p dojo --example an03_machine
```

New vocabulary:

| Tool | What it's for |
| --- | --- |
| `.animsm.ron` / `StateMachine` | clip-flow rules as a data file |
| `params` (`Float`, `Bool`, `Trigger`) | the machine's whole input surface |
| `on_finish` | where a play-once state goes when its clip ends |
| `from: Any` vs `from: State(..)` | rule applies everywhere / in one state |
| `Animator` + `AnimatorLoader` | run a machine on an entity / load + validate the file |
| `set_float` / `set_bool` / `trigger` | gameplay states facts |
| `animator.state()` | gameplay reads the one thing back: where the machine is |

## Exercises

1. **Chapter 2's exercise, again.** Wire H to `animator.trigger("hurt")` (one line, inside
   the `free` check or not — decide, it matters). Compare against your chapter 2 exercise 3
   diff. This is the comparison the whole chapter was for.
2. **A tired hero.** Add a `"winded"` Bool param and a rule: while winded, `Gt("speed", ...)`
   can't reach run (hint: `Is("winded", false)` as a second condition — `when` lists AND).
   Wire it to holding Shift. No Rust beyond one `set_bool`.
3. **Priority archaeology (harder).** Move the `Any → attack` line *below* the idle/run
   transitions and play. Nothing changes — work out why (what does `Any` first-then-state
   actually order?). Then construct the file where declaration order *between two `Any`
   rules* is observable, and verify. The reference chapter's evaluation-order section has
   the precise algorithm; check yourself against it.

The machine runs the show and gameplay reports facts. Time to make the facts matter: next
chapter, the dummy returns, and the sword starts connecting — on an exact frame.
