//! [`Assets<T>`]: the storage resource behind every asset type.

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::handle::Handle;

/// Storage for all loaded assets of one type. Loaders dedup by path: loading the same path twice
/// returns the same [`Handle`].
#[derive(Resource)]
pub struct Assets<T: Send + Sync + 'static> {
    items: Vec<T>,
    by_path: FxHashMap<String, u32>,
}

impl<T: Send + Sync + 'static> Default for Assets<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            by_path: FxHashMap::default(),
        }
    }
}

impl<T: Send + Sync + 'static> Assets<T> {
    /// Insert an asset that has no source path (procedurally generated).
    pub fn insert(&mut self, value: T) -> Handle<T> {
        let id = self.items.len() as u32;
        self.items.push(value);
        Handle::new(id)
    }

    /// Insert an asset under a path key, so later loads of the same path dedup to this handle.
    /// Used by loaders; games normally go through a loader API instead.
    pub fn insert_with_path(&mut self, path: impl Into<String>, value: T) -> Handle<T> {
        let handle = self.insert(value);
        self.by_path.insert(path.into(), handle.id());
        handle
    }

    /// Replace the asset behind `handle` in place — every holder of the handle sees the new
    /// data (the hot-reload primitive).
    pub fn replace(&mut self, handle: Handle<T>, value: T) {
        if let Some(slot) = self.items.get_mut(handle.id() as usize) {
            *slot = value;
        } else {
            log::error!("Assets::replace: stale handle {handle:?}");
        }
    }

    /// Insert under a path, or replace in place if the path is already registered.
    pub fn insert_or_replace_with_path(&mut self, path: impl Into<String>, value: T) -> Handle<T> {
        let path = path.into();
        if let Some(handle) = self.handle_for_path(&path) {
            self.replace(handle, value);
            handle
        } else {
            self.insert_with_path(path, value)
        }
    }

    /// The handle previously registered for `path`, if any.
    pub fn handle_for_path(&self, path: &str) -> Option<Handle<T>> {
        self.by_path.get(path).copied().map(Handle::new)
    }

    /// Get an asset by handle.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.items.get(handle.id() as usize)
    }

    /// Get an asset mutably by handle.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.items.get_mut(handle.id() as usize)
    }

    /// Iterate registered (path, handle) pairs, sorted by path (inspector/debug use).
    pub fn paths(&self) -> Vec<(&str, Handle<T>)> {
        let mut entries: Vec<(&str, Handle<T>)> = self
            .by_path
            .iter()
            .map(|(path, id)| (path.as_str(), Handle::new(*id)))
            .collect();
        entries.sort_by_key(|(path, _)| *path);
        entries
    }

    /// Number of stored assets.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
