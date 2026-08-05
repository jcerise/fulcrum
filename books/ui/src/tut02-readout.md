# The Readout

This chapter builds the Bazaar's whole face: the day's numbers top-left, the queue readout
top-right, a hint in the corner — and a tiny market simulation underneath so there's
something true to display. Customers will arrive, wait, and give up, and the HUD will
narrate it every frame. What you won't be able to do is *serve* anyone. Feel that itch;
chapter 3 scratches it.

## Step 1 — the whole readout

Replace `assets/ui/hud.ui.ron`:

```ron
Ui(root: Node(
    size: Fill,
    children: [
        // Top-left: the day's numbers.
        Node(anchor: TopLeft, offset: (4, 4), size: Px((92, 32)), stack: Vertical(2),
            kind: Panel(color: (r: 0.13, g: 0.09, b: 0.07, a: 0.85)),
            children: [
                Node(id: "gold", offset: (7, 7), kind: Label(text: "Gold: 0", size: 8)),
                Node(id: "day", offset: (7, 0), kind: Label(text: "Sundown: 90s", size: 8)),
            ],
        ),
        // Top-right: the line, front first.
        Node(anchor: TopRight, pivot: (1, 0), offset: (-4, 4), size: Px((104, 52)), stack: Vertical(2),
            kind: Panel(color: (r: 0.13, g: 0.09, b: 0.07, a: 0.85)),
            children: [
                Node(offset: (7, 7), kind: Label(text: "The line", size: 8,
                    color: (r: 0.95, g: 0.75, b: 0.35, a: 1.0))),
                Node(id: "q0", offset: (7, 0), kind: Label(text: "", size: 8)),
                Node(id: "q1", offset: (7, 0), kind: Label(text: "", size: 8)),
                Node(id: "q2", offset: (7, 0), kind: Label(text: "", size: 8)),
            ],
        ),
        // Bare label, deliberately: labels never block the pointer.
        Node(id: "hint", anchor: BottomLeft, pivot: (0, 1), offset: (4, -4),
            kind: Label(text: "Esc: pause\nClick the street to call out", size: 8,
                color: (r: 1.0, g: 1.0, b: 1.0, a: 0.7))),
    ],
))
```

Run it — three quiet pieces of UI on the sky. The new machinery, piece by piece:

> **Toolbox — stacks:** `stack: Vertical(2)` makes a node arrange its children top-to-bottom
> with a 2px gap, cursor-style: each child is placed, then the cursor moves past it. Two
> rules to tattoo somewhere. First: **stacked children's anchors and pivots are ignored** —
> the stack owns their position; only `offset` still applies, as a nudge *from the cursor*
> (and the nudge moves the cursor too, which is why it works as padding). Second: the first
> child's `offset: (7, 7)` is the panel's padding — there are no padding or margin fields,
> and this idiom is the official substitute. Later siblings only need `(7, 0)`: the x
> restarts from the panel's edge each row, the y is already where the cursor left it.
>
> **Toolbox — the right-edge pin:** `anchor: TopRight, pivot: (1, 0), offset: (-4, 4)` —
> pin my top-*right* corner to the parent's top-right, then nudge 4px left (negative x) and
> 4px down. Right-aligned and bottom-aligned UI always reads like this: matching corner,
> matching pivot, mirrored signs.
>
> **Toolbox — empty labels as slots:** `q0`–`q2` are three permanent label nodes that render
> nothing while their text is `""`. The layout never grows or shrinks with the queue — the
> game just fills and empties the slots. Fixed slots beat dynamic spawning for every HUD
> that has a natural maximum, and the Bazaar's street fits three customers.

The hint is a bare `Label` with no panel behind it. That's load-bearing foreshadowing:
panels, buttons, and images *block* the pointer (chapter 3), *labels don't* — a watermark
never steals a click.

## Step 2 — a market to report on

The sim side, all of it. Constants and state first — in `src/main.rs` above `setup`:

```rust,ignore
const WARES: [&str; 3] = ["Figs", "Silk", "Lamps"];
const DAY_TICKS: u32 = 5400; // 90 seconds at 60 Hz
const PATIENCE: u32 = 480;

#[derive(Clone, Copy)]
struct Customer {
    wants: usize,
    patience: u32,
}

#[derive(Resource, Default)]
struct Queue(Vec<Customer>);

#[derive(Resource)]
struct DayClock(u32);

#[derive(Resource)]
struct NextArrival(u32);

#[derive(Resource, Default)]
struct Lost(u32);
```

Then the market itself — one sim system, on the fixed tick, because customers arriving is
gameplay, and gameplay must replay:

