# Fulcrum game template

A minimal, standalone Fulcrum game meant to be **copied out of this repo** to start a new
project. It is excluded from the fulcrum workspace and carries everything a game needs to
build on its own:

- `Cargo.toml` — depends only on `fulcrum` (plus the manifest-only `bevy_ecs` that
  `#[derive(Component)]` needs) and carries the debug-build `opt-level = 2` block the
  workspace normally provides.
- `rust-toolchain.toml` — pins the same toolchain as the engine.
- `src/game.rs` / `src/main.rs` — the Fulcrum split: pure simulation on `FixedUpdate` in the
  library, cosmetic frame-side systems and sprite attachment in the binary.
- `tests/determinism.rs` — the headless same-seed-twice gate every milestone game carries.
- `assets/` — asset root, resolved via `CARGO_MANIFEST_DIR` so `cargo run` works from
  anywhere. Ships the 1x1 `white.png` all the milestone games start from.

## Starting a new game

```sh
cp -r games/_template ~/source/my-game
cd ~/source/my-game
rm .gitignore   # or trim it: keep /target, but commit your Cargo.lock
git init
```

Then make it yours:

1. **Point the `fulcrum` dependency at your checkout.** The template's relative path only
   works in place. Either fix the path:

   ```toml
   fulcrum = { path = "../fulcrum/crates/fulcrum" }
   ```

   or use the git dependency (no publishing needed — Cargo finds the workspace member by
   name), optionally with a local patch while hacking on the engine:

   ```toml
   [dependencies]
   fulcrum = { git = "https://github.com/jcerise/fulcrum.git", branch = "main" }

   [patch."https://github.com/jcerise/fulcrum.git"]
   fulcrum = { path = "../fulcrum/crates/fulcrum" }
   ```

2. **Rename the package.** Change `name` in `Cargo.toml`, then update the `my_game::` imports
   in `src/main.rs` and `tests/determinism.rs` to the new crate name.

3. Build and run:

   ```sh
   cargo run             # window with a square; WASD/arrows to move
   cargo test            # headless determinism gate
   ```

When you upgrade the engine, keep `bevy_ecs` in your manifest in lockstep with fulcrum's
workspace pin — a version mismatch makes derived components foreign types to the engine's ECS.
