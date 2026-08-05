//! Bazaar simulation: customers queue, patience drains, and every sale lands through a UI
//! click. Pure logic — the window, sprites, and the skinned UI live in the binary.
//!
//! The whole game is played through buttons, which makes it the UI Book's proving ground:
//! clicks arrive as [`UiEvent`]s (replay-safe commands), and everything else is ordinary
//! deterministic simulation.

use fulcrum::prelude::*;

/// One thing the stall sells. `id` doubles as the serve button's `UiId` in `ui/hud.ui.ron` —
/// the layout file and the stock list meet on this string.
pub struct Ware {
    pub id: &'static str,
    pub name: &'static str,
    pub price: u32,
}

/// The stock list. Order matters: [`Customer::wants`] indexes into it.
pub const WARES: [Ware; 3] = [
    Ware {
        id: "serve_figs",
        name: "Figs",
        price: 3,
    },
    Ware {
        id: "serve_silk",
        name: "Silk",
        price: 12,
    },
    Ware {
        id: "serve_lamps",
        name: "Lamps",
        price: 7,
    },
];

/// Ticks in a trading day (90 seconds at 60 Hz).
pub const DAY_TICKS: u32 = 5400;
/// Ticks a customer waits before walking off.
pub const PATIENCE: u32 = 480;
/// Ticks between arrivals, rolled per customer.
pub const ARRIVAL_GAP: std::ops::Range<i32> = 120..300;
/// A street call (a click on the world, not the UI) hurries the next arrival by this much.
pub const CALL_BONUS: u32 = 90;
/// The street only fits so many; anyone extra passes by.
pub const QUEUE_LIMIT: usize = 3;

/// Someone waiting at the stall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Customer {
    /// Index into [`WARES`].
    pub wants: usize,
    /// Ticks left before they walk off.
    pub patience: u32,
}

/// The line at the stall, front first.
#[derive(Resource, Default)]
pub struct Queue(pub Vec<Customer>);

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

/// Ticks left in the trading day.
#[derive(Resource)]
pub struct DayClock(pub u32);

impl Default for DayClock {
    fn default() -> Self {
        Self(DAY_TICKS)
    }
}

/// Ticks until the next customer wanders up.
#[derive(Resource)]
pub struct NextArrival(pub u32);

impl Default for NextArrival {
    fn default() -> Self {
        Self(90)
    }
}

/// Installs the bazaar simulation: every system here runs on the fixed tick.
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
            (
                menu_clicks,
                serve_clicks,
                street_calls,
                day_clock,
                arrivals,
                patience,
            )
                .chain(),
        );
    }
}

/// Esc toggles pause; Resume, Quit, and Next Day arrive as [`UiEvent`]s. Clicks travel as
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

fn day_clock(mut clock: ResMut<DayClock>, mut state: ResMut<GameState>) {
    if *state != GameState::Trading {
        return;
    }
    clock.0 -= 1;
    if clock.0 == 0 {
        *state = GameState::DayOver;
    }
}

/// Customers wander up on a rolled timer. A full street means they pass by — the timer
/// still rerolls, so the rhythm of the day doesn't depend on how well you're doing.
fn arrivals(
    mut arrival: ResMut<NextArrival>,
    mut queue: ResMut<Queue>,
    mut rng: ResMut<SimRng>,
    state: Res<GameState>,
) {
    if *state != GameState::Trading {
        return;
    }
    arrival.0 = arrival.0.saturating_sub(1);
    if arrival.0 == 0 {
        arrival.0 = rng.range_i32(ARRIVAL_GAP) as u32;
        if queue.0.len() < QUEUE_LIMIT {
            queue.0.push(Customer {
                wants: rng.range_i32(0..WARES.len() as i32) as usize,
                patience: PATIENCE,
            });
        }
    }
}

/// Everyone in line loses patience together; whoever runs out walks off, counted as lost.
fn patience(mut queue: ResMut<Queue>, mut takings: ResMut<Takings>, state: Res<GameState>) {
    if *state != GameState::Trading {
        return;
    }
    for customer in &mut queue.0 {
        customer.patience -= 1;
    }
    let before = queue.0.len();
    queue.0.retain(|customer| customer.patience > 0);
    takings.lost += (before - queue.0.len()) as u32;
}
