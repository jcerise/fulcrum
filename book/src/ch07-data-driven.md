# Going Data-Driven: Prefabs and Scenes

Everything so far spawned entities from Rust. That's fine for six gems in a ring; it's misery
for a real level, and it makes every content tweak a recompile. This chapter is where Fulcrum
shows its hand: **content is files.** From here on, Grove's entities are data, and the code
only supplies behavior.

## The component registry

The bridge between files and the ECS is the `ComponentRegistry`: a map from stable string
names to typed serialization. Engine components are pre-registered; yours take one line each:

```rust,ignore
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct MoveStats { pub speed: f32 }

let app = app.register_component::<MoveStats>("MoveStats");
```

Anything registered can appear in a prefab, a scene, the inspector — and, in the future,
a mod.

## Prefabs

A prefab is one entity's definition:

```text
Prefab(
    components: {
        "Name": ("player"),
        "Transform2D": (translation: (72.0, 72.0)),
        "Sprite": (sheet: "creatures.json", region: "player idle 0", z: 5.0),
        "Animator": (machine: "anim/player.animsm.ron"),
        "PlayerTag": (),
        "MoveStats": (speed: 110.0),
    },
)
```

Notes on the dialect: `()` means "this component, all defaults" — that's how markers like
`PlayerTag` appear. Optional fields take bare values (no `Some(...)`). And assets are always
referenced **by path** — handles never appear in files; the engine resolves them when the
entity spawns.

Spawning is one call, from anywhere:

```rust,ignore
let slime = prefabs.load("prefabs/gem.prefab.ron")?;
commands.spawn_prefab_at(slime, vec2(300.0, 90.0));
```

You get the `Entity` back immediately; the components apply at the next tick boundary, in
queue order — deterministic, like everything in the sim.

## Scenes

A scene is a level: prefab instances plus one-off entities, loaded and unloaded as a unit.

```text
Scene(
    entities: [
        ( components: { "Name": ("map"), "Transform2D": (), "Tilemap": (asset: "maps/grove.map.ron") } ),
        ( prefab: "prefabs/player.prefab.ron" ),
        ( prefab: "prefabs/fox.prefab.ron", at: (520.0, 300.0) ),
        ( prefab: "prefabs/gem.prefab.ron", at: (150.0, 260.0) ),
        // ... seven more gems
    ],
)
```

```rust,ignore
let level = scenes.load("scenes/grove.scene.ron")?;
spawner.load(level);          // spawn everything, tagged as members of this scene
spawner.unload(level);        // despawn exactly those entities
```

`unload` + `load` at the next tick boundary is the level-transition (and the restart) idiom —
Grove's "play again" is literally those two lines.

## Hot reload

With `hot_reload: true` (the debug-build default), the engine watches your asset root.
Run Grove and, while it's running:

- repaint `tiles.png` — the garden re-textures in place;
- edit the map RON — the level re-meshes;
- change `gem.prefab.ron` — future spawns (say, after a restart) use the new definition;
- tweak the animation state machine — the player's gait changes live.

The rule of thumb: **assets reload in place; live entities are not retro-patched.** A prefab
edit shows up when something next spawns from it — restarting the scene is the workflow. One
honest caveat: a mid-run reload makes that run non-reproducible, which is exactly what you'd
expect and only matters for replays.

Grove's `main.rs` after this chapter contains *zero* entity composition — check
`games/grove/` in the repository to see the finished shape.
