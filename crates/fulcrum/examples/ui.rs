//! Steps 6-7 (phase 3) acceptance: RON HUD with panel/labels/button, per-frame set_label,
//! clicks reaching the simulation, and layout hot reload (the file rewrites itself darker red
//! after ~5 simulated seconds). `UI_AUTOPLAY=1` injects a synthetic click for headless demos.
//! Run: `cargo run -p fulcrum --example ui`

use fulcrum::prelude::*;

const HUD: &str = r#"Ui(root: Node(
    anchor: TopLeft, offset: (16, 16), size: Fit, stack: Vertical(6),
    kind: Panel(color: (r: 0.05, g: 0.05, b: 0.12, a: 0.8)),
    children: [
        Node(id: "score", kind: Label(text: "Score: 0", size: 16)),
        Node(id: "status", kind: Label(text: "clicks: 0", size: 16, color: (r: 0.5, g: 1.0, b: 0.6, a: 1.0))),
        Node(id: "clicker", size: Px((180, 32)), kind: Button(text: "Click me")),
    ],
))"#;

const HUD_V2: &str = r#"Ui(root: Node(
    anchor: TopLeft, offset: (16, 16), size: Fit, stack: Vertical(6),
    kind: Panel(color: (r: 0.35, g: 0.05, b: 0.05, a: 0.9)),
    children: [
        Node(id: "score", kind: Label(text: "Score: 0", size: 16)),
        Node(id: "status", kind: Label(text: "clicks: 0", size: 16, color: (r: 1.0, g: 0.8, b: 0.3, a: 1.0))),
        Node(id: "clicker", size: Px((180, 32)), kind: Button(text: "Click me")),
        Node(kind: Label(text: "hot reloaded!", size: 16, color: (r: 1.0, g: 0.4, b: 0.4, a: 1.0))),
    ],
))"#;

#[derive(Resource, Default)]
struct Clicks(u32);

fn asset_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("fulcrum-ui-example");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hud.ui.ron"), HUD).unwrap();
    dir
}

fn setup(mut ui: UiLoader) {
    ui.load("hud.ui.ron").expect("hud loads");
}

fn drive_labels(mut ui: UiQuery, time: Res<Time>, clicks: Res<Clicks>) {
    ui.set_label("score", format!("Score: {}", time.tick));
    ui.set_label("status", format!("clicks: {}", clicks.0));
}

/// Simulation-side click handling: UiEvents cross the frame/tick boundary like input.
fn count_clicks(mut events: EventReader<UiEvent>, mut clicks: ResMut<Clicks>) {
    for event in events.read() {
        let UiEvent::Clicked(id) = event;
        if id == "clicker" {
            clicks.0 += 1;
        }
    }
}

/// Rewrite the layout file mid-run: the tree respawns from disk.
fn hot_rewrite(time: Res<Time>, mut done: Local<bool>) {
    if !*done && time.tick >= 300 {
        *done = true;
        std::fs::write(asset_dir().join("hud.ui.ron"), HUD_V2).unwrap();
        println!("rewrote hud.ui.ron; the panel should restyle live");
    }
}

/// Optional demo autopilot (UI_AUTOPLAY=1): click the button with injected input at ~2s.
fn autopilot(
    mut input: ResMut<Input>,
    time: Res<Time>,
    buttons: Query<(&UiRect, &UiId), With<UiButton>>,
    mut phase: Local<u32>,
) {
    if std::env::var_os("UI_AUTOPLAY").is_none() {
        return;
    }
    // Stretch scaling: UI pixels == screen pixels, so UiRect centers are cursor positions.
    let Some(center) = buttons
        .iter()
        .find(|(_, id)| id.0 == "clicker")
        .map(|(placed, _)| placed.rect.center())
    else {
        return;
    };
    match (*phase, time.tick) {
        (0, 100..) => {
            input.push_cursor(center);
            *phase = 1;
        }
        (1, 110..) => {
            input.push_mouse_button(MouseButton::Left, true);
            *phase = 2;
        }
        (2, 120..) => {
            input.push_mouse_button(MouseButton::Left, false);
            *phase = 3;
        }
        _ => {}
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "ui".into(),
        clear_color: Color::rgb(0.12, 0.13, 0.18),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(asset_dir()))
    .insert_resource(Clicks::default())
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(count_clicks)
    .add_frame_system(drive_labels)
    .add_frame_system(hot_rewrite)
    .add_frame_system(autopilot)
    .run();
}
