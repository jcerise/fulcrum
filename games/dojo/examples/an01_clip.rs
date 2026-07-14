//! Animation Book, chapter 1: a clip plays. One loader call turns an Aseprite export into a
//! sprite sheet plus named clips; an `AnimationPlayer` component plays one on a sprite.
//! Nothing here updates anything — the engine advances every player, every tick.

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut aseprite: AsepriteLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);

    let hero = aseprite.load("hero.json").expect("hero sheet loads");
    commands.spawn((
        Sprite::from_sheet(hero.sheet, 0).with_z(1.0),
        Transform2D::from_xy(120.0, 90.0),
        AnimationPlayer::play(hero.clips["idle"]),
    ));

    let dummy = aseprite.load("dummy.json").expect("dummy sheet loads");
    commands.spawn((
        Sprite::from_sheet(dummy.sheet, 0).with_z(1.0),
        Transform2D::from_xy(200.0, 90.0),
        AnimationPlayer::play(dummy.clips["idle"]),
    ));
}

fn main() {
    Fulcrum::new("an01: a clip plays")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .run();
}
