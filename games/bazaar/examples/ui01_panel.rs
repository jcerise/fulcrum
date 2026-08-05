//! UI Book, chapter 1: a panel appears. One loader call turns a RON file into a live UI
//! tree on a 320x180 virtual grid. Nothing here updates anything — layout runs every
//! frame whether or not you look at it.
//! Run: `cargo run -p bazaar --example ui01_panel`

use fulcrum::prelude::*;

fn setup(mut ui: UiLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::IntegerScale {
        width: 320.0,
        height: 180.0,
    };
    camera.center = vec2(160.0, 90.0);
    ui.load("ui/ui01.ui.ron").expect("layout loads");
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "ui01: a panel appears".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.46, 0.65, 0.75),
        hot_reload: true,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .run();
}
