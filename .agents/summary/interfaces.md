# Interfaces

## Signals

| Object | Signal | Arguments | Emitted when |
|---|---|---|---|
| `RascacielosBackend` | `game-changed` | none | A game is generated, loaded or started from a share code |
| `RascacielosGame` | `value-changed` | position, old value, new value (`CompatValue`) | A cell value changes, including through undo/redo |
| | `earmark-changed` | position, earmark index, enabled | An earmark toggles |
| | `action-completed` | `StackAction` | An undoable action, undo or redo finishes |
| | `completed` | none | Every cell is filled, no height repeats and every clue holds |
| | `tick` | none | Every second while the clock runs |
| | `paused` | bool | Pause toggles |
| `RascacielosCell` | `selected`, `grab-picker` | none | Selection and picker requests from the cell |
| `RascacielosValuePicker`, `RascacielosEarmarkPicker` | `finished` | none | The picker should close |

The backend also has notify signals for its properties: `pref-*`,
`earmark-mode`, `keyboard-pressed-recently` and `tgame`.

## Actions

| Group | Actions | Defined in |
|---|---|---|
| `app` (group inserted on the window) | `new-game`, `start-generated-game`, `back`, `print-generator-dialog`, `preferences-dialog`, `shortcuts-dialog`, `about-dialog`, `toggle-fullscreen`, `zoom-in`, `zoom-out`, `zoom-reset` | `window.rs` `setup_actions()` |
| `app` (on the `GApplication`) | `quit` | `main.rs` |
| `game-view` | `undo`, `redo`, `reset-board`, `toggle-pause`, `earmark-mode`, `share-puzzle`, `export-puzzle`, `save-game-as`, `completed-dialog` | `game_view.rs` |
| `grid` | `move-up`, `move-down`, `move-left`, `move-right`, `escape` | `grid.rs` |
| `cell` | `insert-value`, `insert-earmark`, `clear`, `show-picker` | `cell.rs` |

Keyboard shortcuts are set in each widget's `class_init` with
`utils::new_shortcut*`. Check `setup_actions()` in each file for the exact
list and parameter types before you use an action name from a template or a
test. `ui_tests.rs` drives the app through these names. The shortcut
`app.swap-view` does not match any action (see `review_notes.md`).
GTK treats `_` and `-` the same in action names, so
`app.preferences_dialog` reaches `preferences-dialog`.

## Backend facade

Widgets change the game only through `RascacielosBackend::game_*`:
`game_set_value`, `game_clear_cell`, `game_toggle_earmark`, `game_undo`,
`game_redo`, `game_reset`, `game_toggle_pause`, `game_start_clock`, and
queries such as `game_value`, `game_fixed`, `game_earmarks`,
`game_earmark_is_possible`, `game_broken`, `game_clue`,
`game_clue_conflict`, `game_size`, `game_difficulty`, `game_highscore`. The
`game!(backend)` macro in `lib/utils.rs` borrows the current game and panics
when there is none.

Game lifecycle: `game_generate(done)`, `game_load(path)` (tries JSON, then
the text format), `check_clipboard(s)` and `tgame_start()` for share codes,
`game_save_as(path)`, `save_game`, `archive_game`, `save_printed_board`,
`delete_save`.

## Model API (no GTK)

| Module | Entry points |
|---|---|
| `puzzle` | `Puzzle::{empty, from_solution, clue, set_clue, given, set_given, validate, is_solved_by, to_share_code, from_share_code, to_text, from_text}`, `line_coords`, `visible`, `clue_conflict`, `is_latin`, `clues_from_solution`, `Side`, `Tier`, `ClueConflict`, `PuzzleError`, `MIN_SIZE`, `MAX_SIZE` |
| `solver` | `solve(puzzle, max_tier) -> Option<(solution, tier)>`, `find_solutions(puzzle, limit)`, `count_solutions`, `initial_candidates(puzzle) -> Option<Vec<u16>>` |
| `generator` | `random_latin_square`, `generate(n, tier, rng, cancel) -> Option<Generated>`, `available(n, tier)`, `Generated = (Puzzle, Vec<u8>)` |

## External integration points

| Point | Details |
|---|---|
| GSettings | Schema `io.github.nacho.rascacielos`. See `data_models.md`. Plain Cargo runs need `glib-compile-schemas data/` and `GSETTINGS_SCHEMA_DIR=data`. |
| Files | `$XDG_DATA_HOME/rascacielos/`. See `formats.md`. |
| Clipboard | The start view reads share codes while "Custom" is selected. The game view copies the share code. |
| D-Bus | `data/io.github.nacho.rascacielos.service.in` makes the app D-Bus activatable. |
| Printing | `GtkPrintOperation`, either through the print dialog or exported to PDF in tests. |
| Translations | `po/POTFILES.in` lists the Rust and UI files. The pot target is `rascacielos-pot`. |
