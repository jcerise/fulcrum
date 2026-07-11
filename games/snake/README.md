# snake

The Fulcrum Book's **From Zero** game: your first game, built in six chapters for engineers
who've never made one (`book/src/fz01-what-is-a-game.md` onward).

- `src/game.rs` — the whole simulation: a `VecDeque` of cells, an input buffer, the rules.
  No sprites, no sounds; it runs headless.
- `src/main.rs` — the presentation: projections of that state into sprites and text, plus
  sound reacting to events. Never writes sim state.
- `tests/gameplay.rs` — the payoff: a greedy bot plays the game in CI, corner-case rules are
  pinned, and a same-seed fingerprint guards determinism.
- `examples/fz01..fz04` — each chapter's runnable program.

`cargo run -p snake` — WASD/arrows steer, Enter restarts.
