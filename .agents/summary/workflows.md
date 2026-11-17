# Workflows

## Starting a game

1. The start view stores the size (`AdwToggleGroup`) and difficulty in
   `RascacielosData`.
2. `app.start-generated-game` → `RascacielosBackend::game_generate`, which runs
   `RascacielosGame::generate` in `gio::spawn_blocking` and then emits
   `game-changed`.
3. On `game-changed` the window shows the game view, and the grid, clue
   frame and pickers reload from the new game and reconnect their hooks.

## Finishing a game

`RascacielosGame` emits `completed` when every cell is filled, no height repeats
and every clue holds. The game view plays the completion animation, saves
the highscore for the size and difficulty and shows the completed dialog.

## Release

1. Update the version in `Cargo.toml` and `meson.build`.
2. Add a `<release>` to `data/io.github.nacho.rascacielos.metainfo.xml.in.in`
   with the date and the changes since the last tag
   (`git log vPREVIOUS..HEAD --oneline --no-merges`).
3. `cargo fmt && cargo update -p rascacielos`
4. Commit as "Release X.Y.Z", tag `vX.Y.Z`, push with tags.
