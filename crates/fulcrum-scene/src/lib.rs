//! Fulcrum's data-driven layer: the [`ComponentRegistry`] (string names ↔ typed components) and,
//! as phase 3 progresses, prefabs and scenes built on it.

pub mod defs;
pub mod prefab;
pub mod registry;
pub mod scene;

use fulcrum_core::{Component, Fulcrum, IntoScheduleConfigs, Plugin, Transform2D, Update};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use defs::{AnimationPlayerDef, SpriteDef, TextDef, TilemapDef};
pub use prefab::{PrefabAsset, PrefabLoader, PrefabQueue, SpawnPrefabExt};
pub use registry::{ComponentOps, ComponentRegistry, SceneError};
pub use scene::{SceneAsset, SceneLoader, SceneMember, SceneSpawner, save_world};

fn registry_mut(app: &mut Fulcrum) -> bevy_ecs::world::Mut<'_, ComponentRegistry> {
    if app.world().get_resource::<ComponentRegistry>().is_none() {
        app.world_mut()
            .insert_resource(ComponentRegistry::default());
    }
    app.world_mut().resource_mut::<ComponentRegistry>()
}

/// The game-facing one-liner: `app.register_component::<Health>("Health")`. Works before or
/// after `DefaultPlugins`.
pub trait RegisterComponentExt: Sized {
    /// Register `T` for use in prefabs, scenes, and the inspector under `name`.
    fn register_component<T: Component + Serialize + DeserializeOwned + Default>(
        self,
        name: &str,
    ) -> Self;
}

impl RegisterComponentExt for Fulcrum {
    fn register_component<T: Component + Serialize + DeserializeOwned + Default>(
        mut self,
        name: &str,
    ) -> Self {
        registry_mut(&mut self).register::<T>(name);
        self
    }
}

/// Installs the registry (with the serializable built-ins) and the def-resolver systems.
/// Part of `DefaultPlugins`.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut Fulcrum) {
        {
            let mut registry = registry_mut(app);
            registry.register::<Transform2D>("Transform2D");
            registry.register::<SpriteDef>("Sprite");
            registry.register::<TextDef>("Text");
            registry.register::<AnimationPlayerDef>("AnimationPlayer");
            registry.register::<TilemapDef>("Tilemap");
        }
        app.world_mut()
            .insert_resource(fulcrum_asset::Assets::<PrefabAsset>::default());
        app.world_mut()
            .insert_resource(fulcrum_asset::Assets::<SceneAsset>::default());
        app.world_mut().insert_resource(PrefabQueue::default());
        app.world_mut().insert_resource(SceneSpawner::default());
        // Queued prefab/scene work applies first thing each tick. NOTE: add DefaultPlugins
        // before game plugins so this runs before game systems (single-threaded FixedUpdate
        // runs in registration order for unordered systems).
        app.add_systems(fulcrum_core::FixedUpdate, prefab::apply_spawn_queues);
        // Resolvers are cosmetic (they attach visuals); chained because both touch texture
        // storage.
        app.add_systems(
            Update,
            (defs::resolve_plain_defs, defs::resolve_aseprite_defs).chain(),
        );
    }
}
