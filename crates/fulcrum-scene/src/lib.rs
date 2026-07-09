//! Fulcrum's data-driven layer: the [`ComponentRegistry`] (string names ↔ typed components) and,
//! as phase 3 progresses, prefabs and scenes built on it.

pub mod defs;
pub mod registry;

use fulcrum_core::{Component, Fulcrum, IntoScheduleConfigs, Plugin, Transform2D, Update};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use defs::{AnimationPlayerDef, SpriteDef, TextDef, TilemapDef};
pub use registry::{ComponentOps, ComponentRegistry, SceneError};

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
        // Resolvers are cosmetic (they attach visuals); chained because both touch texture
        // storage.
        app.add_systems(
            Update,
            (defs::resolve_plain_defs, defs::resolve_aseprite_defs).chain(),
        );
    }
}
