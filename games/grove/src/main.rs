//! Grove: the Fulcrum Book's tutorial game. `cargo run -p grove`
//! WASD moves. Collect every gem; don't get caught. Enter restarts. F12 opens the inspector.

use fulcrum::prelude::*;
use grove::game::{
    self, FoxTag, GamePlugin, GemCollected, Gems, GroveState, LevelScene, PlayerTag,
};

#[derive(Resource)]
struct Sounds {
    ding: Handle<Sound>,
}

fn setup(
    mut scenes: SceneLoader,
    mut spawner: ResMut<SceneSpawner>,
    mut ui: UiLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
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
    ui.load("ui/hud.ui.ron").expect("hud loads");
    let music = sounds.load("music.wav");
    audio.play_music(sounds.assets(), music, true);
    audio.set_master_volume(0.7);
    commands.insert_resource(Sounds {
        ding: sounds.load("ding.wav"),
    });
}

fn camera_follow(
    mut camera: ResMut<Camera2D>,
    players: Query<&Transform2D, With<PlayerTag>>,
    time: Res<Time>,
) {
    if let Ok(player) = players.single() {
        let center = camera.center;
        camera.center = center + (player.translation - center) * (5.0 * time.frame_delta).min(1.0);
    }
}

fn face_sprites(mut sprites: Query<(&game::FacingLeft, &mut Sprite)>) {
    for (facing, mut sprite) in &mut sprites {
        if sprite.flip_x != facing.0 {
            sprite.flip_x = facing.0;
        }
    }
}

fn hud(mut ui: UiQuery, gems: Res<Gems>, state: Res<GroveState>) {
    ui.set_label("gems", format!("Gems: {}/{}", gems.collected, gems.total));
    ui.set_visible("banner", *state != GroveState::Playing);
    match *state {
        GroveState::Won => ui.set_label("banner_text", "You collected them all!"),
        GroveState::Caught => ui.set_label("banner_text", "The fox got you!"),
        GroveState::Playing => {}
    }
}

fn chime(
    mut events: EventReader<GemCollected>,
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    sound_assets: Res<Assets<Sound>>,
    mut counter: Local<u32>,
) {
    let Some(sounds) = sounds else { return };
    for _ in events.read() {
        *counter += 1;
        audio.play_with(
            &sound_assets,
            sounds.ding,
            PlayParams {
                volume: 0.8,
                pitch: 1.0 + (*counter % 4) as f32 * 0.06,
                pan: 0.0,
            },
        );
    }
}

fn debug_circles(
    input: Res<Input>,
    mut on: Local<bool>,
    mut gizmos: ResMut<Gizmos>,
    players: Query<&Transform2D, With<PlayerTag>>,
    foxes: Query<&Transform2D, With<FoxTag>>,
) {
    if input.just_pressed(Key::F1) {
        *on = !*on;
    }
    if !*on {
        return;
    }
    for p in &players {
        gizmos.circle(p.translation, game::PICKUP_RANGE, Color::GREEN);
    }
    for f in &foxes {
        gizmos.circle(
            f.translation,
            game::FOX_AGGRO,
            Color::rgba(1.0, 0.5, 0.2, 0.3),
        );
    }
}

fn main() {
    env_logger::init();
    let app = Fulcrum::with_config(FulcrumConfig {
        title: "Grove".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.16, 0.24, 0.16),
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
        .add_frame_system(chime)
        .add_frame_system(debug_circles)
        .run();
}
