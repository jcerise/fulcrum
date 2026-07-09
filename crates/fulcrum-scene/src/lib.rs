//! Fulcrum's data-driven layer: the [`ComponentRegistry`] (string names ↔ typed components) and,
//! as phase 3 progresses, prefabs and scenes built on it.

pub mod defs;
pub mod prefab;
pub mod registry;
pub mod scene;

use fulcrum_core::{Component, Fulcrum, IntoScheduleConfigs, Plugin, Transform2D, Update};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use defs::{AnimationPlayerDef, AnimatorDef, SpriteDef, TextDef, TilemapDef};
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

/// Hot reload: re-parse changed prefab/scene files in place. Live entities are not
/// retro-patched — edits affect future spawns (respawn via scene reload is the workflow).
fn reload_data_assets(
    mut events: fulcrum_core::EventReader<fulcrum_asset::AssetEvent>,
    server: bevy_ecs::prelude::Res<fulcrum_asset::AssetServer>,
    mut prefabs: bevy_ecs::prelude::ResMut<fulcrum_asset::Assets<PrefabAsset>>,
    mut scenes: bevy_ecs::prelude::ResMut<fulcrum_asset::Assets<SceneAsset>>,
) {
    for event in events.read() {
        let path = &event.path;
        if let Some(handle) = prefabs.handle_for_path(path) {
            match server
                .read_bytes(path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    prefab::parse_prefab(path, &String::from_utf8_lossy(&b))
                        .map_err(|e| e.to_string())
                }) {
                Ok(asset) => {
                    prefabs.replace(handle, asset);
                    log::info!("reloaded prefab {path} (affects future spawns)");
                }
                Err(error) => log::error!("hot reload: {error}"),
            }
        }
        if let Some(handle) = scenes.handle_for_path(path) {
            match server
                .read_bytes(path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    scene::parse_scene(path, &String::from_utf8_lossy(&b))
                        .map_err(|e| e.to_string())
                }) {
                Ok(asset) => {
                    scenes.replace(handle, asset);
                    log::info!("reloaded scene {path}");
                }
                Err(error) => log::error!("hot reload: {error}"),
            }
        }
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
            registry.register::<defs::AnimatorDef>("Animator");
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
            (
                defs::resolve_plain_defs,
                defs::resolve_tilemap_defs,
                defs::resolve_aseprite_defs,
                defs::resolve_animator_defs,
            )
                .chain(),
        );
        app.register_event::<fulcrum_asset::AssetEvent>();
        app.add_systems(Update, reload_data_assets);
    }
}
