//! `mod.ron`: the file at every mod's root that names it and declares what it needs.
//!
//! ```ron
//! Mod(
//!     id: "more_slimes",              // [a-z0-9_], unique across loaded mods
//!     name: "More Slimes!",
//!     version: "0.1.0",
//!     engine_version: "0.1",          // warn on mismatch
//!     load_after: ["core_tweaks"],    // ordering constraints
//!     scripts: ["scripts/init.lua"],  // entry points, run in listed order
//! )
//! ```

use serde::Deserialize;

/// A parsed, validated `mod.ron`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename = "Mod")]
pub struct ModManifest {
    /// Stable identity: lowercase letters, digits, underscores.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Mod version (informational; recorded in replays).
    #[serde(default)]
    pub version: String,
    /// Engine version this mod targets; mismatches warn, never block.
    #[serde(default)]
    pub engine_version: String,
    /// Mods that must load before this one.
    #[serde(default)]
    pub load_after: Vec<String>,
    /// Lua entry points relative to the mod root, run in order.
    #[serde(default)]
    pub scripts: Vec<String>,
}

/// Manifest problems, phrased for mod authors.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The RON didn't parse.
    #[error("{path}: {message}")]
    Parse {
        /// The manifest path.
        path: String,
        /// Parser diagnostics.
        message: String,
    },
    /// The id contains characters outside `[a-z0-9_]`.
    #[error("{path}: mod id `{id}` may only contain lowercase letters, digits, and underscores")]
    BadId {
        /// The manifest path.
        path: String,
        /// The offending id.
        id: String,
    },
}

/// Parse and validate manifest text.
pub fn parse_manifest(path: &str, source: &str) -> Result<ModManifest, ManifestError> {
    let manifest: ModManifest = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .map_err(|error| ManifestError::Parse {
            path: path.to_string(),
            message: error.to_string(),
        })?;
    let id_ok = !manifest.id.is_empty()
        && manifest
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !id_ok {
        return Err(ManifestError::BadId {
            path: path.to_string(),
            id: manifest.id,
        });
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_manifest() {
        let manifest = parse_manifest(
            "mods/more_slimes/mod.ron",
            r#"Mod(
                id: "more_slimes",
                name: "More Slimes!",
                version: "0.1.0",
                engine_version: "0.1",
                load_after: ["core_tweaks"],
                scripts: ["scripts/init.lua"],
            )"#,
        )
        .unwrap();
        assert_eq!(manifest.id, "more_slimes");
        assert_eq!(manifest.load_after, vec!["core_tweaks"]);
        assert_eq!(manifest.scripts, vec!["scripts/init.lua"]);
    }

    #[test]
    fn minimal_manifest_defaults() {
        let manifest = parse_manifest("mod.ron", r#"Mod(id: "tiny", name: "Tiny")"#).unwrap();
        assert!(manifest.scripts.is_empty());
        assert!(manifest.load_after.is_empty());
    }

    #[test]
    fn bad_ids_are_rejected_with_a_clear_message() {
        let Err(error) = parse_manifest("mod.ron", r#"Mod(id: "Bad Id!", name: "x")"#) else {
            panic!("bad id accepted");
        };
        assert!(error.to_string().contains("Bad Id!"));
    }
}
