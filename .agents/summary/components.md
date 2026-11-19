# Components

## Application and window

| Component | File | Responsibility |
|---|---|---|
| `RascacielosApplication` | `main.rs` | `adw::Application`. Registers resources, sets up gettext and the GLib logger, creates the window. |
| `RascacielosWindow` | `window.rs`, `ui/window.ui` | Owns the backend, switches between start and game views, holds the `app.*` actions, zoom, fullscreen, accent colour and the about dialog. |
| `RascacielosMenuButton` | `menu_button.rs`, `ui/menu-button.ui` | Primary menu. Swaps its fullscreen entry with the window state. |

## Views

| Component | File | Responsibility |
|---|---|---|
| `RascacielosStartView` | `start_view.rs`, `ui/start-view.ui` | Size toggle group (4–7), difficulty rows, "Custom" row for share codes from the clipboard, open file. Makes the Very hard row insensitive when `generator::available` rejects it for the size, and moves the selection to Hard. |
| `RascacielosGameView` | `game_view.rs`, `ui/game-view.ui` | `adw::BreakpointBin` with the board, header bar, timer, pause overlay, undo/redo, earmark mode, share/export/save-as, completed dialog and animation. Switches between wide and vertical button layouts. |

## Board widgets

| Component | File | Responsibility |
|---|---|---|
| `RascacielosClueFrame` | `clue_frame.rs` | Surrounds the grid with a band of clues (`CLUE_BAND`) on each side. Creates 4 × 7 `RascacielosClue` widgets and places each one next to its row or column. |
| `RascacielosClue` | `clue.rs` | One clue drawn as a box with a tip pointing into the grid (`clue_path`, `clue_box`). Turns red on a clue conflict. |
| `RascacielosGrid` | `grid.rs` | The N×N cells, separators, selection, keyboard moves (`grid.move-*`), highlighters, warnings and the completed animation. |
| `RascacielosCell` | `cell.rs` | One cell: the value label, the earmark labels, click handling, `cell.*` actions, warnings and highlight state. |
| `RascacielosGridLayout` | `grid_layout.rs` | Layout manager that keeps the board square and adds margins that shrink with the window. |

## Pickers

| Component | File | Responsibility |
|---|---|---|
| `RascacielosPickerPopover` | `picker_popover.rs` | One popover shared by all cells, showing either the value or the earmark picker. |
| `RascacielosValuePicker` | `value_picker.rs` | Buttons 1..N and clear. Implements `PickerBase`. |
| `RascacielosEarmarkPicker` | `earmark_picker.rs` | Toggle buttons 1..N for earmarks. Implements `PickerBase`. |

## Dialogs and printing

| Component | File | Responsibility |
|---|---|---|
| `RascacielosPreferencesDialog` | `preferences_dialog.rs`, `ui/preferences-dialog.ui` | Binds the switches to the backend `pref_*` properties. |
| `RascacielosPrintGeneratorDialog` | `print_generator_dialog.rs`, `ui/print-generator-dialog.ui` | Size, difficulty, count, puzzles per page and "include current puzzle". Generates off the main thread and prints. |
| `RascacielosPrinter` | `printer.rs` | Lays out several puzzles per page and draws them with `gtk::Snapshot` into a `GtkPrintOperation`. `export_pdf` exists for tests only. |
| Shortcuts dialog | `ui/shortcuts-dialog.ui` | Builder file only. `window.rs` loads it. |

## State and model

| Component | File | Responsibility |
|---|---|---|
| `RascacielosBackend` | `lib/backend.rs` | Settings bindings, current game and the temporary share-code game (`tgame`), autosave every 5 minutes, save/load/archive, highscores, the `game_*` facade. |
| `RascacielosGame` | `lib/game.rs` | Board wrapper with the undo/redo stack (`StackItem`, `StackAction`), clock and signals. Generates and solves puzzles. |
| `RascacielosBoard` | `lib/board.rs` | Play state: cells, duplicate counts, broken cells, candidates, completion, clue conflicts, JSON and text conversions. |
| `RascacielosData` | `lib/data.rs` | JSON data file with highscores per size and difficulty, last size and difficulty, and print options. |
| `Puzzle` | `lib/puzzle.rs` | Clues and givens, validation, visibility, clue conflicts, share code, text format. |
| Solver | `lib/solver.rs` | Candidate propagation by tier, backtracking search, initial candidates. |
| Generator | `lib/generator.rs` | Random Latin square and clue/cell removal for a target tier. `available(n, tier)` gates the start view and the print dialog. |
| Enums | `lib/enums.rs` | `DifficultyCategory`, `Coord`, `RascacielosDirection`, `ZoomLevel`, `CompatValue`. |
| Utils | `lib/utils.rs` | Shortcut helpers (`new_shortcut*`), `get_cloned`, the `game!` macro. |

## Tests

| Where | What | How it runs |
|---|---|---|
| `lib/puzzle.rs`, `lib/solver.rs`, `lib/generator.rs`, `lib/board.rs` | Model, formats, solver tiers, generator | `cargo test` |
| `lib/game.rs` | Undo/redo, solve, load, save. All in one `#[test]` because the game clock needs the default main context. | `cargo test` |
| `lib/generator.rs` `measure_availability` | Success rate per size and tier | Ignored. Release mode, `--nocapture`, optional `MEASURE_TIER`. |
| `ui_tests.rs` | Opens every dialog through actions, prints to PDF | Ignored, needs a display, `--test-threads=1` |
