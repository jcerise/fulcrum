---
id: 2d-essentials
title: "Phase 2: 2D essentials — camera, text, audio, animation, tilemaps (milestone: Asteroids)"
status: in-progress
priority: 2
created: 2026-07-08
steps_completed: 8
steps_total: 9
tags: [engine, camera, text, audio, animation, tilemap]
---

# Phase 2: 2D essentials (milestone: Asteroids)

## Summary

Build the features every real 2D game needs on top of the phase-1 core: a proper 2D camera with pixel-art-friendly viewport scaling, sprite sheets/atlases, debug gizmos, text rendering, audio via kira, frame animation clips with Aseprite import, and tilemaps. The milestone is an Asteroids-style arcade game in `games/asteroids` with sound, a text HUD, and animated sprites.

## Context

- **Prerequisite:** `plans/001-core-skeleton.md` complete — workspace, `Fulcrum` app, sprite batch, `Transform2D`/interpolation, `Assets`/`Handle`, `Input`, `SimRng`, `Time`, Pong.
- New crates this phase: `crates/fulcrum-audio` (kira), `crates/fulcrum-anim` (clips + Aseprite import). Camera, atlas, gizmos, text, and tilemap rendering all live in `crates/fulcrum-render`.
- New workspace deps: `kira` (audio), `fontdue` (glyph rasterization), `serde` + `serde_json` (Aseprite JSON), `ron` (tilemap format).
- Conventions from phase 1 that must hold: world units = pixels, +Y up; sim mutations only in `FixedUpdate`; audio/text/gizmos are cosmetic and exempt from determinism; every new public type joins `fulcrum::prelude` and gets a doc example.
- Determinism note for this phase: animation *playback state* advances in `FixedUpdate` (it can gate gameplay, e.g. attack frames); audio playback and gizmos are fire-and-forget cosmetics.

## Steps

### Step 1: Camera2D with viewport scaling modes

**Files:** `crates/fulcrum-render/src/camera.rs`, `crates/fulcrum-render/src/batch.rs` (use camera in projection), `crates/fulcrum-core/src/input.rs` (fix `mouse_world`)

**Requires review:** true

Public API + a locked scaling-mode design; affects every game's feel.

```rust
#[derive(Resource)]
pub struct Camera2D {
    pub center: Vec2,          // world position at screen center
    pub zoom: f32,             // 1.0 = 1 world unit : 1 physical pixel
    pub rotation: f32,
    pub scaling: ScalingMode,
}
pub enum ScalingMode {
    Stretch,                       // window pixels = world pixels (phase-1 behavior)
    FixedHeight(f32),              // world height fixed; width follows aspect
    Letterbox { width: f32, height: f32 },   // fixed virtual resolution, bars
    IntegerScale { width: f32, height: f32 } // pixel-perfect integer zoom, bars
}
```

- Replace the step-6 (phase 1) projection function with one derived from `Camera2D` + `WindowInfo`; letterbox/integer modes set a scissor/viewport and clear bars to black.
- `Camera2D::world_to_screen(Vec2) -> Vec2` and `screen_to_world(Vec2) -> Vec2`; `Input::mouse_world` now routes through the camera.
- Camera is a resource, not a component (one camera; split-screen is out of scope — note in docs).
- Camera movement is cosmetic: games may move it in `Update`; interpolating camera follow is documented as an `Update`-side lerp toward a sim position.

**Acceptance criteria:**
- [ ] Each `ScalingMode` verified manually at 3 window sizes (document with screenshots in the PR)
- [ ] `screen_to_world(world_to_screen(p)) ≈ p` unit test for all modes
- [ ] Zoom and rotation render correctly; `mouse_world` stays accurate under zoom/rotation/letterbox

---

### Step 2: Texture atlases and sprite regions

**Files:** `crates/fulcrum-render/src/atlas.rs`, `crates/fulcrum-render/src/sprite.rs` (region support), `crates/fulcrum-render/src/batch.rs` (UV rects)

**Requires review:** false

```rust
pub struct SpriteSheet {                    // asset
    pub texture: Handle<Texture>,
    pub regions: Vec<Rect>,                 // pixel rects
    pub names: FxHashMap<String, u32>,      // optional named regions
}
impl SpriteSheet {
    pub fn from_grid(texture: Handle<Texture>, tile: Vec2, cols: u32, rows: u32) -> Self;
}
```

