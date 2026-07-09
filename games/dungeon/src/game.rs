//! Dungeon simulation: tile collision, player control, monster AI, combat. Pure logic —
//! entity composition lives in prefabs/scenes, visuals in the binary.

use fulcrum::prelude::*;
use serde::{Deserialize, Serialize};

/// Tag from the player prefab.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct PlayerTag;

/// Tag + bounty from monster prefabs.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct MonsterTag {
    /// Gold dropped on death.
    pub gold: u32,
}

/// Hit points (shared by player and monsters).
#[derive(Component, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Health {
    pub max: i32,
    pub current: i32,
}

/// Movement speed, units/second.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct MoveStats {
    pub speed: f32,
}

/// Touch damage with a cooldown (ticks).
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct Melee {
    pub damage: i32,
    pub cooldown: u32,
    #[serde(default)]
    pub timer: u32,
}

/// Per-monster wander state (attached at runtime, not from data).
#[derive(Component, Default)]
pub struct Wander {
    pub direction: Vec2,
    pub ticks_left: u32,
}

/// Which way the player last moved (drives sprite flipping in the binary).
#[derive(Component, Default)]
pub struct FacingLeft(pub bool);

/// Player attack cooldown state.
#[derive(Resource, Default)]
pub struct AttackCooldown(pub u32);

/// Gold collected.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Gold(pub u32);

/// Sim state.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    #[default]
    Playing,
    Paused,
    Dead,
}

/// The level scene handle (for death-restart reloads).
#[derive(Resource)]
pub struct LevelScene(pub Handle<SceneAsset>);

pub const PLAYER_RADIUS: f32 = 6.0;
pub const MONSTER_RADIUS: f32 = 7.0;
pub const ATTACK_RANGE: f32 = 22.0;
pub const ATTACK_COOLDOWN: u32 = 30;
pub const AGGRO_RANGE: f32 = 110.0;

/// Installs the dungeon simulation and registers the game's data-driven components.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Gold::default());
        app.world_mut().insert_resource(GameState::default());
        app.world_mut().insert_resource(AttackCooldown::default());
        app.register_event::<UiEvent>();
        app.add_systems(
            FixedUpdate,
            (
                pause_and_restart,
                control_player,
                monster_ai,
                monster_touch,
                cleanup_dead,
            )
                .chain(),
        );
    }
}

/// Register the game's serializable components (call on the app before loading scenes).
pub fn register_components(app: Fulcrum) -> Fulcrum {
    app.register_component::<PlayerTag>("PlayerTag")
        .register_component::<MonsterTag>("MonsterTag")
        .register_component::<Health>("Health")
        .register_component::<MoveStats>("MoveStats")
        .register_component::<Melee>("Melee")
}

/// Tilemap entities (statically disjoint from the player and monsters, whose transforms other
/// systems mutate).
pub type MapQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Tilemap, &'static Transform2D),
    (Without<PlayerTag>, Without<MonsterTag>),
>;

/// Is the world position blocked by a wall tile?
pub fn blocked(maps: &MapQuery, assets: &Assets<TilemapAsset>, position: Vec2) -> bool {
    for (map, transform) in maps.iter() {
        let Some(asset) = assets.get(map.asset) else {
            continue;
        };
        match asset.world_to_tile(transform, position) {
            Some((x, y)) => {
                if asset.tile_at("walls", x, y).unwrap_or(0) != 0 {
                    return true;
                }
            }
            None => return true, // outside the map counts as solid
        }
    }
    false
}

fn move_with_collision(
    maps: &MapQuery,
    assets: &Assets<TilemapAsset>,
    from: Vec2,
    delta: Vec2,
    radius: f32,
) -> Vec2 {
    let mut position = from;
    // Axis-separated, corner-sampled: classic tile collision.
    let try_axis = |position: &mut Vec2, step: Vec2| {
        let target = *position + step;
        let corners = [
            target + vec2(-radius, -radius),
            target + vec2(radius, -radius),
            target + vec2(radius, radius),
            target + vec2(-radius, radius),
        ];
        if !corners.iter().any(|&corner| blocked(maps, assets, corner)) {
            *position = target;
        }
    };
    try_axis(&mut position, vec2(delta.x, 0.0));
    try_axis(&mut position, vec2(0.0, delta.y));
    position
}

