//! Asset hot reload: a filesystem watcher on the asset root, surfaced as [`AssetEvent`]s.
//!
//! Dev-tool only (enabled by `FulcrumConfig::hot_reload`, default debug builds). Subsystems
//! subscribe with an `EventReader<AssetEvent>` and reload whatever they own in place via
//! [`Assets::replace`](crate::Assets::replace), so existing handles stay valid.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use bevy_ecs::message::Message;
use bevy_ecs::prelude::Resource;
use notify::{RecursiveMode, Watcher};

/// A file under the asset root changed. `path` is asset-relative with forward slashes — the
/// same string games pass to loaders.
#[derive(Message, Clone, Debug)]
pub struct AssetEvent {
    /// Asset-relative path of the modified file.
    pub path: String,
}

/// Owns the watcher thread and the event channel.
#[derive(Resource)]
pub struct AssetWatcher {
    /// Kept alive for its Drop (stops watching).
    _watcher: notify::RecommendedWatcher,
    receiver: Mutex<Receiver<notify::Result<notify::Event>>>,
    roots: Vec<PathBuf>,
}

impl AssetWatcher {
    /// Watch one root recursively (convenience for [`start_all`](Self::start_all)).
    pub fn start(root: impl Into<PathBuf>) -> Option<Self> {
        Self::start_all(vec![root.into()])
    }

    /// Watch every root recursively (the base asset dir plus each mod mount). Returns `None`
    /// (with a log) if nothing can be watched — hot reload is best-effort, never fatal.
    pub fn start_all(roots: Vec<PathBuf>) -> Option<Self> {
        let (sender, receiver) = channel();
        let mut watcher = match notify::recommended_watcher(sender) {
            Ok(watcher) => watcher,
            Err(error) => {
                log::warn!("hot reload disabled: {error}");
                return None;
            }
        };
        let mut canonical_roots = Vec::new();
        for root in roots {
            let canonical = match root.canonicalize() {
                Ok(canonical) => canonical,
                Err(error) => {
                    log::warn!("hot reload: cannot canonicalize {root:?}: {error}");
                    continue;
                }
            };
            if let Err(error) = watcher.watch(&canonical, RecursiveMode::Recursive) {
                log::warn!("hot reload: cannot watch {canonical:?}: {error}");
                continue;
            }
            log::info!("hot reload watching {canonical:?}");
            canonical_roots.push(canonical);
        }
        if canonical_roots.is_empty() {
            return None;
        }
        Some(Self {
            _watcher: watcher,
            receiver: Mutex::new(receiver),
            roots: canonical_roots,
        })
    }

    /// Drain pending filesystem events into asset-relative paths (deduplicated per call).
    pub fn drain(&self) -> Vec<String> {
        let mut changed = Vec::new();
        let receiver = self.receiver.lock().unwrap();
        while let Ok(event) = receiver.try_recv() {
            let Ok(event) = event else { continue };
            if !matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
            ) {
                continue;
            }
            for path in event.paths {
                let Ok(canonical) = path.canonicalize() else {
                    continue;
                };
                for root in &self.roots {
                    if let Ok(relative) = canonical.strip_prefix(root) {
                        let relative = relative.to_string_lossy().replace('\\', "/");
                        if !changed.contains(&relative) {
                            changed.push(relative);
                        }
                        break;
                    }
                }
            }
        }
        changed
    }
}

/// Per-frame debouncer: suppresses repeat events for the same path within 100 ms (editors often
/// write files several times in quick succession).
#[derive(Default)]
pub struct Debounce(rustc_hash::FxHashMap<String, Instant>);

impl Debounce {
    /// Should this path's event fire now?
    pub fn allow(&mut self, path: &str) -> bool {
        let now = Instant::now();
        match self.0.get(path) {
            Some(last) if now.duration_since(*last) < Duration::from_millis(100) => false,
            _ => {
                self.0.insert(path.to_string(), now);
                true
            }
        }
    }
}
