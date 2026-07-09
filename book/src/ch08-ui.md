# Menus and HUDs: the UI

World-space text was a stopgap; HUDs belong on the screen, not in the garden. Fulcrum's game
UI is a retained tree of nodes, laid out with a model small enough to hold in your head:

- Every node has an **anchor** (which point of its parent it pins to), a **pivot** (which
  point of itself sits on that anchor), an **offset**, and a **size** — `Px(w, h)`, `Fill`,
  or `Fit` (shrink-wrap the content).
- A node can **stack** its children vertically or horizontally with a gap, which covers most
  menu layouts without a constraint solver.
- Coordinates are *UI pixels* — the camera's letterbox virtual resolution — top-left origin,
  +Y down, drawn above the world and untouched by camera movement or zoom.

And like everything else, layouts are files:

```text
Ui(root: Node(
    anchor: TopLeft, size: Fill,
    children: [
        Node(
            anchor: TopLeft, offset: (8, 8), size: Fit,
            kind: Panel(color: (r: 0.0, g: 0.0, b: 0.0, a: 0.5)),
            children: [
                Node(id: "gems", kind: Label(text: "Gems: 0/8", size: 16)),
            ],
        ),
        Node(
            id: "banner", visible: false,
            anchor: Center, pivot: (0.5, 0.5), size: Fit, stack: Vertical(6),
            kind: Panel(color: (r: 0.03, g: 0.05, b: 0.03, a: 0.9)),
            children: [
                Node(id: "banner_text", kind: Label(text: "", size: 24)),
                Node(kind: Label(text: "press Enter", size: 8)),
            ],
        ),
    ],
))
```

Widgets: `Panel` (color, optional image, optional nine-slice), `Label`, `Button` (state
colors + caption), `Image`. Load it once — `ui.load("ui/hud.ui.ron")` — and drive the dynamic
bits by id from a frame system:

```rust,ignore
fn hud(mut ui: UiQuery, gems: Res<Gems>, state: Res<GroveState>) {
    ui.set_label("gems", format!("Gems: {}/{}", gems.collected, gems.total));
    ui.set_visible("banner", *state != GroveState::Playing);
}
```

Layouts are stateless on purpose: hot reload just respawns the tree, and your `hud` system
repopulates it next frame. Restyle the HUD while the game runs; it's the same live loop as
every other asset.

## Buttons and the sim boundary

Give a button node an `id` and clicks arrive as events — `UiEvent::Clicked("resume")` —
buffered exactly like input, so the **simulation** reads them:

```rust,ignore
fn pause_menu(mut events: EventReader<UiEvent>, mut state: ResMut<GameState>) {
    for event in events.read() {
        let UiEvent::Clicked(id) = event;
        if id == "resume" { *state = GameState::Playing; }
    }
}
```

Hover and pressed styling happen for free; `Res<UiFocus>` tells world-click code when the
pointer is over UI so a menu click doesn't also swing a sword. (Grove keeps its UI minimal —
a HUD and an end-of-round banner; the dungeon game in the repository shows a pause menu with
working buttons and an inventory toggle built from the same pieces.)
