//! The Lua ↔ ECS bindings: the modder-facing world API, documented in `lua_api.md`.
//!
//! Everything flows through the phase-3 [`ComponentRegistry`] — components cross the boundary
//! as Lua tables ↔ RON values ↔ typed components, with zero per-component glue. World access
//! happens only while the exclusive Lua tick stage runs (a scoped world pointer guards it);
//! mutations apply immediately in call order, which is deterministic because the whole stage
//! is single-threaded inside `FixedUpdate`.

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use fulcrum_asset::{AssetServer, Assets};
use fulcrum_core::{Input, Key, Time};
use fulcrum_scene::{ComponentRegistry, PrefabAsset, prefab};
use mlua::{Lua, Value as LuaValue, Variadic};

use crate::runtime::{LuaCtx, LuaRuntime};

/// A simulation event emitted by a mod (or by game code for mods): named, with a data payload.
/// Rust systems read these with `EventReader<ModEvent>`; other mods receive them via
/// `fulcrum.on_event(name, fn)`.
#[derive(bevy_ecs::prelude::Message, Clone, Debug)]
pub struct ModEvent {
    /// Event name (by convention `snake_case`).
    pub name: String,
    /// Structured payload.
    pub payload: ron::Value,
}

/// Scoped world pointer: set only while the Lua tick stage runs.
struct WorldPtr(*mut World);
unsafe impl Send for WorldPtr {}

fn with_world<R>(lua: &Lua, f: impl FnOnce(&mut World) -> mlua::Result<R>) -> mlua::Result<R> {
    let pointer = lua
        .app_data_ref::<WorldPtr>()
        .map(|p| p.0)
        .unwrap_or(std::ptr::null_mut());
    if pointer.is_null() {
        return Err(mlua::Error::runtime(
            "world access is only available inside on_init/on_tick/on_event callbacks",
        ));
    }
    // SAFETY: the pointer is installed by `run_*_with_world`, which holds `&mut World`
    // exclusively for the duration and clears it before returning. Lua execution is
    // single-threaded within that scope.
    let world = unsafe { &mut *pointer };
    f(world)
}

// --- value conversion -------------------------------------------------------------------

fn ron_to_lua(lua: &Lua, value: &ron::Value) -> mlua::Result<LuaValue> {
    use ron::value::Number;
    Ok(match value {
        ron::Value::Bool(b) => LuaValue::Boolean(*b),
        ron::Value::Char(c) => LuaValue::String(lua.create_string(c.to_string())?),
        ron::Value::String(s) => LuaValue::String(lua.create_string(s)?),
        ron::Value::Bytes(b) => LuaValue::String(lua.create_string(b)?),
        ron::Value::Unit => LuaValue::Boolean(true), // markers read as `true`
        ron::Value::Option(Some(inner)) => ron_to_lua(lua, inner)?,
        ron::Value::Option(None) => LuaValue::Nil,
        ron::Value::Number(number) => match number {
            Number::I8(v) => LuaValue::Integer(*v as i64),
            Number::I16(v) => LuaValue::Integer(*v as i64),
            Number::I32(v) => LuaValue::Integer(*v as i64),
            Number::I64(v) => LuaValue::Integer(*v),
            Number::U8(v) => LuaValue::Integer(*v as i64),
            Number::U16(v) => LuaValue::Integer(*v as i64),
            Number::U32(v) => LuaValue::Integer(*v as i64),
            Number::U64(v) => LuaValue::Integer(*v as i64),
            other => LuaValue::Number(other.into_f64()),
        },
        ron::Value::Seq(items) => {
            let table = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                table.set(i + 1, ron_to_lua(lua, item)?)?;
            }
            LuaValue::Table(table)
        }
        ron::Value::Map(map) => {
            let table = lua.create_table()?;
            for (key, entry) in map.iter() {
                let key = match key {
                    ron::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                table.set(key, ron_to_lua(lua, entry)?)?;
            }
            LuaValue::Table(table)
        }
    })
}

