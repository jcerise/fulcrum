//! Chapter 12: spatial queries and pathfinding — the fox learns to hunt through the hedges,
//! and gem pickup stops being an every-gem distance loop.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;
#[derive(Component)]
struct Fox {
    path: Vec<(u32, u32)>,
}
#[derive(Component)]
struct Gem;

/// The grove's walkability, built once from the tilemap's `walls` layer.
#[derive(Resource)]
struct Nav(NavGrid);

fn setup_map(mut commands: Commands, mut maps: TilemapLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: 480.0,
        height: 270.0,
    };
    camera.center = vec2(320.0, 190.0);
    camera.zoom = 0.75;

    let map = maps.load("maps/grove.map.ron").expect("map loads");
    commands.spawn((Tilemap { asset: map, z: 0.0 }, Transform2D::default()));
    // Wall tiles cost None (blocked); everything else costs 10 (the uniform step).
    let nav = NavGrid::from_tilemap(
        maps.assets().get(map).expect("just loaded"),
        "walls",
        Vec2::ZERO, // the tilemap entity's translation: its bottom-left corner
        |tile| (tile == 0).then_some(10),
    )
    .expect("map has a walls layer");
    commands.insert_resource(Nav(nav));
}

/// Separate startup system: TilemapLoader and AsepriteLoader both write texture assets.
fn setup_creatures(mut commands: Commands, mut aseprite: AsepriteLoader) {
    let art = aseprite.load("creatures.json").expect("sheet loads");
    commands.spawn((
        Sprite::from_sheet(art.sheet, 0).with_z(5.0),
        Transform2D::from_xy(72.0, 72.0),
        AnimationPlayer::play(art.clips["player_idle"]),
        Player,
    ));
    commands.spawn((
        Sprite::from_sheet(art.sheet, 4).with_z(5.0),
        Transform2D::from_xy(520.0, 300.0),
        AnimationPlayer::play(art.clips["fox"]),
        Fox { path: Vec::new() },
    ));
    for (x, y) in [(150.0, 260.0), (300.0, 90.0), (430.0, 200.0), (90.0, 330.0)] {
        commands.spawn((
            Sprite::from_sheet(art.sheet, 6).with_z(3.0),
            Transform2D::from_xy(x, y),
            AnimationPlayer::play(art.clips["gem"]),
            Gem,
            SpatialIndexed, // opt in to the spatial grid
        ));
    }
}

fn movement(
    mut players: Query<&mut Transform2D, With<Player>>,
    nav: Res<Nav>,
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
    let step = dir.normalize_or_zero() * 90.0 * time.fixed_delta;
    // Axis-separated clamp against the same nav grid the fox plans with.
    let target = vec2(transform.translation.x + step.x, transform.translation.y);
    if walkable(&nav.0, target) {
        transform.translation = target;
    }
    let target = vec2(transform.translation.x, transform.translation.y + step.y);
    if walkable(&nav.0, target) {
        transform.translation = target;
    }
}

fn walkable(nav: &NavGrid, world: Vec2) -> bool {
    nav.world_to_cell(world)
        .is_some_and(|(x, y)| nav.is_walkable(x, y))
}

/// Re-plan every 20 ticks, then walk the waypoints. A* costs microseconds on a 40x24 grid —
/// re-planning per tick would be fine too; the interval just shows the idiom.
fn fox_hunt(
    mut foxes: Query<(&mut Transform2D, &mut Fox), Without<Player>>,
    players: Query<&Transform2D, With<Player>>,
    nav: Res<Nav>,
    time: Res<Time>,
) {
    let Ok(player) = players.single() else { return };
    for (mut transform, mut fox) in &mut foxes {
        if time.tick.is_multiple_of(20)
            && let (Some(from), Some(to)) = (
                nav.0.world_to_cell(transform.translation),
                nav.0.world_to_cell(player.translation),
            )
            && let Some(path) = astar(&nav.0, from, to)
        {
            fox.path = simplify_path(&nav.0, &path); // line-of-sight shortcuts
            fox.path.reverse(); // walk it back-to-front with pop()
        }
        while let Some(&next) = fox.path.last() {
            let waypoint = nav.0.cell_center(next.0, next.1);
            let to_waypoint = waypoint - transform.translation;
            if to_waypoint.length() < 4.0 {
                fox.path.pop();
                continue;
            }
            transform.translation += to_waypoint.normalize() * 65.0 * time.fixed_delta;
            break;
        }
    }
}

/// One grid lookup instead of a loop over every gem in the world.
fn collect(
    mut commands: Commands,
    players: Query<&Transform2D, With<Player>>,
    grid: Res<SpatialGrid>,
    gems: Query<(), With<Gem>>,
) {
    let Ok(player) = players.single() else { return };
    for entity in grid.query_circle(player.translation, 14.0) {
        if gems.get(entity).is_ok() {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Watch the fox think: F1 draws its current plan.
fn draw_path(
    foxes: Query<(&Transform2D, &Fox)>,
    nav: Res<Nav>,
    mut gizmos: ResMut<Gizmos>,
    input: Res<Input>,
    mut on: Local<bool>,
) {
    if input.just_pressed(Key::F1) {
        *on = !*on;
    }
    if !*on {
        return;
    }
    for (transform, fox) in &foxes {
        let mut from = transform.translation;
        for &(x, y) in fox.path.iter().rev() {
            let to = nav.0.cell_center(x, y);
            gizmos.line(from, to, Color::rgba(1.0, 0.5, 0.2, 0.8));
            from = to;
        }
    }
}

fn main() {
    Fulcrum::new("ch12: pathfinding")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .with_plugin(SpatialPlugin::default())
        .add_startup((setup_map, setup_creatures))
        .add_system((movement, fox_hunt, collect).chain())
        .add_frame_system(draw_path)
        .run();
}
