//! From Zero, chapter 4: rules. Apples, growth, death, and restarts — the simulation now
//! lives in the real crate (`games/snake/src/game.rs`), and this example runs it with the
//! bare view from chapter 2. No sound, no score display yet: that's chapter 5.

use fulcrum::prelude::*;
use snake::game::{self, Apple, CELL, GRID_H, GRID_W, GamePlugin, Snake};

#[derive(Component)]
struct SegmentView;
#[derive(Resource)]
struct Square(Handle<Texture>);

fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: GRID_W as f32 * CELL,
        height: GRID_H as f32 * CELL,
    };
    camera.center = vec2(GRID_W as f32 * CELL / 2.0, GRID_H as f32 * CELL / 2.0);
    let square = assets.load("white.png");
    for x in 0..GRID_W {
        for y in 0..GRID_H {
            let shade = if (x + y) % 2 == 0 { 0.10 } else { 0.12 };
            commands.spawn((
                Sprite::new(square)
                    .with_color(Color::rgb(shade, shade + 0.02, shade))
                    .with_size(Vec2::splat(CELL))
                    .with_z(-1.0),
                Transform2D::from_translation(game::cell_center((x, y))),
            ));
        }
    }
    commands.insert_resource(Square(square));
}

fn project_snake(
    snake: Res<Snake>,
    mut segments: Query<(Entity, &mut Transform2D, &mut Sprite), With<SegmentView>>,
    square: Option<Res<Square>>,
    mut commands: Commands,
) {
    let Some(square) = square else { return };
    let mut views: Vec<_> = segments.iter_mut().collect();
    for _ in views.len()..snake.body.len() {
        commands.spawn((
            Sprite::new(square.0)
                .with_size(Vec2::splat(CELL - 2.0))
                .with_z(2.0),
            Transform2D::default(),
            SegmentView,
        ));
    }
    for (entity, _, _) in views.iter().skip(snake.body.len()) {
        commands.entity(*entity).try_despawn();
    }
    for (index, ((_, transform, sprite), cell)) in
        views.iter_mut().zip(snake.body.iter()).enumerate()
    {
        transform.translation = game::cell_center(*cell);
        sprite.color = if index == 0 {
            Color::rgb(0.55, 1.0, 0.45)
        } else {
            Color::rgb(0.2, 0.65, 0.25)
        };
    }
}

/// The sim spawns apple *entities*; the view gives each one a sprite. (The sim placed a
/// `Transform2D` already — position is gameplay; how it looks is ours.)
fn dress_apples(
    undressed: Query<Entity, (With<Apple>, Without<Sprite>)>,
    square: Option<Res<Square>>,
    mut commands: Commands,
) {
    let Some(square) = square else { return };
    for apple in &undressed {
        commands.entity(apple).try_insert(
            Sprite::new(square.0)
                .with_color(Color::rgb(1.0, 0.35, 0.3))
                .with_size(Vec2::splat(CELL - 4.0))
                .with_z(1.0),
        );
    }
}

fn main() {
    Fulcrum::new("fz04: rules")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        .with_plugin(DefaultPlugins)
        .with_plugin(GamePlugin)
        .add_startup(setup)
        .add_frame_system(project_snake)
        .add_frame_system(dress_apples)
        .run();
}
