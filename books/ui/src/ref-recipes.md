# Recipes

Patterns the tutorial didn't need, each one shaped from the same three moves the tutorial
drilled: widgets are components, styling is data, and frame systems re-assert whatever
must stay true. Working code for the first three lives in `games/bazaar`; the rest cite
their implementation homes.

## A progress bar

There is no bar widget; there's a panel whose width you own. Nest a fill panel inside a
frame panel, give the fill `Px`, and drive the width from a frame system through the
component escape hatch:

```rust,ignore
fn patience_bar(queue: Res<Queue>, mut nodes: Query<(&UiId, &mut UiNode)>) {
    let fraction = queue.0.first().map_or(0.0, |c| c.patience as f32 / PATIENCE as f32);
    for (id, mut node) in &mut nodes {
        if id.0 == "patience_fill" {
            node.size = UiSize::Px(vec2((60.0 * fraction).round(), 4.0));
        }
    }
}
```

```ron
Node(size: Px((64, 8)), kind: Panel(color: (r: 0.09, g: 0.06, b: 0.05, a: 0.94)),
    children: [
        Node(id: "patience_fill", offset: (2, 2), size: Px((60, 4)),
            kind: Panel(color: (r: 0.55, g: 0.75, b: 0.35, a: 1.0))),
    ],
),
```

`.round()` keeps the edge on the pixel grid. Layout re-solves every frame, so a size write
is exactly as ordinary as a text write.

## An icon on a button

Buttons can't wear textures, so compose: the Bazaar's counter (chapter 3, or
`games/bazaar/assets/ui/hud.ui.ron`) stacks an `Image` node above each button inside a
`kind`-less column. For an icon *inside* the button face, overlap instead of stacking —
children of a non-stacking node place freely, and later children draw on top:

```ron
Node(id: "serve_figs", size: Px((72, 18)), kind: Button(text: "  Figs", text_size: 8),
    children: [
        Node(anchor: CenterLeft, pivot: (0, 0.5), offset: (3, 0), size: Px((12, 12)),
            kind: Image(image: "ui/figs.png")),
    ],
),
```

The leading spaces in the caption budge the (always-centered) text off the icon — crude,
honest, and exactly as much text-layout control as the engine offers. The image blocks the
pointer, but the click still lands: the button below it is the top-most *button* under the
cursor.

## A theme resource

Chapter 4's `apply_theme` generalizes to any game: a `Skin` struct of textures + colors, a
resource choosing one, and one idempotent frame system asserting it onto queried widgets —
`games/bazaar/src/main.rs` is the reference implementation. The discipline that keeps it
trivial: **the layout file authors one complete skin** (so the file previews sensibly and
hot reload has something true to show for a frame), and the theme only overwrites fields
it owns. Address widgets broadly (`Query<&mut UiPanel>`) for uniform restyling, or by
`UiId` for per-widget styling; there is no cascade to fight because there is no cascade.

## A skin that ships as a mod

Layouts and their textures load through the asset server's mount stack
(`crates/fulcrum-asset/src/vfs.rs`): a mounted mod that provides `ui/day_frame.png`
shadows the base file, path for path. A pure reskin is therefore a mod containing only
art — no code, no layout changes — and a total-conversion skin can shadow `ui/hud.ui.ron`
itself, rearranging the whole HUD as long as it keeps the ids the game drives. The main
book's modding chapter covers mounting; from the UI's side there is nothing to do, which
is the point.

## Your own font

The UI has one global font: the `DefaultFont` resource, consumed by both text measurement
and drawing. Swap it once at startup and every label, button caption, and `Fit`
measurement follows:

```rust,ignore
fn crest_font(mut commands: Commands, mut assets: AssetLoader) {
    let font = assets.load_font("fonts/YourFont.ttf");
    commands.insert_resource(DefaultFont(font));
}
```

`load_font` falls back to the built-in Press Start 2P (with a logged error) if the file is
missing or invalid — a bad font ships ugly, not broken. Sizes are free-form but the
engine rounds to integer pixels when rasterizing; bitmap-style fonts want their native
size or multiples of it. This is the one *global* styling lever the engine offers — a
per-theme font is `DefaultFont` re-inserted by your theme system.

## Clicking without a window

Two altitudes, both from `games/bazaar/tests/gameplay.rs` (chapter 5 walks them):

- **Command level** — `outbox.send("ui:click", "resume")`, then two ticks. Tests the
  UI-to-sim contract with no layout loaded. Reaches everything a button can reach, because
  it *is* what a button sends.
- **Pointer level** — build with `UiPlugin`, a default (`Stretch`) `Camera2D`, a
  `WindowInfo`, load the real layout headless, `run_schedule(Update)` as your frame,
  read the button's `UiRect`, and walk the cursor through hover/press/release on separate
  frames. Tests ids, geometry, occlusion, and the dispatcher in one pass.

Assert visibility with `Option<&UiRect>` (hidden nodes have none) and focus with
`UiFocus`. If a UI behavior can't be tested at either altitude, it's probably cosmetic —
which is the system telling you it doesn't need a test.

## Pixel-perfect UI, the checklist

- `ScalingMode::IntegerScale { .. }` — UI pixels land on whole physical pixels at any
  window size. This is most of the battle.
- Text sizes in multiples of 8 (the built-in font's native grid): 8 for body, 16 for
  headlines. Between-sizes rasterize but look boiled.
- Integer offsets and `Px` sizes everywhere; if code computes a size, `.round()` it (see
  the progress bar).
- Nine-slice margins sized so corners are meaningful at your smallest panel — a panel
  smaller than `left + right` starts dropping slices.
- One skin's textures should share margins with its siblings' (chapter 4), so the layout's
  `nine_slice` values stay true across every skin.
