/* -*- Mode: rust; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/*
 * Copyright © 2026 Ignacio Casal Quinteiro
 *
 * This file is part of Rascacielos.
 *
 * Rascacielos is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Rascacielos is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Rascacielos. If not, see <http://www.gnu.org/licenses/>.
 */

//! A skyscrapers solver that only uses the techniques allowed by a
//! difficulty tier, so it can grade puzzles, plus a backtracking search to
//! count solutions.
//!
//! Each cell keeps a bitmask of the heights it can still hold (bit `h` for
//! height `h`). The techniques, from easiest to hardest:
//!
//! - Easy: the edge-clue rules (a clue of 1 means the tallest building is
//!   next to it, a clue of N means the line climbs 1..N, a cell at distance
//!   `d` from a clue `c` is at most `N - c + 1 + d` tall, and a clue of 2 or
//!   more cannot have the tallest building next to it), applied once at the
//!   start, plus naked and hidden singles in rows and columns.
//! - Medium: listing every arrangement of a row or column that fits its
//!   candidates and both of its clues, and keeping only the heights that
//!   appear in one of them.
//! - Hard: trying each candidate of a cell and ruling it out when the easy
//!   techniques then find a contradiction.
//! - Very hard: the same, but looking for the contradiction with the medium
//!   techniques.

use crate::lib::puzzle::{Puzzle, Side, Tier, bit, full_mask, line_coords, visible};

/// Raised when the candidates can no longer lead to a solution.
#[derive(Debug, PartialEq, Eq)]
struct Contradiction;

type Step = Result<bool, Contradiction>;

#[derive(Clone)]
struct Solver<'a> {
    puzzle: &'a Puzzle,
    n: usize,
    candidates: Vec<u16>,
    /// The hardest tier that made progress so far.
    tier_used: Tier,
}

fn single_height(mask: u16) -> Option<u8> {
    (mask.count_ones() == 1).then(|| mask.trailing_zeros() as u8)
}

impl<'a> Solver<'a> {
    fn new(puzzle: &'a Puzzle) -> Result<Self, Contradiction> {
        let n = puzzle.size();
        let mut solver = Self {
            puzzle,
            n,
            candidates: vec![full_mask(n); n * n],
            tier_used: Tier::Easy,
        };
        for row in 0..n {
            for col in 0..n {
                if let Some(h) = puzzle.given(row, col) {
                    solver.restrict(row * n + col, bit(h))?;
                }
            }
        }
        solver.apply_clue_rules()?;
        Ok(solver)
    }

    /// Keeps only the heights in `mask` for cell `i`.
    fn restrict(&mut self, i: usize, mask: u16) -> Step {
        let old = self.candidates[i];
        let new = old & mask;
        if new == 0 {
            return Err(Contradiction);
        }
        self.candidates[i] = new;
        Ok(new != old)
    }

    fn apply_clue_rules(&mut self) -> Result<(), Contradiction> {
        let n = self.n;
        for side in Side::ALL {
            for index in 0..n {
                let Some(clue) = self.puzzle.clue(side, index) else {
                    continue;
                };
                let clue = clue as usize;
                for (d, (row, col)) in line_coords(n, side, index).enumerate() {
                    let i = row * n + col;
                    let mask = if clue == 1 && d == 0 {
                        bit(n as u8)
                    } else if clue == n {
                        bit(d as u8 + 1)
                    } else {
                        // Heights 1..=n - clue + 1 + d.
                        let max = (n + 1 + d - clue).min(n);
                        let mut mask = full_mask(max);
                        if clue >= 2 && d == 0 {
                            mask &= !bit(n as u8);
                        }
                        mask
                    };
                    self.restrict(i, mask)?;
                }
            }
        }
        Ok(())
    }

    fn row(&self, row: usize) -> impl Iterator<Item = usize> + use<> {
        let n = self.n;
        (0..n).map(move |col| row * n + col)
    }

    fn col(&self, col: usize) -> impl Iterator<Item = usize> + use<> {
        let n = self.n;
        (0..n).map(move |row| row * n + col)
    }

    /// Every row and column, as lists of cell indexes.
    fn units(&self) -> Vec<Vec<usize>> {
        (0..self.n)
            .flat_map(|i| [self.row(i).collect(), self.col(i).collect()])
            .collect()
    }

    fn is_solved(&self) -> bool {
        self.candidates.iter().all(|m| m.count_ones() == 1)
    }

    fn grid(&self) -> Vec<u8> {
        self.candidates
            .iter()
            .map(|m| single_height(*m).unwrap_or(0))
            .collect()
    }

