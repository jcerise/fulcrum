//! Prefabs: reusable entity definitions in RON, spawned through the component registry.
//!
//! ```ron
//! Prefab(
//!     components: {
//!         "Transform2D": (translation: (0.0, 0.0)),
//!         "Sprite": (sheet: "creatures.json", region: "slime 0"),
//!         "Health": (max: 10, current: 10),
//!     },
//!     children: [ Prefab( components: { /* ... */ } ) ],
//! )
//! ```
//!
//! `commands.spawn_prefab(handle)` returns the `Entity` immediately; the components are applied
//! by an exclusive system at the start of the next `FixedUpdate`, in queue order —
//! deterministic. Children become separate entities linked with [`Parent`]/[`Children`]; their
//! transforms are composed with the parent's **once, at spawn** (there is no ongoing transform
//! propagation).

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Commands, Entity, Resource};
use bevy_ecs::world::World;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{Children, Parent, Transform2D, Vec2};
use serde::Deserialize;

use crate::registry::ComponentRegistry;

#[derive(Deserialize)]
#[serde(rename = "Prefab")]
struct PrefabDef {
    #[serde(default)]
    components: BTreeMap<String, ron::Value>,
    #[serde(default)]
    children: Vec<PrefabDef>,
}

/// One entity's worth of prefab data. Component order is name-sorted (deterministic apply).
#[derive(Clone)]
pub(crate) struct PrefabNode {
    pub(crate) components: Vec<(String, ron::Value)>,
    pub(crate) children: Vec<PrefabNode>,
}

impl From<PrefabDef> for PrefabNode {
    fn from(def: PrefabDef) -> Self {
        Self {
            components: def.components.into_iter().collect(),
            children: def.children.into_iter().map(Into::into).collect(),
        }
    }
}

/// A parsed prefab, ready to spawn any number of times.
pub struct PrefabAsset {
    pub(crate) root: PrefabNode,
}

/// Parse prefab RON text.
pub(crate) fn parse_prefab(path: &str, source: &str) -> Result<PrefabAsset, crate::SceneError> {
    let def: PrefabDef = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .map_err(|error| crate::SceneError::Parse {
            path: path.to_string(),
            message: error.to_string(),
        })?;
    Ok(PrefabAsset { root: def.into() })
}

/// A queued prefab instantiation (entity already reserved).
pub(crate) struct QueuedSpawn {
    pub(crate) entity: Entity,
    pub(crate) prefab: Handle<PrefabAsset>,
    pub(crate) position: Option<Vec2>,
    /// Tag the spawned tree as a member of this scene (used by scene loading).
    pub(crate) scene: Option<Handle<crate::scene::SceneAsset>>,
}

/// Spawns waiting to be applied at the next tick boundary.
#[derive(Resource, Default)]
pub struct PrefabQueue(pub(crate) Vec<QueuedSpawn>);

/// `commands.spawn_prefab(...)` — the game-facing spawn API.
pub trait SpawnPrefabExt {
    /// Queue a prefab instantiation; components apply at the start of the next tick.
    fn spawn_prefab(&mut self, prefab: Handle<PrefabAsset>) -> Entity;
    /// Like [`spawn_prefab`](Self::spawn_prefab), overriding the root `Transform2D` translation.
    fn spawn_prefab_at(&mut self, prefab: Handle<PrefabAsset>, position: Vec2) -> Entity;
}

impl SpawnPrefabExt for Commands<'_, '_> {
    fn spawn_prefab(&mut self, prefab: Handle<PrefabAsset>) -> Entity {
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            world.resource_mut::<PrefabQueue>().0.push(QueuedSpawn {
                entity,
                prefab,
                position: None,
                scene: None,
            });
        });
        entity
    }

    fn spawn_prefab_at(&mut self, prefab: Handle<PrefabAsset>, position: Vec2) -> Entity {
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            world.resource_mut::<PrefabQueue>().0.push(QueuedSpawn {
                entity,
                prefab,
                position: Some(position),
                scene: None,
            });
        });
        entity
    }
}

/// Compose a child's local transform with its parent's (applied once at spawn).
fn compose(parent: &Transform2D, child: &Transform2D) -> Transform2D {
    let (sin, cos) = parent.rotation.sin_cos();
    let scaled = child.translation * parent.scale;
    Transform2D {
        translation: parent.translation
            + Vec2::new(
                scaled.x * cos - scaled.y * sin,
                scaled.x * sin + scaled.y * cos,
            ),
        rotation: parent.rotation + child.rotation,
        scale: parent.scale * child.scale,
    }
}

