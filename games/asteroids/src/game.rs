//! Asteroids simulation: ship physics, screen wrap, bullets, splitting rocks, waves, lives.
//! Pure logic — no sprites or audio — so it runs headless. Sounds are signalled via
//! [`GameSound`] events that the binary turns into playback.

use fulcrum::prelude::*;

/// Playfield size in world units (matches the letterbox virtual resolution).
pub const ARENA: Vec2 = Vec2::new(800.0, 600.0);
/// Ship collision radius.
pub const SHIP_RADIUS: f32 = 9.0;
/// Ship turn rate, radians/second.
pub const ROTATE_SPEED: f32 = 3.8;
/// Ship acceleration, units/second^2.
pub const THRUST: f32 = 330.0;
/// Ship speed cap.
pub const MAX_SPEED: f32 = 420.0;
/// Bullet speed.
pub const BULLET_SPEED: f32 = 520.0;
/// Bullet lifetime in ticks.
pub const BULLET_TTL: u32 = 55;
/// Minimum ticks between shots.
pub const FIRE_COOLDOWN: u32 = 12;
/// Post-respawn invulnerability in ticks.
pub const INVULN_TICKS: u32 = 120;
/// Starting lives.
pub const START_LIVES: u32 = 3;

/// The player's ship.
#[derive(Component)]
pub struct Ship;

/// Whether the ship is thrusting this tick (drives the flame animation).
#[derive(Component)]
pub struct Thrusting(pub bool);

/// Ticks of remaining spawn protection.
#[derive(Component)]
pub struct Invulnerable(pub u32);

/// A rock. Size 3 = big, 2 = medium, 1 = small.
#[derive(Component)]
pub struct Asteroid {
    /// 3, 2, or 1.
    pub size: u8,
}

/// A shot in flight.
#[derive(Component)]
pub struct Bullet {
    /// Remaining ticks before it fizzles.
    pub ttl: u32,
}

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// Angular velocity, radians/second (cosmetic tumble for rocks — still simulated for
/// determinism).
#[derive(Component)]
pub struct Spin(pub f32);

/// Points scored.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Score(pub u32);

/// Ships remaining.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct Lives(pub u32);

/// Current wave number (0 = none spawned yet).
#[derive(Resource, Default, Clone, Copy)]
pub struct Wave(pub u32);

#[derive(Resource, Default)]
struct FireCooldown(u32);

/// Playing or waiting for Enter.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    /// In play.
    #[default]
    Playing,
    /// Out of lives; Enter restarts.
    GameOver,
}

/// Sim-emitted audio cues, consumed by the binary's sound system.
#[derive(Event)]
pub enum GameSound {
    /// A shot was fired.
    Shoot,
    /// A rock (or the ship) blew up.
    Explode,
}

/// Collision radius for a rock size.
pub fn asteroid_radius(size: u8) -> f32 {
    match size {
        3 => 26.0,
        2 => 13.0,
        _ => 7.0,
    }
}

/// Points for destroying a rock of the given size.
pub fn asteroid_score(size: u8) -> u32 {
    match size {
        3 => 20,
        2 => 50,
        _ => 100,
    }
}

/// Installs the Asteroids simulation.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Score::default());
        app.world_mut().insert_resource(Lives(START_LIVES));
        app.world_mut().insert_resource(Wave::default());
        app.world_mut().insert_resource(FireCooldown::default());
        app.world_mut().insert_resource(GameState::default());
        app.register_event::<GameSound>();
        app.add_systems(Startup, spawn_ship);
        app.add_systems(
            FixedUpdate,
            (
                control_ship,
                integrate,
                wrap_positions,
                tick_bullets,
                bullets_hit_asteroids,
                asteroids_hit_ship,
                next_wave,
                restart_on_enter,
            )
                .chain(),
        );
    }
}

/// Spawn the player at the arena center. Public so the binary can order sprite attachment.
pub fn spawn_ship(mut commands: Commands) {
    commands.spawn((
        Ship,
        Thrusting(false),
        Invulnerable(0),
        Transform2D::default(),
        Velocity(Vec2::ZERO),
    ));
}