fn lua_to_ron(value: &LuaValue) -> mlua::Result<ron::Value> {
    Ok(match value {
        LuaValue::Nil => ron::Value::Unit,
        LuaValue::Boolean(b) => ron::Value::Bool(*b),
        LuaValue::Integer(i) => ron::Value::Number(ron::value::Number::new(*i)),
        LuaValue::Number(n) => ron::Value::Number(ron::value::Number::new(*n)),
        LuaValue::String(s) => ron::Value::String(s.to_str()?.to_string()),
        LuaValue::Table(table) => {
            let length = table.raw_len();
            if length > 0 {
                // Array-like -> Seq.
                let mut items = Vec::with_capacity(length);
                for i in 1..=length {
                    items.push(lua_to_ron(&table.get::<LuaValue>(i)?)?);
                }
                ron::Value::Seq(items)
            } else {
                let mut map = ron::value::Map::new();
                for pair in table.pairs::<String, LuaValue>() {
                    let (key, entry) = pair?;
                    map.insert(ron::Value::String(key), lua_to_ron(&entry)?);
                }
                ron::Value::Map(map)
            }
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "cannot convert {} to component data",
                other.type_name()
            )));
        }
    })
}

fn entity_from(id: i64) -> Entity {
    Entity::from_bits(id as u64)
}

fn parse_key(name: &str) -> Option<Key> {
    use Key::*;
    Some(match name {
        "A" => A,
        "B" => B,
        "C" => C,
        "D" => D,
        "E" => E,
        "F" => F,
        "G" => G,
        "H" => H,
        "I" => I,
        "J" => J,
        "K" => K,
        "L" => L,
        "M" => M,
        "N" => N,
        "O" => O,
        "P" => P,
        "Q" => Q,
        "R" => R,
        "S" => S,
        "T" => T,
        "U" => U,
        "V" => V,
        "W" => W,
        "X" => X,
        "Y" => Y,
        "Z" => Z,
        "Space" => Space,
        "Enter" => Enter,
        "Escape" => Escape,
        "Tab" => Tab,
        "Shift" => Shift,
        "Ctrl" => Ctrl,
        "Alt" => Alt,
        "Up" => Up,
        "Down" => Down,
        "Left" => Left,
        "Right" => Right,
        _ => return None,
    })
}

// --- the API ----------------------------------------------------------------------------

