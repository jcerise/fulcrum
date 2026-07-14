//! Animation Book, chapter 3: the state machine. Identical behavior to an02 — idle, run,
//! one-shot attack — but every clip-switching rule now lives in `anim/hero.animsm.ron`, and
//! this file never mentions a clip again. Gameplay reports facts; the machine decides what
//! they look like. Tune the transitions in the RON file and rerun: no recompile.

use fulcrum::prelude::*;

#[derive(Component)]
struct HeroTag;

fn setup(mut commands: Commands, mut animators: AnimatorLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);

    let machine = animators
        .load("anim/hero.animsm.ron")
        .expect("hero machine loads");
    commands.spawn((
        HeroTag,
        Sprite::from_sheet(Handle::INVALID, 0).with_z(1.0),
        Transform2D::from_xy(160.0, 90.0),
        Animator::new(machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
}

/// Compare with an02's `control`: the clip rules are gone. What's left is gameplay — move,
/// face, report.
fn control(
    mut heroes: Query<(&mut Transform2D, &mut Animator, &mut Sprite), With<HeroTag>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut animator, mut sprite)) = heroes.single_mut() else {
        return;
    };
    let free = matches!(animator.state(), "idle" | "run");

    let mut dir = Vec2::ZERO;
    if free {
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
    }
    let velocity = if dir == Vec2::ZERO {
        Vec2::ZERO
    } else {
        dir.normalize() * 90.0
    };
    transform.translation += velocity * time.fixed_delta;
    if velocity.x != 0.0 {
        sprite.flip_x = velocity.x < 0.0;
    }

    animator.set_float("speed", velocity.length());
    if free && input.just_pressed(Key::Space) {
        animator.trigger("attack");
    }
}

fn main() {
    Fulcrum::new("an03: logic as data")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_system(control)
        .run();
}
