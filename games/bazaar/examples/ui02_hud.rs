//! UI Book, chapter 2: the readout HUD. A miniature market sim (customers arrive, wait,
//! and give up) drives every label each frame through `UiQuery`. You can watch the whole
//! day happen; you just can't do anything about it yet — that's chapter 3.
//! Run: `cargo run -p bazaar --example ui02_hud`

use fulcrum::prelude::*;

const WARES: [&str; 3] = ["Figs", "Silk", "Lamps"];
const DAY_TICKS: u32 = 5400;
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

fn setup(mut ui: UiLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::IntegerScale {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);
    ui.load("ui/ui02.ui.ron").expect("layout loads");
}

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

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "ui02: the readout".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.46, 0.65, 0.75),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .insert_resource(Queue::default())
    .insert_resource(DayClock(DAY_TICKS))
    .insert_resource(NextArrival(90))
    .insert_resource(Lost::default())
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(market)
    .add_frame_system(hud)
    .run();
}
