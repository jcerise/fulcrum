//! Bazaar: the UI Book's tutorial game. `cargo run -p bazaar`
//! Serve the queue by clicking the ware the front customer wants before their patience runs
//! out. Click the street to call out. T swaps the skin, Esc pauses, F12 opens the inspector.
//!
//! Everything in this file is presentation: the camera, the stall sprites, the HUD labels,
//! and the theme. The simulation lives in `src/game.rs` and never touches any of it.

use bazaar::game::{DayClock, GamePlugin, GameState, QUEUE_LIMIT, Queue, Takings, WARES};
use fulcrum::prelude::*;

/// A skin: one nine-slice frame texture plus the colors that go with it. Swapping the
/// active skin restyles every panel and button in place — the layout never changes.
struct Skin {
    frame: &'static str,
    panel_tint: Color,
    buttons: ButtonStyle,
    sky: Color,
}

const SKINS: [Skin; 2] = [
    // Day market: sun-bleached wood and parchment.
    Skin {
        frame: "ui/day_frame.png",
        panel_tint: Color::rgb(1.0, 1.0, 1.0),
        buttons: ButtonStyle {
            normal: Color::rgb(0.45, 0.29, 0.15),
            hover: Color::rgb(0.56, 0.38, 0.20),
            pressed: Color::rgb(0.33, 0.21, 0.11),
        },
        sky: Color::rgb(0.46, 0.65, 0.75),
    },
    // Night market: lacquer, indigo, and lamplight.
    Skin {
        frame: "ui/night_frame.png",
        panel_tint: Color::rgb(0.85, 0.87, 1.0),
        buttons: ButtonStyle {
            normal: Color::rgb(0.20, 0.22, 0.42),
            hover: Color::rgb(0.28, 0.31, 0.55),
            pressed: Color::rgb(0.13, 0.14, 0.30),
        },
        sky: Color::rgb(0.09, 0.10, 0.22),
    },
];

/// Which skin is active. Cosmetic state, so it lives in the binary, not the sim.
#[derive(Resource, Default)]
struct ThemeIndex(usize);

/// Customers are tinted by what they want — the doll's color is the shopping list.
const WARE_COLORS: [Color; 3] = [
    Color::rgb(0.55, 0.75, 0.35), // figs
    Color::rgb(0.85, 0.45, 0.75), // silk
    Color::rgb(0.95, 0.75, 0.35), // lamps
];

/// A fixed spot in the queue line (0 = front). Dolls are permanent; the queue decides
/// which of them are visible and what color they wear.
#[derive(Component)]
struct Doll(usize);

// `UiLoader` owns the texture loader while it runs (panel images), so it can't share a
// system with `AssetLoader` — the stall's sprites load in `setup_stall` instead.
fn setup(mut ui: UiLoader, mut camera: ResMut<Camera2D>) {
    // A fixed 320x180 virtual grid: UI pixels are crisp integers on any window size.
    camera.scaling = ScalingMode::IntegerScale {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);
    ui.load("ui/hud.ui.ron").expect("hud loads");
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

/// Project the queue onto the dolls: tint by ware, vanish when the spot is empty.
fn dolls(queue: Res<Queue>, mut dolls: Query<(&Doll, &mut Sprite)>) {
    for (doll, mut sprite) in &mut dolls {
        sprite.color = match queue.0.get(doll.0) {
            Some(customer) => WARE_COLORS[customer.wants],
            None => Color::rgba(0.0, 0.0, 0.0, 0.0),
        };
    }
}

/// Re-drive every dynamic label and panel each frame. Layouts are stateless — after a hot
/// reload this system repopulates the fresh tree without noticing anything happened.
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

/// T cycles the skin. Frame-level edge latch: `Input` state only changes per tick, and a
/// fast monitor runs several frames per tick — without the latch, one press could cycle
/// the theme two or three times.
fn swap_theme(input: Res<Input>, mut theme: ResMut<ThemeIndex>, mut was_down: Local<bool>) {
    let down = input.pressed(Key::T);
    if down && !*was_down {
        theme.0 = (theme.0 + 1) % SKINS.len();
    }
    *was_down = down;
}

/// Assert the active skin onto the tree, every frame. Idempotent on purpose: a hot reload
/// respawns the layout with its authored (day) styling, and this system re-themes the new
/// tree the same frame. Only panels that already wear a frame are restyled — plain color
/// panels keep whatever the layout says.
fn apply_theme(
    theme: Res<ThemeIndex>,
    mut assets: AssetLoader,
    mut config: ResMut<FulcrumConfig>,
    mut panels: Query<&mut UiPanel>,
    mut buttons: Query<&mut UiButton>,
) {
    let skin = &SKINS[theme.0];
    let frame = assets.load(skin.frame);
    for mut panel in &mut panels {
        if panel.image.is_some() && panel.image != Some(frame) {
            panel.image = Some(frame);
        }
        if panel.image.is_some() && panel.color != skin.panel_tint {
            panel.color = skin.panel_tint;
        }
    }
    for mut button in &mut buttons {
        if button.style.normal != skin.buttons.normal {
            button.style = skin.buttons;
        }
    }
    if config.clear_color != skin.sky {
        config.clear_color = skin.sky;
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Bazaar".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.46, 0.65, 0.75),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .insert_resource(ThemeIndex::default())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_startup(setup_stall)
    .add_frame_system(hud)
    .add_frame_system(dolls)
    .add_frame_system(swap_theme)
    .add_frame_system(apply_theme)
    .run();
}
