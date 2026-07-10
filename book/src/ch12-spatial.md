# The Fox Learns to Hunt: Spatial Queries and Pathfinding

Grove's fox has a dirty secret: it walks straight at you and bumps into hedges. And gem pickup
checks the distance to *every* gem, every tick — fine for eight, ruinous for eight hundred.
`fulcrum-spatial` fixes both, deterministically.

## The spatial grid: "what's near this point?"

Add `SpatialPlugin::default()` and tag entities that should be findable:

```rust,ignore
commands.spawn((Sprite::from_sheet(art.sheet, 6), Transform2D::from_xy(x, y),
                Gem, SpatialIndexed));
```

Every tick the engine rebuilds a uniform grid of all `SpatialIndexed` positions. Three queries
run against it, each returning entities in a **deterministic order** (grid cells visit in a
fixed pattern; ties break by entity id — same world, same result sequence, every run):

```rust,ignore
grid.query_circle(center, radius)      // Vec<Entity> within radius
grid.query_rect(rect)                  // Vec<Entity> inside a rectangle (drag-selection!)
grid.nearest(center, radius, |e| ...)  // closest entity passing your filter
```

Pickup becomes one lookup instead of a loop over the world:

```rust,ignore
fn collect(mut commands: Commands, players: Query<&Transform2D, With<Player>>,
           grid: Res<SpatialGrid>, gems: Query<(), With<Gem>>) {
    let Ok(player) = players.single() else { return };
    for entity in grid.query_circle(player.translation, 14.0) {
        if gems.get(entity).is_ok() {
            commands.entity(entity).try_despawn();
        }
    }
}
```

The baseline, from the engine's own tests: 10,000 indexed entities rebuild *and* answer 500
circle queries in under 2 ms. You will not outgrow this in a 2D game.

## The nav grid: "how do I get there?"

Pathfinding starts from the tilemap you already have. One call turns a layer into walkability:

```rust,ignore
let nav = NavGrid::from_tilemap(
    maps.assets().get(map).expect("just loaded"),
    "walls",
    Vec2::ZERO,                        // the tilemap entity's translation
    |tile| (tile == 0).then_some(10),  // None = blocked, Some(cost) = walkable
).expect("map has a walls layer");
```

The closure is your terrain policy: return `None` for blocked, or a cost per step — return
`Some(30)` for swamp tiles and paths will route around them when a detour is cheaper. Then ask
for a path:

```rust,ignore
if let Some(path) = astar(&nav, from_cell, to_cell) {
    fox.path = simplify_path(&nav, &path);  // line-of-sight shortcuts: fewer, straighter legs
}
```

A* here is 8-directional with no corner cutting (a diagonal never squeezes between two blocked
tiles — your fox will not clip through hedge corners), and its tie-breaking is fixed, so equal-
cost mazes yield the *same* path every run. On Grove's 40×24 grid a path costs microseconds;
the example re-plans every 20 ticks only to show the idiom. Following the path is plain
movement code: walk toward `nav.cell_center(next)`, pop the waypoint when close, repeat — and
the example draws the fox's current plan with gizmos on F1, which is the single best debugging
habit this chapter can teach you.

## When there are two hundred foxes

A* answers "one traveler, one goal." For *crowds* sharing a destination — every unit in a
selection ordered to the same spot — compute a `FlowField` once instead:

```rust,ignore
let field = FlowField::compute(&nav, &[goal_cell]);   // Dijkstra from the goal(s), once
let direction = field.sample(&nav, unit_position);    // then O(1) per unit per tick
```

Every walkable cell gets an arrow pointing downhill toward the nearest goal; unreachable cells
are flagged, never pointed into. A 512×512 field computes in ~35 ms — and on a game-sized map,
per move order, it's nothing. The RTS slice in `games/rts-slice` runs its whole battle on this:
one field per move command, shared by the group, with `query_circle` providing separation so
units don't stack. Two hundred units path and fight in ~0.4 ms a tick.

Everything in this chapter runs in `FixedUpdate` and follows the determinism rules — spatial
results are ordered, A* ties are fixed, flow fields are pure functions of the grid. That's not
incidental. It's what lets chapter 14 replay a battle from nothing but its inputs.

```text
cargo run -p grove --example ch12_pathfinding
```

Run from the fox. It cuts through the hedge gaps now. Press F1 to watch it think.
