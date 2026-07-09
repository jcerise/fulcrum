//! Registry acceptance: round trips, one-line game registration, and named errors.

use fulcrum_core::{Component, Fulcrum, Transform2D, vec2};
use fulcrum_scene::{ComponentRegistry, RegisterComponentExt, SceneError, ScenePlugin};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
struct Health {
    max: u32,
    current: u32,
}

fn ron_value(text: &str) -> ron::Value {
    ron::from_str(text).unwrap()
}

#[test]
fn transform_round_trips_through_the_registry() {
    let mut app = Fulcrum::new("test").with_plugin(ScenePlugin);
    let world = app.world_mut();
    let value = ron_value("(translation: (12.5, -3.0), rotation: 0.5, scale: (2.0, 2.0))");

    let entity = {
        let id = world.spawn_empty().id();
        world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
            let mut entity = world.entity_mut(id);
            registry
                .insert_on(&mut entity, "Transform2D", &value)
                .unwrap();
        });
        id
    };

    let transform = *world.entity(entity).get::<Transform2D>().unwrap();
    assert_eq!(transform.translation, vec2(12.5, -3.0));
    assert_eq!(transform.rotation, 0.5);

    // Extract and re-insert on a fresh entity: identical component.
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        let extracted = registry
            .extract_from(&world.entity(entity), "Transform2D")
            .unwrap();
        let clone_id = world.spawn_empty().id();
        let mut clone = world.entity_mut(clone_id);
        registry
            .insert_on(&mut clone, "Transform2D", &extracted)
            .unwrap();
        let round = *world.entity(clone_id).get::<Transform2D>().unwrap();
        assert_eq!(round, transform);
    });
}

#[test]
fn game_component_registers_with_one_line() {
    let mut app = Fulcrum::new("test")
        .with_plugin(ScenePlugin)
        .register_component::<Health>("Health");
    let world = app.world_mut();
    let value = ron_value("(max: 30, current: 12)");

    let id = world.spawn_empty().id();
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        let mut entity = world.entity_mut(id);
        registry.insert_on(&mut entity, "Health", &value).unwrap();
        let extracted = registry.extract_from(&world.entity(id), "Health").unwrap();
        assert_eq!(
            extracted.into_rust::<Health>().unwrap(),
            Health {
                max: 30,
                current: 12
            }
        );
    });
}

#[test]
fn registration_order_does_not_matter() {
    // register_component before the plugin installs the registry.
    let mut app = Fulcrum::new("test")
        .register_component::<Health>("Health")
        .with_plugin(ScenePlugin);
    let registry = app.world_mut().resource::<ComponentRegistry>();
    assert!(registry.contains("Health"));
    assert!(
        registry.contains("Transform2D"),
        "built-ins still registered"
    );
}

#[test]
fn errors_name_the_component_and_problem() {
    let mut app = Fulcrum::new("test").with_plugin(ScenePlugin);
    let world = app.world_mut();
    let id = world.spawn_empty().id();
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        let mut entity = world.entity_mut(id);

        let unknown = registry
            .insert_on(&mut entity, "Frobnicator", &ron_value("()"))
            .unwrap_err();
        assert!(matches!(unknown, SceneError::UnknownComponent(ref n) if n == "Frobnicator"));
        assert!(unknown.to_string().contains("Frobnicator"));

        let bad = registry
            .insert_on(
                &mut entity,
                "Transform2D",
                &ron_value("(translation: \"oops\")"),
            )
            .unwrap_err();
        let message = bad.to_string();
        assert!(
            message.contains("Transform2D"),
            "names the component: {message}"
        );
    });
}
