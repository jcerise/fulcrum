//! The Lua runtime: one shared, sandboxed interpreter running every mod's scripts on the
//! simulation tick, deterministically.
//!
//! Mods register callbacks instead of owning a loop:
//!
//! ```lua
//! fulcrum.on_init(function() ... end)                 -- once, after world setup
//! fulcrum.on_tick(function(tick) ... end)             -- every simulation tick
//! fulcrum.on_event("unit_died", function(e) ... end)  -- sim events (step 3 wires payloads)
//! ```
//!
//! Callbacks run in **mod load order**, budgeted by an instruction-count hook (a runaway
//! `while true do end` aborts with an error naming the mod; the game keeps running). A
//! callback that errors three times in a row is disabled and surfaced in the log.

use std::path::PathBuf;
use std::sync::Mutex;

use bevy_ecs::prelude::Resource;
use fulcrum_core::{FxHashMap, SimRng};
use mlua::{Function, HookTriggers, Lua, RegistryKey, VmState};

use crate::sandbox;

/// Instructions a single callback may execute per invocation before being aborted.
const INSTRUCTION_BUDGET: u64 = 50_000_000;
/// Hook granularity (budget is checked every this-many instructions).
const HOOK_EVERY: u32 = 100_000;
/// Consecutive errors before a callback is disabled.
const MAX_ERROR_STREAK: u32 = 3;

/// Shared state Lua-side closures reach through `Lua::app_data`.
pub(crate) struct LuaCtx {
    /// The mod whose code is currently executing (drives logs, RNG, and require).
    pub current_mod: String,
    /// Per-mod deterministic RNG streams (seeded by `(app seed, mod id)`).
    pub rngs: FxHashMap<String, SimRng>,
    /// The app's base seed.
    pub base_seed: u64,
    /// Instructions consumed by the current callback (reset per invocation).
    pub instructions: u64,
    /// `os.clock` value: sim ticks converted to seconds.
    pub sim_time: f64,
    /// Mod id -> mod root directory (for `require`).
    pub mod_roots: FxHashMap<String, PathBuf>,
    /// `require` cycle detection: modules currently mid-load.
    pub loading: Vec<String>,
    /// Callback registrations made while evaluating a script (drained after each entry runs).
    pub pending: Vec<(String, Registration)>,
    /// Events emitted via `fulcrum.emit` during the current batch (dispatched afterwards).
    pub emitted: Vec<(String, ron::Value)>,
}

/// A callback a script registered.
pub(crate) enum Registration {
    Init(RegistryKey),
    Tick(RegistryKey),
    Event(String, RegistryKey),
}

struct Callback {
    key: RegistryKey,
    error_streak: u32,
    disabled: bool,
}

impl Callback {
    fn new(key: RegistryKey) -> Self {
        Self {
            key,
            error_streak: 0,
            disabled: false,
        }
    }
}

#[derive(Default)]
struct ModCallbacks {
    on_init: Vec<Callback>,
    on_tick: Vec<Callback>,
    on_event: FxHashMap<String, Vec<Callback>>,
}

/// The engine's Lua interpreter plus every registered mod's callbacks, in load order.
#[derive(Resource)]
pub struct LuaRuntime {
    lua: Mutex<Lua>,
    mods: Vec<(String, ModCallbacks)>,
}

