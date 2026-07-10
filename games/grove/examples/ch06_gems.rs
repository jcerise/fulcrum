//! Chapter 6: gems, events, and sound — the game loop takes shape.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;
#[derive(Component)]
struct Gem;

#[derive(Resource, Default)]
struct Collected(u32);

/// Sim -> presentation messages: the sim announces, the frame side reacts.
#[derive(Event)]
struct GemCollected;

#[derive(Resource)]
struct Ding(Handle<Sound>);

fn setup(
    mut commands: Commands,
    mut aseprite: AsepriteLoader,
    mut sounds: SoundLoader,
    mut assets: AssetLoader,
) {
    let art = aseprite.load("creatures.json").expect("sheet loads");
    commands.spawn((
        Sprite::from_sheet(art.sheet, 0),
        Transform2D::default(),
        AnimationPlayer::play(art.clips["player_idle"]),
        Player,
    ));
    for i in 0..6 {
        let angle = i as f32 / 6.0 * std::f32::consts::TAU;
        commands.spawn((
            Sprite::from_sheet(art.sheet, 6),
            Transform2D::from_translation(vec2(angle.cos(), angle.sin()) * 100.0),
            AnimationPlayer::play(art.clips["gem"]),
            Gem,
        ));
    }
    commands.insert_resource(Ding(sounds.load("ding.wav")));
    // Score as world-space text for now; chapter 8 replaces this with real UI.
    commands.spawn((
        Text::new("Gems: 0").with_z(10.0),
        Transform2D::from_xy(-230.0, 160.0),
    ));
    let _ = &mut assets;
}

fn movement(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) {
        dir.x -= 1.0
    }
    if input.pressed(Key::D) {
        dir.x += 1.0
    }
    if input.pressed(Key::S) {
        dir.y -= 1.0
    }
    if input.pressed(Key::W) {
        dir.y += 1.0
    }
    for mut transform in &mut players {
        transform.translation += dir.normalize_or_zero() * 150.0 * time.fixed_delta;
    }
}

fn collect(
    mut commands: Commands,
    gems: Query<(Entity, &Transform2D), With<Gem>>,
    players: Query<&Transform2D, (With<Player>, Without<Gem>)>,
    mut collected: ResMut<Collected>,
    mut events: EventWriter<GemCollected>,
) {
    let Ok(player) = players.single() else { return };
    for (gem, at) in &gems {
        if at.translation.distance(player.translation) < 14.0 {
            commands.entity(gem).despawn();
            collected.0 += 1;
            events.write(GemCollected);
        }
    }
}

/// Frame-side: react to sim events with sound and text (cosmetic, non-deterministic is fine).
fn present(
    mut events: EventReader<GemCollected>,
    mut audio: ResMut<Audio>,
    ding: Option<Res<Ding>>,
    sounds: Res<Assets<Sound>>,
    collected: Res<Collected>,
    mut labels: Query<&mut Text>,
) {
    let Some(ding) = ding else { return };
    for _ in events.read() {
        audio.play(&sounds, ding.0);
    }
    for mut label in &mut labels {
        let value = format!("Gems: {}", collected.0);
        if label.value != value {
            label.value = value;
        }
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Grove".into(),
        clear_color: Color::rgb(0.16, 0.24, 0.16),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .insert_resource(Collected::default())
    .with_plugin(DefaultPlugins)
    .add_event::<GemCollected>()
    .add_startup(setup)
    .add_system((movement, collect).chain())
    .add_frame_system(present)
    .run();
}
