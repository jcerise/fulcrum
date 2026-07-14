//! The Dojo simulation: a hero, a training dummy, and rules keyed to animation frames.
//!
//! This game exists to demonstrate Fulcrum's central animation claim: animation is
//! *simulation state*. The sword connects exactly when the attack clip shows its extension
//! frame, and the dummy's rebound bonks you exactly when its wobble swings back — facts a
//! headless test can assert tick by tick, because clips and state machines load and advance
//! without a GPU. See The Animation Book (`books/animation/`).

use fulcrum::prelude::*;

/// Arena size in world units (letterboxed by the camera; 16 px art at 1:1).
pub const ARENA_W: f32 = 320.0;
/// See [`ARENA_W`].
pub const ARENA_H: f32 = 180.0;

/// Hero walk speed, world units per second.
pub const HERO_SPEED: f32 = 90.0;
/// The attack clip's frame indices are gameplay constants: the blade is out on this frame.
pub const STRIKE_FRAME: usize = 2;
/// How far the extension frame reaches.
pub const STRIKE_RANGE: f32 = 26.0;
/// The dummy's hit clip swings back toward you on this frame.
pub const REBOUND_FRAME: usize = 1;
/// Stand closer than this during the rebound and you get bonked.
pub const REBOUND_RANGE: f32 = 16.0;

/// The player. Facing is simulation state (the strike checks it); the sprite flip that
/// *shows* it lives in the presentation.
#[derive(Component)]
pub struct Hero {
    /// True when the hero last moved leftward.
    pub facing_left: bool,
    /// Has the current swing already connected? One swing, one bonk.
    pub swing_connected: bool,
}

/// The training dummy.
#[derive(Component)]
pub struct Dummy {
    /// Has the current wobble already bonked the hero back?
    pub rebounded: bool,
}

/// Scorekeeping: hits you landed on the dummy, hits its rebound landed on you.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bonks {
    /// Sword strikes that connected.
    pub landed: u32,
    /// Rebounds that caught the hero standing too close.
    pub taken: u32,
}

/// Installs the whole simulation. Requires [`AnimPlugin`] (part of `DefaultPlugins`;
/// headless tests add it explicitly) — the animation system *is* gameplay here.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Bonks::default());
        app.add_systems(Startup, spawn_dojo);
        app.add_systems(FixedUpdate, (control_hero, strike, rebound).chain());
    }
}

