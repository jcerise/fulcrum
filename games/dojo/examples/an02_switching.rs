//! Animation Book, chapter 2: switching clips by hand. Movement swaps idle/run, Space plays
//! a one-shot attack that locks movement until it finishes. Every line of `control` is
//! correct and necessary — and chapter 3 deletes all of it. This is the "before" picture.

use fulcrum::prelude::*;

/// Clip handles, stashed at startup so the control system can switch between them.
#[derive(Resource)]
struct Art {
    idle: Handle<AnimationClip>,
    run: Handle<AnimationClip>,
    attack: Handle<AnimationClip>,
}

#[derive(Component)]
struct HeroTag;

fn setup(mut commands: Commands, mut aseprite: AsepriteLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);

    let hero = aseprite.load("hero.json").expect("hero sheet loads");
    commands.insert_resource(Art {
        idle: hero.clips["idle"],
        run: hero.clips["run"],
        attack: hero.clips["attack"],
    });
    commands.spawn((
        HeroTag,
        Sprite::from_sheet(hero.sheet, 0).with_z(1.0),
        Transform2D::from_xy(160.0, 90.0),
        AnimationPlayer::play(hero.clips["idle"]),
    ));
}

/// The hand-rolled version: clip-switching logic tangled into gameplay. It works. It also
/// grows a new `if` for every state you add, and every game rewrites it. Count the rules
/// this function encodes — chapter 3 moves all of them into a data file.
fn control(
    mut heroes: Query<(&mut Transform2D, &mut AnimationPlayer, &mut Sprite), With<HeroTag>>,
    art: Option<Res<Art>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let (Some(art), Ok((mut transform, mut player, mut sprite))) = (art, heroes.single_mut())
    else {
        return;
    };

    // Rule: an attack in progress locks everything until its clip finishes.
    let attacking = player.clip == art.attack;
    if attacking && !player.finished() {
        return;
    }
    // Rule: a finished attack hands back to idle.
    if attacking && player.finished() {
        *player = AnimationPlayer::play(art.idle);
    }

    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::D) {
        dir.x += 1.0;
    }
    if input.pressed(Key::S) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::W) {
        dir.y += 1.0;
    }

    // Rule: Space starts the attack clip from frame zero, unconditionally.
    if input.just_pressed(Key::Space) {
        *player = AnimationPlayer::play(art.attack);
        return;
    }

    if dir != Vec2::ZERO {
        transform.translation += dir.normalize() * 90.0 * time.fixed_delta;
        if dir.x != 0.0 {
            sprite.flip_x = dir.x < 0.0;
        }
        // Rule: moving means the run clip — restart(), not play(), so holding a key doesn't
        // restart the clip every tick.
        player.restart(art.run);
    } else {
        // Rule: still means idle.
        player.restart(art.idle);
    }
}

fn main() {
    Fulcrum::new("an02: switching clips by hand")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system(control)
        .run();
}
