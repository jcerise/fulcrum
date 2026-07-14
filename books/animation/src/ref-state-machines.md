# State Machine Files

The complete `.animsm.ron` contract and the exact runtime semantics. Implementation:
`crates/fulcrum-anim/src/state_machine.rs`; acceptance tests:
`crates/fulcrum-anim/tests/state_machine.rs`.

## The file, every field

```ron
StateMachine(
    initial: "idle",                          // required; must name a state
    params: {                                 // optional; the machine's whole input surface
        "speed":  Float(0.0),                 // float, with default
        "armed":  Bool(true),                 // bool, with default
        "attack": Trigger,                    // one-tick pulse, no default
    },
    states: {                                 // required; at least the initial state
        "idle":   (clip: "hero.json#idle"),
        "attack": (clip: "hero.json#attack", on_finish: "idle"),
    },
    transitions: [                            // optional (a one-state machine is legal)
        (from: State("idle"), to: "attack", when: [Triggered("attack")]),
        (from: Any,           to: "idle",   when: []),   // see: empty `when`
    ],
)
```

- **`states`**: every state binds one clip by `"file.json#tag"` reference — the Aseprite
  file loads on demand when the machine does. `on_finish` names where to go when the
  state's clip finishes; it only ever fires for non-looping clips (a tag with `repeat` —
  see the Aseprite chapter), because looping clips never finish.
- **`params`**: three kinds. `Float` and `Bool` hold their last-set value and carry
  defaults, applied on the machine's first tick *without* clobbering values set earlier.
  `Trigger` is edge-shaped: set it, and it exists for exactly one evaluation.
- **`transitions`**: `from` is `Any` (checked from every state) or `State("name")`. `to`
  names a state. `when` is a list of conditions, **AND-ed**; an empty list is always true
  (an unconditional `Any` transition makes its target a black hole — valid, occasionally
  useful, usually a bug).

### Conditions

| Condition | True when |
| --- | --- |
| `Gt("p", x)` | float param `p` > x (strictly) |
| `Lt("p", x)` | float param `p` < x (strictly) |
| `Is("p", b)` | bool param `p` == b |
| `Triggered("p")` | trigger `p` fired since the last evaluation |

There is no OR — write two transitions to the same target instead. There is no
Ge/Le/epsilon-equality on floats, deliberately.

## Validation

Machines validate completely at load: unknown initial state, unresolvable clip references,
`on_finish`/`from`/`to` naming missing states, conditions using undeclared params or the
wrong param kind — **all problems are collected and reported in one error**, each naming
its state or transition. Fix a broken file in one round trip, not five.

## Evaluation, exactly

`Animator` + `AnimationPlayer` entities are driven by one `FixedUpdate` system that runs
**before** clip advance each tick. Per animator, per tick:

1. **First tick only:** apply param defaults (existing values win), enter `initial` —
   entering a state means: record it, and hard-reset the player to the state's clip at
   frame zero (a `play`, not a `restart`).
2. Find the first matching transition, in this exact order: every `Any` transition, in
   declaration order — **skipping those targeting the current state** (no self-re-entry,
   which is what makes trigger-spam into an active state harmless) — then the current
   state's `State(..)` transitions, in declaration order. First match wins; later matches
   are not consulted.
3. If nothing matched and the player reports `finished()`: take `on_finish`, if the state
   has one.
4. Enter the chosen state if it differs from the current one.
5. **Clear all triggers** — fired or not, consumed or not, triggers live exactly one
   evaluation.

Consequences worth knowing cold:

- **Declaration order is priority.** The dojo's `Any → hit` outranking `Any → attack` is
  nothing but line order. Put interruptions above intentions.
- **A trigger set after this tick's evaluation ran** (system ordering) is seen by the
  *next* tick's evaluation — it does not vanish. Either way, one-tick latency between a
  gameplay fact and the visible state change is the design; frame-keyed rules read
  `state()`/`frame_index` and stay in lockstep automatically.
- **`on_finish` is a fallback, not a rule**: an explicit transition matching on the same
  tick wins over it.
- **Timing arithmetic**: a state entered on tick N whose clip lasts K ticks shows frames
  through tick N+K−1 and (absent earlier transitions) hands over via `on_finish` on tick
  N+K. The engine's `headless_load` test pins this exactly.

## The `Animator` component

```rust,ignore
let machine: Handle<StateMachineAsset> = animators.load("anim/hero.animsm.ron")?;
commands.spawn((sprite, transform, Animator::new(machine),
                AnimationPlayer::play(Handle::INVALID)));  // machine installs the real clip
```

| Call | Meaning |
| --- | --- |
| `set_float("speed", v)` / `set_bool("armed", b)` | state a fact (sticky) |
| `trigger("attack")` | state a one-tick fact |
| `state()` | current state's name (`""` until the first tick) |

Params are stored per-entity — a hundred entities can share one machine asset and be in a
hundred different states.

`AnimatorLoader` is the loading `SystemParam` (`load(path)` — cached, validated, resolves
clip references by loading their Aseprite files). It works headless; see the Aseprite
chapter.

## Prefabs and scenes

Data-driven games don't call `Animator::new` — prefabs declare it
(`"Animator": (machine: "anim/player.animsm.ron")` — see grove and dungeon), and the scene
system resolves the def into a live `Animator` + `AnimationPlayer`. One caveat with teeth:
**def resolution runs on the render clock**, so prefab-spawned animators don't exist in
headless runs. Sims built on prefabs must treat animators as optional decoration
(`Option<&mut Animator>`, write-only — the dungeon pattern); sims that *read* animation
state, dojo-style, should spawn their animators directly in simulation code. The tutorial's
chapter 4 opens with this exact choice; make it consciously.