/// Load the state machines and spawn both fighters. `AnimatorLoader` works headless — clip
/// timing and regions are simulation data; only texture upload needs a GPU — so this exact
/// startup runs in the windowed game and in tests.
fn spawn_dojo(mut commands: Commands, mut animators: AnimatorLoader) {
    let hero_machine = animators
        .load("anim/hero.animsm.ron")
        .expect("hero machine loads");
    let dummy_machine = animators
        .load("anim/dummy.animsm.ron")
        .expect("dummy machine loads");

    // Sprites start with an invalid sheet on purpose: the player writes the real sheet and
    // region on the first tick, from whatever clip the machine enters.
    commands.spawn((
        Hero {
            facing_left: false,
            swing_connected: false,
        },
        Sprite::from_sheet(Handle::INVALID, 0).with_z(2.0),
        Transform2D::from_xy(ARENA_W * 0.3, ARENA_H * 0.5),
        Animator::new(hero_machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
    commands.spawn((
        Dummy { rebounded: false },
        Sprite::from_sheet(Handle::INVALID, 0).with_z(1.0),
        Transform2D::from_xy(ARENA_W * 0.75, ARENA_H * 0.5),
        Animator::new(dummy_machine),
        AnimationPlayer::play(Handle::INVALID),
    ));
}

/// Movement and intent. The sim *tells* the machine facts (`speed`, triggers) and *asks* it
/// one question — what state are we in? — to decide whether the hero is committed to an
/// animation (attacking, flinching) and can't act.
pub fn control_hero(
    mut heroes: Query<(&mut Transform2D, &mut Hero, &mut Animator)>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut hero, mut animator)) = heroes.single_mut() else {
        return;
    };
    // Committed states lock the controls: attacking and flinching both mean "not now".
    // (On the very first tick the state is still empty — that counts as locked, one tick.)
    let free = matches!(animator.state(), "idle" | "run");

    let mut dir = Vec2::ZERO;
    if free {
        if input.pressed(Key::A) || input.pressed(Key::Left) {
            dir.x -= 1.0;
        }
        if input.pressed(Key::D) || input.pressed(Key::Right) {
            dir.x += 1.0;
        }
        if input.pressed(Key::S) || input.pressed(Key::Down) {
            dir.y -= 1.0;
        }
        if input.pressed(Key::W) || input.pressed(Key::Up) {
            dir.y += 1.0;
        }
    }
    let velocity = if dir == Vec2::ZERO {
        Vec2::ZERO
    } else {
        dir.normalize() * HERO_SPEED
    };
    if velocity.x != 0.0 {
        hero.facing_left = velocity.x < 0.0;
    }
    let next = transform.translation + velocity * time.fixed_delta;
    transform.translation = vec2(
        next.x.clamp(8.0, ARENA_W - 8.0),
        next.y.clamp(8.0, ARENA_H - 8.0),
    );

    animator.set_float("speed", velocity.length());
    if free && input.just_pressed(Key::Space) {
        animator.trigger("attack");
    }
}

/// The frame-keyed hitbox. Not "when Space was pressed", not "after 0.2 seconds" — the
/// strike connects on the exact tick the attack clip shows its extension frame. Animation
/// advances in `FixedUpdate`, so this is deterministic and testable.
#[allow(clippy::type_complexity)] // standard ECS system shape
pub fn strike(
    mut heroes: Query<(&Transform2D, &mut Hero, &Animator, &AnimationPlayer)>,
    mut dummies: Query<(&Transform2D, &mut Animator), (With<Dummy>, Without<Hero>)>,
    mut bonks: ResMut<Bonks>,
) {
    for (at, mut hero, animator, player) in &mut heroes {
        if animator.state() != "attack" {
            hero.swing_connected = false;
            continue;
        }
        if player.frame_index != STRIKE_FRAME || hero.swing_connected {
            continue;
        }
        for (dummy_at, mut dummy_animator) in &mut dummies {
            let delta = dummy_at.translation - at.translation;
            let in_front = if hero.facing_left {
                delta.x < 0.0
            } else {
                delta.x > 0.0
            };
            if in_front && delta.length() <= STRIKE_RANGE {
                dummy_animator.trigger("hit");
                hero.swing_connected = true;
                bonks.landed += 1;
            }
        }
    }
}

/// The dummy fights back — with physics-free comedy. Its hit clip tilts away, then swings
/// back through center; stand inside the arc on the return frame and you take a bonk. Same
/// mechanism as [`strike`], pointed the other way: the *dummy's* frames are the hitbox now.
#[allow(clippy::type_complexity)] // standard ECS system shape
pub fn rebound(
    mut dummies: Query<(&Transform2D, &mut Dummy, &Animator, &AnimationPlayer)>,
    mut heroes: Query<(&Transform2D, &mut Animator), (With<Hero>, Without<Dummy>)>,
    mut bonks: ResMut<Bonks>,
) {
    for (at, mut dummy, animator, player) in &mut dummies {
        if animator.state() != "hit" {
            dummy.rebounded = false;
            continue;
        }
        if player.frame_index != REBOUND_FRAME || dummy.rebounded {
            continue;
        }
        for (hero_at, mut hero_animator) in &mut heroes {
            if hero_at.translation.distance(at.translation) <= REBOUND_RANGE {
                hero_animator.trigger("hurt");
                dummy.rebounded = true;
                bonks.taken += 1;
            }
        }
    }
}
