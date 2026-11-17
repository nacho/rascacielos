# Formats

## Share code (`Puzzle::to_share_code`)

`#R` + size digit + difficulty (`e`, `m`, `h`, `v`, `u` for unknown) + the
top, bottom, left and right clues (N digits each) + the N² cells row by row.
`0` is a missing clue or an empty cell. The start view reads it from the
clipboard while "Custom" is selected.

Example (4×4, hard): `#R4h03212133431212200000000001000000`

## Text (`Puzzle::to_text`, exported as `.txt`)

The grid surrounded by its clues, `.` for missing clues and empty cells, any
character in the corners (`+` when exporting):

```
+.321+
4....1
3....2
1.1..2
2.....
+2133+
```

## JSON saves (`board.rs`)

```json
{
  "version": 1,
  "size": 4,
  "difficulty_category": "Medium",
  "time_elapsed": 12.0,
  "clues": [[0, 3, 2, 1], [2, 1, 3, 3], [4, 3, 1, 2], [1, 2, 2, 0]],
  "cells": [{"position": [0, 0], "value": 1, "fixed": true, "earmarks": []}]
}
```

Clues are top, bottom, left, right with 0 for missing; values and earmarks
are heights 1..N (0 for an empty cell); `time_elapsed` is −1 when the timer
is off. The active game is `$XDG_DATA_HOME/rascacielos/savefile`; finished
and printed puzzles are archived in `finished/` and `printed/`.
