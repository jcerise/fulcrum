# A Window

Every Fulcrum game is a `Fulcrum` app: a builder you configure, hand plugins and systems, and
`run()`. Here is the whole of chapter one:

```rust,ignore
use fulcrum::prelude::*;

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "Grove".into(),
        clear_color: Color::rgb(0.16, 0.24, 0.16),
        ..Default::default()
    })
    .with_plugin(DefaultPlugins)
    .run();
}
```

Run it (`cargo run -p grove --example ch01_window`) and you get a 1280×720 window cleared to a
mossy green, closing cleanly, vsync'd. Two things are worth understanding before we move on.

## `FulcrumConfig`

Configuration you'd otherwise hunt through ten subsystems for lives in one struct:

| Field | Default | What it does |
| --- | --- | --- |
| `title` | `"Fulcrum"` | Window title. |
| `window_size` | `(1280, 720)` | Initial size in physical pixels. |
| `tick_rate` | `60` | Simulation ticks per second (chapter 3). |
| `seed` | a constant | Seed for the simulation RNG (chapter 10). |
| `clear_color` | black | What the window clears to each frame. |
| `gizmos_enabled` | debug builds | Debug overlay drawing (chapter 9). |
| `hot_reload` | debug builds | Watch `assets/` and reload live (chapter 7). |

`Fulcrum::new("title")` is shorthand when the defaults are fine. The config is also inserted
as a **resource** — any system can read it later via `Res<FulcrumConfig>`.

## `DefaultPlugins`

A *plugin* is a unit of setup: something that adds systems and resources to your app. The
engine's own features are plugins, and `DefaultPlugins` is the bundle that puts a game on
screen: the window and renderer, asset loading, input, audio, animation, the data-driven
layer, UI, and (in debug builds) the inspector. Add it right after the builder and forget
about it.

You'll write your own plugin in chapter 10, when Grove's logic outgrows `main.rs` — a game's
plugin looks exactly like the engine's:

```rust,ignore
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        // add systems, insert resources...
    }
}
```

Nothing draws yet because nothing exists yet. Time to fix that.
