-- Three bonus gems, spawned through the same prefab the scene uses. The game code has no
-- idea this mod exists — the gems are just more entities with GemTag, so collection, the
-- HUD count, and the win condition all pick them up automatically.

fulcrum.on_init(function()
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 250, y = 250 })
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 480, y = 130 })
    fulcrum.spawn_prefab("prefabs/gem.prefab.ron", { x = 120, y = 100 })
    fulcrum.log("planted 3 bonus gems")
end)

-- A taunt, on the simulation clock (deterministic, replay-safe).
fulcrum.on_tick(function(tick)
    if tick > 0 and tick % 1800 == 0 then
        fulcrum.log("the fox grows impatient...")
    end
end)
