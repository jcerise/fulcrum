# The Shop Opens

This chapter makes the Bazaar a game. Serve buttons appear on a counter, a pause menu
answers Esc, the street becomes clickable, and gold finally flows. Along the way the crate
grows up into the standard Fulcrum shape — sim in `src/game.rs`, presentation in
`src/main.rs` — because a button click is the perfect specimen for the dissection this
chapter is really about: how input crosses the sim boundary.

## The idea, before the code

When you release the mouse over a button, the UI does *not* call your code. It sends a
command — `ui:click` with the button's id — into the same pipeline that records input for
replays. At the start of the next tick the command comes back out as a `UiEvent::Clicked`
event, readable from any sim system. Two consequences, both load-bearing:

1. **Clicks are replayable.** A recorded day of shopkeeping replays every button press,
   because presses travel with the input record, not around it.
2. **Clicks are sim-time.** Your handler runs on the fixed tick, one tick after the release
   frame. Events stay readable for that tick and the next — read them promptly.

## Step 1 — grow up into three files

`src/lib.rs`, complete:

```rust,ignore
//! Bazaar: library form, so tests can drive the same simulation the binary ships.

pub mod game;
```

Create `src/game.rs` and move the *simulation* in. The stock list comes first — note the
`id` field, which is about to appear in the layout file too:

```rust,ignore
use fulcrum::prelude::*;

/// One thing the stall sells. `id` doubles as the serve button's `UiId` in `ui/hud.ui.ron` —
/// the layout file and the stock list meet on this string.
pub struct Ware {
    pub id: &'static str,
    pub name: &'static str,
    pub price: u32,
}

/// The stock list. Order matters: `Customer::wants` indexes into it.
pub const WARES: [Ware; 3] = [
    Ware { id: "serve_figs", name: "Figs", price: 3 },
    Ware { id: "serve_silk", name: "Silk", price: 12 },
    Ware { id: "serve_lamps", name: "Lamps", price: 7 },
];
```

Your chapter-2 resources move here too, made `pub`, plus two newcomers — a scoreboard and
a state:

```rust,ignore
/// The day's numbers — the whole scoreboard.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Takings {
    pub gold: u32,
    pub served: u32,
    pub lost: u32,
}

/// Sim state.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    #[default]
    Trading,
    Paused,
    DayOver,
}
```

The plugin declares the whole simulation in one place, in a deliberate order:

```rust,ignore
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Queue::default());
        app.world_mut().insert_resource(Takings::default());
        app.world_mut().insert_resource(GameState::default());
        app.world_mut().insert_resource(DayClock::default());
        app.world_mut().insert_resource(NextArrival::default());
        // Headless (no UiPlugin) the sim still runs: UiFocus just stays false.
        if app.world().get_resource::<UiFocus>().is_none() {
            app.world_mut().insert_resource(UiFocus::default());
        }
        app.register_event::<UiEvent>();
        app.add_systems(
            FixedUpdate,
            (menu_clicks, serve_clicks, street_calls, day_clock, arrivals, patience).chain(),
        );
    }
}
```

Now the three systems that read the UI. Menus first:

```rust,ignore
/// Esc toggles pause; Resume, Quit, and Next Day arrive as `UiEvent`s. Clicks travel as
/// commands, so a replay of a day includes every button press.
fn menu_clicks(
    mut state: ResMut<GameState>,
    input: Res<Input>,
    mut events: EventReader<UiEvent>,
    mut queue: ResMut<Queue>,
    mut takings: ResMut<Takings>,
    mut clock: ResMut<DayClock>,
    mut arrival: ResMut<NextArrival>,
) {
    if input.just_pressed(Key::Escape) {
        *state = match *state {
            GameState::Trading => GameState::Paused,
            GameState::Paused => GameState::Trading,
            over => over,
        };
    }
    for event in events.read() {
        let UiEvent::Clicked(id) = event;
        match id.as_str() {
            "resume" if *state == GameState::Paused => *state = GameState::Trading,
            "quit" if *state == GameState::Paused => std::process::exit(0),
            "next_day" if *state == GameState::DayOver => {
                *queue = Queue::default();
                *takings = Takings::default();
                *clock = DayClock::default();
                *arrival = NextArrival::default();
                *state = GameState::Trading;
            }
            _ => {}
        }
    }
}
```

Match on the id, guard on the state. The guards matter: a stale `resume` click while
trading must do nothing. Then the cashier:

```rust,ignore
/// A serve button names its ware; a sale needs the front customer to want it. Clicking the
/// wrong ware does nothing — the customer keeps waiting (and keeps losing patience).
fn serve_clicks(
    mut events: EventReader<UiEvent>,
    mut queue: ResMut<Queue>,
    mut takings: ResMut<Takings>,
    state: Res<GameState>,
) {
    for event in events.read() {
        let UiEvent::Clicked(id) = event;
        if *state != GameState::Trading {
            continue;
        }
        let Some(ware) = WARES.iter().position(|w| w.id == id.as_str()) else {
            continue;
        };
        if queue.0.first().is_some_and(|c| c.wants == ware) {
            queue.0.remove(0);
            takings.gold += WARES[ware].price;
            takings.served += 1;
        }
    }
}
```

