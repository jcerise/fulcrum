---
id: data-driven-structure
title: "Phase 3: Structure — prefabs, scenes, hot reload, animation state machines, UI (milestone: dungeon demo)"
status: complete
priority: 3
created: 2026-07-08
steps_completed: 9
steps_total: 9
tags: [engine, prefabs, scenes, hot-reload, ui, animation, egui]
---

# Phase 3: Data-driven structure (milestone: dungeon demo)

## Summary

Turn Fulcrum data-driven: components serializable by name through a registry, prefabs and scenes authored in RON, asset hot reload, animation state machines, a retained-mode game UI with hot-reloaded RON layouts, and an egui-based debug inspector. The milestone is a roguelike/metroidvania-shaped demo in `games/dungeon`. This phase is the architectural foundation for phase-4 modding — everything authored as data here is what mods will override.

## Context

- **Prerequisites:** `plans/001-core-skeleton.md` and `plans/002-2d-essentials.md` complete.
- New crates: `crates/fulcrum-scene` (component registry, prefabs, scenes), `crates/fulcrum-ui` (retained game UI + egui debug overlay).
- New workspace deps: `ron`, `serde` (already present), `notify` (file watching), `egui`, `egui-wgpu`, `egui-winit`.
- Design principle for the whole phase: **if a modder will need to author it, it must be a file format, not code.** Prefabs, scenes, state machines, and UI layouts are all RON assets with serde types, loaded through `AssetServer` so phase-4's VFS layering applies to them automatically.
- Determinism: prefab/scene spawning happens in `FixedUpdate` order and is deterministic. Hot reload and egui are dev-time/cosmetic and exempt (document: a hot reload mid-run invalidates replay guarantees — acceptable).

## Steps

### Step 1: Component registry — serialize/deserialize components by name

**Files:** `crates/fulcrum-scene/Cargo.toml`, `crates/fulcrum-scene/src/registry.rs`, `crates/fulcrum-scene/src/lib.rs`, registration calls in `fulcrum-core`/`render`/`anim` for built-ins

**Requires review:** true

The keystone: maps string names → typed component insert/extract, enabling prefabs, scenes, the inspector, and Lua bindings (phase 4).

```rust
#[derive(Resource, Default)]
pub struct ComponentRegistry { /* name -> ComponentOps */ }

pub struct ComponentOps {
    pub insert: fn(&mut EntityWorldMut, ron_value: &ron::Value) -> Result<(), SceneError>,
    pub extract: fn(&EntityRef) -> Option<ron::Value>,       // for scene saving + inspector
    pub default_insert: fn(&mut EntityWorldMut),
}

impl ComponentRegistry {
    pub fn register<T: Component + Serialize + DeserializeOwned + Default>(&mut self, name: &str);
}
```

- On `Fulcrum`: `pub fn register_component<T: ...>(self, name: &str) -> Self` — the one-liner games use.
- Register all serializable built-ins under stable names: `"Transform2D"`, `"Sprite"` (serialize texture/sheet by asset *path*, resolved through `AssetServer` on insert — add a serde-friendly `SpriteDef` mirror struct), `"Text"`, `"AnimationPlayer"` (by clip path/name), `"Tilemap"`. Components that are engine-managed (`PreviousTransform2D`) are not registered.
- Asset-path-in-data is the convention everywhere: data files reference assets by path string; handles never appear in files.
- Error type `SceneError` with context (entity, component name, underlying serde error) — errors here must be excellent, they're user-facing authoring errors.

**Acceptance criteria:**
- [ ] Round-trip test: insert `Transform2D` via RON value, extract, compare
- [ ] A game-defined `#[derive(Component, Serialize, Deserialize, Default)]` struct registers with one line and round-trips
- [ ] Unknown component name or bad field yields a `SceneError` naming the component and problem

---

### Step 2: Prefab assets (RON) and spawn API

**Files:** `crates/fulcrum-scene/src/prefab.rs`, example prefabs under `games/dungeon/assets/prefabs/`

**Requires review:** true

Format v1 (`assets/prefabs/slime.prefab.ron`):

