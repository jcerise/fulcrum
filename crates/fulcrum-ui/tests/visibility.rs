//! Visibility must be a full opt-out: a hidden node (or a child of one) draws nothing,
//! claims no pointer focus, and eats no clicks. Regression test for the stale-`UiRect`
//! bug: layout only wrote rects for placed nodes and never removed them, so anything
//! shown once kept drawing and hit-testing after being hidden.

use fulcrum_asset::{AssetServer, Assets};
use fulcrum_core::{Fulcrum, FulcrumConfig, Input, Rect, Update, Vec2};
use fulcrum_render::{Camera2D, Texture, WindowInfo};
use fulcrum_ui::{UiFocus, UiId, UiLoader, UiNode, UiPlugin, UiRect};

const LAYOUT: &str = r#"Ui(root: Node(
    size: Fill,
    children: [
        Node(id: "menu", anchor: Center, pivot: (0.5, 0.5), size: Px((120, 60)),
            kind: Panel(color: (r: 0.1, g: 0.1, b: 0.1, a: 0.9)),
            children: [
                Node(id: "go", size: Px((80, 24)), kind: Button(text: "Go")),
            ],
        ),
    ],
))"#;

/// Headless UI harness: no window, no GPU — layout and interaction run in `Update`
/// against a hand-inserted camera and window size (Stretch: UI px == screen px).
fn build() -> Fulcrum {
    let dir = std::env::temp_dir().join("fulcrum-ui-visibility-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("menu.ui.ron"), LAYOUT).unwrap();
    Fulcrum::with_config(FulcrumConfig::default())
        .insert_resource(AssetServer::new(dir))
        .insert_resource(Assets::<Texture>::default())
        .insert_resource(Camera2D::default())
        .insert_resource(WindowInfo {
            width: 320,
            height: 180,
            scale_factor: 1.0,
        })
        .with_plugin(UiPlugin)
        .add_startup(load_menu)
}

fn load_menu(mut ui: UiLoader) {
    ui.load("menu.ui.ron").expect("menu loads");
}

fn frame(app: &mut Fulcrum) {
    app.world_mut().run_schedule(Update);
}

fn node_rect(app: &mut Fulcrum, id: &str) -> Option<Rect> {
    let world = app.world_mut();
    let mut nodes = world.query::<(&UiId, Option<&UiRect>)>();
    nodes
        .iter(world)
        .find(|(node_id, _)| node_id.0 == id)
        .expect("node exists")
        .1
        .map(|placed| placed.rect)
}

fn set_visible(app: &mut Fulcrum, id: &str, visible: bool) {
    let world = app.world_mut();
    let mut nodes = world.query::<(&UiId, &mut UiNode)>();
    for (node_id, mut node) in nodes.iter_mut(world) {
        if node_id.0 == id {
            node.visible = visible;
        }
    }
}

fn point_at(app: &mut Fulcrum, screen: Vec2) {
    let mut input = app.world_mut().resource_mut::<Input>();
    input.push_cursor(screen);
    input.sample(|s| s);
}

#[test]
fn hiding_a_subtree_stops_drawing_and_hit_testing() {
    let mut app = build();
    app.run_startup();
    frame(&mut app);

    let menu = node_rect(&mut app, "menu").expect("visible menu has a rect");
    let center = menu.center();

    // Pointer over the visible menu: the UI claims it.
    point_at(&mut app, center);
    frame(&mut app);
    assert!(app.world().resource::<UiFocus>().pointer_over_ui);

    // Hide it — the exact toggle `UiQuery::set_visible` performs.
    set_visible(&mut app, "menu", false);
    frame(&mut app);
    assert!(
        node_rect(&mut app, "menu").is_none(),
        "a hidden node must not keep a stale UiRect"
    );
    assert!(
        node_rect(&mut app, "go").is_none(),
        "a child of a hidden node must not keep a stale UiRect"
    );
    frame(&mut app);
    assert!(
        !app.world().resource::<UiFocus>().pointer_over_ui,
        "hidden UI must not claim the pointer"
    );

    // Show it again: same place, hit-testing back on.
    set_visible(&mut app, "menu", true);
    frame(&mut app);
    let shown = node_rect(&mut app, "menu").expect("re-shown menu has a rect");
    assert_eq!((shown.min, shown.max), (menu.min, menu.max));
    frame(&mut app);
    assert!(app.world().resource::<UiFocus>().pointer_over_ui);
}
