//! Mod discovery, ordering, and lifecycle: point [`ModPlugin`] at your `mods/` directory and
//! everything inside becomes part of the game.
//!
//! Load order is deterministic: a topological sort over `load_after` constraints with ties
//! broken by mod id. The resolved order drives VFS mount order (later mods shadow earlier
//! ones and the base game), script execution order, and event dispatch order. Manifest
//! problems (duplicate ids, dependency cycles, missing entry scripts) fail at startup with a
//! message naming the offender — a broken mod set shouldn't half-load.
//!
//! Add `ModPlugin` **after** your game plugin: mod `on_tick` callbacks then run after the
//! game's systems each tick. Runtime enable/disable is out of scope — changing the mod set
//! means restarting.

use std::path::PathBuf;

use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use fulcrum_asset::AssetServer;
use fulcrum_core::{FixedUpdate, Fulcrum, Plugin, Startup};

use crate::manifest::{ModManifest, parse_manifest};
use crate::runtime::LuaRuntime;

/// One loaded mod, in the registry.
#[derive(Clone, Debug)]
pub struct LoadedMod {
    /// Stable id from the manifest.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Declared version.
    pub version: String,
    /// The mod's directory.
    pub root: PathBuf,
}

/// Every loaded mod, in load order (drives the inspector's Mods view and replay metadata).
#[derive(Resource, Default, Clone)]
pub struct ModRegistry {
    /// Mods in resolved load order.
    pub mods: Vec<LoadedMod>,
}

/// Discovers, orders, and runs mods. Opt-in — not part of `DefaultPlugins`.
pub struct ModPlugin {
    /// Directories scanned for mods (each immediate subdirectory containing a `mod.ron`).
    pub mod_dirs: Vec<PathBuf>,
}

impl Default for ModPlugin {
    fn default() -> Self {
        Self {
            mod_dirs: vec![PathBuf::from("mods")],
        }
    }
}

impl ModPlugin {
    /// Scan a single directory.
    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            mod_dirs: vec![dir.into()],
        }
    }
}

/// Discover manifests in `dirs` (sorted scan for determinism).
fn discover(dirs: &[PathBuf]) -> Vec<(ModManifest, PathBuf)> {
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue; // no mods dir is fine
        };
        let mut roots: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("mod.ron").is_file())
            .collect();
        roots.sort();
        for root in roots {
            let manifest_path = root.join("mod.ron");
            let source = std::fs::read_to_string(&manifest_path)
                .unwrap_or_else(|e| panic!("cannot read {manifest_path:?}: {e}"));
            let manifest = parse_manifest(&manifest_path.to_string_lossy(), &source)
                .unwrap_or_else(|e| panic!("mod manifest error: {e}"));
            found.push((manifest, root));
        }
    }
    found
}

/// Topological sort on `load_after`, ties broken by id. Panics on duplicates and cycles with
/// messages naming the mods involved.
fn resolve_order(mut mods: Vec<(ModManifest, PathBuf)>) -> Vec<(ModManifest, PathBuf)> {
    mods.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    for window in mods.windows(2) {
        if window[0].0.id == window[1].0.id {
            panic!(
                "duplicate mod id `{}` ({:?} and {:?})",
                window[0].0.id, window[0].1, window[1].1
            );
        }
    }
    let ids: Vec<String> = mods.iter().map(|(m, _)| m.id.clone()).collect();
    for (manifest, _) in &mods {
        for dep in &manifest.load_after {
            if !ids.contains(dep) {
                log::warn!(
                    "mod `{}` wants to load after `{dep}`, which isn't present",
                    manifest.id
                );
            }
        }
    }

    // Kahn's algorithm with an id-sorted ready list.
    let mut ordered = Vec::with_capacity(mods.len());
    let mut remaining = mods;
    while !remaining.is_empty() {
        let ready = remaining.iter().position(|(candidate, _)| {
            candidate
                .load_after
                .iter()
                .all(|dep| !remaining.iter().any(|(other, _)| other.id == *dep))
        });
        match ready {
            Some(index) => ordered.push(remaining.remove(index)),
            None => {
                let cycle: Vec<&str> = remaining.iter().map(|(m, _)| m.id.as_str()).collect();
                panic!("mod load_after cycle among: {}", cycle.join(", "));
            }
        }
    }
    ordered
}

impl Plugin for ModPlugin {
    fn build(&self, app: &mut Fulcrum) {
        let ordered = resolve_order(discover(&self.mod_dirs));
        let engine_version = env!("CARGO_PKG_VERSION");
        let seed = app.config().seed;

        if app.world().get_resource::<AssetServer>().is_none() {
            app.world_mut().insert_resource(AssetServer::default());
        }
        app.register_event::<crate::bindings::ModEvent>();

        let mut runtime = LuaRuntime::new(seed).expect("Lua runtime initializes");
        crate::bindings::install(&runtime).expect("Lua bindings install");

        let mut registry = ModRegistry::default();
        for (manifest, root) in &ordered {
            if !manifest.engine_version.is_empty()
                && !engine_version.starts_with(&manifest.engine_version)
            {
                log::warn!(
                    "mod `{}` targets engine {}, this is {engine_version}",
                    manifest.id,
                    manifest.engine_version
                );
            }
            // Data: mount on top of the stack (load order = shadow order).
            app.world_mut()
                .resource_mut::<AssetServer>()
                .mount(manifest.id.clone(), root.clone());
            // Scripts: register + run entries now (registration only; world access starts at
            // the on_init stage).
            runtime.register_mod(&manifest.id, root);
            for script in &manifest.scripts {
                if !root.join(script).is_file() {
                    panic!(
                        "mod `{}`: entry script `{script}` does not exist",
                        manifest.id
                    );
                }
                runtime
                    .run_entry(&manifest.id, script)
                    .unwrap_or_else(|e| panic!("{e}"));
            }
            registry.mods.push(LoadedMod {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                root: root.clone(),
            });
            log::info!("loaded mod `{}` from {root:?}", manifest.id);
        }

        // Replays record the loaded mod set and warn on playback if it differs.
        app.world_mut().insert_resource(fulcrum_core::ReplayModSet(
            registry
                .mods
                .iter()
                .map(|m| (m.id.clone(), m.version.clone()))
                .collect(),
        ));
        app.world_mut().insert_resource(registry);
        app.world_mut().insert_resource(runtime);
        app.add_systems(Startup, lua_init_stage);
        app.add_systems(FixedUpdate, lua_tick_stage);
    }
}

/// Exclusive startup stage: every mod's `on_init`, with world access.
fn lua_init_stage(world: &mut World) {
    world.resource_scope(|world, mut runtime: bevy_ecs::world::Mut<LuaRuntime>| {
        crate::bindings::run_init_with_world(&mut runtime, world);
    });
}

/// Exclusive tick stage: every mod's `on_tick`, with world access.
fn lua_tick_stage(world: &mut World) {
    let has_work = world.resource::<LuaRuntime>().has_tick_work();
    if !has_work {
        return;
    }
    world.resource_scope(|world, mut runtime: bevy_ecs::world::Mut<LuaRuntime>| {
        crate::bindings::run_tick_with_world(&mut runtime, world);
    });
}