/// Install the world-facing functions onto the `fulcrum` table. Called once by `ModPlugin`.
pub fn install(runtime: &LuaRuntime) -> mlua::Result<()> {
    runtime.with_lua(|lua| -> mlua::Result<()> {
        let fulcrum: mlua::Table = lua.globals().get("fulcrum")?;

        fulcrum.set(
            "tick",
            lua.create_function(|lua, ()| {
                with_world(lua, |world| Ok(world.resource::<Time>().tick))
            })?,
        )?;

        fulcrum.set(
            "spawn_prefab",
            lua.create_function(|lua, (path, at): (String, Option<mlua::Table>)| {
                with_world(lua, |world| {
                    let handle = prefab_by_path(world, &path).map_err(mlua::Error::runtime)?;
                    let entity = world.spawn_empty().id();
                    let position = at.map(|t| {
                        fulcrum_core::vec2(t.get("x").unwrap_or(0.0), t.get("y").unwrap_or(0.0))
                    });
                    prefab::apply_spawn_now(world, entity, handle, position);
                    Ok(entity.to_bits() as i64)
                })
            })?,
        )?;

        fulcrum.set(
            "despawn",
            lua.create_function(|lua, id: i64| {
                with_world(lua, |world| {
                    let _ = world.try_despawn(entity_from(id));
                    Ok(())
                })
            })?,
        )?;

        fulcrum.set(
            "get",
            lua.create_function(|lua, (id, component): (i64, String)| {
                with_world(lua, |world| {
                    let entity = entity_from(id);
                    let Ok(entity_ref) = world.get_entity(entity) else {
                        return Ok(LuaValue::Nil); // stale entity -> nil, never an error
                    };
                    let registry = world.resource::<ComponentRegistry>();
                    match registry.extract_from(&entity_ref, &component) {
                        Some(value) => ron_to_lua(lua, &value),
                        None => Ok(LuaValue::Nil),
                    }
                })
            })?,
        )?;

        let set_impl = |lua: &Lua, (id, component, value): (i64, String, LuaValue)| {
            with_world(lua, |world| {
                let entity = entity_from(id);
                if world.get_entity(entity).is_err() {
                    log::warn!("fulcrum.set: entity {id} is gone; ignored");
                    return Ok(());
                }
                let data = lua_to_ron(&value)?;
                world.resource_scope(|world, registry: bevy_ecs::world::Mut<ComponentRegistry>| {
                    let mut target = world.entity_mut(entity);
                    registry
                        .insert_on(&mut target, &component, &data)
                        .map_err(|e| mlua::Error::runtime(e.to_string()))
                })
            })
        };
        fulcrum.set("set", lua.create_function(set_impl)?)?;
        fulcrum.set("insert", lua.create_function(set_impl)?)?;

        fulcrum.set(
            "query",
            lua.create_function(|lua, names: Variadic<String>| {
                with_world(lua, |world| {
                    let mut entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
                    entities.sort_unstable(); // deterministic order
                    let rows = lua.create_table()?;
                    let mut row_index = 1;
                    world.resource_scope(
                        |world,
                         registry: bevy_ecs::world::Mut<ComponentRegistry>|
                         -> mlua::Result<()> {
                            for entity in entities {
                                let entity_ref = world.entity(entity);
                                let row = lua.create_table()?;
                                row.set("entity", entity.to_bits() as i64)?;
                                let mut matched = true;
                                for name in names.iter() {
                                    match registry.extract_from(&entity_ref, name) {
                                        Some(value) => {
                                            row.set(name.as_str(), ron_to_lua(lua, &value)?)?;
                                        }
                                        None => {
                                            matched = false;
                                            break;
                                        }
                                    }
                                }
                                if matched {
                                    rows.set(row_index, row)?;
                                    row_index += 1;
                                }
                            }
                            Ok(())
                        },
                    )?;
                    Ok(rows)
                })
            })?,
        )?;

        fulcrum.set(
            "query_circle",
            lua.create_function(|lua, (x, y, radius): (f32, f32, f32)| {
                with_world(lua, |world| {
                    let hits = lua.create_table()?;
                    if let Some(grid) = world.get_resource::<fulcrum_spatial::SpatialGrid>() {
                        for (i, entity) in grid
                            .query_circle(fulcrum_core::vec2(x, y), radius)
                            .into_iter()
                            .enumerate()
                        {
                            hits.set(i + 1, entity.to_bits() as i64)?;
                        }
                    }
                    Ok(hits)
                })
            })?,
        )?;

        fulcrum.set(
            "emit_command",
            lua.create_function(|lua, (name, payload): (String, Option<LuaValue>)| {
                let payload = match payload {
                    Some(LuaValue::String(text)) => text.to_str()?.to_owned(),
                    Some(value) => ron::ser::to_string(&lua_to_ron(&value)?)
                        .map_err(|e| mlua::Error::runtime(e.to_string()))?,
                    None => String::new(),
                };
                with_world(lua, |world| {
                    world
                        .resource_mut::<fulcrum_core::CommandOutbox>()
                        .send(name.clone(), payload.clone());
                    Ok(())
                })
            })?,
        )?;

        fulcrum.set(
            "emit",
            lua.create_function(|lua, (name, payload): (String, Option<LuaValue>)| {
                let payload = match payload {
                    Some(value) => lua_to_ron(&value)?,
                    None => ron::Value::Unit,
                };
                with_world(lua, |world| {
                    world
                        .resource_mut::<bevy_ecs::prelude::Messages<ModEvent>>()
                        .write(ModEvent {
                            name: name.clone(),
                            payload: payload.clone(),
                        });
                    Ok(())
                })?;
                // Also queue for other mods' on_event handlers (dispatched after this batch).
                if let Some(mut ctx) = lua.app_data_mut::<LuaCtx>() {
                    ctx.emitted.push((name, payload));
                }
                Ok(())
            })?,
        )?;

        let input = lua.create_table()?;
        input.set(
            "pressed",
            lua.create_function(|lua, key: String| {
                let Some(key) = parse_key(&key) else {
                    return Ok(false);
                };
                with_world(lua, |world| Ok(world.resource::<Input>().pressed(key)))
            })?,
        )?;
        input.set(
            "just_pressed",
            lua.create_function(|lua, key: String| {
                let Some(key) = parse_key(&key) else {
                    return Ok(false);
                };
                with_world(lua, |world| Ok(world.resource::<Input>().just_pressed(key)))
            })?,
        )?;
        fulcrum.set("input", input)?;

        let audio = lua.create_table()?;
        audio.set(
            "play",
            lua.create_function(|lua, path: String| {
                with_world(lua, |world| {
                    play_sound(world, &path);
                    Ok(())
                })
            })?,
        )?;
        fulcrum.set("audio", audio)?;

        Ok(())
    })
}

