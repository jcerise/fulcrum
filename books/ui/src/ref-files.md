# Layout Files

Implementation: `crates/fulcrum-ui/src/loader.rs` — the `.ui.ron` format, the spawner, hot
reload, and `UiQuery`. Convention: layouts live under `assets/ui/` and end in `.ui.ron`;
the loader only cares that the path resolves through the asset server (which means mods
can shadow layouts — see Recipes).

## The file, every field

A file is one `Ui` wrapping one root `Node`; every field of `Node` has a default and may
be omitted:

```ron
Ui(root: Node(
    id: "root",              // Option<String>: the name for UiQuery / clicks
    anchor: TopLeft,         // one of the nine Anchor points
    pivot: (0, 0),           // Vec2, fraction of self
    offset: (0, 0),          // Vec2, UI pixels, +Y down
    size: Fit,               // Px((w, h)) | Fill | Fit
    stack: None,             // None | Vertical(gap) | Horizontal(gap)
    visible: true,
    kind: None,              // what the node IS — see below
    children: [],            // more Node(...)s
))
```

`kind` variants, with their defaults:

| Kind | Fields (defaults) |
| --- | --- |
| `None` | pure layout node — structure without pixels |
| `Panel(...)` | `color` ((0,0,0,0.6)), `image` (none), `nine_slice` (none) |
| `Label(...)` | `text` (""), `size` (16), `color` (white), `h_align` (Left) |
| `Button(...)` | `text` (""), `text_size` (16), `style` (engine grays) |
| `Image(...)` | `image` (**required** — the only required field in the format), `color` (white) |

Syntax rules that bite:

- **Implicit `Some`.** Optional fields take bare values: `image: "ui/frame.png"`, never
  `Some(...)`.
- **`nine_slice` is a tuple:** `(left, top, right, bottom)`, parentheses, texture pixels.
- **Colors are structs:** `(r: 1.0, g: 0.5, b: 0.0, a: 1.0)`. No hex strings, no named
  colors, no alpha shorthand. Fields you omit default to `0.0` — which makes an omitted
  `a:` *invisible*, a classic first bug; try it.
- **Vec2s are tuples:** `(8, 8)`. Integers are fine where floats are expected.
- Texture paths are relative to the asset root and load through the same server as
  sprites, deduplicated by path.

## Loading and unloading

```rust,ignore
fn setup(mut ui: UiLoader) {
    let root: Entity = ui.load("ui/hud.ui.ron").expect("hud loads");
}
```

`load` parses, spawns one entity per node (both `Parent` and `Children` links written),
tags the root with the path (for hot reload), and resolves textures. Per-node extras: an
`id:` becomes a `UiId` component; a `Button` gets `ButtonState` inserted automatically; a
`Button` *without* an id logs an error at load — clicks from it can never be handled, and
load time is the earliest a layout typo can be caught.

`unload(root)` despawns the root and every descendant, tolerating already-despawned
entities (hot reload can race duplicate events).

`UiLoader` holds the texture loader while it runs, so it cannot share a system with
`AssetLoader` (or any other texture-writing loader) — split your startup into two systems
and move on; the scheduler error names the conflict.

**Headless:** without a GPU there are no textures, so `UiLoader` skips them with a logged
warning — panels fall back to flat color, `Image` nodes become bare layout nodes, and
everything else (tree, ids, layout, buttons, hit-testing) behaves identically. This is
what makes UI tests windowless.

## Hot reload, exactly

With `hot_reload: true` (the debug default), saving a loaded layout file despawns its tree
and respawns it from disk, in place, keyed by the path tagged on the root. Multiple events
for one path in one frame reload once. Layouts are **stateless by design**: nothing
survives the respawn, and nothing needs to — dynamic text and visibility are re-driven by
your frame systems (chapter 2's pattern), runtime styling is re-asserted by your theme
system (chapter 4's pattern). If some state *isn't* re-driven every frame, hot reload is
where you'll find out.

Two boundaries worth knowing. Reload replaces entities, so an `Entity` you cached from
`load` dangles after a reload — look nodes up by `UiId`, not by saved entity. And a reload
mid-run is a content change: it invalidates replay/determinism guarantees for that run
(dev tool, not a shipping feature).

## `UiQuery`

```rust,ignore
fn hud(mut ui: UiQuery, /* sim resources */) {
    ui.set_label("gold", format!("Gold: {gold}"));   // no-op + warn on unknown id
    ui.set_visible("pause", paused);                  // hides/shows the whole subtree
}
```

That is the entire dynamic-content API, and it's deliberately tiny: text and visibility
are the only *universally* dynamic things a HUD has. Lookup is a linear scan, first match
wins — duplicate ids aren't detected, so treat ids as unique-per-file by discipline. For
anything else — colors, sizes, images, styles — query the widget components directly:

```rust,ignore
fn flash_gold(mut labels: Query<(&UiId, &mut UiLabel)>) {
    for (id, mut label) in &mut labels {
        if id.0 == "gold" { label.color = Color::rgb(1.0, 0.9, 0.3); }
    }
}
```

This is the idiomatic escape hatch, not a workaround — the loader spawns ordinary
components precisely so ordinary queries can drive them.

## What deliberately doesn't exist

No includes, templates, prefab references, or reusable snippets — every file is complete
and every panel repeats its colors, which is exactly what keeps a theme system (code
asserting styles over data) trivial to reason about. No variables or named palettes in
the format. No expression language, no data binding — binding is a frame system you write
in Rust, where it can be debugged. No schema versioning: unknown fields are errors, absent
fields are defaults.
