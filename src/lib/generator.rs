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

//! Generates skyscrapers puzzles of a given size and difficulty.
//!
//! The generator fills a random Latin square, starts from the puzzle that
//! shows every clue and every cell, and tries to remove the cells and then
//! the clues in random order. A removal is kept when the solver, limited to
//! the target tier, can still solve the puzzle. Easy puzzles keep every
//! clue. The result is accepted when the next easier tier cannot solve it,
//! so every puzzle has some cells given only where its tier needs them.

use fastrand::Rng;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::lib::puzzle::{MAX_SIZE, MIN_SIZE, Puzzle, Side, Tier, bit, full_mask};
use crate::lib::solver::solve;

/// A random N×N Latin square with heights 1..=n, row-major.
pub fn random_latin_square(n: usize, rng: &mut Rng) -> Vec<u8> {
    fn fill(n: usize, grid: &mut [u8], i: usize, rng: &mut Rng) -> bool {
        if i == n * n {
            return true;
        }
        let (row, col) = (i / n, i % n);
        let mut used = 0u16;
        for k in 0..col {
            used |= bit(grid[row * n + k]);
        }
        for k in 0..row {
            used |= bit(grid[k * n + col]);
        }
        let mut heights: Vec<u8> = (1..=n as u8)
            .filter(|h| full_mask(n) & !used & bit(*h) != 0)
            .collect();
        rng.shuffle(&mut heights);
        for h in heights {
            grid[i] = h;
            if fill(n, grid, i + 1, rng) {
                return true;
            }
        }
        grid[i] = 0;
        false
    }

    let mut grid = vec![0; n * n];
    let filled = fill(n, &mut grid, 0, rng);
    debug_assert!(filled);
    grid
}

#[derive(Clone, Copy)]
enum Item {
    Cell(usize, usize),
    Clue(Side, usize),
}

/// One attempt at a puzzle of exactly `tier`, from a new random solution.
fn attempt(n: usize, tier: Tier, rng: &mut Rng) -> Option<(Puzzle, Vec<u8>)> {
    let solution = random_latin_square(n, rng);
    let mut puzzle = Puzzle::from_solution(n, &solution).ok()?;
    let mut cells = Vec::with_capacity(n * n);
    for row in 0..n {
        for col in 0..n {
            puzzle.set_given(row, col, Some(solution[row * n + col]));
            cells.push(Item::Cell(row, col));
        }
    }
    let mut clues: Vec<Item> = Side::ALL
        .into_iter()
        .flat_map(|side| (0..n).map(move |i| Item::Clue(side, i)))
        .collect();
    rng.shuffle(&mut cells);
    rng.shuffle(&mut clues);
    // Easy puzzles keep every clue.
    if tier == Tier::Easy {
        clues.clear();
    }

    for item in cells.into_iter().chain(clues) {
        let mut candidate = puzzle.clone();
        match item {
            Item::Cell(row, col) => candidate.set_given(row, col, None),
            Item::Clue(side, i) => candidate.set_clue(side, i, None),
        }
        if solve(&candidate, tier).is_some() {
            puzzle = candidate;
        }
    }

    if let Some(easier) = tier.easier()
        && solve(&puzzle, easier).is_some()
    {
        return None;
    }
    Some((puzzle, solution))
}

/// Whether puzzles of size `n` and `tier` can be generated in reasonable
/// time: 4×4 grids almost never need the very hard techniques.
pub fn available(n: usize, tier: Tier) -> bool {
    (MIN_SIZE..=MAX_SIZE).contains(&n) && !(n == 4 && tier == Tier::VeryHard)
}