fn prefab_by_path(
    world: &mut World,
    path: &str,
) -> Result<fulcrum_asset::Handle<PrefabAsset>, String> {
    if let Some(handle) = world
        .resource::<Assets<PrefabAsset>>()
        .handle_for_path(path)
    {
        return Ok(handle);
    }
    let bytes = world
        .resource::<AssetServer>()
        .read_bytes(path)
        .map_err(|e| e.to_string())?;
    let asset = prefab::parse_prefab_public(path, &String::from_utf8_lossy(&bytes))
        .map_err(|e| e.to_string())?;
    Ok(world
        .resource_mut::<Assets<PrefabAsset>>()
        .insert_with_path(path, asset))
}

fn play_sound(world: &mut World, path: &str) {
    use fulcrum_audio::{Audio, Sound, decode_sound};
    let handle = match world.resource::<Assets<Sound>>().handle_for_path(path) {
        Some(handle) => handle,
        None => {
            let Ok(bytes) = world.resource::<AssetServer>().read_bytes(path) else {
                log::error!("fulcrum.audio.play: cannot read {path}");
                return;
            };
            match decode_sound(path, bytes) {
                Ok(sound) => world
                    .resource_mut::<Assets<Sound>>()
                    .insert_with_path(path, sound),
                Err(error) => {
                    log::error!("fulcrum.audio.play: {error}");
                    return;
                }
            }
        }
    };
    world.resource_scope(|world, mut audio: bevy_ecs::world::Mut<Audio>| {
        audio.play(world.resource::<Assets<Sound>>(), handle);
    });
}

// --- driving the tick with world access ---------------------------------------------------

fn with_world_installed<R>(
    runtime: &mut LuaRuntime,
    world: &mut World,
    f: impl FnOnce(&mut LuaRuntime) -> R,
) -> R {
    runtime.with_lua(|lua| lua.set_app_data(WorldPtr(world as *mut World)));
    let result = f(runtime);
    runtime.with_lua(|lua| lua.set_app_data(WorldPtr(std::ptr::null_mut())));
    result
}

/// Run every mod's `on_init` with world access.
pub fn run_init_with_world(runtime: &mut LuaRuntime, world: &mut World) {
    with_world_installed(runtime, world, |runtime| runtime.run_init());
    dispatch_emitted(runtime, world);
}

/// Run every mod's `on_tick` with world access, then dispatch mod-emitted events (one round).
pub fn run_tick_with_world(runtime: &mut LuaRuntime, world: &mut World) {
    let (tick, tick_rate) = {
        let time = world.resource::<Time>();
        (time.tick, (1.0 / time.fixed_delta).round() as u32)
    };
    with_world_installed(runtime, world, |runtime| {
        runtime.run_tick(tick, tick_rate.max(1))
    });
    dispatch_emitted(runtime, world);
}

/// Deliver events emitted during the batch to Lua `on_event` handlers (single round: handlers
/// may emit further events, which Rust sees next read but Lua sees next tick — no cascades).
fn dispatch_emitted(runtime: &mut LuaRuntime, world: &mut World) {
    let emitted = runtime.with_lua(|lua| {
        lua.app_data_mut::<LuaCtx>()
            .map(|mut ctx| std::mem::take(&mut ctx.emitted))
            .unwrap_or_default()
    });
    if emitted.is_empty() {
        return;
    }
    with_world_installed(runtime, world, |runtime| {
        for (name, payload) in emitted {
            let value = runtime
                .with_lua(|lua| ron_to_lua(lua, &payload))
                .unwrap_or(mlua::Value::Nil);
            runtime.dispatch_event(&name, value);
        }
    });
}