- Extend `Sprite` with `pub region: Option<SpriteRegion>` where `SpriteRegion { sheet: Handle<SpriteSheet>, index: u32 }`; when set, the batcher emits that sub-rect's UVs and uses the sheet's texture. `Sprite::from_sheet(sheet, index)` constructor.
- Batcher change: sort key uses the resolved texture id, so sprites from one sheet still batch into one draw call.
- `Rect { min: Vec2, max: Vec2 }` goes in fulcrum-core (shared math type) with `contains`, `overlaps` helpers — games need AABB tests anyway.

**Acceptance criteria:**
- [ ] A grid sheet renders distinct frames by index; 1,000 sprites from one sheet = 1 draw call (log)
- [ ] `Rect::overlaps` unit-tested including touching-edge cases

---

### Step 3: Debug gizmos (immediate-mode overlay)

**Files:** `crates/fulcrum-render/src/gizmos.rs`, `crates/fulcrum-render/src/shader_gizmos.wgsl`

**Requires review:** false

The one blessed immediate-mode API — debug only, drawn above all sprites, cleared every frame.

```rust
#[derive(Resource)]
pub struct Gizmos { /* frame-local vertex buffer */ }
impl Gizmos {
    pub fn line(&mut self, a: Vec2, b: Vec2, color: Color);
    pub fn rect(&mut self, rect: Rect, color: Color);          // outline
    pub fn circle(&mut self, center: Vec2, radius: f32, color: Color); // 32-segment outline
    pub fn point(&mut self, p: Vec2, color: Color);            // small cross
}
```