```rust,ignore
/// Simulation: arrivals and patience, on the fixed tick.
fn market(
    mut queue: ResMut<Queue>,
    mut clock: ResMut<DayClock>,
    mut arrival: ResMut<NextArrival>,
    mut lost: ResMut<Lost>,
    mut rng: ResMut<SimRng>,
) {
    if clock.0 == 0 {
        return;
    }
    clock.0 -= 1;
    arrival.0 = arrival.0.saturating_sub(1);
    if arrival.0 == 0 {
        arrival.0 = rng.range_i32(120..300) as u32;
        if queue.0.len() < 3 {
            queue.0.push(Customer {
                wants: rng.range_i32(0..3) as usize,
                patience: PATIENCE,
            });
        }
    }
    for customer in &mut queue.0 {
        customer.patience -= 1;
    }
    let before = queue.0.len();
    queue.0.retain(|customer| customer.patience > 0);
    lost.0 += (before - queue.0.len()) as u32;
}
```

`SimRng` is the seeded simulation RNG — same seed, same customers, same day. Everything
here is ticks: patience is a tick count, sundown is a tick count. No wall-clock anywhere.

## Step 3 — drive the labels

The presentation side is one frame system that re-states everything, every frame:

```rust,ignore
/// Presentation: project the sim onto the labels, every frame.
fn hud(mut ui: UiQuery, queue: Res<Queue>, clock: Res<DayClock>, lost: Res<Lost>) {
    ui.set_label("gold", format!("Lost: {}", lost.0));
    ui.set_label("day", format!("Sundown: {}s", clock.0 / 60));
    for spot in 0..3 {
        let text = match queue.0.get(spot) {
            Some(customer) => format!(
                "{} {}  {}s",
                if spot == 0 { ">" } else { " " },
                WARES[customer.wants],
                customer.patience / 60
            ),
            None => String::new(),
        };
        ui.set_label(&format!("q{spot}"), text);
    }
}
```

(Yes, the gold label reports `Lost:` for now — the only number a shop with no cashier can
produce is how many people gave up. Chapter 3 restores its dignity.)

Register everything — the market on the tick, the readout on the frame:

```rust,ignore
    .insert_resource(Queue::default())         // <-- new
    .insert_resource(DayClock(DAY_TICKS))      // <-- new
    .insert_resource(NextArrival(90))          // <-- new
    .insert_resource(Lost::default())          // <-- new
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(market)                        // <-- new
    .add_frame_system(hud)                     // <-- new
    .run();
```

```text
cargo run -p my-bazaar
```

Ninety seconds of quiet tragedy: customers appear in the line with a `>` marking the front,
their countdowns drain, they vanish, `Lost` climbs, sundown comes.

> **Toolbox — `UiQuery`:** the entire dynamic-content API: `set_label(id, text)` and
> `set_visible(id, bool)`. Lookup is by `id` string, first match wins, and a typo'd id
> logs a warning instead of panicking — run with `RUST_LOG=warn` and misspell one to see.
> Anything fancier than text and visibility (colors, sizes, images) is done with an
> ordinary ECS query — the recipes chapter shows the pattern.
>
> **Toolbox — statelessness:** layouts remember *nothing*. This `hud` system re-asserts
> every label every frame, which sounds wasteful and is actually the design: while the game
> runs, edit `hud.ui.ron` — recolor the heading, save. The tree is despawned, respawned
> from disk... and the labels are correct again by the next frame, because your frame
> system rewrote them. Hot reload costs you nothing *because* nothing in the tree is
> precious.

## Checkpoint

Your program should match `games/bazaar/examples/ui02_hud.rs` (layout:
`assets/ui/ui02.ui.ron`):

```text
cargo run -p bazaar --example ui02_hud
```

## Exercises

1. **A longer street.** Add a `q3` slot to the layout and let the sim queue four customers.
   Two edits in two files — notice which one you could do with the game still running, and
   which one needed a recompile. That boundary is the whole point of layout files.
2. **Padding archaeology.** Set the queue panel's gap to `Vertical(0)` and then `Vertical(6)`,
   live. Then break the padding idiom on purpose: change `q0`'s offset to `(7, 7)` like the
   heading's. Everything below it shifts down 7px — the nudge moves the cursor. Put it back.
3. **A farewell toast (harder).** Add a small centered panel (`id: "toast"`, `visible: false`)
   saying `Somebody left!`. In the game, watch `Lost` for changes from a frame system
   (`Local<u32>` remembering the last value), `set_visible("toast", true)` when it climbs,
   and hide it again two seconds later (`Local<f32>` + `time.frame_delta`). You've just
   written a notification system in ~15 lines; `set_visible` is doing all the lifting.

The stall can count its failures but not its sales. Next chapter the shop opens: serve
buttons, a pause menu, and the click's whole journey across the sim boundary — including
why a button press is, technically, a replayable command.
