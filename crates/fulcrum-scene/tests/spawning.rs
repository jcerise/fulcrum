//! Prefab and scene spawning: end-to-end headless, from files on disk through the loaders.

use fulcrum_core::{Children, Component, Fulcrum, Parent, Transform2D, vec2};
use fulcrum_scene::{
    PrefabLoader, RegisterComponentExt, SceneLoader, SceneMember, ScenePlugin, SceneSpawner,
    SpawnPrefabExt, save_world,
};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
struct Health {
    max: u32,
    current: u32,
}

const SLIME: &str = r#"Prefab(
    components: {
        "Transform2D": (translation: (10.0, 20.0)),
        "Health": (max: 10, current: 10),
    },
    children: [
        Prefab(components: {
            "Transform2D": (translation: (0.0, 8.0)),
            "Health": (max: 1, current: 1),
        }),
    ],
)"#;

const LEVEL: &str = r#"Scene(
    entities: [
        ( prefab: "slime.prefab.ron", at: (100.0, 50.0) ),
        ( prefab: "slime.prefab.ron" ),
        ( components: { "Transform2D": (translation: (-5.0, -5.0)), "Health": (max: 99, current: 99) } ),
    ],
)"#;

fn assets_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fulcrum-scene-test-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("slime.prefab.ron"), SLIME).unwrap();
    std::fs::write(dir.join("level.scene.ron"), LEVEL).unwrap();
    dir
}

fn app(name: &str) -> Fulcrum {
    Fulcrum::new("test")
        .insert_resource(fulcrum_asset::AssetServer::new(assets_dir(name)))
        .with_plugin(ScenePlugin)
        .register_component::<Health>("Health")
}

#[test]
fn prefab_spawns_with_children_and_position_override() {
    let mut app = app("prefab").add_startup(
        |mut commands: bevy_ecs::prelude::Commands, mut prefabs: PrefabLoader| {
            let slime = prefabs.load("slime.prefab.ron").unwrap();
            commands.spawn_prefab(slime);
            commands.spawn_prefab_at(slime, vec2(200.0, 0.0));
        },
    );
    app.run_startup();
    app.tick(); // queue applies at the tick boundary

    let world = app.world_mut();
    let mut roots: Vec<(Transform2D, Health)> = world
        .query_filtered::<(&Transform2D, &Health), bevy_ecs::prelude::With<Children>>()
        .iter(world)
        .map(|(t, h)| (*t, h.clone()))
        .collect();
    roots.sort_by(|a, b| a.0.translation.x.total_cmp(&b.0.translation.x));
    assert_eq!(roots.len(), 2, "two independent spawns");
    assert_eq!(roots[0].0.translation, vec2(10.0, 20.0), "prefab position");
    assert_eq!(roots[1].0.translation, vec2(200.0, 0.0), "override");
    assert_eq!(
        roots[0].1,
        Health {
            max: 10,
            current: 10
        }
    );

    // Children composed with parent transforms, linked via Parent.
    let children: Vec<Transform2D> = world
        .query_filtered::<&Transform2D, bevy_ecs::prelude::With<Parent>>()
        .iter(world)
        .copied()
        .collect();
    assert_eq!(children.len(), 2);
    assert!(children.iter().any(|t| t.translation == vec2(10.0, 28.0)));
}

#[test]
fn unknown_component_in_prefab_skips_but_applies_the_rest() {
    let dir = assets_dir("badcomp");
    std::fs::write(
        dir.join("bad.prefab.ron"),
        r#"Prefab(components: {
            "Frobnicator": (x: 1),
            "Health": (max: 5, current: 5),
        })"#,
    )
    .unwrap();
    let mut app = app("badcomp").add_startup(
        |mut commands: bevy_ecs::prelude::Commands, mut prefabs: PrefabLoader| {
            let bad = prefabs.load("bad.prefab.ron").unwrap();
            commands.spawn_prefab(bad);
        },
    );
    app.run_startup();
    app.tick();
    let world = app.world_mut();
    let healths: Vec<Health> = world.query::<&Health>().iter(world).cloned().collect();
    assert_eq!(
        healths,
        vec![Health { max: 5, current: 5 }],
        "no panic, rest applied"
    );
}

#[test]
fn scene_load_unload_reload_does_not_leak() {
    let mut app = app("scene");
    // Pre-existing entity that must survive unloads.
    app.world_mut().spawn(Health { max: 1, current: 1 });

    let scene = {
        let mut startup_app = app;
        startup_app = startup_app.add_startup(
            |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
                let level = scenes.load("level.scene.ron").unwrap();
                spawner.load(level);
            },
        );
        startup_app.run_startup();
        startup_app
    };
    let mut app = scene;
    app.tick();

    let count_members = |app: &mut Fulcrum| {
        let world = app.world_mut();
        world.query::<&SceneMember>().iter(world).count()
    };
    let count_health = |app: &mut Fulcrum| {
        let world = app.world_mut();
        world.query::<&Health>().iter(world).count()
    };
    // 2 prefab roots + 2 children + 1 inline = 5 members; healths: those 5 + 1 pre-existing.
    assert_eq!(count_members(&mut app), 5);
    assert_eq!(count_health(&mut app), 6);

    // Unload: only scene members disappear.
    let handle = {
        let world = app.world_mut();
        world
            .resource::<fulcrum_asset::Assets<fulcrum_scene::SceneAsset>>()
            .handle_for_path("level.scene.ron")
            .unwrap()
    };
    app.world_mut()
        .resource_mut::<SceneSpawner>()
        .unload(handle);
    app.tick();
    assert_eq!(count_members(&mut app), 0);
    assert_eq!(count_health(&mut app), 1, "pre-existing entity untouched");

    // Reload twice: no leaks, counts identical.
    for _ in 0..2 {
        app.world_mut().resource_mut::<SceneSpawner>().load(handle);
        app.tick();
        assert_eq!(count_members(&mut app), 5);
        app.world_mut()
            .resource_mut::<SceneSpawner>()
            .unload(handle);
        app.tick();
        assert_eq!(count_members(&mut app), 0);
    }
}

#[test]
fn save_world_round_trips() {
    let mut app = app("save");
    app.world_mut().spawn((
        Transform2D::from_xy(7.0, -3.0),
        Health {
            max: 42,
            current: 40,
        },
    ));
    let saved = save_world(app.world_mut());
    assert!(saved.contains("\"Health\""), "saved: {saved}");

    // Reload the saved text as a scene into a fresh app.
    let dir = assets_dir("save");
    std::fs::write(dir.join("saved.scene.ron"), &saved).unwrap();
    let mut fresh = app_with_saved("save");
    fresh.run_startup();
    fresh.tick();
    let world = fresh.world_mut();
    let restored: Vec<(Transform2D, Health)> = world
        .query::<(&Transform2D, &Health)>()
        .iter(world)
        .map(|(t, h)| (*t, h.clone()))
        .collect();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].0.translation, vec2(7.0, -3.0));
    assert_eq!(
        restored[0].1,
        Health {
            max: 42,
            current: 40
        }
    );
}

fn app_with_saved(name: &str) -> Fulcrum {
    app(name).add_startup(
        |mut scenes: SceneLoader, mut spawner: bevy_ecs::prelude::ResMut<SceneSpawner>| {
            let saved = scenes.load("saved.scene.ron").unwrap();
            spawner.load(saved);
        },
    )
}
