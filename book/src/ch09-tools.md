# The Development Loop: Tools

A game grows at the speed of its feedback loop. Fulcrum ships three tools that keep the loop
tight, all free in debug builds.

## Gizmos: immediate-mode debug drawing

The one place Fulcrum lets you draw imperatively — because debug overlays shouldn't require
entities:

```rust,ignore
fn debug_circles(mut gizmos: ResMut<Gizmos>, players: Query<&Transform2D, With<PlayerTag>>) {
    for p in &players {
        gizmos.circle(p.translation, PICKUP_RANGE, Color::GREEN);
        gizmos.line(Vec2::ZERO, p.translation, Color::RED);
        gizmos.rect(Rect::from_center_size(p.translation, vec2(20.0, 20.0)), Color::WHITE);
    }
}
```

World-space, drawn above everything, cleared every frame. `FulcrumConfig::gizmos_enabled`
defaults to on in debug and off in release, where every call becomes a no-op — leave your
debug drawing in; shipped players never pay for it. Grove binds its collision/aggro circles
to F1.

## The inspector (F12)

`DefaultPlugins` includes an [egui](https://github.com/emilk/egui)-based overlay in debug
builds. Press **F12** in any Fulcrum game:

- **Performance** — frame time, tick count, sprites/batches/chunks per frame.
- **Entities** — every entity, named via the `Name` component, with its registered components
  rendered as *editable* widgets. Drag the fox's `MoveStats.speed` while it chases you.
  (Edits go through the same registry as prefabs — and the first one logs a note that the
  run is no longer replay-reproducible.)
- **Assets** — everything loaded, with force-reload buttons.

When the inspector captures your mouse or keyboard it consumes those events before the game's
`Input` sees them, and `Res<DebugUiFocus>` is there if you need to check.

## Hot reload as a way of working

Chapters 7 and 8 covered the mechanics; the habit is the point. A comfortable Grove session
looks like: game running on one monitor, editor on the other —

1. nudge `player.animsm.ron` thresholds until the run animation kicks in right;
2. repaint the hedge tile; watch the garden change;
3. F12, drag the fox faster, decide you hate it, drag it back;
4. once it feels right, copy the number into `fox.prefab.ron` — now it's permanent.

No recompiles. The compile-run-check loop is for *behavior*; content iterates live.