fn pause_and_restart(
    mut state: ResMut<GameState>,
    mut gold: ResMut<Gold>,
    input: Res<Input>,
    mut events: EventReader<UiEvent>,
    scene: Option<Res<LevelScene>>,
    spawner: Option<ResMut<SceneSpawner>>,
) {
    // Escape toggles pause; UI Resume button unpauses; Enter restarts after death.
    if input.just_pressed(Key::Escape) {
        *state = match *state {
            GameState::Playing => GameState::Paused,
            GameState::Paused => GameState::Playing,
            dead => dead,
        };
    }
    for event in events.read() {
        let UiEvent::Clicked(id) = event;
        match id.as_str() {
            "resume" if *state == GameState::Paused => *state = GameState::Playing,
            "quit" if *state == GameState::Paused => std::process::exit(0),
            _ => {}
        }
    }
    if *state == GameState::Dead
        && input.just_pressed(Key::Enter)
        && let (Some(scene), Some(mut spawner)) = (scene, spawner)
    {
        spawner.unload(scene.0);
        spawner.load(scene.0);
        *gold = Gold(0);
        *state = GameState::Playing;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // standard ECS system shapes
fn control_player(
    mut players: Query<
        (
            &mut Transform2D,
            &MoveStats,
            &mut FacingLeft,
            Option<&mut Animator>,
        ),
        With<PlayerTag>,
    >,
    mut monsters: Query<(&Transform2D, &mut Health), (With<MonsterTag>, Without<PlayerTag>)>,
    maps: MapQuery,
    map_assets: Res<Assets<TilemapAsset>>,
    newcomers: Query<Entity, (With<PlayerTag>, Without<FacingLeft>)>,
    mut commands: Commands,
    mut cooldown: ResMut<AttackCooldown>,
    input: Res<Input>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    for entity in &newcomers {
        commands.entity(entity).insert(FacingLeft::default());
    }
    cooldown.0 = cooldown.0.saturating_sub(1);
    if *state != GameState::Playing {
        return;
    }
    let Ok((mut transform, stats, mut facing, animator)) = players.single_mut() else {
        return;
    };
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
    let velocity = if dir == Vec2::ZERO {
        Vec2::ZERO
    } else {
        dir.normalize() * stats.speed
    };
    if velocity.x != 0.0 {
        facing.0 = velocity.x < 0.0;
    }
    transform.translation = move_with_collision(
        &maps,
        &map_assets,
        transform.translation,
        velocity * time.fixed_delta,
        PLAYER_RADIUS,
    );

    let attacking = input.just_pressed(Key::Space) && cooldown.0 == 0;
    if attacking {
        cooldown.0 = ATTACK_COOLDOWN;
        for (monster_at, mut health) in &mut monsters {
            if monster_at.translation.distance(transform.translation) <= ATTACK_RANGE {
                health.current -= 1;
            }
        }
    }
    if let Some(mut animator) = animator {
        animator.set_float("speed", velocity.length());
        if attacking {
            animator.trigger("attack");
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // standard ECS system shapes
fn monster_ai(
    mut monsters: Query<
        (&mut Transform2D, &MoveStats, &mut Wander),
        (With<MonsterTag>, Without<PlayerTag>),
    >,
    newcomers: Query<Entity, (With<MonsterTag>, Without<Wander>)>,
    mut commands: Commands,
    players: Query<&Transform2D, With<PlayerTag>>,
    maps: MapQuery,
    map_assets: Res<Assets<TilemapAsset>>,
    mut rng: ResMut<SimRng>,
    time: Res<Time>,
    state: Res<GameState>,
) {
    for entity in &newcomers {
        commands.entity(entity).insert(Wander::default());
    }
    if *state != GameState::Playing {
        return;
    }
    let player_at = players.single().map(|t| t.translation).ok();
    for (mut transform, stats, mut wander) in &mut monsters {
        let to_player = player_at.map(|p| p - transform.translation);
        let chase = to_player.is_some_and(|d| d.length() < AGGRO_RANGE);
        let direction = if chase {
            to_player.unwrap().normalize_or_zero()
        } else {
            if wander.ticks_left == 0 {
                wander.ticks_left = rng.range_i32(40..140) as u32;
                let angle = rng.range_f32(0.0..std::f32::consts::TAU);
                // Sometimes stand still.
                wander.direction = if rng.chance(0.3) {
                    Vec2::ZERO
                } else {
                    vec2(angle.cos(), angle.sin())
                };
            }
            wander.ticks_left -= 1;
            wander.direction
        };
        let speed = if chase {
            stats.speed
        } else {
            stats.speed * 0.5
        };
        transform.translation = move_with_collision(
            &maps,
            &map_assets,
            transform.translation,
            direction * speed * time.fixed_delta,
            MONSTER_RADIUS,
        );
    }
}

#[allow(clippy::type_complexity)] // standard ECS system shapes
fn monster_touch(
    mut monsters: Query<(&Transform2D, &mut Melee), (With<MonsterTag>, Without<PlayerTag>)>,
    mut players: Query<(&Transform2D, &mut Health), With<PlayerTag>>,
    mut state: ResMut<GameState>,
) {
    if *state != GameState::Playing {
        return;
    }
    let Ok((player_at, mut health)) = players.single_mut() else {
        return;
    };
    for (monster_at, mut melee) in &mut monsters {
        melee.timer = melee.timer.saturating_sub(1);
        let touching = monster_at.translation.distance(player_at.translation)
            <= PLAYER_RADIUS + MONSTER_RADIUS + 2.0;
        if touching && melee.timer == 0 {
            melee.timer = melee.cooldown;
            health.current -= melee.damage;
            if health.current <= 0 {
                *state = GameState::Dead;
            }
        }
    }
}

fn cleanup_dead(
    mut commands: Commands,
    dead: Query<(Entity, &Health, Option<&MonsterTag>), Without<PlayerTag>>,
    mut gold: ResMut<Gold>,
) {
    for (entity, health, monster) in &dead {
        if health.current <= 0 {
            if let Some(monster) = monster {
                gold.0 += monster.gold;
            }
            commands.entity(entity).despawn();
        }
    }
}
