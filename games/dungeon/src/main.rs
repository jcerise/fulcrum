//! Dungeon: the Fulcrum phase-3 milestone. `cargo run -p dungeon`
//!
//! WASD moves, Space attacks, I toggles inventory, Escape pauses (Resume/Quit buttons),
//! Enter restarts after death, F1 shows collision gizmos, F12 opens the inspector.
//!
//! Everything on screen is data: the map, player, monsters, and HUD are RON files under
//! `assets/` — edit any of them while the game runs and watch them reload (see README).

use dungeon::game::{self, GamePlugin, GameState, Gold, Health, LevelScene, MonsterTag, PlayerTag};
use fulcrum::prelude::*;

fn setup(
    mut scenes: SceneLoader,
    mut spawner: ResMut<SceneSpawner>,
    mut ui: UiLoader,
    mut camera: ResMut<Camera2D>,
    mut commands: Commands,
) {
    camera.scaling = ScalingMode::IntegerScale {
        width: 640.0,
        height: 360.0,
    };
    let level = scenes.load("scenes/level1.scene.ron").expect("level loads");
    spawner.load(level);
    commands.insert_resource(LevelScene(level));
    ui.load("ui/hud.ui.ron").expect("hud loads");
}

/// Camera follows the player (cosmetic lerp, Update-side).
fn camera_follow(
    mut camera: ResMut<Camera2D>,
    players: Query<&Transform2D, With<PlayerTag>>,
    time: Res<Time>,
) {
    if let Ok(player) = players.single() {
        let target = player.translation;
        let t = (6.0 * time.frame_delta).min(1.0);
        let center = camera.center;
        camera.center = center + (target - center) * t;
    }
}

/// Face sprites the way they move.
fn face_sprites(mut sprites: Query<(&game::FacingLeft, &mut Sprite)>) {
    for (facing, mut sprite) in &mut sprites {
        if sprite.flip_x != facing.0 {
            sprite.flip_x = facing.0;
        }
    }
}

fn hud(
    mut ui: UiQuery,
    players: Query<&Health, With<PlayerTag>>,
    gold: Res<Gold>,
    state: Res<GameState>,
    input: Res<Input>,
    mut inventory_open: Local<bool>,
    mut was_down: Local<bool>,
) {
    if let Ok(health) = players.single() {
        ui.set_label("hp", format!("HP {}/{}", health.current.max(0), health.max));
    }
    ui.set_label("gold", format!("Gold: {}", gold.0));

    // Frame-edge latch for the inventory toggle.
    let down = input.pressed(Key::I);
    if down && !*was_down {
        *inventory_open = !*inventory_open;
    }
    *was_down = down;
    ui.set_visible("inventory", *inventory_open);
    ui.set_visible("pause", *state == GameState::Paused);
    ui.set_visible("dead", *state == GameState::Dead);
}

/// F1: collision + aggro circles.
fn debug_circles(
    input: Res<Input>,
    mut on: Local<bool>,
    mut gizmos: ResMut<Gizmos>,
    players: Query<&Transform2D, With<PlayerTag>>,
    monsters: Query<&Transform2D, With<MonsterTag>>,
) {
    if input.just_pressed(Key::F1) {
        *on = !*on;
    }
    if !*on {
        return;
    }
    for player in &players {
        gizmos.circle(player.translation, game::PLAYER_RADIUS, Color::GREEN);
        gizmos.circle(
            player.translation,
            game::ATTACK_RANGE,
            Color::rgba(0.4, 0.9, 1.0, 0.5),
        );
    }
    for monster in &monsters {
        gizmos.circle(
            monster.translation,
            game::MONSTER_RADIUS,
            Color::rgb(1.0, 0.5, 0.3),
        );
        gizmos.circle(
            monster.translation,
            game::AGGRO_RANGE,
            Color::rgba(1.0, 0.4, 0.3, 0.15),
        );
    }
}

fn main() {
    env_logger::init();
    let app = Fulcrum::with_config(FulcrumConfig {
        title: "Dungeon".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.02, 0.02, 0.04),
        gizmos_enabled: true,
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    game::register_components(app)
        .with_plugin(DefaultPlugins)
        .with_plugin(GamePlugin)
        .add_startup(setup)
        .add_frame_system(camera_follow)
        .add_frame_system(face_sprites)
        .add_frame_system(hud)
        .add_frame_system(debug_circles)
        .run();
}
