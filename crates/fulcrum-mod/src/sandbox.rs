//! The Lua sandbox: capability omission plus deterministic replacements.
//!
//! Removed outright: `io`, `os` (a stub keeps `os.clock` on sim time), `package`/`require`
//! (replaced by a per-mod loader), `dofile`, `load`/`loadstring`, `collectgarbage` (stubbed).
//! Replaced: `print` logs with the mod's id; `math.random`/`math.randomseed` draw from a
//! per-mod fork of the simulation RNG, so scripts are deterministic and one mod's rolls never
//! depend on another mod's presence.
//!
//! This is an **anti-footgun sandbox, not a security boundary**: it stops mods from
//! accidentally breaking determinism or touching the machine, but a hostile mod is still
//! hostile code in-process.

use fulcrum_core::SimRng;
use mlua::{Lua, Value, Variadic};

use crate::runtime::LuaCtx;

/// Deterministic per-mod RNG seed: independent of load order.
pub(crate) fn mod_seed(base_seed: u64, mod_id: &str) -> u64 {
    // FNV-1a over the id, mixed with the app seed.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in mod_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^ base_seed
}

fn with_current_rng<R>(lua: &Lua, f: impl FnOnce(&mut SimRng) -> R) -> mlua::Result<R> {
    let mut ctx = lua
        .app_data_mut::<LuaCtx>()
        .ok_or_else(|| mlua::Error::runtime("runtime context missing"))?;
    let ctx = &mut *ctx;
    let mod_id = ctx.current_mod.clone();
    let base_seed = ctx.base_seed;
    let rng = ctx
        .rngs
        .entry(mod_id.clone())
        .or_insert_with(|| SimRng::seeded(mod_seed(base_seed, &mod_id)));
    Ok(f(rng))
}

/// Strip and replace the global environment. Called once at runtime creation.
pub(crate) fn apply(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();

    // Capabilities removed.
    for name in ["io", "package", "dofile", "load", "loadstring", "require"] {
        globals.set(name, Value::Nil)?;
    }

    // os: only a sim-time clock survives.
    let os_stub = lua.create_table()?;
    os_stub.set(
        "clock",
        lua.create_function(|lua, ()| {
            let ctx = lua
                .app_data_ref::<LuaCtx>()
                .ok_or_else(|| mlua::Error::runtime("runtime context missing"))?;
            Ok(ctx.sim_time)
        })?,
    )?;
    globals.set("os", os_stub)?;

    // collectgarbage: harmless stub (mods have no business managing the GC).
    globals.set(
        "collectgarbage",
        lua.create_function(|_, _: Variadic<Value>| Ok(0))?,
    )?;

    // print -> engine log, prefixed with the mod id.
    globals.set(
        "print",
        lua.create_function(|lua, args: Variadic<Value>| {
            let parts: Vec<String> = args
                .iter()
                .map(|v| v.to_string().unwrap_or_else(|_| "?".into()))
                .collect();
            let who = lua
                .app_data_ref::<LuaCtx>()
                .map(|ctx| ctx.current_mod.clone())
                .unwrap_or_default();
            log::info!("[mod:{who}] {}", parts.join("\t"));
            Ok(())
        })?,
    )?;

    // math.random / math.randomseed -> the per-mod deterministic stream.
    let math: mlua::Table = globals.get("math")?;
    math.set(
        "random",
        lua.create_function(|lua, args: Variadic<i64>| {
            with_current_rng(lua, |rng| match args.len() {
                0 => Ok(Value::Number(rng.unit_f32() as f64)),
                1 => {
                    let m = args[0];
                    if m < 1 {
                        return Err(mlua::Error::runtime("math.random(m): m must be >= 1"));
                    }
                    Ok(Value::Integer(1 + rng.range_i32(0..m as i32) as i64))
                }
                _ => {
                    let (m, n) = (args[0], args[1]);
                    if n < m {
                        return Err(mlua::Error::runtime("math.random(m, n): n must be >= m"));
                    }
                    Ok(Value::Integer(
                        m + rng.range_i32(0..(n - m + 1) as i32) as i64,
                    ))
                }
            })?
        })?,
    )?;
    math.set(
        "randomseed",
        lua.create_function(|lua, _: Variadic<Value>| {
            let who = lua
                .app_data_ref::<LuaCtx>()
                .map(|ctx| ctx.current_mod.clone())
                .unwrap_or_default();
            log::warn!("[mod:{who}] math.randomseed is a no-op: streams are engine-seeded");
            Ok(())
        })?,
    )?;

    Ok(())
}