And the one that makes the *world* clickable:

```rust,ignore
/// A click on the street calls out to the crowd and hurries the next arrival. `UiFocus` is
/// the boundary guard: a click that landed on a panel or button never reaches the street.
fn street_calls(
    input: Res<Input>,
    focus: Res<UiFocus>,
    state: Res<GameState>,
    mut arrival: ResMut<NextArrival>,
) {
    if *state == GameState::Trading
        && input.mouse_just_pressed(MouseButton::Left)
        && !focus.pointer_over_ui
    {
        arrival.0 = arrival.0.saturating_sub(CALL_BONUS);
    }
}
```

The rest — `day_clock`, `arrivals`, `patience` — are your chapter-2 market, moved in,
gated on `GameState::Trading`, and with `Lost` folded into `Takings`. Make the upgrades
yourself, then check against the shipped `games/bazaar/src/game.rs`, which has all of it
verbatim (`CALL_BONUS`, `PATIENCE`, and friends included).

> **Toolbox — `UiEvent` and the boundary:** `register_event::<UiEvent>()` + an
> `EventReader<UiEvent>` in a *sim* system is the entire receiving end. Two readers (menus,
> cashier) each see every event — readers have independent cursors. The sending end you
> never touch: an engine system hit-tests the pointer every frame, tracks press/release,
> and emits the command on release-over-the-same-button. Buttons without an `id` emit
> nothing (the loader logs an error at load time — the earliest a typo can be caught).
>
> **Toolbox — `UiFocus`:** one resource, one flag: `pointer_over_ui`. Panels, buttons, and
> images set it; bare labels and pure layout nodes don't (the chapter-2 hint never blocks
> the street). Every world-click system you ever write starts with `!focus.pointer_over_ui`
> — skip it and buying silk also shouts at the street.

## Step 2 — the layout grows hands

Three additions to `assets/ui/hud.ui.ron`, all inside the root's `children`. The counter —
a horizontal stack of three columns, each an icon over a button:

```ron
Node(anchor: BottomCenter, pivot: (0.5, 1), offset: (0, -4), size: Px((242, 50)), stack: Horizontal(6),
    kind: Panel(color: (r: 0.13, g: 0.09, b: 0.07, a: 0.85)),
    children: [
        Node(offset: (7, 7), stack: Vertical(2), children: [
            Node(offset: (28, 0), size: Px((16, 16)), kind: Image(image: "ui/figs.png")),
            Node(id: "serve_figs", size: Px((72, 18)), kind: Button(text: "Figs 3g", text_size: 8)),
        ]),
        Node(offset: (0, 7), stack: Vertical(2), children: [
            Node(offset: (28, 0), size: Px((16, 16)), kind: Image(image: "ui/silk.png")),
            Node(id: "serve_silk", size: Px((72, 18)), kind: Button(text: "Silk 12g", text_size: 8)),
        ]),
        Node(offset: (0, 7), stack: Vertical(2), children: [
            Node(offset: (28, 0), size: Px((16, 16)), kind: Image(image: "ui/lamps.png")),
            Node(id: "serve_lamps", size: Px((72, 18)), kind: Button(text: "Lamps 7g", text_size: 8)),
        ]),
    ],
),
```

The columns are `kind`-less pure layout nodes — structure without pixels. The `Image` sits
*above* its button because a `Button` can't carry a texture (a deliberate limit; the
reference chapter has the full list). Note the icon's `offset: (28, 0)`: centering by
arithmetic, `(72 - 16) / 2`, because stacked children can't ask for alignment.

The pause menu and the day summary, both born hidden:

```ron
Node(id: "pause", visible: false, anchor: Center, pivot: (0.5, 0.5), size: Px((120, 76)), stack: Vertical(4),
    kind: Panel(color: (r: 0.09, g: 0.06, b: 0.05, a: 0.94)),
    children: [
        Node(offset: (12, 8), kind: Label(text: "PAUSED", size: 16)),
        Node(id: "resume", size: Px((104, 18)), offset: (8, 0), kind: Button(text: "Resume", text_size: 8)),
        Node(id: "quit", size: Px((104, 18)), offset: (8, 0), kind: Button(text: "Quit", text_size: 8)),
    ],
),
Node(id: "summary", visible: false, anchor: Center, pivot: (0.5, 0.5), size: Px((150, 78)), stack: Vertical(4),
    kind: Panel(color: (r: 0.09, g: 0.06, b: 0.05, a: 0.94)),
    children: [
        Node(offset: (19, 8), kind: Label(text: "SUNDOWN", size: 16)),
        Node(id: "summary_text", offset: (8, 0), kind: Label(text: "", size: 8)),
        Node(id: "summary_gold", offset: (8, 0), kind: Label(text: "", size: 8,
            color: (r: 0.95, g: 0.75, b: 0.35, a: 1.0))),
        Node(id: "next_day", size: Px((134, 18)), offset: (8, 0), kind: Button(text: "Next day", text_size: 8)),
    ],
),
```