impl LuaRuntime {
    /// Build the sandboxed interpreter.
    pub fn new(base_seed: u64) -> Result<Self, mlua::Error> {
        let lua = Lua::new();
        lua.set_app_data(LuaCtx {
            current_mod: String::new(),
            rngs: FxHashMap::default(),
            base_seed,
            instructions: 0,
            sim_time: 0.0,
            mod_roots: FxHashMap::default(),
            loading: Vec::new(),
            pending: Vec::new(),
            emitted: Vec::new(),
        });
        sandbox::apply(&lua)?;
        install_fulcrum_table(&lua)?;
        install_require(&lua)?;
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(HOOK_EVERY),
            |lua, _debug| {
                let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() else {
                    return Ok(VmState::Continue);
                };
                ctx.instructions += HOOK_EVERY as u64;
                if ctx.instructions > INSTRUCTION_BUDGET {
                    let who = ctx.current_mod.clone();
                    drop(ctx);
                    return Err(mlua::Error::runtime(format!(
                        "mod `{who}` exceeded its per-tick instruction budget"
                    )));
                }
                Ok(VmState::Continue)
            },
        )?;
        Ok(Self {
            lua: Mutex::new(lua),
            mods: Vec::new(),
        })
    }

    /// Register a mod (load order = call order). `root` is the mod's directory.
    pub fn register_mod(&mut self, id: impl Into<String>, root: impl Into<PathBuf>) {
        let id = id.into();
        let lua = self.lua.lock().unwrap();
        if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
            ctx.mod_roots.insert(id.clone(), root.into());
        }
        drop(lua);
        self.mods.push((id, ModCallbacks::default()));
    }

    /// Run one of a mod's entry scripts (path relative to the mod root). Registrations the
    /// script makes are collected into the mod's callback lists.
    pub fn run_entry(&mut self, mod_id: &str, script: &str) -> Result<(), String> {
        let lua = self.lua.lock().unwrap();
        let source = {
            let ctx = lua.app_data_ref::<LuaCtx>().expect("ctx");
            let Some(root) = ctx.mod_roots.get(mod_id) else {
                return Err(format!("unknown mod `{mod_id}`"));
            };
            std::fs::read_to_string(root.join(script))
                .map_err(|e| format!("mod `{mod_id}`: cannot read `{script}`: {e}"))?
        };
        set_current(&lua, mod_id);
        reset_budget(&lua);
        let result = lua
            .load(&source)
            .set_name(format!("@{mod_id}/{script}"))
            .exec()
            .map_err(|e| format!("mod `{mod_id}` `{script}`: {e}"));
        let pending = {
            let mut ctx = lua.app_data_mut::<LuaCtx>().expect("ctx");
            std::mem::take(&mut ctx.pending)
        };
        drop(lua);
        for (owner, registration) in pending {
            let Some((_, callbacks)) = self.mods.iter_mut().find(|(id, _)| *id == owner) else {
                continue;
            };
            match registration {
                Registration::Init(key) => callbacks.on_init.push(Callback::new(key)),
                Registration::Tick(key) => callbacks.on_tick.push(Callback::new(key)),
                Registration::Event(name, key) => {
                    callbacks
                        .on_event
                        .entry(name)
                        .or_default()
                        .push(Callback::new(key));
                }
            }
        }
        result
    }

    /// Invoke every `on_init` callback, in load order.
    pub fn run_init(&mut self) {
        self.invoke_all(|callbacks| &mut callbacks.on_init, ());
    }

    /// Invoke every `on_tick` callback, in load order, with the current tick number.
    pub fn run_tick(&mut self, tick: u64, tick_rate: u32) {
        {
            let lua = self.lua.lock().unwrap();
            if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
                ctx.sim_time = tick as f64 / tick_rate as f64;
            }
        }
        self.invoke_all(|callbacks| &mut callbacks.on_tick, tick);
    }

    /// Invoke every handler registered for a named event, in load order.
    pub fn dispatch_event(&mut self, name: &str, payload: mlua::Value) {
        let lua = self.lua.lock().unwrap();
        for (mod_id, callbacks) in &mut self.mods {
            let Some(handlers) = callbacks.on_event.get_mut(name) else {
                continue;
            };
            for callback in handlers.iter_mut() {
                invoke_one(&lua, mod_id, callback, payload.clone());
            }
        }
    }

    /// Are there any registered tick callbacks (cheap skip for modless games)?
    pub fn has_tick_work(&self) -> bool {
        self.mods.iter().any(|(_, c)| !c.on_tick.is_empty())
    }

    /// Evaluate an expression and stringify the result (tests and the inspector).
    pub fn eval_string(&self, code: &str) -> Result<String, mlua::Error> {
        let lua = self.lua.lock().unwrap();
        let value: mlua::Value = lua.load(code).eval()?;
        value.to_string()
    }

    /// Run `f` with the interpreter locked (used by the bindings layer).
    pub(crate) fn with_lua<R>(&self, f: impl FnOnce(&Lua) -> R) -> R {
        let lua = self.lua.lock().unwrap();
        f(&lua)
    }

    fn invoke_all<A: mlua::IntoLuaMulti + Clone>(
        &mut self,
        select: impl Fn(&mut ModCallbacks) -> &mut Vec<Callback>,
        args: A,
    ) {
        let lua = self.lua.lock().unwrap();
        for (mod_id, callbacks) in &mut self.mods {
            for callback in select(callbacks).iter_mut() {
                invoke_one(&lua, mod_id, callback, args.clone());
            }
        }
    }
}

fn invoke_one<A: mlua::IntoLuaMulti>(lua: &Lua, mod_id: &str, callback: &mut Callback, args: A) {
    if callback.disabled {
        return;
    }
    set_current(lua, mod_id);
    reset_budget(lua);
    let Ok(function): Result<Function, _> = lua.registry_value(&callback.key) else {
        callback.disabled = true;
        return;
    };
    match function.call::<()>(args) {
        Ok(()) => callback.error_streak = 0,
        Err(error) => {
            callback.error_streak += 1;
            log::error!(
                "[mod:{mod_id}] callback error ({} in a row): {error}",
                callback.error_streak
            );
            if callback.error_streak >= MAX_ERROR_STREAK {
                callback.disabled = true;
                log::error!("[mod:{mod_id}] callback disabled after repeated errors");
            }
        }
    }
}

