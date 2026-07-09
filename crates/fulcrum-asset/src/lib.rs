//! Fulcrum asset system: typed [`Handle<T>`] identifiers, [`Assets<T>`] storage, and the
//! [`AssetServer`] that reads asset bytes from disk.
//!
//! Loading is synchronous by design. Every disk read funnels through
//! [`AssetServer::read_bytes`] — the single seam behind which hot reload (phase 3) and the
//! layered mod VFS (phase 4) will slot in without touching individual loaders.

pub mod assets;
pub mod handle;
pub mod watch;

use std::path::PathBuf;

use bevy_ecs::prelude::Resource;

pub use assets::Assets;
pub use handle::Handle;
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

/// Resolves asset paths against the asset root directory and reads their bytes.
///
/// The default root is `assets/` relative to the working directory.
#[derive(Resource, Debug, Clone)]
pub struct AssetServer {
    root: PathBuf,
}

impl Default for AssetServer {
    fn default() -> Self {
        Self::new("assets")
    }
}

impl AssetServer {
    /// An asset server rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The asset root directory.
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Read the raw bytes of the asset at `path` (relative to the asset root).
    ///
    /// This is the engine's one disk-read seam: all loaders call it, and future layers (hot
    /// reload, mod VFS) replace what's behind it.
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        std::fs::read(self.root.join(path)).map_err(|source| AssetError::Io {
            path: path.to_string(),
            source,
        })
    }
}
