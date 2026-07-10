# Mods: the VFS and Lua

Chapter 7's promise — content is data — had a quiet corollary: if the game reads everything
through data files, *someone else's* data files work too. Fulcrum makes modding first-class
with one plugin:

```rust,ignore
.with_plugin(ModPlugin::from_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/mods")))
```

That line discovers every mod under `mods/`, mounts their assets, and runs their scripts.
Grove ships one to prove it:

```text
games/grove/mods/more_gems/
├── mod.ron              Mod(id: "more_gems", name: "More Gems", version: "1.0.0",
│                            scripts: ["scripts/init.lua"])
└── scripts/init.lua
```

## Assets: the layered VFS

Since chapter 1, every asset load has gone through one seam: the `AssetServer`. What you didn't
see is that it's a *stack* of mount points. The base game's `assets/` is the bottom layer; each
mod mounts on top, in load order (a mod can declare `load_after` in its manifest; cycles and
duplicate ids are startup errors).

Two consequences, both free:

- **Additive content.** A mod ships `units/berserker.unit.ron` and any game that *lists* a
  directory (`server.list("units", "ron")`) discovers it. Nothing in the game names the file.
- **Overrides.** A mod ships `creatures.png` and every load of that *path* now gets the mod's
  version — retexture mods are just files with the same names. Delete the mod directory and
  the original is back.

## Scripts: sandboxed Lua on the simulation clock

`init.lua` from `more_gems`, in its entirety:

```lua
fulcrum.on_init(function()
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 250, y = 250 })
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 480, y = 130 })
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 120, y = 100 })
    fulcrum.log("planted 3 bonus gems")
end)

fulcrum.on_tick(function(tick)
    if tick > 0 and tick % 1800 == 0 then
        fulcrum.log("the fox grows impatient...")
    end
end)
```

The game code has no idea this mod exists. The bonus gems are spawned through the same prefab
the scene uses, so they carry `GemTag` — collection, the HUD count, and the win condition pick
them up automatically. **This is why the component registry earns its keep**: because chapter
7 registered your components by name, Lua can spawn, read, and write them with zero
per-component glue — `fulcrum.get(e, "Health")`, `fulcrum.set(e, ...)`,
`fulcrum.query("Transform2D", "GemTag")`, `fulcrum.query_circle(x, y, r)`.

Scripts run *inside* `FixedUpdate`, and the sandbox keeps them inside the determinism contract
too: no `io`, no `require`ing outside the mod, `os.clock()` is simulation time, and
`math.random` is a per-mod deterministic stream seeded so that one mod's rolls never depend on
another mod's presence. A runaway script trips an instruction budget instead of hanging your
game; a script that errors three ticks running is muted, not fatal. Mods record and replay
like any other part of the simulation.

## Events: the two-way channel

Games and mods talk through named events:

```lua
fulcrum.on_event("unit_died", function(e)     -- game announced something
    fulcrum.emit("spawn_effect", { effect = "fx/explosion.fx.ron", x = e.x, y = e.y })
end)
```

Rust reads mod emissions with `EventReader<ModEvent>` and announces things by writing
`ModEvent`s — deliveries are exactly-once in both directions. There's also
`fulcrum.emit_command` for the *player-command* channel, which matters in chapter 14. The full
API is one page: `crates/fulcrum-mod/src/lua_api.md`. For a complete worked mod — new unit
type, scripted reinforcements, on-death effects — read `games/rts-slice/mods/sample_mod`.

```text
cargo run -p grove --example ch13_mods
```

Eleven gems instead of eight. Now delete `mods/more_gems`, run again: eight. Put it back:
eleven. That's the whole modding story from the player's side — a directory.
