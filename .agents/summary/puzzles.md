# Puzzles, solver and generator

## Model (`src/lib/puzzle.rs`)

- `Puzzle { n, clues: [Vec<Option<u8>>; 4], givens: Vec<Option<u8>> }`,
  sizes `MIN_SIZE..=MAX_SIZE` (4..=7). Clues are indexed by `Side::index()`
  (Top, Bottom, Left, Right) then by column or row; givens are row-major.
- `line_coords(n, side, i)` lists the cells a clue looks along, nearest
  first. `visible()` counts new maxima.
- `clue_conflict(clue, line)`: `Full` when the line is full and shows a
  different count; `Early` when the filled cells next to the clue already
  show too many, or the tallest building is among them and too few are
  visible. `Full` is shown with duplicate warnings, `Early` with clue
  warnings.

## Solver tiers (`src/lib/solver.rs`)

Candidates are `u16` bitmasks, bit `h` for height `h`. Each tier includes
the easier ones; `propagate` always applies the easiest technique that makes
progress and records the hardest one used.

| Tier | Technique |
|---|---|
| Easy | Edge-clue rules (applied once): clue 1 → N next to it; clue N → 1..N; cell at distance d ≤ N − c + 1 + d; clue ≥ 2 → no N next to it. Plus naked and hidden singles. |
| Medium | Enumerate the arrangements of each row/column that fit candidates and both clues; keep the union. |
| Hard | Trial: rule out a candidate if the Easy techniques then contradict. |
| Very hard | Trial with the Medium techniques. |

`solve()` succeeding means the solution is unique (every deduction is
sound). `find_solutions()` is a backtracking search for puzzles beyond the
tiers; `RascacielosGame::solve` grades those as `Custom`.

## Generator (`src/lib/generator.rs`)

Random Latin square (shuffled backtracking), then start from every clue and
every cell, remove cells then clues in random order while
`solve(target_tier)` still succeeds. Easy keeps every clue. Accept only if
the next easier tier fails.

Measured in release mode (20 attempts each, single thread):

| Size | Easy | Medium | Hard | Very hard |
|---|---|---|---|---|
| 4×4 | 20/20 | 20/20 | 3/20 | 0/20 (disabled by `available`) |
| 5×5 | 20/20 | 20/20 | 10/20 | 3/20 |
| 6×6 | 20/20 | 20/20 | 9/20 | 6/20, ~3.6 s total |
| 7×7 | 20/20 | 20/20 | 12/20 | 10/20, ~31 s total |

Re-measure after changing the solver and update `available()`.
