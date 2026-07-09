//! Grove's simulation: movement with tile collision, gem collection, and one persistent fox.
//! Pure logic — entity composition lives in `assets/` (prefabs + scene), visuals in `main.rs`.

use fulcrum::prelude::*;
use serde::{Deserialize, Serialize};

/// Marks the player (from `player.prefab.ron`).
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct PlayerTag;

/// Marks a collectible gem.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct GemTag;

/// Marks the fox.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct FoxTag;

/// Movement speed in units/second (data-driven per prefab).
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct MoveStats {
    pub speed: f32,
}

/// Runtime fox wander state.
#[derive(Component, Default)]
pub struct Wander {
    pub direction: Vec2,
    pub ticks_left: u32,
}

/// Which way the player last moved (drives sprite flipping in `main.rs`).
#[derive(Component, Default)]
pub struct FacingLeft(pub bool);

/// Collected / total gems this round.
#[derive(Resource, Default, Clone, Copy)]
pub struct Gems {
    pub collected: u32,
    pub total: u32,
}

/// Round state.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroveState {
    #[default]
    Playing,
    Won,
    Caught,
}

/// A gem was picked up this tick (the binary plays a chime).
#[derive(Event)]
pub struct GemCollected;

/// The level scene (win/lose restarts reload it).
#[derive(Resource)]
pub struct LevelScene(pub Handle<SceneAsset>);

pub const PLAYER_RADIUS: f32 = 6.0;
pub const PICKUP_RANGE: f32 = 12.0;
pub const FOX_RADIUS: f32 = 7.0;
pub const FOX_AGGRO: f32 = 130.0;

/// Tilemap entities (statically disjoint from things whose transforms the sim mutates).
pub type MapQuery<'w, 's> =
    Query<'w, 's, (&'static Tilemap, &'static Transform2D), (Without<PlayerTag>, Without<FoxTag>)>;

/// Installs the Grove simulation and registers its data-driven components.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Gems::default());
        app.world_mut().insert_resource(GroveState::default());
        app.register_event::<GemCollected>();
        app.add_systems(
            FixedUpdate,
            (control_player, fox_ai, collect_gems, end_and_restart).chain(),
        );
    }
}

/// One-line component registration for the app builder.
pub fn register_components(app: Fulcrum) -> Fulcrum {
    app.register_component::<PlayerTag>("PlayerTag")
        .register_component::<GemTag>("GemTag")
        .register_component::<FoxTag>("FoxTag")
        .register_component::<MoveStats>("MoveStats")
}

fn blocked(maps: &MapQuery, assets: &Assets<TilemapAsset>, position: Vec2) -> bool {
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
            None => return true,
        }
    }
    false
}

/// Axis-separated tile collision: try X, then Y, sampling the mover's corners.
pub fn move_with_collision(
    maps: &MapQuery,
    assets: &Assets<TilemapAsset>,
    from: Vec2,
    delta: Vec2,
    radius: f32,
) -> Vec2 {
    let mut position = from;
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // standard ECS system shapes
fn control_player(
    mut players: Query<(&mut Transform2D, &MoveStats, Option<&mut Animator>), With<PlayerTag>>,
    newcomers: Query<Entity, (With<PlayerTag>, Without<FacingLeft>)>,
    mut facings: Query<&mut FacingLeft>,
    mut commands: Commands,
    maps: MapQuery,
    map_assets: Res<Assets<TilemapAsset>>,
    input: Res<Input>,
    time: Res<Time>,
    state: Res<GroveState>,
) {
    for entity in &newcomers {
        commands.entity(entity).insert(FacingLeft::default());
    }
    if *state != GroveState::Playing {
        return;
    }
    let Ok((mut transform, stats, animator)) = players.single_mut() else {
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
    let velocity = if dir == Vec2::ZERO {
        Vec2::ZERO
    } else {
        dir.normalize() * stats.speed
    };
    if velocity.x != 0.0
        && let Ok(mut facing) = facings.single_mut()
    {
        facing.0 = velocity.x < 0.0;
    }
    transform.translation = move_with_collision(
        &maps,
        &map_assets,
        transform.translation,
        velocity * time.fixed_delta,
        PLAYER_RADIUS,
    );
    if let Some(mut animator) = animator {
        animator.set_float("speed", velocity.length());
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // standard ECS system shapes
fn fox_ai(
    mut foxes: Query<(&mut Transform2D, &MoveStats), (With<FoxTag>, Without<PlayerTag>)>,
    newcomers: Query<Entity, (With<FoxTag>, Without<Wander>)>,
    mut wanders: Query<&mut Wander>,
    mut commands: Commands,
    players: Query<&Transform2D, With<PlayerTag>>,
    maps: MapQuery,
    map_assets: Res<Assets<TilemapAsset>>,
    mut rng: ResMut<SimRng>,
    time: Res<Time>,
    mut state: ResMut<GroveState>,
) {
    for entity in &newcomers {
        commands.entity(entity).insert(Wander::default());
    }
    if *state != GroveState::Playing {
        return;
    }
    let Ok(player) = players.single().map(|t| t.translation) else {
        return;
    };
    for (mut transform, stats) in &mut foxes {
        let to_player = player - transform.translation;
        let direction = if to_player.length() < FOX_AGGRO {
            to_player.normalize_or_zero()
        } else if let Ok(mut wander) = wanders.single_mut() {
            if wander.ticks_left == 0 {
                wander.ticks_left = rng.range_i32(30..120) as u32;
                let angle = rng.range_f32(0.0..std::f32::consts::TAU);
                wander.direction = if rng.chance(0.25) {
                    Vec2::ZERO
                } else {
                    vec2(angle.cos(), angle.sin())
                };
            }
            wander.ticks_left -= 1;
            wander.direction
        } else {
            Vec2::ZERO
        };
        transform.translation = move_with_collision(
            &maps,
            &map_assets,
            transform.translation,
            direction * stats.speed * time.fixed_delta,
            FOX_RADIUS,
        );
        if to_player.length() < PLAYER_RADIUS + FOX_RADIUS {
            *state = GroveState::Caught;
        }
    }
}

#[allow(clippy::type_complexity)] // standard ECS system shapes
fn collect_gems(
    mut commands: Commands,
    gems: Query<(Entity, &Transform2D), With<GemTag>>,
    players: Query<&Transform2D, (With<PlayerTag>, Without<GemTag>)>,
    mut score: ResMut<Gems>,
    mut state: ResMut<GroveState>,
    mut events: EventWriter<GemCollected>,
) {
    if *state != GroveState::Playing {
        return;
    }
    let Ok(player) = players.single() else { return };
    // Total is (re)counted from the world so scene reloads just work.
    let remaining = gems.iter().len() as u32;
    score.total = score.collected + remaining;
    for (gem, at) in &gems {
        if at.translation.distance(player.translation) < PICKUP_RANGE {
            commands.entity(gem).despawn();
            score.collected += 1;
            events.write(GemCollected);
            if score.collected >= score.total {
                *state = GroveState::Won;
            }
        }
    }
}

fn end_and_restart(
    mut state: ResMut<GroveState>,
    mut score: ResMut<Gems>,
    input: Res<Input>,
    scene: Option<Res<LevelScene>>,
    spawner: Option<ResMut<SceneSpawner>>,
) {
    if *state == GroveState::Playing || !input.just_pressed(Key::Enter) {
        return;
    }
    if let (Some(scene), Some(mut spawner)) = (scene, spawner) {
        spawner.unload(scene.0);
        spawner.load(scene.0);
        *score = Gems::default();
        *state = GroveState::Playing;
    }
}
