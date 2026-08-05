# Widgets and Interaction

Implementation: `crates/fulcrum-ui/src/widgets.rs` (the components) and
`crates/fulcrum-ui/src/interact.rs` (the hundred lines that are the entire interaction
model). A node may carry several widget components; they layer panel → image → button →
label, and a node's text always draws above that node's own background.

## The four widgets

```rust,ignore
pub struct UiPanel {
    pub color: Color,                    // the fill — or the tint, once `image` is set
    pub image: Option<Handle<Texture>>,  // background texture
    pub nine_slice: Option<[f32; 4]>,    // [left, top, right, bottom], TEXTURE pixels
}
// Default: translucent black (0, 0, 0, 0.6), no image.

pub struct UiLabel {
    pub text: String,   // multi-line via '\n' only — there is no wrapping
    pub size: f32,      // UI pixels; the built-in font is sharpest at multiples of 8
    pub color: Color,
    pub h_align: HAlign, // Left | Center | Right, within the node's laid-out width
}
// Default: "", 16.0, white, Left.

pub struct UiButton { pub text: String, pub text_size: f32, pub style: ButtonStyle }
pub struct ButtonStyle { pub normal: Color, pub hover: Color, pub pressed: Color }
pub struct ButtonState { pub hovered: bool, pub pressed: bool } // engine-written, per frame

pub struct UiImage { pub image: Handle<Texture>, pub color: Color } // whole texture, stretched
```

| Widget | Draws | Blocks pointer | Intrinsic size |
| --- | --- | --- | --- |
| `UiPanel` | color, or nine-sliced/stretched texture x tint | yes | none |
| `UiLabel` | text in the one global font | **no** | its text |
| `UiButton` | state-colored rect + centered white caption | yes | its caption |
| `UiImage` | texture stretched to the rect, tinted | yes | none |

Nine-slice: corners keep their texture-pixel size 1:1; edges and center **stretch** (never
tile). Zero-or-negative slices (margins meeting or exceeding the texture or the rect) are
skipped. Sampling is nearest-neighbor throughout. Nine-slice belongs to *panels only*.

Button captions are hardcoded: always centered, always white, vertically centered. A
button cannot wear a texture. Icon buttons are compositions — image node above, or
overlapping, a button node.

Text renders in the single global `DefaultFont` (embedded Press Start 2P unless you swap
the resource — see Recipes). No per-widget font field exists, on purpose.

**Sharp edges, hand-builders' edition.** The loader handles all of this for you; if you
spawn widgets with `Commands` instead: `UiButton::default()` has `text_size: 0.0` (only
the file format defaults it to 16), a button without a `ButtonState` component never
reacts, and a tree without both `Parent` *and* `Children` links doesn't lay out.

## Interaction, exactly

One engine system (`interact_ui`, every frame, before layout) does all of it:

1. Map the cursor to UI space via `Camera2D::screen_to_ui`.
2. Set `UiFocus::pointer_over_ui` — true iff any placed panel/button/image rect contains
   the pointer. Labels and `kind`-less nodes never block.
3. Edge-detect the left mouse button *at frame level* (input state is per-tick; a fast
   monitor shows the same tick to several frames).
4. Hover exactly one button: the one containing the pointer with the highest draw order —
   top-most wins, panels never swallow a button drawn above them.
5. On press: remember the button's id. On release: if the pointer is still on the *same*
   button, send the command `ui:click` with the id. Press A, drag to B, release: nothing
   fires — the standard escape hatch every UI owes its users.

`ButtonState` is rewritten every frame (hover/press are frame-cosmetic); the click is the
only part that becomes gameplay. Note the timing: interaction hit-tests against the
*previous* frame's rects (layout runs after it in the frame), so a button that teleported
this frame is clickable at its old spot for one frame. At 60+ fps this has never mattered;
it's documented so that when it does, you'll know it was a law and not a ghost.

## The click's journey

```text
release frame:  interact_ui  ──►  CommandOutbox: ("ui:click", "resume")
next tick:      replay_step  ──►  CommandEvent stream (recorded / replayed here)
same tick:      dispatch_ui_commands  ──►  UiEvent::Clicked("resume")
that tick + the next:  your EventReader<UiEvent> in FixedUpdate
```

This is why clicks replay, why handlers run on the tick after the release frame, and why
tests can inject `outbox.send("ui:click", id)` and be indistinguishable from a mouse. The
`ui:` command-name prefix is reserved for the engine. `UiEvent::Clicked` is the **only**
UI event.

`UiFocus` is the sim-side etiquette flag: read it in any world-click system and do nothing
when the pointer is over UI. It's a plain resource — `GamePlugin`-style code should insert
a default when running without `UiPlugin` (headless sim tests).

## What deliberately doesn't exist

No slider, text input, checkbox, radio, dropdown, scroll view, progress bar, tooltip,
list/grid, tabs, modal stack, or drag handle — the widget floor is "colored rects, text,
and clicks", and everything richer is composed in layout + game code (Recipes has the
common ones). No hover/press/focus-change events — poll `ButtonState` if a frame system
cares. No keyboard focus, tab order, or gamepad navigation: menus in shipped Fulcrum games
read raw `Input` in the sim and call it a feature, because *that* input is recorded too.
Left mouse button only; no scroll wheel, right-click, double-click, or drag in the UI
layer. No per-state button textures, no caption colors, no disabled state — hide the
button or guard the handler.
