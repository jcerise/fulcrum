//! Scenes: collections of prefab instances and inline entities, loaded and unloaded as a unit.
//!
//! ```ron
//! Scene(
//!     entities: [
//!         ( prefab: "prefabs/slime.prefab.ron", at: (128.0, 64.0) ),
//!         ( components: { "Tilemap": (asset: "maps/level1.map.ron") } ),
//!     ],
//! )
//! ```
//!
//! Level transition pattern: `scenes.unload(current); scenes.load(next);` — both take effect at
//! the next tick boundary, unload first.

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Component, Entity, Resource};
use bevy_ecs::world::World;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{Transform2D, Vec2};
use serde::Deserialize;

use crate::prefab::{PrefabAsset, QueuedSpawn, apply_spawn, parse_prefab};
use crate::registry::ComponentRegistry;

#[derive(Deserialize)]
#[serde(rename = "Scene")]
struct SceneDef {
    entities: Vec<EntryDef>,
}

#[derive(Deserialize)]
struct EntryDef {
    #[serde(default)]
    prefab: Option<String>,
    #[serde(default)]
    at: Option<Vec2>,
    #[serde(default)]
    components: BTreeMap<String, ron::Value>,
}

#[derive(Clone)]
pub(crate) struct SceneEntry {
    prefab: Option<String>,
    at: Option<Vec2>,
    components: Vec<(String, ron::Value)>,
}

/// A parsed scene, loadable any number of times (though usually once).
pub struct SceneAsset {
    pub(crate) entries: Vec<SceneEntry>,
}

/// Tag on every entity a scene spawned; unloading despawns by this tag.
#[derive(Component, Clone, Copy)]
pub struct SceneMember(pub Handle<SceneAsset>);

/// Queue scene loads/unloads; applied at the next tick boundary (unloads first).
#[derive(Resource, Default)]
pub struct SceneSpawner {
    loads: Vec<Handle<SceneAsset>>,
    unloads: Vec<Handle<SceneAsset>>,
}

impl SceneSpawner {
    /// Spawn every entity in the scene at the next tick boundary.
    pub fn load(&mut self, scene: Handle<SceneAsset>) {
        self.loads.push(scene);
    }

    /// Despawn every entity this scene spawned, at the next tick boundary.
    pub fn unload(&mut self, scene: Handle<SceneAsset>) {
        self.unloads.push(scene);
    }
}

pub(crate) fn parse_scene(path: &str, source: &str) -> Result<SceneAsset, crate::SceneError> {
    let def: SceneDef = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .map_err(|error| crate::SceneError::Parse {
            path: path.to_string(),
            message: error.to_string(),
        })?;
    Ok(SceneAsset {
        entries: def
            .entities
            .into_iter()
            .map(|entry| SceneEntry {
                prefab: entry.prefab,
                at: entry.at,
                components: entry.components.into_iter().collect(),
            })
            .collect(),
    })
}

/// Get (or load from disk) a prefab by path — used when a scene references a prefab that hasn't
/// been loaded yet.
fn prefab_by_path(world: &mut World, path: &str) -> Option<Handle<PrefabAsset>> {
    if let Some(handle) = world
        .resource::<Assets<PrefabAsset>>()
        .handle_for_path(path)
    {
        return Some(handle);
    }
    let bytes = match world.resource::<AssetServer>().read_bytes(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::error!("scene: {error}");
            return None;
        }
    };
    match parse_prefab(path, &String::from_utf8_lossy(&bytes)) {
        Ok(asset) => Some(
            world
                .resource_mut::<Assets<PrefabAsset>>()
                .insert_with_path(path, asset),
        ),
        Err(error) => {
            log::error!("scene: {error}");
            None
        }
    }
}

/// Process queued unloads then loads. Called from the shared exclusive spawn system.
pub(crate) fn apply_scene_queues(world: &mut World) {
    let (unloads, loads) = {
        let mut spawner = world.resource_mut::<SceneSpawner>();
        (
            std::mem::take(&mut spawner.unloads),
            std::mem::take(&mut spawner.loads),
        )
    };
    for scene in unloads {
        let members: Vec<Entity> = world
            .query::<(Entity, &SceneMember)>()
            .iter(world)
            .filter(|(_, member)| member.0 == scene)
            .map(|(entity, _)| entity)
            .collect();
        for entity in members {
            world.despawn(entity);
        }
    }
    for scene in loads {
        let Some(entries) = world
            .resource::<Assets<SceneAsset>>()
            .get(scene)
            .map(|asset| asset.entries.clone())
        else {
            log::error!("SceneSpawner::load: unknown scene handle");
            continue;
        };
        for entry in entries {
            let entity = world.spawn_empty().id();
            if let Some(prefab_path) = &entry.prefab {
                let Some(prefab) = prefab_by_path(world, prefab_path) else {
                    world.despawn(entity);
                    continue;
                };
                apply_spawn(
                    world,
                    QueuedSpawn {
                        entity,
                        prefab,
                        position: entry.at,
                        scene: Some(scene),
                    },
                );
            } else {
                world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
                    let mut target = world.entity_mut(entity);
                    for (name, value) in &entry.components {
                        if let Err(error) = registry.insert_on(&mut target, name, value) {
                            log::error!("scene: {error}; component skipped");
                        }
                    }
                });
                world.entity_mut(entity).insert(SceneMember(scene));
                if let Some(position) = entry.at {
                    let mut target = world.entity_mut(entity);
                    if let Some(mut transform) = target.get_mut::<Transform2D>() {
                        transform.translation = position;
                    } else {
                        target.insert(Transform2D::from_translation(position));
                    }
                }
            }
        }
    }
}

/// Serialize every entity with at least one registered component to scene RON — a dev tool
/// (inspector "save scene"), not a runtime feature.
pub fn save_world(world: &mut World) -> String {
    let mut entity_ids: Vec<Entity> = world.query::<Entity>().iter(world).collect();
    entity_ids.sort_unstable();

    let mut out = String::from("Scene(\n    entities: [\n");
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        for entity in entity_ids {
            let entity_ref = world.entity(entity);
            let mut parts = Vec::new();
            for name in registry.names() {
                if let Some(value) = registry.extract_from(&entity_ref, name)
                    && let Ok(text) = ron::to_string(&value)
                {
                    parts.push(format!("\"{name}\": {text}"));
                }
            }
            if !parts.is_empty() {
                out.push_str(&format!(
                    "        ( components: {{ {} }} ),\n",
                    parts.join(", ")
                ));
            }
        }
    });
    out.push_str("    ],\n)\n");
    out
}

/// One-line scene loading: `let level = scenes.load("scenes/level1.scene.ron")?;`
#[derive(bevy_ecs::system::SystemParam)]
pub struct SceneLoader<'w> {
    server: bevy_ecs::prelude::Res<'w, AssetServer>,
    scenes: bevy_ecs::prelude::ResMut<'w, Assets<SceneAsset>>,
}

impl SceneLoader<'_> {
    /// Load and parse a scene file, deduplicated by path.
    pub fn load(&mut self, path: &str) -> Result<Handle<SceneAsset>, AssetError> {
        if let Some(handle) = self.scenes.handle_for_path(path) {
            return Ok(handle);
        }
        let bytes = self.server.read_bytes(path)?;
        let asset = parse_scene(path, &String::from_utf8_lossy(&bytes)).map_err(|error| {
            AssetError::Decode {
                path: path.to_string(),
                message: error.to_string(),
            }
        })?;
        Ok(self.scenes.insert_with_path(path, asset))
    }
}
