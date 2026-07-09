# Dungeon

The Fulcrum phase-3 milestone: **everything on screen is data.** The map, the player, the
monsters, and the UI are RON/JSON files under `assets/`; the game code only registers components
(`Health`, `Melee`, ...) and adds systems.

Run: `cargo run -p dungeon`

| Key | Action |
| --- | --- |
| WASD | move |
| Space | attack (state-machine animation, hits in a radius) |
| I | toggle inventory panel |
| Escape | pause menu (Resume / Quit buttons) |
| Enter | restart after death (reloads the scene) |
| F1 | collision + aggro gizmos |
| F12 | egui inspector |

## The live-edit loop

Start the game, then edit any of these **while it runs**:

- `assets/maps/dungeon.map.ron` — change tiles; the map re-meshes in place.
- `assets/prefabs/slime.prefab.ron` — raise `Health`/`Melee`; affects the next spawn
  (die and press Enter to reload the scene and see it).
- `assets/ui/hud.ui.ron` — restyle the HUD; the tree respawns instantly.
- `assets/anim/player.animsm.ron` / `assets/creatures.json` — tweak animations.
- `assets/tiles.png` / `assets/creatures.png` — repaint; textures swap in place.

Hot reload is on by default in debug builds (`FulcrumConfig::hot_reload`).

## Determinism

`tests/determinism.rs` drives the whole game headless from the same data files: same seed +
scripted input twice produces bit-identical positions, health, and gold — and walls block
because tile *data* loads in the simulation (only textures are cosmetic).
