//! Chapter 13's claim, kept honest: the more_gems mod plants three bonus gems through the
//! same prefab the scene uses — eleven total instead of eight, no game code changed.

use fulcrum::prelude::*;
use grove::game::{self, GamePlugin, Gems};

fn build(with_mods: bool) -> Fulcrum {
    let mut app = Fulcrum::new("grove mods test").insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )));
    if with_mods {
        app = app.with_plugin(ModPlugin::from_dir(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/mods"
        )));
    }
    game::register_components(app)
        .with_plugin(ScenePlugin)
        .with_plugin(GamePlugin)
        .add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("scenes/grove.scene.ron").unwrap();
                spawner.load(level);
            },
        )
}

fn total_after_a_tick(with_mods: bool) -> u32 {
    let mut app = build(with_mods);
    app.run_startup();
    for _ in 0..3 {
        app.tick(); // scene + mod spawns apply; collect_gems recounts the total
    }
    app.world().resource::<Gems>().total
}

#[test]
fn more_gems_mod_plants_three_bonus_gems() {
    assert_eq!(total_after_a_tick(false), 8, "the scene alone spawns 8");
    assert_eq!(total_after_a_tick(true), 11, "the mod adds 3 more");
}
