//! Chapter 1: a window. This is the smallest Fulcrum program.

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
