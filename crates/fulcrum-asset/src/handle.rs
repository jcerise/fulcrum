//! Typed, copyable asset identifiers.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A lightweight, copyable reference to an asset stored in [`Assets<T>`](crate::Assets).
///
/// Handles are cheap ids — cloning one never clones the asset. Data files always reference
/// assets by path; handles exist only at runtime.
pub struct Handle<T> {
    id: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) fn new(id: u32) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// The raw index of this handle within its `Assets<T>` storage.
    pub fn id(&self) -> u32 {
        self.id
    }
}

// Manual impls: derives would incorrectly bound `T`.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T> Eq for Handle<T> {}
impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", std::any::type_name::<T>(), self.id)
    }
}
