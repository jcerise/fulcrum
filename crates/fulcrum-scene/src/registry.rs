//! The [`ComponentRegistry`]: string names ↔ typed components, via serde.
//!
//! This is the keystone of everything data-driven: prefabs, scenes, the inspector, and (phase 4)
//! Lua bindings all reach typed components exclusively through this map. The convention across
//! all data files: **assets are referenced by path string** — handles never appear in files
//! (path-bearing components register serde-friendly `*Def` mirrors that resolver systems turn
//! into the real thing).

use bevy_ecs::prelude::Resource;
use bevy_ecs::world::{EntityRef, EntityWorldMut};
use fulcrum_core::Component;
use rustc_hash::FxHashMap;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Errors from data-driven spawning. These are user-facing authoring errors — they must name
/// the thing that's wrong.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    /// A data file referenced a component name nobody registered.
    #[error("unknown component `{0}` (register it with app.register_component::<T>(\"{0}\"))")]
    UnknownComponent(String),
    /// A component's RON value didn't match its Rust shape.
    #[error("component `{name}`: {message}")]
    InvalidValue {
        /// The registered component name.
        name: String,
        /// What serde rejected.
        message: String,
    },
    /// A data file failed to parse at all.
    #[error("failed to parse `{path}`: {message}")]
    Parse {
        /// The asset path.
        path: String,
        /// Parser diagnostics.
        message: String,
    },
}

/// The type-erased operations stored per registered component.
pub struct ComponentOps {
    /// Deserialize a RON value and insert it on the entity.
    pub insert: fn(&mut EntityWorldMut, &ron::Value) -> Result<(), SceneError>,
    /// Serialize the entity's component to a RON value (`None` if absent).
    pub extract: fn(&EntityRef) -> Option<ron::Value>,
    /// Insert the component's `Default` value.
    pub default_insert: fn(&mut EntityWorldMut),
}

/// Maps registered names to component operations.
#[derive(Resource, Default)]
pub struct ComponentRegistry {
    ops: FxHashMap<String, ComponentOps>,
}

fn insert_impl<T: Component + DeserializeOwned>(
    name: &str,
    entity: &mut EntityWorldMut,
    value: &ron::Value,
) -> Result<(), SceneError> {
    let component: T = value
        .clone()
        .into_rust()
        .map_err(|error| SceneError::InvalidValue {
            name: name.to_string(),
            message: error.to_string(),
        })?;
    entity.insert(component);
    Ok(())
}

fn extract_impl<T: Component + Serialize>(entity: &EntityRef) -> Option<ron::Value> {
    let component = entity.get::<T>()?;
    let text = ron::to_string(component).ok()?;
    ron::from_str(&text).ok()
}

impl ComponentRegistry {
    /// Register `T` under `name`. The name is the stable identity used in every data file;
    /// changing it breaks existing prefabs/scenes.
    pub fn register<T: Component + Serialize + DeserializeOwned + Default>(&mut self, name: &str) {
        // A generic fn can't be a plain fn pointer with the name captured, so each T gets a
        // trampoline pairing it with the registered name at call time via a lookup.
        fn insert_trampoline<T: Component + DeserializeOwned>(
            entity: &mut EntityWorldMut,
            value: &ron::Value,
        ) -> Result<(), SceneError> {
            insert_impl::<T>(std::any::type_name::<T>(), entity, value)
        }
        if self.ops.contains_key(name) {
            log::warn!("component `{name}` registered twice; keeping the first registration");
            return;
        }
        self.ops.insert(
            name.to_string(),
            ComponentOps {
                insert: insert_trampoline::<T>,
                extract: extract_impl::<T>,
                default_insert: |entity| {
                    entity.insert(T::default());
                },
            },
        );
    }

    /// Deserialize `value` as the component registered under `name` and insert it on `entity`.
    pub fn insert_on(
        &self,
        entity: &mut EntityWorldMut,
        name: &str,
        value: &ron::Value,
    ) -> Result<(), SceneError> {
        let ops = self
            .ops
            .get(name)
            .ok_or_else(|| SceneError::UnknownComponent(name.to_string()))?;
        (ops.insert)(entity, value).map_err(|error| match error {
            // Re-key the error with the registered name (the trampoline only knows the type).
            SceneError::InvalidValue { message, .. } => SceneError::InvalidValue {
                name: name.to_string(),
                message,
            },
            other => other,
        })
    }

    /// Serialize the component registered under `name` from `entity`, if present.
    pub fn extract_from(&self, entity: &EntityRef, name: &str) -> Option<ron::Value> {
        (self.ops.get(name)?.extract)(entity)
    }

    /// All registered names, sorted (deterministic iteration for scene saving).
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.ops.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Is `name` registered?
    pub fn contains(&self, name: &str) -> bool {
        self.ops.contains_key(name)
    }
}
