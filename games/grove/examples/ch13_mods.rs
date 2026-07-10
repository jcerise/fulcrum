//! Chapter 13: the finished Grove, made moddable with one plugin line. Everything in
//! `games/grove/mods/` loads at startup: `more_gems` plants three bonus gems through the same
//! prefab the scene uses.

use fulcrum::prelude::*;
use grove::game::{self, GamePlugin, LevelScene};

fn setup(
    mut scenes: SceneLoader,
    mut spawner: ResMut<SceneSpawner>,
    mut camera: ResMut<Camera2D>,
    mut commands: Commands,
) {
    camera.scaling = ScalingMode::Letterbox {
        width: 480.0,
        height: 270.0,
    };
    camera.center = vec2(72.0, 72.0);
    let level = scenes.load("scenes/grove.scene.ron").expect("scene loads");
    spawner.load(level);
    commands.insert_resource(LevelScene(level));
}

fn camera_follow(
    mut camera: ResMut<Camera2D>,
    players: Query<&Transform2D, With<game::PlayerTag>>,
    time: Res<Time>,
) {
    if let Ok(player) = players.single() {
        let center = camera.center;
        camera.center = center + (player.translation - center) * (5.0 * time.frame_delta).min(1.0);
    }
}

fn main() {
    env_logger::init();
    let app = Fulcrum::new("ch13: grove + mods")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        // The one line: discover, mount, and run everything in mods/.
        .with_plugin(ModPlugin::from_dir(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/mods"
        )));
    game::register_components(app)
        .with_plugin(GamePlugin)
        .add_startup(setup)
        .add_frame_system(camera_follow)
        .run();
}