```ron
Prefab(
    components: {
        "Transform2D": (translation: (0.0, 0.0), rotation: 0.0, scale: (1.0, 1.0)),
        "Sprite": (sheet: "creatures.json", region: "slime_idle_0"),
        "AnimationPlayer": (clip: "creatures.json#slime_idle"),
        "Health": (max: 10, current: 10),
    },
    children: [ Prefab( components: { ... } ) ],   // optional, spawned as child entities
)
```

```rust
pub struct PrefabAsset { /* parsed, unresolved RON */ }

pub trait SpawnPrefabExt {   // implemented for Commands
    fn spawn_prefab(&mut self, prefab: Handle<PrefabAsset>) -> Entity;
    fn spawn_prefab_at(&mut self, prefab: Handle<PrefabAsset>, pos: Vec2) -> Entity;  // overrides Transform2D.translation
}
```

- Deferred application: `spawn_prefab` queues; a `FixedUpdate`-first exclusive system applies queued spawns through the registry (deterministic order = queue order).
- Children: introduce minimal `Parent(Entity)` / `Children(Vec<Entity>)` components in fulcrum-core with **no transform propagation yet** (children get world transforms at spawn from their RON relative to the parent; full hierarchy propagation is out of scope — Notes).
- `AssetServer::load_prefab(path)`; prefab files may reference other assets by path, loaded on first spawn.

**Acceptance criteria:**
- [ ] Spawning the slime prefab yields an entity with all four components resolved (real handles, playing animation)
- [ ] `spawn_prefab_at` overrides position; two spawns are independent entities
- [ ] A prefab referencing a game-registered custom component works
- [ ] Missing asset path inside a prefab → placeholder texture + error log, not a panic

---

### Step 3: Scene assets — load, instantiate, unload

**Files:** `crates/fulcrum-scene/src/scene.rs`, example scene under `games/dungeon/assets/scenes/`

**Requires review:** false

```ron
Scene(
    entities: [
        ( prefab: "prefabs/slime.prefab.ron", at: (128.0, 64.0) ),          // prefab instance
        ( components: { "Tilemap": (asset: "maps/level1.map.ron"), ... } ), // inline entity
    ],
)
```

```rust
pub struct SceneAsset { /* parsed entries */ }

#[derive(Component)] pub struct SceneMember(pub Handle<SceneAsset>);  // tag on every spawned entity

impl SceneSpawner {  // Resource
    pub fn load(&mut self, scene: Handle<SceneAsset>);     // queues spawn of all entries
    pub fn unload(&mut self, scene: Handle<SceneAsset>);   // despawns all SceneMember-tagged entities
}
```

- Applied by the same exclusive system as prefabs (shared queue → deterministic).
- Scene *saving* (world → RON via `extract`) implemented as `SceneSpawner::save_world(&World, filter) -> String` — dev-tool grade, used by the inspector later; not a runtime feature.
- Document the pattern for level transitions: `unload(current); load(next);` both take effect at the next tick boundary.

**Acceptance criteria:**
- [ ] Loading the example scene spawns tilemap + prefab instances; unload removes exactly its entities (pre-existing entities untouched)
- [ ] Load→unload→load cycles don't leak entities (count assertion in a headless test)
- [ ] `save_world` output re-loads to an equivalent scene (round-trip on registered components)

---

### Step 4: Asset hot reload

**Files:** `crates/fulcrum-asset/src/watch.rs`, `crates/fulcrum-asset/src/lib.rs` (reload plumbing), handlers in `fulcrum-render` (textures), `fulcrum-anim` (clips/sheets), `fulcrum-scene` (prefabs)

**Requires review:** false

- `notify` watcher on the assets root (dev builds only: behind `FulcrumConfig::hot_reload: bool`, default true in debug, false in release). Watcher thread sends changed paths over a channel; a per-frame `Update` system drains it (debounced 100 ms per path).
- `AssetEvent { Modified(AssetPath) }` as an ECS event; each asset subsystem reacts:
  - **Textures:** re-decode + re-upload in place (same handle → all sprites update automatically). Same for fonts (clear glyph cache).
  - **SpriteSheets/clips/Aseprite JSON:** re-parse in place; `AnimationPlayer`s clamp frame indices if the clip shrank.
  - **Prefabs:** re-parse the asset (affects future spawns only — live entities are not retro-patched; document this).
  - **Tilemaps:** re-parse + mark all chunks dirty.
  - **Scenes/UI:** handled in their own steps (UI re-applies live, step 7).
