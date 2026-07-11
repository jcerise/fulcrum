//! From Zero, chapter 2: the game *is* its state. A snake drawn entirely from data — nothing
//! moves yet, because there are no rules yet. Change the cells in `main` and rerun.

use std::collections::VecDeque;

use fulcrum::prelude::*;

const GRID_W: i32 = 24;
const GRID_H: i32 = 18;
const CELL: f32 = 16.0;

/// A grid square, `(column, row)` from the bottom-left.
type Cell = (i32, i32);

/// The entire snake: an ordered list of cells, head first. This resource is the single
/// source of truth — the sprites below are a *view* of it.
#[derive(Resource)]
struct Snake {
    body: VecDeque<Cell>,
}

/// Marks sprites that mirror one body segment each.
#[derive(Component)]
struct SegmentView;

/// The one texture in the whole game: an 8x8 white square, tinted per use.
#[derive(Resource)]
struct Square(Handle<Texture>);

/// Grid coordinates -> world coordinates (cell centers; the grid's corner is the origin).
fn cell_center(cell: Cell) -> Vec2 {
    vec2(
        cell.0 as f32 * CELL + CELL / 2.0,
        cell.1 as f32 * CELL + CELL / 2.0,
    )
}

fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    // Show exactly the 384x288 grid regardless of window size (bars fill the rest).
    camera.scaling = ScalingMode::Letterbox {
        width: GRID_W as f32 * CELL,
        height: GRID_H as f32 * CELL,
    };
    camera.center = vec2(GRID_W as f32 * CELL / 2.0, GRID_H as f32 * CELL / 2.0);

    let square = assets.load("white.png");
    // A checkerboard floor, one tinted square per grid cell.
    for x in 0..GRID_W {
        for y in 0..GRID_H {
            let shade = if (x + y) % 2 == 0 { 0.10 } else { 0.12 };
            commands.spawn((
                Sprite::new(square)
                    .with_color(Color::rgb(shade, shade + 0.02, shade))
                    .with_size(Vec2::splat(CELL))
                    .with_z(-1.0),
                Transform2D::from_translation(cell_center((x, y))),
            ));
        }
    }
    // One apple, hard-coded for now (chapter 4 makes it appear by the rules).
    commands.spawn((
        Sprite::new(square)
            .with_color(Color::rgb(1.0, 0.35, 0.3))
            .with_size(Vec2::splat(CELL - 4.0))
            .with_z(1.0),
        Transform2D::from_translation(cell_center((18, 12))),
    ));
    commands.insert_resource(Square(square));
}

/// The projection: every frame, make the sprites agree with the state. Spawn views if the
/// body grew, despawn extras if it shrank, then reposition all of them. State -> pixels,
/// one direction only.
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
        transform.translation = cell_center(*cell);
        sprite.color = if index == 0 {
            Color::rgb(0.55, 1.0, 0.45) // the head, brighter
        } else {
            Color::rgb(0.2, 0.65, 0.25)
        };
    }
}

fn main() {
    Fulcrum::new("fz02: the game is its state")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        )))
        // The snake, as pure data. Reshape it and rerun — the view just follows.
        .insert_resource(Snake {
            body: VecDeque::from([
                (12, 9),
                (11, 9),
                (10, 9),
                (10, 8),
                (10, 7),
                (11, 7),
                (12, 7),
            ]),
        })
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_frame_system(project_snake)
        .run();
}