    /// Removes the height of every solved cell from the rest of its row and
    /// column.
    fn naked_singles(&mut self) -> Step {
        let mut progress = false;
        for unit in self.units() {
            for &i in &unit {
                if single_height(self.candidates[i]).is_none() {
                    continue;
                }
                let mask = self.candidates[i];
                for &j in &unit {
                    if j != i {
                        progress |= self.restrict(j, !mask)?;
                    }
                }
            }
        }
        Ok(progress)
    }

    /// Solves a cell when it is the only place left for a height in its row
    /// or column.
    fn hidden_singles(&mut self) -> Step {
        let mut progress = false;
        for unit in self.units() {
            for h in 1..=self.n as u8 {
                let mut places = unit.iter().filter(|&&i| self.candidates[i] & bit(h) != 0);
                let Some(&first) = places.next() else {
                    return Err(Contradiction);
                };
                if places.next().is_none() {
                    progress |= self.restrict(first, bit(h))?;
                }
            }
        }
        Ok(progress)
    }

    /// Restricts each line to the heights used by the arrangements that fit
    /// its candidates and clues.
    fn line_arrangements(&mut self) -> Step {
        let n = self.n;
        let mut progress = false;
        for (start, index) in [Side::Left, Side::Top]
            .into_iter()
            .flat_map(|side| (0..n).map(move |i| (side, i)))
        {
            let cells: Vec<usize> = line_coords(n, start, index)
                .map(|(row, col)| row * n + col)
                .collect();
            let masks: Vec<u16> = cells.iter().map(|&i| self.candidates[i]).collect();
            let near = self.puzzle.clue(start, index);
            let far = self.puzzle.clue(start.opposite(), index);
            let allowed = arrangements_union(n, &masks, near, far);
            for (&i, mask) in cells.iter().zip(allowed) {
                progress |= self.restrict(i, mask)?;
            }
        }
        Ok(progress)
    }

    /// Rules out the first candidate found that leads to a contradiction
    /// with the techniques up to `inner`.
    fn trial(&mut self, inner: Tier) -> Step {
        for i in 0..self.candidates.len() {
            let mask = self.candidates[i];
            if mask.count_ones() < 2 {
                continue;
            }
            for h in 1..=self.n as u8 {
                if mask & bit(h) == 0 {
                    continue;
                }
                let mut attempt = self.clone();
                attempt.candidates[i] = bit(h);
                if attempt.propagate(inner).is_err() {
                    self.restrict(i, !bit(h))?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Applies the easiest technique that makes progress, up to `max_tier`,
    /// until the puzzle is solved or nothing else can be deduced.
    fn propagate(&mut self, max_tier: Tier) -> Result<(), Contradiction> {
        loop {
            let mut progress = false;
            for tier in Tier::ALL {
                if tier > max_tier {
                    break;
                }
                progress = match tier {
                    Tier::Easy => self.naked_singles()? || self.hidden_singles()?,
                    Tier::Medium => self.line_arrangements()?,
                    Tier::Hard => self.trial(Tier::Easy)?,
                    Tier::VeryHard => self.trial(Tier::Medium)?,
                };
                if progress {
                    self.tier_used = self.tier_used.max(tier);
                    break;
                }
            }
            if !progress {
                return Ok(());
            }
            if self.is_solved() {
                // Check the clues once every height is known.
                return self.line_arrangements().map(|_| ());
            }
        }
    }
}

/// For each position of a line, the heights that appear there in some
/// arrangement that uses each height once, fits `masks` and shows `near`
/// buildings from the start and `far` from the end. All zero when there is
/// no such arrangement.
fn arrangements_union(n: usize, masks: &[u16], near: Option<u8>, far: Option<u8>) -> Vec<u16> {
    struct Search<'a> {
        n: usize,
        masks: &'a [u16],
        near: Option<u8>,
        far: Option<u8>,
        line: Vec<u8>,
        union: Vec<u16>,
    }

    impl Search<'_> {
        fn run(&mut self, used: u16, tallest: u8, seen: u8) {
            let n = self.n;
            let pos = self.line.len();
            if pos == n {
                let from_far = visible(self.line.iter().rev().copied());
                if self.near.is_none_or(|c| c == seen) && self.far.is_none_or(|c| c == from_far) {
                    for (u, h) in self.union.iter_mut().zip(&self.line) {
                        *u |= bit(*h);
                    }
                }
                return;
            }
            for h in 1..=n as u8 {
                if self.masks[pos] & bit(h) == 0 || used & bit(h) != 0 {
                    continue;
                }
                let (tallest, seen) = if h > tallest {
                    (h, seen + 1)
                } else {
                    (tallest, seen)
                };
                // Each building still to place adds at most one visible
                // building, and only the heights above the tallest so far can.
                if let Some(c) = self.near {
                    let can_add = (n as u8 - tallest).min((n - pos - 1) as u8);
                    if seen > c || seen + can_add < c {
                        continue;
                    }
                }
                self.line.push(h);
                self.run(used | bit(h), tallest, seen);
                self.line.pop();
            }
        }
    }

    let mut search = Search {
        n,
        masks,
        near,
        far,
        line: Vec::with_capacity(n),
        union: vec![0; n],
    };
    search.run(0, 0, 0);
    search.union
}

/// Solves `puzzle` with the techniques up to `max_tier`. Returns the
/// solution and the hardest tier it needed, or `None` if those techniques
/// are not enough or the puzzle has no solution. A solution found this way
/// is the only one, since every deduction holds for all solutions.
pub fn solve(puzzle: &Puzzle, max_tier: Tier) -> Option<(Vec<u8>, Tier)> {
    let mut solver = Solver::new(puzzle).ok()?;
    solver.propagate(max_tier).ok()?;
    if !solver.is_solved() {
        return None;
    }
    let grid = solver.grid();
    puzzle
        .is_solved_by(&grid)
        .then_some((grid, solver.tier_used))
}

/// The heights each cell can hold from the clue rules and the given cells
/// alone, as bitmasks with bit `h` set for height `h`. `None` when they
/// contradict each other.
pub fn initial_candidates(puzzle: &Puzzle) -> Option<Vec<u16>> {
    let mut solver = Solver::new(puzzle).ok()?;
    solver.naked_singles().ok()?;
    Some(solver.candidates)
}

/// Finds up to `limit` solutions of `puzzle` with a backtracking search.
pub fn find_solutions(puzzle: &Puzzle, limit: usize) -> Vec<Vec<u8>> {
    fn search(solver: Solver, limit: usize, found: &mut Vec<Vec<u8>>) {
        let mut solver = solver;
        if solver.propagate(Tier::Medium).is_err() {
            return;
        }
        if solver.is_solved() {
            let grid = solver.grid();
            if solver.puzzle.is_solved_by(&grid) {
                found.push(grid);
            }
            return;
        }
        // Branch on the cell with the fewest candidates.
        let (i, mask) = solver
            .candidates
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, m)| m.count_ones() > 1)
            .min_by_key(|(_, m)| m.count_ones())
            .unwrap();
        for h in 1..=solver.n as u8 {
            if mask & bit(h) == 0 {
                continue;
            }
            let mut branch = solver.clone();
            branch.candidates[i] = bit(h);
            search(branch, limit, found);
            if found.len() >= limit {
                return;
            }
        }
    }