/// Apply one queued spawn (also used by scene loading).
pub(crate) fn apply_spawn(world: &mut World, spawn: QueuedSpawn) {
    if world.get_entity(spawn.entity).is_err() {
        return; // despawned before the tick boundary
    }
    let Some(root) = world
        .resource::<Assets<PrefabAsset>>()
        .get(spawn.prefab)
        .map(|asset| asset.root.clone())
    else {
        log::error!("spawn_prefab: unknown prefab handle; entity left empty");
        return;
    };
    apply_node(world, spawn.entity, &root, None, spawn.scene);
    if let Some(position) = spawn.position {
        let mut entity = world.entity_mut(spawn.entity);
        if let Some(mut transform) = entity.get_mut::<Transform2D>() {
            transform.translation = position;
        } else {
            entity.insert(Transform2D::from_translation(position));
        }
    }
}

fn apply_node(
    world: &mut World,
    entity: Entity,
    node: &PrefabNode,
    parent_transform: Option<Transform2D>,
    scene: Option<Handle<crate::scene::SceneAsset>>,
) {
    world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
        let mut entity = world.entity_mut(entity);
        for (name, value) in &node.components {
            if let Err(error) = registry.insert_on(&mut entity, name, value) {
                log::error!("prefab: {error}; component skipped");
            }
        }
    });
    if let Some(scene) = scene {
        world
            .entity_mut(entity)
            .insert(crate::scene::SceneMember(scene));
    }
    // Children get world transforms composed once at spawn (no ongoing propagation).
    if let Some(parent) = &parent_transform {
        let local = world
            .entity(entity)
            .get::<Transform2D>()
            .copied()
            .unwrap_or_default();
        world.entity_mut(entity).insert(compose(parent, &local));
    }
    let own_transform = world
        .entity(entity)
        .get::<Transform2D>()
        .copied()
        .unwrap_or_default();

    let mut child_entities = Vec::with_capacity(node.children.len());
    for child in &node.children {
        let child_entity = world.spawn_empty().id();
        apply_node(world, child_entity, child, Some(own_transform), scene);
        world.entity_mut(child_entity).insert(Parent(entity));
        child_entities.push(child_entity);
    }
    if !child_entities.is_empty() {
        world.entity_mut(entity).insert(Children(child_entities));
    }
}

/// Immediately apply a prefab to a reserved entity (Lua bindings; runs inside the exclusive
/// mod stage, so "immediate" is still deterministic call order).
pub fn apply_spawn_now(
    world: &mut World,
    entity: Entity,
    prefab: Handle<PrefabAsset>,
    position: Option<Vec2>,
) {
    apply_spawn(
        world,
        QueuedSpawn {
            entity,
            prefab,
            position,
            scene: None,
        },
    );
}

/// Parse prefab RON text (public for the mod bindings; games use [`PrefabLoader`]).
pub fn parse_prefab_public(path: &str, source: &str) -> Result<PrefabAsset, crate::SceneError> {
    parse_prefab(path, source)
}

/// Exclusive system, first in `FixedUpdate`: apply all queued prefab (and scene) work.
pub(crate) fn apply_spawn_queues(world: &mut World) {
    crate::scene::apply_scene_queues(world);
    let queued = std::mem::take(&mut world.resource_mut::<PrefabQueue>().0);
    for spawn in queued {
        apply_spawn(world, spawn);
    }
}

/// One-line prefab loading: `let slime = prefabs.load("prefabs/slime.prefab.ron")?;`
#[derive(bevy_ecs::system::SystemParam)]
pub struct PrefabLoader<'w> {
    server: bevy_ecs::prelude::Res<'w, AssetServer>,
    prefabs: bevy_ecs::prelude::ResMut<'w, Assets<PrefabAsset>>,
}

impl PrefabLoader<'_> {
    /// Load and parse a prefab file, deduplicated by path.
    pub fn load(&mut self, path: &str) -> Result<Handle<PrefabAsset>, AssetError> {
        if let Some(handle) = self.prefabs.handle_for_path(path) {
            return Ok(handle);
        }
        let bytes = self.server.read_bytes(path)?;
        let source = String::from_utf8_lossy(&bytes);
        let asset = parse_prefab(path, &source).map_err(|error| AssetError::Decode {
            path: path.to_string(),
            message: error.to_string(),
        })?;
        Ok(self.prefabs.insert_with_path(path, asset))
    }
}
