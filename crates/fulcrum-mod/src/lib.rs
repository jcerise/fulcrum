//! Fulcrum modding: mod manifests (this step), the sandboxed deterministic Lua runtime, mod
//! loading, and the Lua↔ECS bindings (subsequent steps).

pub mod manifest;

pub use manifest::{ManifestError, ModManifest, parse_manifest};
