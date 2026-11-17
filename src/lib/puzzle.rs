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

//! The skyscrapers puzzle: an N×N grid of building heights from 1 to N,
//! the clues around its edges and the cells given at the start. This module
//! has no GTK dependency.

use std::fmt;

pub const MIN_SIZE: usize = 4;
pub const MAX_SIZE: usize = 7;

/// The edge of the grid a clue is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The side on the other end of the same lines.
    pub fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// How hard a puzzle is, from the techniques needed to solve it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Easy,
    Medium,
    Hard,
    VeryHard,
}

impl Tier {
    pub const ALL: [Tier; 4] = [Tier::Easy, Tier::Medium, Tier::Hard, Tier::VeryHard];

    /// The next easier tier, if any.
    pub fn easier(self) -> Option<Tier> {
        match self {
            Tier::Easy => None,
            Tier::Medium => Some(Tier::Easy),
            Tier::Hard => Some(Tier::Medium),
            Tier::VeryHard => Some(Tier::Hard),
        }
    }

    fn to_char(self) -> char {
        match self {
            Tier::Easy => 'e',
            Tier::Medium => 'm',
            Tier::Hard => 'h',
            Tier::VeryHard => 'v',
        }
    }

    fn from_char(c: char) -> Option<Option<Tier>> {
        match c {
            'e' => Some(Some(Tier::Easy)),
            'm' => Some(Some(Tier::Medium)),
            'h' => Some(Some(Tier::Hard)),
            'v' => Some(Some(Tier::VeryHard)),
            'u' => Some(None),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PuzzleError {
    Size,
    Clue,
    Given,
    ShareCode,
    Text,
}

impl fmt::Display for PuzzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PuzzleError::Size => "Invalid puzzle size",
            PuzzleError::Clue => "Invalid clue",
            PuzzleError::Given => "Invalid given cell",
            PuzzleError::ShareCode => "Invalid shared puzzle code",
            PuzzleError::Text => "Invalid puzzle text",
        };
        f.write_str(s)
    }
}

/// The cells of the line that the clue at `index` on `side` looks along,
/// as (row, col) pairs, nearest to the clue first.
pub fn line_coords(n: usize, side: Side, index: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..n).map(move |d| match side {
        Side::Top => (d, index),
        Side::Bottom => (n - 1 - d, index),
        Side::Left => (index, d),
        Side::Right => (index, n - 1 - d),
    })
}

/// How many buildings are visible looking along `heights`, nearest first:
/// a building is visible when it is taller than every building before it.
pub fn visible(heights: impl IntoIterator<Item = u8>) -> u8 {
    let mut tallest = 0;
    let mut count = 0;
    for h in heights {
        if h > tallest {
            tallest = h;
            count += 1;
        }
    }
    count
}

/// Whether `grid` (row-major, heights 1..=n) holds every height exactly
/// once in each row and each column.
pub fn is_latin(n: usize, grid: &[u8]) -> bool {
    if grid.len() != n * n {
        return false;
    }
    let full = full_mask(n);
    (0..n).all(|i| {
        let mut row = 0u16;
        let mut col = 0u16;
        for j in 0..n {
            row |= bit(grid[i * n + j]);
            col |= bit(grid[j * n + i]);
        }
        row == full && col == full
    })
}

/// All the clues of a solved grid, indexed by `Side::index()`.
pub fn clues_from_solution(n: usize, solution: &[u8]) -> [Vec<u8>; 4] {
    Side::ALL.map(|side| {
        (0..n)
            .map(|i| visible(line_coords(n, side, i).map(|(r, c)| solution[r * n + c])))
            .collect()
    })
}

pub(crate) fn bit(h: u8) -> u16 {
    1 << h
}

