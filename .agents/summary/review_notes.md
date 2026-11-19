# Review notes

These notes come from the consistency and completeness check of this
knowledge base. They are findings, not fixes. Nothing listed here has been
changed in the code.

## Inconsistencies in the code

| Finding | Where | Notes |
|---|---|---|
| Meson requires `gtk4 >= 4.19.3`, but Cargo uses `v4_22` and the docs say GTK 4.22 | `meson.build` | A system with GTK 4.20 passes configure and then fails in Cargo. |
| The picker popover's imp struct is `RascacielosNumberPicker`, but its type name is `RascacielosPickerPopover` | `picker_popover.rs` | Harmless, but it makes searching harder. |
| Misspelled names: `new_unitialized`, `cell_possibilies` | Several widgets, `lib/board.rs` | Inherited spelling. Search for these exact names. |
| The release profile comment says "the board generator is ~10x faster in o2" | `Cargo.toml` | Written for Sudoku's generator. Not re-measured for the skyscrapers generator. |

## Tooling notes

- CI installs Rust with `actions-rs/toolchain@v1`, which is no longer
  maintained. `dtolnay/rust-toolchain` is the usual replacement.
- The pre-commit hook only checks `cargo fmt`. Clippy runs only in CI.

## Documentation gaps

- **CSS.** `resources/style.css` classes and custom properties
  (`rascacielos-*`) are not catalogued. Read the stylesheet together with the
  `add_css_class` calls before you change styling.
- **Keyboard shortcuts.** The full list lives in `class_init` of `window.rs`,
  `game_view.rs`, `grid.rs` and `cell.rs` and in `ui/shortcuts-dialog.ui`.
  The docs don't duplicate it.
- **Highlighting and warnings.** The interaction between the highlighter,
  duplicate, earmark, clue and solution warnings in `grid.rs` and `cell.rs` is
  only summarised.
- **Translations.** Only Spanish exists. `po/POTFILES.in` must list every
  file with translatable strings. Nothing checks this automatically.
- **Language support.** All code is Rust or Meson and was analysed. CSS, SVG
  and PO content was not reviewed in depth.

## Recommendations

1. Raise the Meson GTK check to `>= 4.22` so that it matches the Cargo
   feature.
2. Re-run `measure_availability` after any solver change and update the
   table in `puzzles.md` and `generator::available`.
3. Update `formats.md` whenever `JSON_VERSION`, the share code or the text
   format changes.