- Because reload replaces data behind existing handles, `Assets<T>` gets `pub fn replace(&mut self, h: Handle<T>, value: T)` and path→handle lookup `pub fn handle_for(&self, path: &AssetPath) -> Option<Handle<T>>`.

**Acceptance criteria:**
- [ ] Editing a PNG while the game runs updates on-screen sprites within ~1 s, no restart
- [ ] Editing an Aseprite JSON mid-animation doesn't panic (clamping test with a shrunk clip, headless via synthetic event)
- [ ] Release build with `hot_reload: false` spawns no watcher thread

---

### Step 5: Animation state machines

**Files:** `crates/fulcrum-anim/src/state_machine.rs`, example asset in `games/dungeon/assets/anim/`

**Requires review:** true

Data format (`player.animsm.ron`):

```ron
StateMachine(
    initial: "idle",
    params: { "speed": Float(0.0), "grounded": Bool(true), "attack": Trigger },
    states: {
        "idle":   ( clip: "player.json#idle" ),
        "run":    ( clip: "player.json#run" ),
        "attack": ( clip: "player.json#attack", on_finish: "idle" ),  // non-looping exit
    },
    transitions: [
        ( from: "idle",  to: "run",    when: [ Gt("speed", 0.1) ] ),
        ( from: "run",   to: "idle",   when: [ Lt("speed", 0.1) ] ),
        ( from: Any,     to: "attack", when: [ Triggered("attack") ] ),
    ],
)
```

```rust
pub struct StateMachineAsset { /* parsed graph, validated on load */ }

#[derive(Component)]
pub struct Animator { /* machine handle, current state, param values */ }
impl Animator {
    pub fn new(machine: Handle<StateMachineAsset>) -> Self;
    pub fn set_float(&mut self, name: &str, v: f32);
    pub fn set_bool(&mut self, name: &str, v: bool);
    pub fn trigger(&mut self, name: &str);
    pub fn state(&self) -> &str;
}
```

- `FixedUpdate` system (after game systems set params, before the clip-advance system): evaluate transitions in **declaration order** (first match wins — deterministic), switch the entity's `AnimationPlayer` clip on state change, consume triggers each tick, handle `on_finish` when `AnimationPlayer::finished()`.
- Conditions: `Gt/Lt/Eq(f32)`, `Is(bool)`, `Triggered`; `when` lists are AND-ed. `Any`-state transitions are checked before per-state ones and don't re-enter the current state.
- Validation at load: unknown states/params/clips are `SceneError`-style load errors listing every problem, not first-only.
- Register `Animator` in the component registry (machine referenced by path) so prefabs can carry it.

**Acceptance criteria:**
- [ ] Headless test drives params through idle→run→attack→(on_finish)→idle and asserts state + active clip per tick
- [ ] Trigger fires at most one transition and clears; unconsumed triggers clear at tick end
- [ ] Invalid machine file reports all errors at once with state/param names

---

### Step 6: Retained game UI — node tree, layout, rendering

**Files:** `crates/fulcrum-ui/Cargo.toml`, `crates/fulcrum-ui/src/lib.rs`, `crates/fulcrum-ui/src/node.rs`, `crates/fulcrum-ui/src/layout.rs`, `crates/fulcrum-ui/src/render.rs`

**Requires review:** true

The layout model is the decision here; it's deliberately small — anchors + stacking, not full flexbox.