/// Generates a puzzle of size `n` that needs exactly `tier`, with its only
/// solution. Returns `None` if `cancel` is set before one is found.
pub fn generate(
    n: usize,
    tier: Tier,
    rng: &mut Rng,
    cancel: &AtomicBool,
) -> Option<(Puzzle, Vec<u8>)> {
    while !cancel.load(Ordering::Relaxed) {
        if let Some(result) = attempt(n, tier, rng) {
            return Some(result);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::puzzle::is_latin;
    use crate::lib::solver::count_solutions;
    use std::time::Instant;

    fn check(n: usize, tier: Tier, seed: u64) -> (Puzzle, Vec<u8>) {
        let mut rng = Rng::with_seed(seed);
        let (puzzle, solution) = generate(n, tier, &mut rng, &AtomicBool::new(false)).unwrap();
        assert_eq!(puzzle.size(), n);
        assert!(puzzle.is_solved_by(&solution));
        assert_eq!(
            solve(&puzzle, tier).map(|(grid, _)| grid),
            Some(solution.clone())
        );
        if let Some(easier) = tier.easier() {
            assert_eq!(solve(&puzzle, easier), None);
        }
        assert_eq!(count_solutions(&puzzle, 2), 1);
        (puzzle, solution)
    }

    #[test]
    fn latin_squares() {
        let mut rng = Rng::with_seed(1);
        for n in MIN_SIZE..=MAX_SIZE {
            assert!(is_latin(n, &random_latin_square(n, &mut rng)));
        }
    }

    #[test]
    fn small_puzzles() {
        for (i, tier) in Tier::ALL.into_iter().enumerate() {
            for n in [4, 5] {
                if available(n, tier) {
                    check(n, tier, i as u64);
                }
            }
        }
    }

    #[test]
    fn easy_puzzles_keep_every_clue() {
        let (puzzle, _) = check(6, Tier::Easy, 3);
        assert_eq!(puzzle.clue_count(), 4 * 6);
    }

    #[test]
    fn seeded_generation_repeats() {
        assert_eq!(check(5, Tier::Medium, 42), check(5, Tier::Medium, 42));
    }

    #[test]
    fn cancel() {
        let mut rng = Rng::with_seed(1);
        assert_eq!(
            generate(6, Tier::VeryHard, &mut rng, &AtomicBool::new(true)),
            None
        );
    }

    /// Generates every available size and difficulty and prints them with
    /// timings. Run with `cargo test --release -- --ignored --nocapture
    /// every_size_and_difficulty`.
    #[test]
    #[ignore = "slow in debug builds"]
    fn every_size_and_difficulty() {
        for n in MIN_SIZE..=MAX_SIZE {
            for tier in Tier::ALL {
                if !available(n, tier) {
                    continue;
                }
                for seed in 0..3 {
                    let start = Instant::now();
                    let (puzzle, _) = check(n, tier, seed);
                    println!(
                        "{n}×{n} {tier:?}: {} ms, {} clues, {} givens\n{}",
                        start.elapsed().as_millis(),
                        puzzle.clue_count(),
                        puzzle.given_count(),
                        puzzle.to_text()
                    );
                }
            }
        }
    }

    /// Measures how often an attempt succeeds for each size and difficulty.
    /// Every combination succeeded in at least one attempt out of ten when
    /// this was written. Run with `cargo test --release --
    /// --ignored --nocapture measure_availability`.
    #[test]
    #[ignore = "only used to tune the generator"]
    fn measure_availability() {
        for n in MIN_SIZE..=MAX_SIZE {
            for tier in Tier::ALL {
                let mut rng = Rng::with_seed(7);
                if std::env::var("MEASURE_TIER").is_ok_and(|t| t != format!("{tier:?}")) {
                    continue;
                }
                let start = Instant::now();
                let attempts = 20;
                let results: Vec<_> = (0..attempts)
                    .filter_map(|_| attempt(n, tier, &mut rng))
                    .collect();
                let found = results.len().max(1) as f64;
                let clues: usize = results.iter().map(|(p, _)| p.clue_count()).sum();
                let givens: usize = results.iter().map(|(p, _)| p.given_count()).sum();
                println!(
                    "{n}×{n} {tier:?}: {}/{attempts} in {} ms, {:.1} clues, {:.1} givens",
                    results.len(),
                    start.elapsed().as_millis(),
                    clues as f64 / found,
                    givens as f64 / found,
                );
            }
        }
    }
}