    let mut found = Vec::new();
    if let Ok(solver) = Solver::new(puzzle) {
        search(solver, limit, &mut found);
    }
    found
}

/// Counts the solutions of `puzzle`, stopping at `limit`.
pub fn count_solutions(puzzle: &Puzzle, limit: usize) -> usize {
    find_solutions(puzzle, limit).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLUTION: [u8; 16] = [
        1, 2, 3, 4, //
        2, 3, 4, 1, //
        4, 1, 2, 3, //
        3, 4, 1, 2, //
    ];

    /// A 7×7 Latin square that is not a shifted copy of its first row.
    const SOLUTION_7: [u8; 49] = [
        3, 6, 1, 4, 7, 2, 5, //
        1, 4, 7, 2, 5, 3, 6, //
        7, 2, 5, 3, 6, 1, 4, //
        5, 3, 6, 1, 4, 7, 2, //
        6, 1, 4, 7, 2, 5, 3, //
        4, 7, 2, 5, 3, 6, 1, //
        2, 5, 3, 6, 1, 4, 7, //
    ];

    fn mask(heights: &[u8]) -> u16 {
        heights.iter().map(|h| bit(*h)).fold(0, |a, b| a | b)
    }

    fn givens_puzzle(n: usize, solution: &[u8], skip: &[(usize, usize)]) -> Puzzle {
        let mut puzzle = Puzzle::empty(n).unwrap();
        for row in 0..n {
            for col in 0..n {
                if !skip.contains(&(row, col)) {
                    puzzle.set_given(row, col, Some(solution[row * n + col]));
                }
            }
        }
        puzzle
    }

    #[test]
    fn clue_rules() {
        let mut puzzle = Puzzle::empty(4).unwrap();
        puzzle.set_clue(Side::Left, 0, Some(1));
        puzzle.set_clue(Side::Top, 1, Some(4));
        puzzle.set_clue(Side::Right, 2, Some(3));
        puzzle.set_clue(Side::Bottom, 3, Some(2));
        let solver = Solver::new(&puzzle).unwrap();
        let c = |row: usize, col: usize| solver.candidates[row * 4 + col];
        // A clue of 1 puts the 4 next to it.
        assert_eq!(c(0, 0), mask(&[4]));
        // A clue of 4 makes the column climb from the top.
        assert_eq!(c(0, 1), mask(&[1]));
        assert_eq!(c(3, 1), mask(&[4]));
        // A clue of 3 from the right: at most 2, then at most 3.
        assert_eq!(c(2, 3), mask(&[1, 2]));
        assert_eq!(c(2, 2), mask(&[1, 2, 3]));
        assert_eq!(c(2, 0), mask(&[1, 2, 3, 4]));
        // A clue of 2 cannot have the 4 next to it.
        assert_eq!(c(3, 3), mask(&[1, 2, 3]));
    }

    #[test]
    fn initial_candidates_use_clues_and_givens() {
        let mut puzzle = Puzzle::empty(4).unwrap();
        puzzle.set_clue(Side::Left, 0, Some(1));
        puzzle.set_given(1, 1, Some(2));
        let candidates = initial_candidates(&puzzle).unwrap();
        assert_eq!(candidates[0], mask(&[4]));
        // The 4 next to the clue rules it out from the rest of row 0.
        assert_eq!(candidates[1], mask(&[1, 3]));
        assert_eq!(candidates[5], mask(&[2]));
        // The given 2 is ruled out from its row and column.
        assert_eq!(candidates[4], mask(&[1, 3]));
        assert_eq!(candidates[9], mask(&[1, 3, 4]));
        assert_eq!(candidates[15], full_mask(4));
    }

    #[test]
    fn hidden_single() {
        let puzzle = Puzzle::empty(4).unwrap();
        let mut solver = Solver::new(&puzzle).unwrap();
        // Only cell (0, 2) of the first row can still hold a 3.
        for col in [0, 1, 3] {
            solver.candidates[col] = mask(&[1, 2, 4]);
        }
        assert_eq!(solver.naked_singles(), Ok(false));
        assert_eq!(solver.hidden_singles(), Ok(true));
        assert_eq!(solver.candidates[2], mask(&[3]));
    }

    #[test]
    fn line_arrangements_use_both_clues() {
        // With 3 visible from the left and 2 from the right, the 4 is third
        // and the first two cells climb: 1 2 4 3, 1 3 4 2 or 2 3 4 1.
        let union = arrangements_union(4, &[full_mask(4); 4], Some(3), Some(2));
        assert_eq!(
            union,
            [mask(&[1, 2]), mask(&[2, 3]), mask(&[4]), mask(&[1, 2, 3])]
        );
        let none = arrangements_union(4, &[mask(&[4]), 30, 30, 30], Some(2), None);
        assert_eq!(none, [0; 4]);
    }

    #[test]
    fn easy_puzzle_needs_only_singles() {
        let puzzle = givens_puzzle(4, &SOLUTION, &[(0, 0), (1, 1), (2, 2), (3, 3)]);
        assert_eq!(
            solve(&puzzle, Tier::Easy),
            Some((SOLUTION.to_vec(), Tier::Easy))
        );
        assert_eq!(count_solutions(&puzzle, 2), 1);
    }

    #[test]
    fn several_solutions() {
        // Heights 1 and 3 can be swapped in rows 0 and 3, columns 0 and 2.
        let puzzle = givens_puzzle(4, &SOLUTION, &[(0, 0), (0, 2), (3, 0), (3, 2)]);
        assert_eq!(count_solutions(&puzzle, 2), 2);
        assert_eq!(solve(&puzzle, Tier::VeryHard), None);
        assert_eq!(count_solutions(&Puzzle::empty(5).unwrap(), 2), 2);
    }

    #[test]
    fn no_solution() {
        let mut puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        puzzle.set_clue(Side::Top, 0, Some(1));
        puzzle.set_clue(Side::Bottom, 0, Some(1));
        assert_eq!(count_solutions(&puzzle, 2), 0);
        assert_eq!(solve(&puzzle, Tier::VeryHard), None);
    }

    #[test]
    fn solutions_found_by_logic_are_unique() {
        for (n, solution) in [(4, &SOLUTION[..]), (7, &SOLUTION_7[..])] {
            let puzzle = Puzzle::from_solution(n, solution).unwrap();
            let count = count_solutions(&puzzle, 2);
            assert!(count >= 1);
            if let Some((grid, _)) = solve(&puzzle, Tier::VeryHard) {
                assert_eq!(grid, solution);
                assert_eq!(count, 1);
            }
        }
    }

    #[test]
    fn harder_tiers_solve_at_least_as_much() {
        let puzzle = Puzzle::from_solution(7, &SOLUTION_7).unwrap();
        let mut solved = false;
        for tier in Tier::ALL {
            match solve(&puzzle, tier) {
                Some((grid, used)) => {
                    assert_eq!(grid, SOLUTION_7);
                    assert!(used <= tier);
                    solved = true;
                }
                None => assert!(!solved, "{tier:?} failed after an easier tier worked"),
            }
        }
    }
}