/// The direction the ship's nose points (sprite tip is +Y at rotation 0).
pub fn facing(rotation: f32) -> Vec2 {
    vec2(-rotation.sin(), rotation.cos())
}

#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
fn control_ship(
    mut ships: Query<(&mut Transform2D, &mut Velocity, &mut Thrusting), With<Ship>>,
    mut commands: Commands,
    mut cooldown: ResMut<FireCooldown>,
    mut sounds: EventWriter<GameSound>,
    input: Res<Input>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    cooldown.0 = cooldown.0.saturating_sub(1);
    if *state != GameState::Playing {
        return;
    }
    let Ok((mut transform, mut velocity, mut thrusting)) = ships.single_mut() else {
        return;
    };
    if input.pressed(Key::A) {
        transform.rotation += ROTATE_SPEED * time.fixed_delta;
    }
    if input.pressed(Key::D) {
        transform.rotation -= ROTATE_SPEED * time.fixed_delta;
    }
    let dir = facing(transform.rotation);
    thrusting.0 = input.pressed(Key::W);
    if thrusting.0 {
        velocity.0 += dir * THRUST * time.fixed_delta;
        if velocity.0.length() > MAX_SPEED {
            velocity.0 = velocity.0.normalize() * MAX_SPEED;
        }
    }
    if input.pressed(Key::Space) && cooldown.0 == 0 {
        cooldown.0 = FIRE_COOLDOWN;
        commands.spawn((
            Bullet { ttl: BULLET_TTL },
            Transform2D::from_translation(transform.translation + dir * 12.0),
            Velocity(dir * BULLET_SPEED),
        ));
        sounds.write(GameSound::Shoot);
    }
}

fn integrate(
    mut movers: Query<(&mut Transform2D, &Velocity, Option<&Spin>)>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    for (mut transform, velocity, spin) in &mut movers {
        transform.translation += velocity.0 * time.fixed_delta;
        if let Some(spin) = spin {
            transform.rotation += spin.0 * time.fixed_delta;
        }
    }
}

fn wrap_positions(mut movers: Query<&mut Transform2D, With<Velocity>>) {
    let half = ARENA / 2.0;
    let margin = 24.0;
    for mut transform in &mut movers {
        let p = &mut transform.translation;
        if p.x > half.x + margin {
            p.x = -half.x - margin;
        } else if p.x < -half.x - margin {
            p.x = half.x + margin;
        }
        if p.y > half.y + margin {
            p.y = -half.y - margin;
        } else if p.y < -half.y - margin {
            p.y = half.y + margin;
        }
    }
}

