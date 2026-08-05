# Nodes and Layout

Everything here lives in `crates/fulcrum-ui` (`node.rs`, `layout.rs`); the solver's unit
tests at the bottom of `layout.rs` are executable versions of this chapter's rules. UI
space: top-left origin, +Y down, in virtual "UI pixels", zoom- and camera-independent.

## `UiNode`, every field

```rust,ignore
pub struct UiNode {
    pub anchor: Anchor,   // point of the PARENT to pin to        (default TopLeft)
    pub pivot: Vec2,      // point of SELF placed on that anchor  (default (0, 0))
    pub offset: Vec2,     // pixel nudge, +Y down                 (default (0, 0))
    pub size: UiSize,     // Px(Vec2) | Fill | Fit                (default Fit)
    pub stack: StackDir,  // None | Vertical(gap) | Horizontal(gap)  (default None)
    pub visible: bool,    // hidden = no draw, no hit-test, subtree skipped
}
```

`Anchor` is nine named points; `anchor.fraction()` maps them to `(0..1, 0..1)` of the
parent rect — `TopLeft` is `(0, 0)`, `Center` `(0.5, 0.5)`, `BottomRight` `(1, 1)`. A tree
is any entity with a `UiNode`; hierarchy is the engine-wide `Parent`/`Children` pair, and
**both** halves must be present (the loader writes both; hand-built trees must too). Every
`UiNode` entity *without* a `Parent` is a root, and all roots lay out against the full
viewport — multiple independent trees coexist. Trees are solved in `Entity` order for
determinism.

## Placement, exactly

For an unstacked node with resolved size `S` inside parent rect `P`:

```text
anchor_point = P.min + anchor.fraction() * P.size()
top_left     = anchor_point - pivot * S + offset
```

Size resolves per policy:

| Policy | Unstacked | Inside a stack |
| --- | --- | --- |
| `Px(v)` | exactly `v` | exactly `v` |
| `Fill` | the parent's whole rect | parent's width, own measured height (see below) |
| `Fit` | measured content | measured content |

`Fit` measures bottom-up: the max of the node's *intrinsic* size and its stacked children
(sizes plus gaps, each child's `offset` included). Intrinsic size comes only from text —
a `UiLabel`'s text or a `UiButton`'s caption, measured against the default font. Panels
and images have **zero** intrinsic size: an image-only node under `Fit` collapses to
nothing. Give it `Px` or `Fill`.

`Fill` in a stack is tuned for vertical stacks: it spans `parent_size.x` and measures its
own main axis. In a `Horizontal` stack the cross axis is *still* the width — treat `Fill`
inside horizontal stacks as unsupported.

## Stacks, exactly

A stacking parent walks a cursor from its rect's top-left corner. Per visible child:

1. Resolve the child's size (stacked rules above).
2. Place it at `cursor + child.offset` — the child's `anchor` and `pivot` are **ignored**.
3. Advance the cursor to the child's far edge plus the gap. The child's `offset` moved the
   child *and* the cursor — which is why `offset` doubles as the padding idiom: first child
   `(pad, pad)`, later children `(pad, 0)`.

There is no cross-axis alignment inside a stack; center children by arithmetic offset.

## `UiRect` and draw order

The layout system runs every frame (in `Update`, after hot reload and interaction, before
extraction) and writes each *placed* node a `UiRect { rect, order }`. `order` is the
pre-order traversal index, and it is the draw order and the hit-test priority: later
siblings and deeper nodes draw on top and win the pointer. There is no z-index override —
draw order *is* tree order, rearrange the file to rearrange the layers.

Hidden nodes (and every descendant of a hidden node) are skipped entirely and their
`UiRect` is **removed**: a hidden node draws nothing, blocks nothing, clicks never. "Is it
really gone" is checkable in a test as `Option<&UiRect>::is_none()`.

Layout is stateless and full: every frame rebuilds the tree snapshot and re-solves. There
is no dirty-tracking, no caching, and no layout-on-demand — at HUD scale (hundreds of
nodes) this is well inside the frame budget, and it's what makes hot reload and runtime
mutation free of invalidation bugs.

## The viewport

The UI space's size comes from the camera:

| `ScalingMode` | UI space |
| --- | --- |
| `Stretch` | the window, in physical pixels |
| `FixedHeight(h)` | `h * aspect` x `h` |
| `Letterbox { w, h }` | exactly `w` x `h` |
| `IntegerScale { w, h }` | exactly `w` x `h`, drawn at whole-number scale |

Pointer mapping is `Camera2D::screen_to_ui(screen, window)`; under `Stretch` it's the
identity, which is why headless tests use it. Zoom never affects UI space.

## What deliberately doesn't exist

No flexbox, no grid, no constraint solver — anchors + stacks were chosen over taffy to
keep the model teachable in one page (`plans/003-data-driven-structure.md` records the
decision). No padding/margin fields (the offset idiom), no min/max sizes, no percentage
sizes, no aspect-ratio locks, no weighted `Fill` distribution, no wrapping stacks, no
per-child stack alignment, no rotation or scale on nodes, no z-override, no clipping —
content that overflows a rect draws anyway (long text included: there is no word wrap,
only explicit `\n`). When a layout fights you, the answer is almost always another
`kind`-less structural node, not a missing feature.
