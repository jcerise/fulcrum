//! Bindings acceptance: Lua spawns from prefabs, mutates registered components, queries
//! deterministically, and events cross the Lua/Rust boundary.

use bevy_ecs::prelude::Messages;
use fulcrum_core::{Component, Fulcrum, Transform2D};
use fulcrum_mod::{LuaRuntime, ModEvent, run_init_with_world, run_tick_with_world};
use fulcrum_scene::{RegisterComponentExt, ScenePlugin};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
struct Health {
    max: i64,
    current: i64,
}

const SLIME: &str = r#"Prefab(
    components: {
        "Transform2D": (translation: (5.0, 6.0)),
        "Health": (max: 10, current: 10),
    },
)"#;

fn setup(name: &str, script: &str) -> (Fulcrum, LuaRuntime) {
    let dir = std::env::temp_dir().join(format!("fulcrum-bind-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("mod/scripts")).unwrap();
    std::fs::create_dir_all(dir.join("assets/prefabs")).unwrap();
    std::fs::write(dir.join("assets/prefabs/slime.prefab.ron"), SLIME).unwrap();
    std::fs::write(dir.join("mod/scripts/init.lua"), script).unwrap();

    let mut app = Fulcrum::new("bind test")
        .insert_resource(fulcrum_asset::AssetServer::new(dir.join("assets")))
        .with_plugin(ScenePlugin)
        .register_component::<Health>("Health");
    app.world_mut().init_resource::<Messages<ModEvent>>();

    let mut runtime = LuaRuntime::new(7).unwrap();
    fulcrum_mod::bindings::install(&runtime).unwrap();
    runtime.register_mod("testmod", dir.join("mod"));
    runtime.run_entry("testmod", "scripts/init.lua").unwrap();
    (app, runtime)
}

#[test]
fn lua_spawns_buffs_and_rust_observes() {
    let (mut app, mut runtime) = setup(
        "spawn",
        r#"
        fulcrum.on_init(function()
            for i = 1, 10 do
                local e = fulcrum.spawn_prefab("prefabs/slime.prefab.ron", { x = i * 10, y = 0 })
                local hp = fulcrum.get(e, "Health")
                fulcrum.set(e, "Health", { max = hp.max * 2, current = hp.max * 2 })
            end
        end)
        "#,
    );
    app.run_startup();
    run_init_with_world(&mut runtime, app.world_mut());

    let world = app.world_mut();
    let slimes: Vec<(Transform2D, Health)> = world
        .query::<(&Transform2D, &Health)>()
        .iter(world)
        .map(|(t, h)| (*t, h.clone()))
        .collect();
    assert_eq!(slimes.len(), 10);
    assert!(
        slimes.iter().all(|(_, h)| h.max == 20 && h.current == 20),
        "buffed by Lua"
    );
    assert!(
        slimes.iter().any(|(t, _)| t.translation.x == 100.0),
        "position override applied"
    );
}

#[test]
fn queries_read_the_world_and_stale_entities_are_nil() {
    let (mut app, mut runtime) = setup(
        "query",
        r#"
        fulcrum.on_init(function()
            first = fulcrum.spawn_prefab("prefabs/slime.prefab.ron")
            fulcrum.spawn_prefab("prefabs/slime.prefab.ron", { x = 50, y = 0 })
        end)
        fulcrum.on_tick(function(tick)
            local rows = fulcrum.query("Transform2D", "Health")
            count = #rows
            total = 0
            for _, row in ipairs(rows) do
                total = total + row.Health.current
            end
            if tick == 1 then
                fulcrum.despawn(first)
                after_despawn = fulcrum.get(first, "Health")
            end
        end)
        "#,
    );
    app.run_startup();
    run_init_with_world(&mut runtime, app.world_mut());
    for _ in 0..3 {
        app.tick();
        run_tick_with_world(&mut runtime, app.world_mut());
    }
    assert_eq!(
        runtime.eval_string("return tostring(count)").unwrap(),
        "1",
        "one left after despawn"
    );
    assert_eq!(runtime.eval_string("return tostring(total)").unwrap(), "10");
    assert_eq!(
        runtime
            .eval_string("return tostring(after_despawn)")
            .unwrap(),
        "nil"
    );
}

#[test]
fn events_cross_both_boundaries() {
    let (mut app, mut runtime) = setup(
        "events",
        r#"
        heard = 0
        fulcrum.on_event("boom", function(payload)
            heard = heard + payload.power
        end)
        fulcrum.on_tick(function(tick)
            if tick == 0 then
                fulcrum.emit("boom", { power = 7 })
            end
        end)
        "#,
    );
    app.run_startup();
    run_tick_with_world(&mut runtime, app.world_mut());

    // Lua handler heard the Lua-emitted event (same batch, one round).
    assert_eq!(runtime.eval_string("return tostring(heard)").unwrap(), "7");
    // Rust sees it as a ModEvent message.
    let world = app.world_mut();
    let messages = world.resource_mut::<Messages<ModEvent>>();
    let events: Vec<&ModEvent> = messages.iter_current_update_messages().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].name, "boom");
}

#[test]
fn lua_driven_worlds_are_deterministic() {
    let run = |seed: u64| {
        let (mut app, mut runtime) = setup(
            &format!("determ-{seed}"),
            r#"
            fulcrum.on_tick(function(tick)
                if math.random() < 0.3 then
                    fulcrum.spawn_prefab("prefabs/slime.prefab.ron",
                        { x = math.random(0, 640), y = math.random(0, 360) })
                end
            end)
            "#,
        );
        // Note: LuaRuntime::new(7) in setup fixes the Lua seed; the app seed is separate here.
        let _ = seed;
        app.run_startup();
        for _ in 0..100 {
            app.tick();
            run_tick_with_world(&mut runtime, app.world_mut());
        }
        let world = app.world_mut();
        let mut positions: Vec<(u32, u32)> = world
            .query::<&Transform2D>()
            .iter(world)
            .map(|t| (t.translation.x.to_bits(), t.translation.y.to_bits()))
            .collect();
        positions.sort_unstable();
        positions
    };
    let a = run(1);
    let b = run(1);
    assert_eq!(a, b, "same seeds, same Lua-spawned world");
    assert!(!a.is_empty(), "spawns happened");
}
