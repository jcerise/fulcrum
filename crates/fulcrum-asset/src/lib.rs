//! Fulcrum asset system: typed [`Handle<T>`] identifiers, [`Assets<T>`] storage, and the
//! [`AssetServer`] that reads asset bytes from disk.
//!
//! Loading is synchronous by design. Every disk read funnels through
//! [`AssetServer::read_bytes`] — the single seam behind which hot reload (phase 3) and the
//! layered mod VFS (phase 4) will slot in without touching individual loaders.

pub mod assets;
pub mod handle;
pub mod vfs;
pub mod watch;

use std::path::PathBuf;

use bevy_ecs::prelude::Resource;

pub use assets::Assets;
pub use handle::Handle;
pub use vfs::Vfs;
pub use watch::{AssetEvent, AssetWatcher, Debounce};

/// Errors produced while loading assets. Loaders log these and fall back to placeholders rather
/// than panicking.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// The file could not be read.
    #[error("failed to read asset `{path}`: {source}")]
    Io {
        /// Asset path as requested (relative to the asset root).
        path: String,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// The file was read but could not be decoded.
    #[error("failed to decode asset `{path}`: {message}")]
    Decode {
        /// Asset path as requested (relative to the asset root).
        path: String,
        /// Decoder error description.
        message: String,
    },
}

/// Resolves asset paths through the layered [`Vfs`] and reads their bytes.
///
/// The base game's asset root is the bottom mount (default `assets/`, relative to the working
/// directory); mods mount on top and shadow it. Every loader in the engine reads through
/// [`read_bytes`](Self::read_bytes), so mounting applies everywhere at once.
#[derive(Resource)]
pub struct AssetServer {
    vfs: Vfs,
}

impl Default for AssetServer {
    fn default() -> Self {
        Self::new("assets")
    }
}

impl AssetServer {
    /// An asset server whose base mount is `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut vfs = Vfs::default();
        vfs.mount("base", root);
        Self { vfs }
    }

    /// Mount a directory on top of the stack (mods; later mounts shadow earlier ones).
    pub fn mount(&mut self, name: impl Into<String>, root: impl Into<PathBuf>) {
        self.vfs.mount(name, root);
    }

    /// Remove a mount by name.
    pub fn unmount(&mut self, name: &str) {
        self.vfs.unmount(name);
    }

    /// All mount roots, bottom-up (the hot-reload watcher watches every one).
    pub fn roots(&self) -> Vec<PathBuf> {
        self.vfs.roots()
    }

    /// Which mount provides `path` right now (debug/inspector aid).
    pub fn source_of(&self, path: &str) -> Option<&str> {
        self.vfs.source_of(path)
    }

    /// Sorted union of `dir/*.ext` across all mounts (shadowed duplicates removed) — the
    /// deterministic way to discover data-driven content like `units/*.unit.ron`.
    pub fn list(&self, dir: &str, ext: &str) -> Vec<String> {
        self.vfs.list(dir, ext)
    }

    /// Read the raw bytes of the asset at `path`, from the topmost mount that has it.
    ///
    /// This is the engine's one disk-read seam: every loader calls it.
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        self.vfs.read(path)
    }
}
