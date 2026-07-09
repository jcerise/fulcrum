//! Chapter 4: a world — tilemap, camera scaling, collision, camera follow.

use fulcrum::prelude::*;

#[derive(Component)]
struct Player;

fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut maps: TilemapLoader,
    mut camera: ResMut<Camera2D>,
) {
    camera.scaling = ScalingMode::Letterbox { width: 480.0, height: 270.0 };
    camera.center = vec2(72.0, 72.0);
    let map = maps.load("maps/grove.map.ron").expect("map loads");
    commands.spawn((Tilemap { asset: map, z: 0.0 }, Transform2D::default()));
    let hero = assets.load("hero.png");
    commands.spawn((
        Sprite::new(hero).with_z(5.0),
        Transform2D::from_xy(72.0, 72.0),
        Player,
    ));
}

fn blocked(
    maps: &Query<(&Tilemap, &Transform2D), Without<Player>>,
    assets: &Assets<TilemapAsset>,
    position: Vec2,
) -> bool {
    for (map, transform) in maps.iter() {
        let Some(asset) = assets.get(map.asset) else { continue };
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

fn movement(
    mut players: Query<&mut Transform2D, With<Player>>,
    maps: Query<(&Tilemap, &Transform2D), Without<Player>>,
    map_assets: Res<Assets<TilemapAsset>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) { dir.x -= 1.0 }
    if input.pressed(Key::D) { dir.x += 1.0 }
    if input.pressed(Key::S) { dir.y -= 1.0 }
    if input.pressed(Key::W) { dir.y += 1.0 }
    let step = dir.normalize_or_zero() * 110.0 * time.fixed_delta;
    for mut transform in &mut players {
        // Axis-separated so you slide along walls instead of sticking to them.
        let radius = 6.0;
        for axis_step in [vec2(step.x, 0.0), vec2(0.0, step.y)] {
            let target = transform.translation + axis_step;
            let corners = [
                target + vec2(-radius, -radius),
                target + vec2(radius, -radius),
                target + vec2(radius, radius),
                target + vec2(-radius, radius),
            ];
            if !corners.iter().any(|&c| blocked(&maps, &map_assets, c)) {
                transform.translation = target;
            }
        }
    }
}

/// Cosmetic: glide the camera toward the player once per frame.
fn camera_follow(
    mut camera: ResMut<Camera2D>,
    players: Query<&Transform2D, With<Player>>,
    time: Res<Time>,
) {
    if let Ok(player) = players.single() {
        let center = camera.center;
        camera.center = center + (player.translation - center) * (5.0 * time.frame_delta).min(1.0);
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
    .add_frame_system(camera_follow)
    .run();
}
