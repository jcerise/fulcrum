# A World: Tilemaps and the Camera

A hero deserves somewhere to walk. Grove's garden is a **tilemap** — a RON file you can read,
diff, and (soon) hot-edit:

```text
Tilemap(
    texture: "tiles.png",        // the tile atlas image
    tile_size: (16.0, 16.0),
    sheet_cols: 2, sheet_rows: 2,
    layers: [
        Layer(name: "ground", tiles: [ [1,1,3,1, ...], ... ]),
        Layer(name: "walls",  tiles: [ [2,2,2,2, ...], ... ]),
    ],
)
```

Rows are written top-to-bottom, the way you see them. `0` means empty; any other number is a
tile from the atlas (`1` = first). Load it and spawn it like anything else:

```rust,ignore
fn setup(mut commands: Commands, mut maps: TilemapLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox { width: 480.0, height: 270.0 };
    let map = maps.load("maps/grove.map.ron").expect("map loads");
    commands.spawn((Tilemap { asset: map, z: 0.0 }, Transform2D::default()));
    // ... player as before
}
```

Under the hood the map renders as cached 32×32-tile chunks, culled against the camera and
re-meshed only when a tile changes — a 256×256 two-layer map draws in one call at over a
thousand frames per second, so you can stop thinking about it.

## The camera

`Camera2D` is a resource (one camera — an engine opinion) with a `center`, a `zoom`, and the
important choice: how the world fits the window.

| `ScalingMode` | Behavior |
| --- | --- |
| `Stretch` | 1 world unit = 1 window pixel; more window shows more world. |
| `FixedHeight(h)` | Fixed vertical world units; width follows the aspect ratio. |
| `Letterbox { w, h }` | A fixed virtual resolution, scaled to fit, black bars elsewhere. |
| `IntegerScale { w, h }` | Letterbox, but only whole-number scales — pixel art stays crisp. |

Grove uses `Letterbox { 480, 270 }`: the game *always* sees the same slice of garden, whatever
the monitor. Camera movement is presentation, so gliding after the player is a frame system:

```rust,ignore
fn camera_follow(mut camera: ResMut<Camera2D>, players: Query<&Transform2D, With<Player>>, time: Res<Time>) {
    if let Ok(player) = players.single() {
        let center = camera.center;
        camera.center = center + (player.translation - center) * (5.0 * time.frame_delta).min(1.0);
    }
}
```

## Walls that mean it

Tile data is simulation state, so collision queries are ordinary functions over the asset:

```rust,ignore
match asset.world_to_tile(map_transform, position) {
    Some((x, y)) => asset.tile_at("walls", x, y).unwrap_or(0) != 0,  // wall?
    None => true,                                                    // off-map = solid
}
```

The movement system tries X and Y separately, sampling the hero's four corners, so you slide
along hedges instead of sticking to them — the classic tile-collision recipe, spelled out in
full in `ch04_world.rs`. Run it and take a walk:

```text
cargo run -p grove --example ch04_world
```
