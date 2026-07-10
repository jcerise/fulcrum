//! The replay state hash: a 64-bit fingerprint of registered simulation state.
//!
//! Covers `Time::tick`, the `SimRng` stream position, and every **registered** component of
//! every entity, walked in canonical order (entities sorted by id, component names sorted) via
//! the [`ComponentRegistry`]'s extract path. Unregistered components and cosmetic resources are
//! invisible to it — register anything whose divergence you want replays to catch. Values hash
//! as their RON text, which round-trips `f32`s exactly (Rust float formatting is
//! shortest-round-trip).

use bevy_ecs::prelude::Entity;
use bevy_ecs::world::World;
use fulcrum_core::{SimRng, Time};
use xxhash_rust::xxh3::Xxh3;

use crate::registry::ComponentRegistry;

/// Fingerprint `world`'s registered simulation state. Installed as the engine's
/// [`StateHasher`](fulcrum_core::StateHasher) by `ScenePlugin`.
pub fn state_hash(world: &mut World) -> u64 {
    let mut hasher = Xxh3::new();
    hasher.update(&world.resource::<Time>().tick.to_le_bytes());
    if let Some(rng) = world.get_resource::<SimRng>() {
        hasher.update(&rng.state_probe().to_le_bytes());
    }
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        let mut entities: Vec<Entity> = {
            let mut query = world.query::<Entity>();
            query.iter(world).collect()
        };
        entities.sort_unstable();
        let names = registry.names();
        for entity in entities {
            let entity_ref = world.entity(entity);
            for name in &names {
                if let Some(value) = registry.extract_from(&entity_ref, name) {
                    hasher.update(&entity.to_bits().to_le_bytes());
                    hasher.update(name.as_bytes());
                    hasher.update(ron::ser::to_string(&value).unwrap_or_default().as_bytes());
                }
            }
        }
    });
    hasher.digest()
}
