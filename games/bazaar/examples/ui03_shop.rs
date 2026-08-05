//! UI Book, chapter 3: the HUD grows hands. The real simulation (`bazaar::game`) runs
//! underneath, and every interaction — serving, pausing, calling to the street — arrives
//! through the UI: buttons as `UiEvent`s, world clicks gated by `UiFocus`. This is the
//! finished game wearing chapter 3's flat colors; chapter 4 dresses it.
//! Run: `cargo run -p bazaar --example ui03_shop`

use bazaar::game::{DayClock, GamePlugin, GameState, QUEUE_LIMIT, Queue, Takings, WARES};
use fulcrum::prelude::*;

const WARE_COLORS: [Color; 3] = [
    Color::rgb(0.55, 0.75, 0.35),
    Color::rgb(0.85, 0.45, 0.75),
    Color::rgb(0.95, 0.75, 0.35),
];

#[derive(Component)]
struct Doll(usize);

fn setup(mut ui: UiLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::IntegerScale {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);
    ui.load("ui/ui03.ui.ron").expect("layout loads");
}

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

fn dolls(queue: Res<Queue>, mut dolls: Query<(&Doll, &mut Sprite)>) {
    for (doll, mut sprite) in &mut dolls {
        sprite.color = match queue.0.get(doll.0) {
            Some(customer) => WARE_COLORS[customer.wants],
            None => Color::rgba(0.0, 0.0, 0.0, 0.0),
        };
    }
}

fn hud(
    mut ui: UiQuery,
    takings: Res<Takings>,
    clock: Res<DayClock>,
    queue: Res<Queue>,
    state: Res<GameState>,
) {
    ui.set_label("gold", format!("Gold: {}", takings.gold));
    ui.set_label("day", format!("Sundown: {}s", clock.0 / 60));
    for spot in 0..QUEUE_LIMIT {
        let text = match queue.0.get(spot) {
            Some(customer) => format!(
                "{} {}  {}s",
                if spot == 0 { ">" } else { " " },
                WARES[customer.wants].name,
                customer.patience / 60
            ),
            None => String::new(),
        };
        ui.set_label(&format!("q{spot}"), text);
    }
    ui.set_visible("pause", *state == GameState::Paused);
    ui.set_visible("summary", *state == GameState::DayOver);
    if *state == GameState::DayOver {
        ui.set_label(
            "summary_text",
            format!("{} served, {} lost", takings.served, takings.lost),
        );
        ui.set_label("summary_gold", format!("{} gold", takings.gold));
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "ui03: the shop opens".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.46, 0.65, 0.75),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_startup(setup_stall)
    .add_frame_system(hud)
    .add_frame_system(dolls)
    .run();
}