- UI lives in **virtual-screen space** (the camera's letterbox virtual resolution, or window pixels for `Stretch`), rendered after world sprites, ignoring the camera transform.
- UI nodes are ECS entities: `UiNode` + kind component + `Parent`/`Children` from step 2.

```rust
#[derive(Component)]
pub struct UiNode {
    pub anchor: Anchor,          // which point of the PARENT this node pins to (9 variants: TopLeft..BottomRight, Center)
    pub pivot: Vec2,             // which point of SELF sits on the anchor, (0..1, 0..1)
    pub offset: Vec2,            // px from anchor
    pub size: UiSize,            // Px(Vec2) | Fill | FitChildren
    pub visible: bool,
}
pub enum StackDir { None, Vertical(f32 /*gap*/), Horizontal(f32) }  // on UiNode: children auto-positioned if not None
```

- Layout pass (`Update`, single system): resolve each node's rect top-down (parent rect → anchor/pivot/offset/size; stacked children override anchor positioning). Store computed `UiRect(Rect)` component (screen px).
- Render pass: walk depth-first, emit sprite-pipeline quads (solid color, image, nine-slice, text via the phase-2 glyph cache). Draw order = tree order (later siblings on top).
- UI is cosmetic/`Update`-side; interactions reach the sim via events (step 7).

**Acceptance criteria:**
- [ ] Headless layout tests: anchored corners, pivot centering, vertical stack with gap, nested fill — assert computed `UiRect`s
- [ ] Resizing the window keeps anchored elements pinned (manual, all scaling modes)
- [ ] 500 UI nodes lay out + render within 0.5 ms in release (log timing)

---

### Step 7: UI widgets, RON layouts, input and events

**Files:** `crates/fulcrum-ui/src/widgets.rs`, `crates/fulcrum-ui/src/loader.rs`, `crates/fulcrum-ui/src/interact.rs`, example layout in `games/dungeon/assets/ui/`

**Requires review:** false

- Widget kind components: `UiPanel { color, image: Option<path>, nine_slice: Option<margins> }`, `UiLabel { text, font, size, color, h_align }`, `UiButton { id: String, style: ButtonStyle }` (normal/hover/pressed panel styles), `UiImage { image, region }`.
- RON layout format mirroring the tree (`assets/ui/hud.ui.ron`):

```ron
Ui( root: Node(
    anchor: TopLeft, offset: (16, -16), size: FitChildren, stack: Vertical(4),
    kind: Panel(color: "#00000080", nine_slice: None),
    children: [
        Node( kind: Label(id: "score", text: "Score: 0", size: 24) ),
        Node( kind: Button(id: "menu", text: "Menu") ),
    ],
))
```

- `UiSpawner` resource: `load(path) -> UiHandle` spawns the tree, `unload(UiHandle)` despawns. Hot reload (step 4) despawns + respawns the tree in place, preserving nothing (documented — UI layouts are stateless; dynamic text is re-driven by game systems each frame).
- Dynamic content: `UiQuery` helper resource — `set_label(ui, "score", format!("Score: {n}"))`, `set_visible(ui, "minimap", false)` — id-based lookup (ids indexed at spawn).
- Interaction system (`Update`): hit-test mouse against `UiRect`s top-most-first, drive button visual state, emit `UiEvent::Clicked(id: String)` ECS events. UI consumes the click (a `UiFocus` resource flag games check before treating a click as world input).
- Sim boundary: `UiEvent`s are readable from `FixedUpdate` (events are buffered per tick alongside input sampling — extend the runner's input snapshot to also snapshot UI events so replays capture them; this matters for phase 4).

**Acceptance criteria:**
- [ ] HUD layout file renders panel + label + working button; hover/pressed styles visible
- [ ] `set_label` updates text every tick without leaking (glyph cache counter)
- [ ] Click on a button emits `UiEvent::Clicked("menu")` readable in `FixedUpdate`, and does NOT also register as a world click
- [ ] Editing the `.ui.ron` file live rebuilds the UI in-place

---

### Step 8: egui debug overlay and world inspector

**Files:** `crates/fulcrum-ui/src/debug/mod.rs`, `crates/fulcrum-ui/src/debug/inspector.rs`

**Requires review:** false

- `DebugUiPlugin` (in `DefaultPlugins` for debug builds; opt-in for release): egui-winit event integration + egui-wgpu render pass drawn last. Toggle overlay with F12.
- Inspector windows:
  - **Entities:** list (id + name from an optional `Name(String)` component + component-name summary), click to expand; registered components render via their `extract` RON value into editable egui widgets (floats/bools/strings/Vec2/Color), writing back through `insert`. Unregistered components listed read-only by type name.
  - **Performance:** fps, frame ms, tick ms, sprite/batch counts, UI layout ms (stats published by each subsystem into a `DiagnosticsStore` resource — add counters where missing).
  - **Assets:** loaded assets by type with paths; button to force-reload one.
- egui gets raw winit events *before* the game (egui consumes what it uses; expose `DebugUiFocus` so game input can ignore clicks over debug windows).
- Editing state via the inspector is a dev action that invalidates determinism — print a one-line notice to the log when the first edit happens.

**Acceptance criteria:**
- [ ] F12 shows inspector; live-editing a slime's `Transform2D.translation` moves it on screen
- [ ] Perf window shows plausible live numbers
- [ ] Typing in an egui text field doesn't move the player (focus handling)

---

### Step 9: Milestone — dungeon demo in games/dungeon

**Files:** `games/dungeon/Cargo.toml`, `games/dungeon/src/*.rs`, `games/dungeon/assets/**`

**Requires review:** false

A roguelike-shaped slice proving the data-driven stack end to end:

- Tilemap dungeon (hand-authored RON map, walls + floor layers) with tile-based collision (`world_to_tile` against the wall layer).
- Player from a **prefab**, with Aseprite animations driven by a **state machine** (idle/run/attack via params from movement + attack trigger on Space).
- 3 monster types as prefabs sharing a `Health`/`Melee` set of game components (all registered); spawned by the **scene**; simple chase-when-near behavior using `SimRng` for wander.
- **UI**: HUD (hearts row + gold label via `set_label`), inventory panel toggled with I (visibility), pause menu with working Resume/Quit buttons (UiEvents).
- Camera follow (Update-side lerp) with `IntegerScale` pixel-art mode.
- Dev experience demo: run the game, edit the map RON / a prefab / the HUD layout live — document the loop in `games/dungeon/README.md` with a short GIF-able script.
- Headless determinism test: 600-tick scripted run twice → identical world hash (positions + health values).

**Acceptance criteria:**
- [ ] Playable: move, collide with walls, attack animated via state machine, kill monsters, pick up gold, HUD updates, pause menu works
- [ ] Player/monsters/UI/map are ALL data files — game code contains no hardcoded entity composition except registration + systems
- [ ] Hot-reload loop demonstrated and documented in the game README
- [ ] Determinism test passes; only `fulcrum::prelude` imports

## Testing

- Headless: registry round-trips (step 1), prefab/scene spawn-unload counts (steps 2–3), state machine tick-by-tick traces (step 5), UI layout rects (step 6), dungeon determinism run (step 9).
- Hot-reload logic tested headless by injecting synthetic `AssetEvent`s (no filesystem dependence in CI); the `notify` watcher itself is manual-check only.
- Manual: live-edit loop (texture, prefab, UI, map), inspector editing, UI focus behavior — checklist in each PR description.

## Notes

- **No full transform hierarchy**: `Parent`/`Children` exist for prefab children and UI, but world-transform propagation for deep game hierarchies is deliberately out of scope until a real game needs it — most 2D games don't, and it complicates interpolation. Revisit if the dungeon demo hurts without it.
- **Registry vs bevy_reflect**: bevy_ecs ships without reflect by default; a serde-based registry is smaller, easier to explain to modders, and sufficient. If field-level reflection needs grow (richer inspector), reconsider — the registry API can be backed by reflect without changing call sites.
- **UI layout model**: anchors+stacking chosen over flexbox (taffy) to keep the mental model teachable in one doc page. If real layouts prove painful in phase 4's RTS UI, taffy can back `UiSize::Fill` semantics without changing the RON format's vocabulary.
- Hot reload retro-patches assets-in-place but never live entities (prefab edits affect future spawns) — respawn via scene reload is the documented workflow.
- The step-7 decision to snapshot UI events into the per-tick input record is load-bearing for phase-4 replays; don't cut it for time.
