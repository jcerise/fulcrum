//! The layered virtual filesystem: ordered mounts where later mounts shadow earlier ones.
//!
//! The base game's `assets/` is mount 0; each mod mounts on top. Every asset read in the
//! engine goes through [`AssetServer::read_bytes`](crate::AssetServer), which resolves through
//! this stack — so a mod overriding `sprites/slime.png` just works, for every loader, with no
//! per-loader code.

use std::path::PathBuf;

use crate::AssetError;

struct Mount {
    name: String,
    root: PathBuf,
}

/// An ordered stack of directory mounts. Reads search top-down (last mounted wins).
#[derive(Default)]
pub struct Vfs {
    mounts: Vec<Mount>,
}

impl Vfs {
    /// Push a mount on top of the stack; its files shadow all earlier mounts.
    pub fn mount(&mut self, name: impl Into<String>, root: impl Into<PathBuf>) {
        self.mounts.push(Mount {
            name: name.into(),
            root: root.into(),
        });
    }

    /// Remove a mount by name (restoring whatever it shadowed).
    pub fn unmount(&mut self, name: &str) {
        self.mounts.retain(|mount| mount.name != name);
    }

    /// The mount roots, bottom-up (for the hot-reload watcher).
    pub fn roots(&self) -> Vec<PathBuf> {
        self.mounts.iter().map(|m| m.root.clone()).collect()
    }

    /// Read `path` from the topmost mount that has it.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        let mut last_error: Option<std::io::Error> = None;
        for mount in self.mounts.iter().rev() {
            match std::fs::read(mount.root.join(path)) {
                Ok(bytes) => return Ok(bytes),
                Err(error) => last_error = Some(error),
            }
        }
        Err(AssetError::Io {
            path: path.to_string(),
            source: last_error
                .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no mounts")),
        })
    }

    /// Which mount currently provides `path` (debug/inspector aid).
    pub fn source_of(&self, path: &str) -> Option<&str> {
        self.mounts
            .iter()
            .rev()
            .find(|mount| mount.root.join(path).is_file())
            .map(|mount| mount.name.as_str())
    }

    /// Union of files under `dir` (asset-relative) with extension `ext`, across all mounts:
    /// shadowed duplicates removed, **sorted** — deterministic, so data-driven discovery
    /// (e.g. "load every `units/*.unit.ron`") is replay-safe.
    pub fn list(&self, dir: &str, ext: &str) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for mount in &self.mounts {
            let Ok(entries) = std::fs::read_dir(mount.root.join(dir)) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().and_then(|e| e.to_str()) == Some(ext)
                    && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                {
                    let relative = if dir.is_empty() {
                        file_name.to_string()
                    } else {
                        format!("{dir}/{file_name}")
                    };
                    if !found.contains(&relative) {
                        found.push(relative);
                    }
                }
            }
        }
        found.sort_unstable();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fulcrum-vfs-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sprites")).unwrap();
        dir
    }

    #[test]
    fn later_mounts_shadow_and_unmount_restores() {
        let base = temp_root("base");
        let modded = temp_root("mod");
        std::fs::write(base.join("sprites/slime.png"), b"base").unwrap();
        std::fs::write(modded.join("sprites/slime.png"), b"modded").unwrap();

        let mut vfs = Vfs::default();
        vfs.mount("base", &base);
        assert_eq!(vfs.read("sprites/slime.png").unwrap(), b"base");
        assert_eq!(vfs.source_of("sprites/slime.png"), Some("base"));

        vfs.mount("more_slimes", &modded);
        assert_eq!(vfs.read("sprites/slime.png").unwrap(), b"modded");
        assert_eq!(vfs.source_of("sprites/slime.png"), Some("more_slimes"));

        vfs.unmount("more_slimes");
        assert_eq!(vfs.read("sprites/slime.png").unwrap(), b"base");
    }

    #[test]
    fn list_unions_dedups_and_sorts() {
        let base = temp_root("list-base");
        let modded = temp_root("list-mod");
        std::fs::create_dir_all(base.join("units")).unwrap();
        std::fs::create_dir_all(modded.join("units")).unwrap();
        std::fs::write(base.join("units/soldier.ron"), b"s").unwrap();
        std::fs::write(base.join("units/worker.ron"), b"w").unwrap();
        std::fs::write(base.join("units/readme.txt"), b"x").unwrap();
        std::fs::write(modded.join("units/soldier.ron"), b"override").unwrap();
        std::fs::write(modded.join("units/zealot.ron"), b"z").unwrap();

        let mut vfs = Vfs::default();
        vfs.mount("base", &base);
        vfs.mount("mod", &modded);
        assert_eq!(
            vfs.list("units", "ron"),
            vec!["units/soldier.ron", "units/worker.ron", "units/zealot.ron"]
        );
        assert_eq!(vfs.read("units/soldier.ron").unwrap(), b"override");
    }

    #[test]
    fn missing_file_reports_the_asset_path() {
        let mut vfs = Vfs::default();
        vfs.mount("base", temp_root("missing"));
        let Err(error) = vfs.read("nope.png") else {
            panic!("read succeeded?")
        };
        assert!(error.to_string().contains("nope.png"));
    }
}