fn set_current(lua: &Lua, mod_id: &str) {
    if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
        ctx.current_mod = mod_id.to_string();
    }
}

fn reset_budget(lua: &Lua) {
    if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
        ctx.instructions = 0;
    }
}

/// The `fulcrum` global: registration + logging (the ECS bindings extend this in step 3).
fn install_fulcrum_table(lua: &Lua) -> mlua::Result<()> {
    let fulcrum = lua.create_table()?;

    let register = |kind: fn(RegistryKey) -> Registration| {
        move |lua: &Lua, function: Function| {
            let key = lua.create_registry_value(function)?;
            let mut ctx = lua
                .app_data_mut::<LuaCtx>()
                .ok_or_else(|| mlua::Error::runtime("runtime context missing"))?;
            let owner = ctx.current_mod.clone();
            ctx.pending.push((owner, kind(key)));
            Ok(())
        }
    };
    fulcrum.set(
        "on_init",
        lua.create_function(register(Registration::Init))?,
    )?;
    fulcrum.set(
        "on_tick",
        lua.create_function(register(Registration::Tick))?,
    )?;
    fulcrum.set(
        "on_event",
        lua.create_function(|lua: &Lua, (name, function): (String, Function)| {
            let key = lua.create_registry_value(function)?;
            let mut ctx = lua
                .app_data_mut::<LuaCtx>()
                .ok_or_else(|| mlua::Error::runtime("runtime context missing"))?;
            let owner = ctx.current_mod.clone();
            ctx.pending.push((owner, Registration::Event(name, key)));
            Ok(())
        })?,
    )?;
    fulcrum.set(
        "log",
        lua.create_function(|lua, message: String| {
            let who = lua
                .app_data_ref::<LuaCtx>()
                .map(|ctx| ctx.current_mod.clone())
                .unwrap_or_default();
            log::info!("[mod:{who}] {message}");
            Ok(())
        })?,
    )?;

    lua.globals().set("fulcrum", fulcrum)
}

/// `require("module")`: resolves `<mod root>/scripts/<module>.lua` within the calling mod,
/// with caching and cycle detection.
fn install_require(lua: &Lua) -> mlua::Result<()> {
    let modules = lua.create_table()?; // cache: "mod:module" -> value
    lua.set_named_registry_value("fulcrum_modules", modules)?;

    lua.globals().set(
        "require",
        lua.create_function(|lua, name: String| {
            let (who, root) = {
                let ctx = lua
                    .app_data_ref::<LuaCtx>()
                    .ok_or_else(|| mlua::Error::runtime("runtime context missing"))?;
                let who = ctx.current_mod.clone();
                let root = ctx.mod_roots.get(&who).cloned().ok_or_else(|| {
                    mlua::Error::runtime(format!("require(\"{name}\"): unknown mod `{who}`"))
                })?;
                (who, root)
            };
            let cache_key = format!("{who}:{name}");
            let modules: mlua::Table = lua.named_registry_value("fulcrum_modules")?;
            if let Ok(cached) = modules.get::<mlua::Value>(cache_key.as_str())
                && !cached.is_nil()
            {
                return Ok(cached);
            }

            {
                let mut ctx = lua.app_data_mut::<LuaCtx>().expect("ctx");
                if ctx.loading.contains(&cache_key) {
                    let chain = ctx.loading.join(" -> ");
                    return Err(mlua::Error::runtime(format!(
                        "require cycle: {chain} -> {cache_key}"
                    )));
                }
                ctx.loading.push(cache_key.clone());
            }

            let path = root.join("scripts").join(format!("{name}.lua"));
            let result = (|| {
                let source = std::fs::read_to_string(&path).map_err(|e| {
                    mlua::Error::runtime(format!(
                        "require(\"{name}\") in mod `{who}`: cannot read {path:?}: {e}"
                    ))
                })?;
                let value: mlua::Value = lua
                    .load(&source)
                    .set_name(format!("@{who}/scripts/{name}.lua"))
                    .eval()?;
                let stored = if value.is_nil() {
                    mlua::Value::Boolean(true)
                } else {
                    value
                };
                modules.set(cache_key.as_str(), stored.clone())?;
                Ok(stored)
            })();

            if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
                ctx.loading.pop();
            }
            result
        })?,
    )
}
