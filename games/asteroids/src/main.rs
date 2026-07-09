//! Asteroids: the Fulcrum phase-2 milestone game. `cargo run -p asteroids`
//!
//! A/D rotate, W thrust (animated flame), Space shoots, Enter restarts after game over,
//! F1 toggles collision-circle gizmos.

use asteroids::game::{
    self, ARENA, Asteroid, Bullet, GamePlugin, GameSound, GameState, Invulnerable, Lives,
    SHIP_RADIUS, Score, Ship, Thrusting, asteroid_radius,
};
use fulcrum::prelude::*;

/// Clip handles from the ship's Aseprite import.
#[derive(Resource)]
struct ShipClips {
    idle: Handle<AnimationClip>,
    thrust: Handle<AnimationClip>,
}

#[derive(Resource)]
struct Sounds {
    shoot: Handle<Sound>,
    explode: Handle<Sound>,
}

#[derive(Resource)]
struct Art {
    rock: Handle<Texture>,
    white: Handle<Texture>,
}

#[derive(Component)]
struct ScoreText;
#[derive(Component)]
struct LivesText;
#[derive(Component)]
struct CenterText;

/// Ship art + camera (AsepriteLoader shares asset storage with AssetLoader, so this runs as
/// its own system, chained before [`setup_av`]).
fn setup_ship(
    mut commands: Commands,
    mut aseprite: AsepriteLoader,
    mut camera: ResMut<Camera2D>,
    ships: Query<Entity, With<Ship>>,
) {
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA.x,
        height: ARENA.y,
    };

    let ship_art = aseprite.load("ship.json").expect("ship.json loads");
    let idle = ship_art.clips["idle"];
    let thrust = ship_art.clips["thrust"];
    for ship in &ships {
        commands.entity(ship).insert((
            Sprite::from_sheet(ship_art.sheet, 0).with_size(vec2(24.0, 24.0)),
            AnimationPlayer::play(idle),
        ));
    }
    commands.insert_resource(ShipClips { idle, thrust });
}

/// Remaining art, audio, and the HUD.
fn setup_av(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
) {
    commands.insert_resource(Art {
        rock: assets.load("asteroid.png"),
        white: assets.load("white.png"),
    });

    let music = sounds.load("music.wav");
    audio.play_music(sounds.assets(), music, true);
    audio.set_master_volume(0.8);
    commands.insert_resource(Sounds {
        shoot: sounds.load("shoot.wav"),
        explode: sounds.load("explode.wav"),
    });

    // HUD.
    commands.spawn((
        ScoreText,
        Text::new("Score: 0").with_z(10.0),
        Transform2D::from_xy(-ARENA.x / 2.0 + 16.0, ARENA.y / 2.0 - 28.0),
    ));
    commands.spawn((
        LivesText,
        Text::new("Lives: 3").with_align(HAlign::Right).with_z(10.0),
        Transform2D::from_xy(ARENA.x / 2.0 - 16.0, ARENA.y / 2.0 - 28.0),
    ));
    commands.spawn((
        CenterText,
        Text::new("")
            .with_size(24.0)
            .with_align(HAlign::Center)
            .with_color(Color::rgb(1.0, 0.4, 0.4))
            .with_z(10.0),
        Transform2D::from_xy(0.0, 0.0),
    ));
}

/// Sim-side (deterministic) animation switching: thrust flame on while W is held.
fn ship_animation(
    mut ships: Query<(&Thrusting, &mut AnimationPlayer), With<Ship>>,
    clips: Option<Res<ShipClips>>,
) {
    let Some(clips) = clips else { return };
    for (thrusting, mut player) in &mut ships {
        player.restart(if thrusting.0 {
            clips.thrust
        } else {
            clips.idle
        });
    }
}

/// Give newly spawned rocks and bullets their sprites (the sim knows nothing about art).
fn dress_newcomers(
    mut commands: Commands,
    art: Option<Res<Art>>,
    rocks: Query<(Entity, &Asteroid), Without<Sprite>>,
    bullets: Query<Entity, (With<Bullet>, Without<Sprite>)>,
) {
    let Some(art) = art else { return };
    for (entity, asteroid) in &rocks {
        let size = asteroid_radius(asteroid.size) * 2.2;
        commands
            .entity(entity)
            .insert(Sprite::new(art.rock).with_size(Vec2::splat(size)));
    }
    for entity in &bullets {
        commands.entity(entity).insert(
            Sprite::new(art.white)
                .with_size(vec2(3.0, 9.0))
                .with_color(Color::rgb(1.0, 0.95, 0.6)),
        );
    }
}

/// Update HUD text from the sim's resources.
#[allow(clippy::type_complexity)] // standard ECS ParamSet shape
fn hud(
    score: Res<Score>,
    lives: Res<Lives>,
    state: Res<GameState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<LivesText>>,
        Query<&mut Text, With<CenterText>>,
    )>,
) {
    if let Ok(mut text) = texts.p0().single_mut() {
        let value = format!("Score: {}", score.0);
        if text.value != value {
            text.value = value;
        }
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        let value = format!("Lives: {}", lives.0);
        if text.value != value {
            text.value = value;
        }
    }
    if let Ok(mut text) = texts.p2().single_mut() {
        let value = match *state {
            GameState::Playing => "",
            GameState::GameOver => "GAME OVER\npress Enter",
        };
        if text.value != value {
            text.value = value.to_string();
        }
    }
}

/// Turn sim sound events into playback, with slight deterministic pitch variation.
fn play_sounds(
    mut events: EventReader<GameSound>,
    mut audio: ResMut<Audio>,
    sound_handles: Option<Res<Sounds>>,
    sound_assets: Res<Assets<Sound>>,
    mut counter: Local<u32>,
) {
    let Some(handles) = sound_handles else { return };
    for event in events.read() {
        *counter = counter.wrapping_add(1);
        let pitch = 0.9 + (*counter % 5) as f32 * 0.05;
        let (handle, volume) = match event {
            GameSound::Shoot => (handles.shoot, 0.5),
            GameSound::Explode => (handles.explode, 0.9),
        };
        audio.play_with(
            &sound_assets,
            handle,
            PlayParams {
                volume,
                pitch,
                pan: 0.0,
            },
        );
    }
}

/// F1 toggles collision-circle gizmos (the debug-draw acceptance demo).
fn debug_circles(
    input: Res<Input>,
    mut on: Local<bool>,
    mut gizmos: ResMut<Gizmos>,
    ships: Query<(&Transform2D, &Invulnerable), With<Ship>>,
    rocks: Query<(&Transform2D, &Asteroid)>,
) {
    if input.just_pressed(Key::F1) {
        *on = !*on;
    }
    if !*on {
        return;
    }
    for (transform, invulnerable) in &ships {
        let color = if invulnerable.0 > 0 {
            Color::rgb(0.3, 0.8, 1.0)
        } else {
            Color::GREEN
        };
        gizmos.circle(transform.translation, SHIP_RADIUS, color);
    }
    for (transform, asteroid) in &rocks {
        gizmos.circle(
            transform.translation,
            asteroid_radius(asteroid.size),
            Color::rgb(1.0, 0.6, 0.2),
        );
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Asteroids".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: Color::rgb(0.02, 0.02, 0.05),
        gizmos_enabled: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup((setup_ship.after(game::spawn_ship), setup_av).chain())
    .add_system(ship_animation)
    .add_frame_system(dress_newcomers)
    .add_frame_system(hud)
    .add_frame_system(play_sounds)
    .add_frame_system(debug_circles)
    .run();
}
