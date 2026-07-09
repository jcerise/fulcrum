//! Chapter 5: animation — Aseprite import, clips, and a data-driven state machine.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands, mut aseprite: AsepriteLoader, mut animators: AnimatorLoader) {
    let art = aseprite.load("creatures.json").expect("sheet loads");
    let machine = animators.load("anim/player.animsm.ron").expect("machine loads");
    commands.spawn((
        Sprite::from_sheet(art.sheet, 0).with_size(vec2(64.0, 64.0)),
        Transform2D::default(),
        Animator::new(machine),
        AnimationPlayer::play(art.clips["player_idle"]),
        Player,
    ));
    // A gem looping its sparkle clip directly (no state machine needed).
    commands.spawn((
        Sprite::from_sheet(art.sheet, 6).with_size(vec2(48.0, 48.0)),
        Transform2D::from_xy(120.0, 0.0),
        AnimationPlayer::play(art.clips["gem"]),
    ));
}

/// The machine switches idle <-> run purely from the `speed` parameter.
fn movement(
    mut players: Query<(&mut Transform2D, &mut Animator), With<Player>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) { dir.x -= 1.0 }
    if input.pressed(Key::D) { dir.x += 1.0 }
    if input.pressed(Key::S) { dir.y -= 1.0 }
    if input.pressed(Key::W) { dir.y += 1.0 }
    let velocity = dir.normalize_or_zero() * 150.0;
    for (mut transform, mut animator) in &mut players {
        transform.translation += velocity * time.fixed_delta;
        animator.set_float("speed", velocity.length());
    }
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Grove".into(),
        clear_color: Color::rgb(0.16, 0.24, 0.16),
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_system(movement)
    .run();
}