/// The bitmask with the bits of heights 1..=n set.
pub(crate) fn full_mask(n: usize) -> u16 {
    ((1u16 << n) - 1) << 1
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Puzzle {
    n: usize,
    /// Indexed by `Side::index()`, then by row or column.
    clues: [Vec<Option<u8>>; 4],
    /// Row-major.
    givens: Vec<Option<u8>>,
}

impl Puzzle {
    /// A puzzle of size `n` with no clues and no given cells.
    pub fn empty(n: usize) -> Result<Self, PuzzleError> {
        if !(MIN_SIZE..=MAX_SIZE).contains(&n) {
            return Err(PuzzleError::Size);
        }
        Ok(Self {
            n,
            clues: std::array::from_fn(|_| vec![None; n]),
            givens: vec![None; n * n],
        })
    }

    /// The puzzle with every clue of `solution` and no given cells.
    pub fn from_solution(n: usize, solution: &[u8]) -> Result<Self, PuzzleError> {
        let mut puzzle = Self::empty(n)?;
        if !is_latin(n, solution) {
            return Err(PuzzleError::Given);
        }
        let clues = clues_from_solution(n, solution);
        for side in Side::ALL {
            for i in 0..n {
                puzzle.set_clue(side, i, Some(clues[side.index()][i]));
            }
        }
        Ok(puzzle)
    }

    pub fn size(&self) -> usize {
        self.n
    }

    pub fn clue(&self, side: Side, index: usize) -> Option<u8> {
        self.clues[side.index()][index]
    }

    pub fn set_clue(&mut self, side: Side, index: usize, clue: Option<u8>) {
        self.clues[side.index()][index] = clue;
    }

    pub fn given(&self, row: usize, col: usize) -> Option<u8> {
        self.givens[row * self.n + col]
    }

    pub fn set_given(&mut self, row: usize, col: usize, height: Option<u8>) {
        self.givens[row * self.n + col] = height;
    }

    pub fn givens(&self) -> &[Option<u8>] {
        &self.givens
    }

    pub fn clue_count(&self) -> usize {
        self.clues.iter().flatten().filter(|c| c.is_some()).count()
    }

    pub fn given_count(&self) -> usize {
        self.givens.iter().filter(|g| g.is_some()).count()
    }

    /// Checks the clues and given cells are in range and that no height is
    /// given twice in a row or column. It does not check the puzzle can be
    /// solved.
    pub fn validate(&self) -> Result<(), PuzzleError> {
        let n = self.n;
        if !(MIN_SIZE..=MAX_SIZE).contains(&n) {
            return Err(PuzzleError::Size);
        }
        let in_range = |v: &u8| (1..=n as u8).contains(v);
        if self.clues.iter().any(|side| side.len() != n)
            || !self.clues.iter().flatten().flatten().all(in_range)
        {
            return Err(PuzzleError::Clue);
        }
        if self.givens.len() != n * n || !self.givens.iter().flatten().all(in_range) {
            return Err(PuzzleError::Given);
        }
        for i in 0..n {
            let mut row = 0u16;
            let mut col = 0u16;
            for j in 0..n {
                for (mask, given) in [(&mut row, self.given(i, j)), (&mut col, self.given(j, i))] {
                    if let Some(h) = given {
                        if *mask & bit(h) != 0 {
                            return Err(PuzzleError::Given);
                        }
                        *mask |= bit(h);
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether `grid` is a solution: a Latin square that matches every
    /// clue and given cell.
    pub fn is_solved_by(&self, grid: &[u8]) -> bool {
        let n = self.n;
        if !is_latin(n, grid) {
            return false;
        }
        let givens_match = self
            .givens
            .iter()
            .zip(grid)
            .all(|(given, h)| given.is_none_or(|g| g == *h));
        let clues = clues_from_solution(n, grid);
        let clues_match = Side::ALL.iter().all(|side| {
            (0..n).all(|i| {
                self.clue(*side, i)
                    .is_none_or(|c| c == clues[side.index()][i])
            })
        });
        givens_match && clues_match
    }

    /// A short code to share the puzzle: `#R`, the size, the difficulty
    /// (`e`, `m`, `h`, `v`, or `u` for unknown), the top, bottom, left and
    /// right clues, then the cells row by row. Missing clues and empty cells
    /// are `0`.
    pub fn to_share_code(&self, tier: Option<Tier>) -> String {
        let digit = |v: Option<u8>| char::from(b'0' + v.unwrap_or(0));
        let mut code = String::from("#R");
        code.push(char::from(b'0' + self.n as u8));
        code.push(tier.map_or('u', Tier::to_char));
        code.extend(self.clues.iter().flatten().map(|c| digit(*c)));
        code.extend(self.givens.iter().map(|g| digit(*g)));
        code
    }

    /// Parses a code made by `to_share_code`. Surrounding whitespace is
    /// ignored.
    pub fn from_share_code(code: &str) -> Result<(Self, Option<Tier>), PuzzleError> {
        let e = PuzzleError::ShareCode;
        let rest = code.trim().strip_prefix("#R").ok_or(e)?;
        let mut chars = rest.chars();
        let n = chars.next().and_then(|c| c.to_digit(10)).ok_or(e)? as usize;
        let tier = chars.next().and_then(Tier::from_char).ok_or(e)?;
        let mut puzzle = Self::empty(n).map_err(|_| e)?;
        let values: Vec<Option<u8>> = chars
            .map(|c| match c.to_digit(10) {
                Some(0) => Ok(None),
                Some(d) => Ok(Some(d as u8)),
                None => Err(e),
            })
            .collect::<Result<_, _>>()?;
        if values.len() != 4 * n + n * n {
            return Err(e);
        }
        for (k, value) in values[..4 * n].iter().enumerate() {
            puzzle.set_clue(Side::ALL[k / n], k % n, *value);
        }
        puzzle.givens.copy_from_slice(&values[4 * n..]);
        puzzle.validate().map_err(|_| e)?;
        Ok((puzzle, tier))
    }

    /// The puzzle as text: the grid surrounded by its clues, `.` for missing
    /// clues and empty cells and `+` in the corners. For example:
    ///
    /// ```text
    /// +.21.+
    /// 2....1
    /// .3...2
    /// ......
    /// 1.....
    /// +1.2.+
    /// ```
    pub fn to_text(&self) -> String {
        let n = self.n;
        let digit = |v: Option<u8>| v.map_or('.', |v| char::from(b'0' + v));
        let mut text = String::new();
        let edge = |text: &mut String, side: Side| {
            text.push('+');
            text.extend((0..n).map(|i| digit(self.clue(side, i))));
            text.push_str("+\n");
        };
        edge(&mut text, Side::Top);
        for row in 0..n {
            text.push(digit(self.clue(Side::Left, row)));
            text.extend((0..n).map(|col| digit(self.given(row, col))));
            text.push(digit(self.clue(Side::Right, row)));
            text.push('\n');
        }
        edge(&mut text, Side::Bottom);
        text
    }

    /// Parses text made by `to_text`. Blank lines and trailing whitespace are
    /// ignored, and any character, including a space, is accepted in the
    /// corners.
    pub fn from_text(text: &str) -> Result<Self, PuzzleError> {
        let e = PuzzleError::Text;
        let lines: Vec<Vec<char>> = text
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .map(|l| l.chars().collect())
            .collect();
        let mut lines = lines;
        let n = lines.len().checked_sub(2).ok_or(e)?;
        let mut puzzle = Self::empty(n).map_err(|_| e)?;
        // A space in the last corner was trimmed with the trailing whitespace.
        for i in [0, n + 1] {
            if lines[i].len() == n + 1 {
                lines[i].push(' ');
            }
        }
        if lines.iter().any(|l| l.len() != n + 2) {
            return Err(e);
        }
        let value = |c: char| match c {
            '.' => Ok(None),
            c => match c.to_digit(10) {
                Some(d) if d > 0 => Ok(Some(d as u8)),
                _ => Err(e),
            },
        };
        for i in 0..n {
            puzzle.set_clue(Side::Top, i, value(lines[0][i + 1])?);
            puzzle.set_clue(Side::Bottom, i, value(lines[n + 1][i + 1])?);
            puzzle.set_clue(Side::Left, i, value(lines[i + 1][0])?);
            puzzle.set_clue(Side::Right, i, value(lines[i + 1][n + 1])?);
            for col in 0..n {
                puzzle.set_given(i, col, value(lines[i + 1][col + 1])?);
            }
        }
        puzzle.validate().map_err(|_| e)?;
        Ok(puzzle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×4 solution and its clues, worked out by hand.
    const SOLUTION: [u8; 16] = [
        1, 2, 3, 4, //
        2, 3, 4, 1, //
        4, 1, 2, 3, //
        3, 4, 1, 2, //
    ];

    #[test]
    fn visibility() {
        assert_eq!(visible([1, 2, 3, 4]), 4);
        assert_eq!(visible([4, 3, 2, 1]), 1);
        assert_eq!(visible([2, 1, 4, 3]), 2);
        assert_eq!(visible([3, 1, 2, 5, 4]), 2);
        assert_eq!(visible([1, 3, 2, 5, 4, 6]), 4);
        assert_eq!(visible([]), 0);
    }

    #[test]
    fn line_coords_start_at_the_clue() {
        let top: Vec<_> = line_coords(4, Side::Top, 1).collect();
        assert_eq!(top, [(0, 1), (1, 1), (2, 1), (3, 1)]);
        let bottom: Vec<_> = line_coords(4, Side::Bottom, 1).collect();
        assert_eq!(bottom, [(3, 1), (2, 1), (1, 1), (0, 1)]);
        let left: Vec<_> = line_coords(4, Side::Left, 2).collect();
        assert_eq!(left, [(2, 0), (2, 1), (2, 2), (2, 3)]);
        let right: Vec<_> = line_coords(4, Side::Right, 2).collect();
        assert_eq!(right, [(2, 3), (2, 2), (2, 1), (2, 0)]);
    }

    #[test]
    fn clues_of_a_known_solution() {
        let clues = clues_from_solution(4, &SOLUTION);
        assert_eq!(clues[Side::Top.index()], [3, 3, 2, 1]);
        assert_eq!(clues[Side::Bottom.index()], [2, 1, 3, 3]);
        assert_eq!(clues[Side::Left.index()], [4, 3, 1, 2]);
        assert_eq!(clues[Side::Right.index()], [1, 2, 2, 2]);
    }

    #[test]
    fn latin_squares() {
        assert!(is_latin(4, &SOLUTION));
        let mut broken = SOLUTION;
        broken.swap(0, 1);
        assert!(!is_latin(4, &broken));
        assert!(!is_latin(4, &SOLUTION[..15]));
    }

    #[test]
    fn solved_by() {
        let mut puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        assert!(puzzle.is_solved_by(&SOLUTION));
        puzzle.set_given(0, 0, Some(2));
        assert!(!puzzle.is_solved_by(&SOLUTION));
    }

    #[test]
    fn validate() {
        assert_eq!(Puzzle::empty(3), Err(PuzzleError::Size));
        assert_eq!(Puzzle::empty(8), Err(PuzzleError::Size));
        let mut puzzle = Puzzle::empty(4).unwrap();
        assert_eq!(puzzle.validate(), Ok(()));
        puzzle.set_clue(Side::Left, 0, Some(5));
        assert_eq!(puzzle.validate(), Err(PuzzleError::Clue));
        puzzle.set_clue(Side::Left, 0, Some(4));
        puzzle.set_given(1, 0, Some(3));
        puzzle.set_given(1, 3, Some(3));
        assert_eq!(puzzle.validate(), Err(PuzzleError::Given));
        puzzle.set_given(1, 3, None);
        puzzle.set_given(2, 0, Some(3));
        assert_eq!(puzzle.validate(), Err(PuzzleError::Given));
    }

    fn sample() -> Puzzle {
        let mut puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        puzzle.set_clue(Side::Top, 0, None);
        puzzle.set_clue(Side::Right, 3, None);
        puzzle.set_given(2, 1, Some(1));
        puzzle
    }

    #[test]
    fn share_code_round_trip() {
        let puzzle = sample();
        let code = puzzle.to_share_code(Some(Tier::Hard));
        assert_eq!(code, "#R4h03212133431212200000000001000000");
        assert_eq!(
            Puzzle::from_share_code(&code),
            Ok((puzzle.clone(), Some(Tier::Hard)))
        );
        let unknown = puzzle.to_share_code(None);
        assert_eq!(
            Puzzle::from_share_code(&format!("  {unknown}\n")),
            Ok((puzzle, None))
        );
    }

    #[test]
    fn bad_share_codes() {
        let code = sample().to_share_code(Some(Tier::Easy));
        let bad = |c: &str| Puzzle::from_share_code(c) == Err(PuzzleError::ShareCode);
        assert!(bad(""));
        assert!(bad(&code.replacen("#R", "#S", 1)));
        assert!(bad(&code[1..]));
        assert!(bad(&code[..code.len() - 1]));
        assert!(bad(&format!("{code}0")));
        assert!(bad(&code.replacen("#R4", "#R3", 1)));
        assert!(bad(&code.replacen("#R4e", "#R4x", 1)));
        // A clue of 5 on a 4×4 grid.
        assert!(bad(&code.replacen("#R4e0", "#R4e5", 1)));
        // A non-digit cell.
        assert!(bad(&format!("{}a", &code[..code.len() - 1])));
        // Height 1 given twice in a row.
        let mut twice = code.clone();
        let cells = 4 + 4 * 4;
        twice.replace_range(cells + 2 * 4 + 2..cells + 2 * 4 + 3, "1");
        assert!(bad(&twice));
    }

    #[test]
    fn text_round_trip() {
        let puzzle = sample();
        let text = puzzle.to_text();
        assert_eq!(text, "+.321+\n4....1\n3....2\n1.1..2\n2.....\n+2133+\n");
        assert_eq!(Puzzle::from_text(&text), Ok(puzzle.clone()));
        let spaced = format!("\n{}\n\n", text.replace('+', " ").replace('\n', "  \n"));
        assert_eq!(Puzzle::from_text(&spaced), Ok(puzzle));
    }

    #[test]
    fn bad_text() {
        let text = sample().to_text();
        let bad = |t: &str| Puzzle::from_text(t) == Err(PuzzleError::Text);
        assert!(bad(""));
        assert!(bad("+..+\n....\n+..+\n"));
        assert!(bad(&text.replacen("4....1", "4...1", 1)));
        assert!(bad(&text.replacen("4....1", "4..x.1", 1)));
        assert!(bad(&text.replacen("4....1", "4.0..1", 1)));
        assert!(bad(&text.replacen("4....1", "5....1", 1)));
        assert!(bad(&text.replacen("4....1", "4.1..1", 1)));
    }
}