- Untextured line-list pipeline; world-space coordinates through the camera; buffer cleared after each render.
- Callable from any schedule (it's cosmetic); typical use is an `Update` system.
- Add `FulcrumConfig::gizmos_enabled: bool` (default true in debug builds, false in release) so shipped games don't pay for stray debug draws.

**Acceptance criteria:**
- [ ] An example draws all four primitives tracking a moving entity; renders above sprites
- [ ] Disabled flag results in zero gizmo GPU work

---

### Step 4: Text rendering — fontdue glyph atlas + Text component

**Files:** `crates/fulcrum-render/src/text.rs`, `crates/fulcrum-asset/src/lib.rs` (font loading), `crates/fulcrum/assets/` (embed a default font)

**Requires review:** true

Public API; also the first dynamically-built atlas in the renderer.

- `Font` asset: fontdue font loaded from TTF/OTF bytes; `AssetServer::load_font(path)`.
- Embed one permissively-licensed default font (e.g. an OFL font checked into the repo with its license file) exposed as `Font::default_handle()` so text works with zero setup.
- Glyph cache: rasterize (font, size, char) on demand into a shelf-packed RGBA atlas texture (grow by adding a second atlas page if full; 1024² initial). Cache key `(font_id, px_size_rounded, char)`.

```rust
#[derive(Component, Clone)]
pub struct Text {
    pub value: String,
    pub font: Handle<Font>,        // Default::default() = built-in font
    pub size: f32,                 // px
    pub color: Color,
    pub h_align: HAlign,           // Left | Center | Right
    pub z: f32,
}
```

- Entities with `Text + Transform2D` render as batched glyph quads through the existing sprite pipeline (glyph atlas is just another texture). No wrapping/rich text — single line + `\n` handling only; document that game UI text gets richer in phase 3.
- Layout: simple horizontal advance + kerning from fontdue; baseline positioning documented (translation = baseline-left of first line, before `h_align` adjustment).

**Acceptance criteria:**
- [ ] "Score: 1234" renders crisply at 16/32/64 px with the default font
- [ ] Alignment modes verified; multi-line via `\n` stacks with correct line height
- [ ] Changing `Text::value` every tick doesn't leak atlas space (cache hit path tested with a counter)

---

### Step 5: Audio — kira integration

**Files:** `crates/fulcrum-audio/Cargo.toml`, `crates/fulcrum-audio/src/lib.rs`

**Requires review:** false

- `AudioPlugin` (added to `DefaultPlugins`): creates a kira `AudioManager` resource; failure to acquire an audio device logs a warning and installs a no-op manager (games must run on machines with no audio).
- `Sound` asset: kira `StaticSoundData` loaded from WAV/OGG via `AssetServer::load_sound(path)`.

```rust
#[derive(Resource)]
pub struct Audio { /* manager, master volume, music track handle */ }
impl Audio {
    pub fn play(&mut self, sound: Handle<Sound>);                       // fire and forget
    pub fn play_with(&mut self, sound: Handle<Sound>, p: PlayParams);   // volume, pitch, pan
    pub fn play_music(&mut self, sound: Handle<Sound>, looping: bool);  // one music slot, replaces current
    pub fn stop_music(&mut self);
    pub fn set_master_volume(&mut self, v: f32);
}
pub struct PlayParams { pub volume: f32, pub pitch: f32, pub pan: f32 } // Default: 1,1,0
```

- Callable from `FixedUpdate` (games trigger sounds from sim events); playback itself is cosmetic and exempt from determinism — document this explicitly.
- Keep kira types fully private; only Fulcrum types in the API.

**Acceptance criteria:**
- [ ] Example plays a one-shot on keypress and looping music; pitch/pan params audibly work
- [ ] With no audio device (CI/headless), engine runs without panic
- [ ] `cargo doc` shows no kira types in public signatures

---

### Step 6: Animation clips and AnimationPlayer

**Files:** `crates/fulcrum-anim/Cargo.toml`, `crates/fulcrum-anim/src/lib.rs`, `crates/fulcrum-anim/src/clip.rs`, `crates/fulcrum-anim/src/player.rs`

**Requires review:** false

```rust
pub struct AnimationClip {                  // asset
    pub sheet: Handle<SpriteSheet>,
    pub frames: Vec<u32>,                   // region indices
    pub frame_ticks: Vec<u32>,              // duration of each frame in SIM TICKS
    pub looping: bool,
}
impl AnimationClip {
    pub fn from_fps(sheet: Handle<SpriteSheet>, frames: Vec<u32>, fps: f32, looping: bool, tick_rate: u32) -> Self;
}

#[derive(Component)]
pub struct AnimationPlayer {
    pub clip: Handle<AnimationClip>,
    pub playing: bool,
    pub tick_in_frame: u32,
    pub frame_index: usize,
}
impl AnimationPlayer {
    pub fn play(clip: Handle<AnimationClip>) -> Self;
    pub fn restart(&mut self, clip: Handle<AnimationClip>);   // no-op if already playing this clip
    pub fn finished(&self) -> bool;                            // non-looping clip at end
}
```

- Frame durations in **ticks, not seconds** — animation advances in a `FixedUpdate` system (`AnimPlugin`), keeping it deterministic and letting gameplay key off `finished()`/current frame (e.g., hitbox on frame 3).
- The advance system writes the current region index into the entity's `Sprite.region`.
- `AnimPlugin` joins `DefaultPlugins`.

**Acceptance criteria:**
- [ ] Headless test: player over a 3-frame clip with known tick durations lands on expected frames at expected ticks; looping wraps, non-looping clamps + `finished()`
- [ ] `restart` with the same clip does not stutter (no frame reset)

---

### Step 7: Aseprite import — JSON sheet export → SpriteSheet + clips

**Files:** `crates/fulcrum-anim/src/aseprite.rs`

**Requires review:** false

- Support Aseprite's standard **JSON (array) + packed PNG** export (`--sheet --format json-array --list-tags`). Direct `.ase` parsing is rejected for now (note in Notes).
- Serde structs for the JSON: frames (filename, frame rect, duration ms), meta.frameTags (name, from, to, direction).

```rust
pub struct AsepriteImport {
    pub sheet: Handle<SpriteSheet>,
    pub clips: FxHashMap<String, Handle<AnimationClip>>,  // tag name -> clip
}
pub fn load_aseprite(assets: &mut ..., json_path: &str, tick_rate: u32) -> Result<AsepriteImport, AsepriteError>;
```

- Frame durations: ms → ticks, rounding to nearest tick, minimum 1 tick; tag directions `forward` and `pingpong` supported (`pingpong` expands the frame list), `reverse` reverses; each tag becomes a looping clip by default.
- One-line convenience on the asset facade: `assets.load_aseprite("player.json")`.
- Check a small fixture (`.json` + `.png`, committed) into `crates/fulcrum-anim/tests/fixtures/`.

**Acceptance criteria:**
- [ ] Fixture with 2 tags loads into 2 clips with correct frame indices and tick durations
- [ ] Pingpong tag expands correctly (unit test on the expansion)
- [ ] Malformed JSON returns a descriptive error, never a panic

---

### Step 8: Tilemaps — asset, component, chunked renderer

**Files:** `crates/fulcrum-render/src/tilemap.rs`, `crates/fulcrum-asset/src/lib.rs` (RON loader registration)

**Requires review:** false

- Own RON format now; Tiled import is deferred (Notes). Format v1:

```ron
Tilemap(
    sheet: "tiles.png",             // + grid params, or a .json aseprite sheet
    tile_size: (16, 16),
    layers: [ Layer( name: "ground", tiles: [ [1, 1, 2, ...], ... ] ) ],  // row-major, 0 = empty, index-1 into sheet
)
```

```rust
pub struct TilemapAsset { pub sheet: Handle<SpriteSheet>, pub tile_size: Vec2, pub layers: Vec<TileLayer> }
pub struct TileLayer { pub name: String, pub tiles: Vec<u32>, pub width: u32, pub height: u32 }

#[derive(Component)]
pub struct Tilemap { pub asset: Handle<TilemapAsset>, pub z: f32 }
// entity also carries Transform2D = world position of tile (0,0)'s min corner

impl TilemapAsset {
    pub fn tile_at(&self, layer: &str, x: u32, y: u32) -> Option<u32>;
    pub fn set_tile(&mut self, layer: &str, x: u32, y: u32, tile: u32);  // marks chunk dirty
    pub fn world_to_tile(&self, map_transform: &Transform2D, world: Vec2) -> Option<(u32, u32)>;
}
```

- Renderer: pre-build vertex buffers per 32×32-tile chunk per layer; re-mesh only dirty chunks; frustum-cull chunks against the camera. This is the path that must scale for RTS/sim maps.
- Tile coordinate convention: (0,0) at bottom-left of the map, matching +Y-up world.

**Acceptance criteria:**
- [ ] A 256×256 map (65k tiles/layer, 2 layers) renders at 60 fps with chunk culling (log visible-chunk count while panning)
- [ ] `set_tile` updates visuals next frame; only the dirty chunk re-meshes (log)
- [ ] `world_to_tile`/`tile_at` round-trip unit tests, including the map's `Transform2D` offset

---

### Step 9: Milestone — Asteroids in games/asteroids

**Files:** `games/asteroids/Cargo.toml`, `games/asteroids/src/main.rs` (+ small modules), `games/asteroids/assets/*`

**Requires review:** false

Exercises everything except tilemaps (verified in step 8's stress example; the phase-3 game is tilemap-centric).

- Ship: thrust/rotate (physics drift), Aseprite-animated thruster flame; screen-wrap using camera letterbox `Letterbox { 800, 600 }` virtual resolution.
- Asteroids: 3 sizes from one sprite sheet, split on hit, spawn/velocity/rotation from `SimRng`.
- Bullets with lifetime; AABB or circle collision via `Rect`/distance helpers.
- Audio: shoot, explosion (pitch-varied via `PlayParams`), looping music.
- Text HUD: score + lives (top corners), "GAME OVER — press Enter" center-aligned.
- Simple game-state enum resource (`Playing`/`GameOver`) driving system early-outs.
- Gizmos toggled by F1 showing collision circles (demonstrates the debug path).
- All gameplay in `FixedUpdate`; verify the phase-1 headless determinism harness passes on a 600-tick scripted run of this game.

**Acceptance criteria:**
- [ ] Playable loop: shoot asteroids, splits, death, score, restart — with sound and animated thruster
- [ ] Only `fulcrum::prelude` imports
- [ ] Headless same-seed scripted-input run twice → identical final score and entity count
- [ ] Engine API friction encountered while building it is fixed in the engine (or filed as a note in `docs/`), not worked around in game code

## Testing

- Unit tests: camera round-trips per scaling mode (step 1), `Rect` ops (step 2), glyph-cache hit counting (step 4), clip advancement (step 6), Aseprite parsing + pingpong expansion against fixtures (step 7), tile coordinate math (step 8).
- Headless determinism: extend `fulcrum-core/tests/determinism.rs` with an animation-driven case (frame index is sim state); asteroids scripted-run test lives in `games/asteroids/tests/`.
- Visual/manual: scaling modes, text crispness, chunk culling logs — record checks in PR descriptions. GPU paths remain local-only (no GPU in CI).

## Notes

- **fontdue over glyphon**: fewer dependencies and full control of the atlas; revisit only if shaping (Arabic/Devanagari etc.) becomes a requirement — that would swap the rasterizer behind `Text`, not change the component API.
- **Aseprite `.ase` direct parsing** rejected for now: the JSON export is stable, documented, and keeps the importer ~200 lines. A `.ase` loader (via the `asefile` crate) can be added later behind the same `AsepriteImport` return type.
- **Tiled (.tmx) import** deferred until a real game needs it; the RON format is the canonical in-engine representation either way, so an importer is additive.
- Kira's clock/scheduling features are deliberately unused — sim-side timing owns gameplay; audio only reacts.
- The one-camera decision (resource, not component) trades split-screen support for a much simpler renderer; revisiting it later means changing `Camera2D` from resource to component + viewport list, which is contained in fulcrum-render.
