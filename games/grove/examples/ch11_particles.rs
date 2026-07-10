//! Chapter 11: particles — a spark burst per gem, fireflies in the air.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;
#[derive(Component)]
struct Gem;

#[derive(Event)]
struct GemCollected(Vec2);

fn setup(mut commands: Commands, mut aseprite: AsepriteLoader) {
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
}

/// Separate startup system: `EffectLoader` and `AsepriteLoader` both write texture assets, so
/// they can't share one system's parameter list.
fn setup_fx(mut commands: Commands, mut effects: EffectLoader) {
    // A looping ambience emitter: Rate mode, lives until despawned.
    let fireflies = effects.load("fx/fireflies.fx.ron").expect("effect loads");
    commands.spawn((
        ParticleEmitter::new(fireflies),
        Transform2D::from_xy(0.0, 0.0),
    ));
}

fn movement(
    mut players: Query<&mut Transform2D, With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let Ok(mut transform) = players.single_mut() else {
        return;
    };
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
    transform.translation += dir.normalize_or_zero() * 90.0 * time.fixed_delta;
}

fn collect(
    mut commands: Commands,
    gems: Query<(Entity, &Transform2D), With<Gem>>,
    players: Query<&Transform2D, (With<Player>, Without<Gem>)>,
    mut events: EventWriter<GemCollected>,
) {
    let Ok(player) = players.single() else { return };
    for (gem, at) in &gems {
        if at.translation.distance(player.translation) < 14.0 {
            commands.entity(gem).despawn();
            events.write(GemCollected(at.translation));
        }
    }
}

/// Presentation: the sim said where; a one-shot Burst emitter says how it looks.
fn sparkle(
    mut events: EventReader<GemCollected>,
    mut effects: EffectLoader,
    mut commands: Commands,
) {
    for GemCollected(at) in events.read() {
        if let Ok(spark) = effects.load("fx/spark.fx.ron") {
            commands.spawn_effect_at(spark, *at); // one_shot: despawns itself when done
        }
    }
}

fn main() {
    Fulcrum::new("ch11: particles")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_event::<GemCollected>()
        .add_startup((setup, setup_fx))
        .add_system((movement, collect).chain())
        .add_frame_system(sparkle)
        .run();
}
