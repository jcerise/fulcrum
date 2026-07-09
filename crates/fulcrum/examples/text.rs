//! Step-4 (phase 2) acceptance: crisp pixel-font text at 16/32/64 px, alignment modes, and
//! multi-line stacking. Run: `cargo run -p fulcrum --example text`

use fulcrum::prelude::*;

fn setup(mut commands: Commands, mut gizmos: ResMut<Gizmos>) {
    commands.spawn((
        Text::new("Score: 1234"),
        Transform2D::from_xy(-500.0, 300.0),
    ));
    commands.spawn((
        Text::new("Score: 1234").with_size(32.0),
        Transform2D::from_xy(-500.0, 240.0),
    ));
    commands.spawn((
        Text::new("Score: 1234")
            .with_size(64.0)
            .with_color(Color::rgb(1.0, 0.8, 0.2)),
        Transform2D::from_xy(-500.0, 140.0),
    ));

    // Alignment demo around x = 200 (marked with a line by the overlay system).
    commands.spawn((
        Text::new("left aligned").with_size(16.0),
        Transform2D::from_xy(200.0, 0.0),
    ));
    commands.spawn((
        Text::new("center aligned")
            .with_size(16.0)
            .with_align(HAlign::Center),
        Transform2D::from_xy(200.0, -40.0),
    ));
    commands.spawn((
        Text::new("right aligned")
            .with_size(16.0)
            .with_align(HAlign::Right),
        Transform2D::from_xy(200.0, -80.0),
    ));

    // Multi-line.
    commands.spawn((
        Text::new("line one\nline two\nline three")
            .with_size(24.0)
            .with_color(Color::GREEN),
        Transform2D::from_xy(-500.0, -100.0),
    ));
    let _ = &mut gizmos;
}

fn overlay(mut gizmos: ResMut<Gizmos>) {
    gizmos.line(
        vec2(200.0, 20.0),
        vec2(200.0, -100.0),
        Color::rgb(1.0, 0.3, 0.3),
    );
}

fn main() {
    Fulcrum::with_config(FulcrumConfig {
        title: "text".into(),
        clear_color: Color::rgb(0.1, 0.1, 0.14),
        gizmos_enabled: true,
        ..Default::default()
    })
    .with_plugin(DefaultPlugins)
    .add_startup(setup)
    .add_frame_system(overlay)
    .run();
}
