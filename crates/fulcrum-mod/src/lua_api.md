# The Fulcrum Lua API

Everything a mod script can touch. Scripts run **inside the simulation tick** and must be
deterministic — the sandbox enforces most of this for you (no `io`/`os`/`load`; `math.random`
is engine-seeded per mod; wall clocks don't exist).

## Lifecycle

```lua
fulcrum.on_init(function() ... end)         -- once, after world setup, in mod load order
fulcrum.on_tick(function(tick) ... end)     -- every simulation tick, in mod load order
fulcrum.on_event(name, function(e) ... end) -- named sim events (from games or other mods)
```

A callback that errors three times in a row is disabled (logged). A callback that runs too
long (instruction budget) is aborted with an error naming your mod — the game survives.

## Entities and components

Component names are the same stable strings used in prefab files; tables mirror the RON shape.

```lua
local e = fulcrum.spawn_prefab("prefabs/slime.prefab.ron", { x = 100, y = 50 })  -- position optional
fulcrum.despawn(e)

local hp = fulcrum.get(e, "Health")       -- table copy, or nil (absent component / stale entity)
fulcrum.set(e, "Health", { max = 20, current = 20 })
fulcrum.insert(e, "Burning", { ticks_left = 120 })   -- alias of set

-- Rows of every entity having ALL named components, in deterministic order:
for _, row in ipairs(fulcrum.query("Transform2D", "Health")) do
    -- row.entity, row.Transform2D, row.Health
end
```

Stale entities never error: `get` returns `nil`, `set` warns and ignores, `despawn` is a no-op.

## Engine access

```lua
fulcrum.tick()                       -- current simulation tick (integer)
fulcrum.input.pressed("Space")       -- tick-sampled keyboard (also just_pressed)
fulcrum.emit("boom", { power = 7 })  -- sim event: Rust reads EventReader<ModEvent>;
                                     -- other mods' on_event("boom") handlers hear it too
fulcrum.audio.play("sounds/boom.ogg")-- cosmetic playback (loads through the VFS)
fulcrum.log("hello")                 -- info log, prefixed with your mod id
print("also fine")                   -- same
os.clock()                           -- SIMULATION seconds (tick / tick_rate), not wall time
math.random(), math.random(n), math.random(m, n)  -- your mod's deterministic stream
require("util")                      -- loads <your mod>/scripts/util.lua (cached, cycle-checked)
```

## Determinism rules for script authors

1. World access only inside callbacks (elsewhere it errors).
2. All randomness through `math.random` — never derive it from table iteration order
   (`pairs` order is not guaranteed; use `ipairs` or sort keys when it matters).
3. There is no wall clock, file IO, or network. That's on purpose.

Events emitted during a tick reach other mods' handlers once, after all `on_tick` callbacks
finish (no same-tick cascades); Rust systems read them the same tick or the next.
