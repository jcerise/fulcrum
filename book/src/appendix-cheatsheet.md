# Cheat Sheet

Everything comes from `use fulcrum::prelude::*;`.

## App

```rust,ignore
Fulcrum::new("title")                      // defaults
Fulcrum::with_config(FulcrumConfig { .. }) // explicit
    .insert_resource(MyRes::default())
    .with_plugin(DefaultPlugins)           // window, render, assets, input, audio, anim, scenes, UI
    .with_plugin(SpatialPlugin::default()) // spatial grid (rebuilds each tick)
    .with_plugin(ModPlugin::from_dir("mods")) // discover + mount + script mods
    .with_plugin(MyGamePlugin)
    .add_event::<MyEvent>()
    .add_startup(setup)                    // once, before the first tick
    .add_system(sim_system)                // FixedUpdate: the deterministic simulation
    .add_system((a, b, c).chain())         // explicit ordering
    .add_frame_system(cosmetic_system)     // Update: once per frame, presentation only
    .register_component::<T>("T")          // data files / inspector / prefabs
    .run();
```

## Systems: what you can ask for

```rust,ignore
fn system(
    mut commands: Commands,                          // spawn/despawn/insert (deferred)
    query: Query<(&A, &mut B), (With<C>, Without<D>)>,
    res: Res<Time>, mut rng: ResMut<SimRng>,         // resources
    mut reader: EventReader<E>, mut writer: EventWriter<E>,
    mut assets: AssetLoader,                         // textures + fonts (+ add_sheet)
    mut sounds: SoundLoader, mut aseprite: AsepriteLoader,
    mut animators: AnimatorLoader, mut maps: TilemapLoader,
    mut prefabs: PrefabLoader, mut scenes: SceneLoader,
    mut effects: EffectLoader,                       // particle effects (cached by path)
    mut ui: UiQuery,                                 // set_label / set_visible by id
) { }
```

Loaders that write texture assets (`AssetLoader`, `AsepriteLoader`, `TilemapLoader`,
`EffectLoader`) conflict with each other — put them in separate systems.

## Core components & resources

| Thing | Use |
| --- | --- |
| `Transform2D { translation, rotation, scale }` | Where. +Y up, 1 unit = 1 px. |
| `Sprite::new(tex)` / `Sprite::from_sheet(sheet, i)` | What it looks like: `.with_color/size/z`, `flip_x/y`, `anchor`. |
| `Text::new("...")` | World-space text: `.with_size/color/align/z`. |
| `AnimationPlayer::play(clip)` | Frame animation in ticks; `restart()`, `finished()`. |
| `Animator::new(machine)` | State machine: `set_float`, `set_bool`, `trigger`, `state()`. |
| `Tilemap { asset, z }` | A map at this entity's translation. |
| `Name("...")` | Inspector label; registered as `"Name"`. |
| `Time` | `fixed_delta`/`tick` (sim) — `frame_delta`/`alpha` (frames). |
| `Input` | `pressed/just_pressed/just_released(Key)`, mouse in screen/world space. |
| `SimRng` | `range_f32/i32`, `chance`, `fork` — all sim randomness. |
| `Camera2D` | `center`, `zoom`, `rotation`, `scaling`; `screen_to_world`. |
| `Gizmos` | `line/rect/circle/point` — debug only, world space. |
| `Audio` | `play(_with)`, `play_music`, `set_master_volume`. |
| `SceneSpawner` | `load(scene)` / `unload(scene)` at the next tick. |
| `ParticleEmitter::new(fx)` | Particles at this entity; `commands.spawn_effect_at(fx, pos)` for one-shots. |
| `SpatialIndexed` | Opt into `SpatialGrid`: `query_circle/rect`, `nearest` — deterministic order. |
| `NavGrid` / `astar` / `FlowField` | Grid pathfinding: `from_tilemap`, `simplify_path`, `FlowField::compute/sample`. |
| `CommandOutbox` | `send(name, payload)` — the replayable player-command channel. |
| `ReplayRecorder` | With `FulcrumConfig::record_replays`; save via `Fulcrum::save_replay`. |
| `UiFocus` / `DebugUiFocus` | Is the pointer on UI / the inspector? |

## Data files

| File | Format |
| --- | --- |
| `*.prefab.ron` | `Prefab(components: { "Name": (..), ... }, children: [..])` |
| `*.scene.ron` | `Scene(entities: [ (prefab: "..", at: (x, y)), (components: {..}) ])` |
| `*.map.ron` | `Tilemap(texture, tile_size, sheet_cols/rows, layers)` |
| `*.animsm.ron` | `StateMachine(initial, params, states, transitions)` |
| `*.ui.ron` | `Ui(root: Node(anchor, size, stack, kind, children))` |
| `*.fx.ron` | `ParticleEffect(texture, mode: Burst(n)/Rate(hz), lifetime, size, colors, additive)` |
| `mod.ron` | `Mod(id, name, version, load_after, scripts)` — one per mod directory |
| `*.freplay` | Recorded run: header + per-tick inputs/commands + state hashes |
| `*.json` | Aseprite export (json-array + tags) |

Dialect: `()` = all defaults; optional fields take bare values; assets by path, never handles.

## Lua (mods — full contract in `crates/fulcrum-mod/src/lua_api.md`)

```lua
fulcrum.on_init(fn) / on_tick(fn) / on_event(name, fn)
e = fulcrum.spawn_prefab("prefabs/x.prefab.ron", { x = 0, y = 0 })
fulcrum.get(e, "Health") / set / insert / despawn
fulcrum.query("Transform2D", "Health")      -- rows, deterministic order
fulcrum.query_circle(x, y, r)               -- entity ids via the spatial grid
fulcrum.emit("boom", {...})                 -- sim event (Rust: EventReader<ModEvent>)
fulcrum.emit_command("move", {...})         -- replayable command channel
fulcrum.tick() / fulcrum.input.pressed("W") / fulcrum.audio.play("s.wav") / fulcrum.log("hi")
math.random()                               -- per-mod deterministic stream
```

## Debug keys (DefaultPlugins)

F12 — inspector. Your games conventionally use F1 for gizmo overlays and Enter for restarts.