fn tick_bullets(mut commands: Commands, mut bullets: Query<(Entity, &mut Bullet)>) {
    for (entity, mut bullet) in &mut bullets {
        bullet.ttl = bullet.ttl.saturating_sub(1);
        if bullet.ttl == 0 {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
fn bullets_hit_asteroids(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform2D), With<Bullet>>,
    asteroids: Query<(Entity, &Transform2D, &Asteroid)>,
    mut score: ResMut<Score>,
    mut rng: ResMut<SimRng>,
    mut sounds: EventWriter<GameSound>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    let mut spent_bullets: Vec<Entity> = Vec::new();
    let mut destroyed: Vec<Entity> = Vec::new();
    for (bullet, bullet_at) in &bullets {
        if spent_bullets.contains(&bullet) {
            continue;
        }
        for (rock, rock_at, asteroid) in &asteroids {
            if destroyed.contains(&rock) {
                continue;
            }
            let hit = bullet_at.translation.distance(rock_at.translation)
                < asteroid_radius(asteroid.size);
            if !hit {
                continue;
            }
            spent_bullets.push(bullet);
            destroyed.push(rock);
            score.0 += asteroid_score(asteroid.size);
            sounds.write(GameSound::Explode);
            // Split into two smaller rocks flying apart.
            if asteroid.size > 1 {
                for _ in 0..2 {
                    let angle = rng.range_f32(0.0..std::f32::consts::TAU);
                    let speed = rng.range_f32(60.0..160.0) * (4 - asteroid.size) as f32;
                    spawn_asteroid(
                        &mut commands,
                        asteroid.size - 1,
                        rock_at.translation,
                        vec2(angle.cos(), angle.sin()) * speed,
                        rng.range_f32(-2.0..2.0),
                    );
                }
            }
            break;
        }
    }
    for entity in spent_bullets.into_iter().chain(destroyed) {
        commands.entity(entity).despawn();
    }
}

#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
fn asteroids_hit_ship(
    mut ships: Query<(&mut Transform2D, &mut Velocity, &mut Invulnerable), With<Ship>>,
    asteroids: Query<(&Transform2D, &Asteroid), Without<Ship>>,
    mut lives: ResMut<Lives>,
    mut state: ResMut<GameState>,
    mut sounds: EventWriter<GameSound>,
) {
    if *state != GameState::Playing {
        return;
    }
    let Ok((mut transform, mut velocity, mut invulnerable)) = ships.single_mut() else {
        return;
    };
    invulnerable.0 = invulnerable.0.saturating_sub(1);
    if invulnerable.0 > 0 {
        return;
    }
    let hit = asteroids.iter().any(|(rock, asteroid)| {
        transform.translation.distance(rock.translation)
            < asteroid_radius(asteroid.size) + SHIP_RADIUS
    });
    if hit {
        sounds.write(GameSound::Explode);
        lives.0 = lives.0.saturating_sub(1);
        if lives.0 == 0 {
            *state = GameState::GameOver;
        } else {
            transform.translation = Vec2::ZERO;
            transform.rotation = 0.0;
            velocity.0 = Vec2::ZERO;
            invulnerable.0 = INVULN_TICKS;
        }
    }
}

fn spawn_asteroid(commands: &mut Commands, size: u8, at: Vec2, velocity: Vec2, spin: f32) {
    commands.spawn((
        Asteroid { size },
        Transform2D::from_translation(at),
        Velocity(velocity),
        Spin(spin),
    ));
}

/// When the field is clear, spawn the next wave along the arena edges, away from the ship.
fn next_wave(
    mut commands: Commands,
    asteroids: Query<(), With<Asteroid>>,
    mut wave: ResMut<Wave>,
    mut rng: ResMut<SimRng>,
    state: Res<GameState>,
) {
    if *state != GameState::Playing || asteroids.iter().len() > 0 {
        return;
    }
    wave.0 += 1;
    for _ in 0..(2 + wave.0.min(6)) {
        // Somewhere along the border, clear of the center spawn area.
        let along = rng.range_f32(-0.5..0.5);
        let position = if rng.chance(0.5) {
            vec2(along * ARENA.x, ARENA.y / 2.0 - 10.0)
        } else {
            vec2(ARENA.x / 2.0 - 10.0, along * ARENA.y)
        };
        let angle = rng.range_f32(0.0..std::f32::consts::TAU);
        let speed = rng.range_f32(40.0..110.0);
        spawn_asteroid(
            &mut commands,
            3,
            position,
            vec2(angle.cos(), angle.sin()) * speed,
            rng.range_f32(-1.2..1.2),
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // standard ECS system shapes
fn restart_on_enter(
    mut commands: Commands,
    mut ships: Query<(&mut Transform2D, &mut Velocity, &mut Invulnerable), With<Ship>>,
    leftovers: Query<Entity, Or<(With<Asteroid>, With<Bullet>)>>,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut wave: ResMut<Wave>,
    mut state: ResMut<GameState>,
    input: Res<Input>,
) {
    if *state != GameState::GameOver || !input.just_pressed(Key::Enter) {
        return;
    }
    for entity in &leftovers {
        commands.entity(entity).despawn();
    }
    *score = Score::default();
    *lives = Lives(START_LIVES);
    *wave = Wave::default();
    *state = GameState::Playing;
    if let Ok((mut transform, mut velocity, mut invulnerable)) = ships.single_mut() {
        *transform = Transform2D::default();
        velocity.0 = Vec2::ZERO;
        invulnerable.0 = INVULN_TICKS;
    }
}