`visible: false` hides the node *and its subtree* from drawing and hit-testing both — a
hidden Resume button can't be hovered, can't be clicked, and sends nothing. Visibility is
the sim's lever on the UI, and it's about to be pulled every frame.

## Step 3 — presentation catches up

In `src/main.rs`, the `hud` system gains the visibility calls and the honest gold label:

```rust,ignore
    ui.set_label("gold", format!("Gold: {}", takings.gold));
    ui.set_visible("pause", *state == GameState::Paused);
    ui.set_visible("summary", *state == GameState::DayOver);
    if *state == GameState::DayOver {
        ui.set_label(
            "summary_text",
            format!("{} served, {} lost", takings.served, takings.lost),
        );
        ui.set_label("summary_gold", format!("{} gold", takings.gold));
    }
```

And the street gets pixels — a startup system for the backdrop, and permanent "doll"
sprites the queue possesses:

```rust,ignore
// `UiLoader` owns the texture loader while it runs (panel images), so it can't share a
// system with `AssetLoader` — the stall's sprites load in `setup_stall` instead.
fn setup_stall(mut commands: Commands, mut assets: AssetLoader) {
    commands.spawn((
        Sprite::new(assets.load("ground.png")),
        Transform2D::from_xy(160.0, 24.0),
    ));
    commands.spawn((
        Sprite::new(assets.load("stall.png")).with_z(1.0),
        Transform2D::from_xy(76.0, 82.0),
    ));
    let customer = assets.load("customer.png");
    for spot in 0..QUEUE_LIMIT {
        commands.spawn((
            Sprite::new(customer).with_z(2.0),
            Transform2D::from_xy(180.0 + spot as f32 * 44.0, 60.0),
            Doll(spot),
        ));
    }
}

/// Project the queue onto the dolls: tint by ware, vanish when the spot is empty.
fn dolls(queue: Res<Queue>, mut dolls: Query<(&Doll, &mut Sprite)>) {
    for (doll, mut sprite) in &mut dolls {
        sprite.color = match queue.0.get(doll.0) {
            Some(customer) => WARE_COLORS[customer.wants],
            None => Color::rgba(0.0, 0.0, 0.0, 0.0),
        };
    }
}
```

(`Doll` is a one-field component; `WARE_COLORS` three tints matching the ware order —
customers literally wear their shopping list. Both are in the shipped `main.rs`.) Register
`GamePlugin`, `setup_stall`, and `dolls`, delete the chapter-2 inline sim, and:

```text
cargo run -p my-bazaar
```

Play a day. Watch a serve button darken as you press it and lighten on hover — that's
`ButtonState`, driven by the engine, styled by `ButtonStyle`'s three colors (defaults, for
now). Click the street; the next customer hurries. Press Esc mid-rush and take a breath.

## Checkpoint

Your crate should now match the shipped game minus the skin. The chapter's snapshot is
`games/bazaar/examples/ui03_shop.rs` (layout: `assets/ui/ui03.ui.ron`):

```text
cargo run -p bazaar --example ui03_shop
```

New vocabulary:

| Term | What it is |
| --- | --- |
| `UiEvent::Clicked(id)` | a button press, delivered to the sim on the next tick |
| `ui:click` command | the replayable form a click travels as |
| `UiFocus` | resource flag: is the pointer over any panel/button/image? |
| blocker | a node kind that sets `UiFocus` (labels aren't one) |
| `ButtonState` / `ButtonStyle` | engine-driven hover/press flags / the three colors they pick |

## Exercises

1. **Hotkeys for regulars.** Serve with the 1/2/3 keys: in `serve_clicks`, treat
   `input.just_pressed(Key::Key1)` like a `serve_figs` click. Keyboard input is recorded
   input, so replays still hold — but notice the asymmetry you've created: the button path
   goes through hit-testing and `ButtonState`, the key path doesn't light the button up.
2. **The tut.** Punish wrong serves: `-2` patience seconds for the front customer when the
   clicked ware isn't what they want. One `if let` in `serve_clicks`. Then decide — should
   a mis-click while the street is empty cost anything? You're doing game design in a
   click handler now; that's the point of getting clicks into the sim.
3. **Closing time (harder).** During `DayOver`, hide the whole counter:
   give the counter panel `id: "shop"` and `set_visible` it on state. Then verify the
   important half: with the summary up, click where the Figs button *was* and confirm the
   street doesn't hear it either — hidden UI neither draws nor hit-tests, but the summary
   panel under your cursor still blocks. Three overlapping truths in one click.

The Bazaar works. It also looks like a tax form: five gray rectangles on a pretty sky. Next
chapter is the one the book is named for — nine-slice frames, tints, and a theme system
that redresses every panel, live, in two keystrokes.
