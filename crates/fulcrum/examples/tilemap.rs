//! Step-8 (phase 2) acceptance: a 256x256 two-layer tilemap (131k tiles) rendered with chunk
//! culling. Arrows pan, Q/E zoom, Space toggles a tile (dirty-chunk rebuild). Prints stats once
//! per second. Run: `cargo run -p fulcrum --example tilemap --release`

use fulcrum::prelude::*;

#[derive(Resource)]
struct MapHandle(Handle<TilemapAsset>);

fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let texture = assets.load("crate.png"); // 16x16 -> 2x2 grid of 8px tiles
    let sheet = assets.add_sheet(SpriteSheet::from_grid(texture, vec2(8.0, 8.0), 2, 2));

    let size = 256u32;
    let make_layer = |name: &str, f: &dyn Fn(u32, u32) -> u32| TileLayer {
        name: name.into(),
        tiles: (0..size * size).map(|i| f(i % size, i / size)).collect(),
        width: size,
        height: size,
    };
    // Ground: checker of tiles 1/2. Detail: sparse tile 4.
    let ground = make_layer("ground", &|x, y| 1 + (x / 4 + y / 4) % 2);
    let detail = make_layer("detail", &|x, y| u32::from(x % 7 == 0 && y % 5 == 0) * 4);
    let map = TilemapAsset::new(sheet, vec2(16.0, 16.0), vec![ground, detail]);

    let handle = commands.spawn_empty().id(); // placeholder to satisfy borrow rules
    commands.entity(handle).despawn();
    commands.queue(move |world: &mut World| {
        let handle = world.resource_mut::<Assets<TilemapAsset>>().insert(map);
        world.spawn((
            Tilemap {
                asset: handle,
                z: 0.0,
            },
            // Center the 4096x4096-unit map on the origin.
            Transform2D::from_xy(-2048.0, -2048.0),
        ));
        world.insert_resource(MapHandle(handle));
    });
}

fn drive(
    mut camera: ResMut<Camera2D>,
    input: Res<Input>,
    time: Res<Time>,
    map: Option<Res<MapHandle>>,
    mut maps: ResMut<Assets<TilemapAsset>>,
    mut toggle: Local<bool>,
) {
    let dt = time.frame_delta;
    let mut pan = Vec2::ZERO;
    if input.pressed(Key::Left) {
        pan.x -= 1.0
    }
    if input.pressed(Key::Right) {
        pan.x += 1.0
    }
    if input.pressed(Key::Down) {
        pan.y -= 1.0
    }
    if input.pressed(Key::Up) {
        pan.y += 1.0
    }
    let zoom = camera.zoom;
    camera.center += pan * 600.0 * dt / zoom;
    if input.pressed(Key::Q) {
        camera.zoom *= 1.0 - dt
    }
    if input.pressed(Key::E) {
        camera.zoom *= 1.0 + dt
    }

    if input.just_pressed(Key::Space)
        && let Some(map) = map
        && let Some(asset) = maps.get_mut(map.0)
    {
        *toggle = !*toggle;
        asset.set_tile("ground", 128, 128, if *toggle { 3 } else { 1 });
    }
}

fn report(time: Res<Time>, stats: Res<RenderStats>, mut acc: Local<f32>, mut frames: Local<u32>) {
    *acc += time.frame_delta;
    *frames += 1;
    if *acc >= 1.0 {
        println!(
            "fps: {:>4} | quads: {:>6} | batches: {} | visible chunks: {}",
            *frames, stats.sprites, stats.batches, stats.tilemap_chunks
        );
        *acc = 0.0;
        *frames = 0;
    }
}

fn main() {
    Fulcrum::new("tilemap")
        .insert_resource(AssetServer::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/assets"
        )))
        .with_plugin(DefaultPlugins)
        .add_startup(setup)
        .add_frame_system(drive)
        .add_frame_system(report)
        .run();
}
