/* -*- Mode: rust; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/*
 * Copyright © 2026 Johan GAY
 * Copyright © 2026 Ignacio Casal Quinteiro
 *
 * This file is part of Rascacielos, based on GNOME Sudoku.
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

use core::fmt;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
};

use crate::lib::enums::Coord;
use crate::lib::enums::DifficultyCategory;
use crate::lib::puzzle::{ClueConflict, Puzzle, PuzzleError, Side, clue_conflict, line_coords};

//You should never be interacting with SudokuBoard directly
//everything goes through SudokuGame and SudokuBackend
//
//Values and earmarks are 0-based: index h holds the building of height h + 1.
#[derive(Clone, Debug)]
pub struct SudokuBoard {
    puzzle: Puzzle,
    pub cells: Vec<Vec<SudokuBoardCell>>,
    pub total_fixed: usize,
    total_earmarks: Cell<usize>,
    pub total_filled: Cell<usize>,
    pub broken_coords: RefCell<HashSet<Coord>>,
    /// How many times each value is in each row and column, indexed by
    /// value then by row or column.
    in_row: Vec<Vec<Cell<usize>>>,
    in_col: Vec<Vec<Cell<usize>>>,

    pub difficulty_category: DifficultyCategory,
    pub loaded_time: f64,
}

#[derive(Default, Clone, Debug)]
pub struct SudokuBoardCell {
    pub value: Cell<Option<usize>>,
    pub earmarks: Vec<Cell<bool>>,

    pub solution: usize,
    pub fixed: bool,
    /// The cells in the same row or column, this one included.
    pub aligned_coords: Vec<Coord>,
}

impl SudokuBoard {
    /// An empty board for `puzzle`, with its given cells filled and fixed.
    pub fn new(puzzle: Puzzle) -> Self {
        let n = puzzle.size();
        let cells = (0..n)
            .map(|row| {
                (0..n)
                    .map(|col| SudokuBoardCell {
                        earmarks: vec![Cell::new(false); n],
                        aligned_coords: (0..n)
                            .map(|c| Coord { row, col: c })
                            .chain((0..n).filter(|r| *r != row).map(|r| Coord { row: r, col }))
                            .collect(),
                        ..Default::default()
                    })
                    .collect()
            })
            .collect();
        let counts = || (0..n).map(|_| vec![Cell::new(0); n]).collect();

        let mut board = Self {
            puzzle,
            cells,
            total_fixed: 0,
            total_earmarks: Default::default(),
            total_filled: Default::default(),
            broken_coords: Default::default(),
            in_row: counts(),
            in_col: counts(),
            difficulty_category: Default::default(),
            loaded_time: Default::default(),
        };
        for row in 0..n {
            for col in 0..n {
                let given = board.puzzle.given(row, col);
                board.init_cell(row, col, given.map(|h| h as usize - 1));
            }
        }
        board
    }

    pub fn from_generated(puzzle: Puzzle, solution: &[u8], difficulty: DifficultyCategory) -> Self {
        let mut board = Self::new(puzzle);
        board.set_solution(solution);
        board.difficulty_category = difficulty;
        board
    }

    pub fn size(&self) -> usize {
        self.puzzle.size()
    }

    pub fn puzzle(&self) -> &Puzzle {
        &self.puzzle
    }

    pub fn clue(&self, side: Side, index: usize) -> Option<u8> {
        self.puzzle.clue(side, index)
    }

    /// Stores `solution`, row-major with heights 1..=n.
    pub fn set_solution(&mut self, solution: &[u8]) {
        let n = self.size();
        for row in 0..n {
            for col in 0..n {
                self.cells[row][col].solution = solution[row * n + col] as usize - 1;
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        let n = self.size();
        let broken: bool = !self.broken_coords.borrow().is_empty();
        !broken
            && self.total_filled.get() == n * n
            && Side::ALL
                .iter()
                .all(|side| (0..n).all(|i| self.clue_conflict(*side, i).is_none()))
    }

    /// Whether the values entered so far contradict the clue at `index` on
    /// `side`. `None` when there is no clue there.
    pub fn clue_conflict(&self, side: Side, index: usize) -> Option<ClueConflict> {
        let clue = self.clue(side, index)?;
        let line: Vec<Option<u8>> = line_coords(self.size(), side, index)
            .map(|(row, col)| self.cells[row][col].value.get().map(|v| v as u8 + 1))
            .collect();
        clue_conflict(clue, &line)
    }

    pub fn is_empty(&self) -> bool {
        self.total_filled.get() == self.total_fixed && self.total_earmarks.get() == 0
    }

    pub fn cell(&self, pos: Coord) -> &SudokuBoardCell {
        &self.cells[pos.row][pos.col]
    }

    pub fn enable_earmark(&self, pos: Coord, num: usize) {
        self.cells[pos.row][pos.col].earmarks[num].set(true);
        self.total_earmarks.update(|x| x + 1);
    }

    pub fn disable_earmark(&self, pos: Coord, num: usize) {
        self.cells[pos.row][pos.col].earmarks[num].set(false);
        self.total_earmarks.update(|x| x - 1);
    }

    pub fn disable_all_earmarks(&self, pos: Coord) {
        for num in 0..self.size() {
            if self.cell(pos).earmarks[num].get() {
                self.disable_earmark(pos, num);
            }
        }
    }

    pub fn remove(&self, pos: Coord) {
        let old_val = self.cells[pos.row][pos.col].value.get().unwrap();
        self.remove_from_occurrences(pos.row, pos.col, old_val);
        self.remove_breakages_for(pos.row, pos.col, old_val);
        self.total_filled.update(|x| x - 1);
        self.cells[pos.row][pos.col].value.set(None);
    }

    pub fn set_value(&self, pos: Coord, new_val: Option<usize>) {
        if let Some(new_val) = new_val {
            self.insert(pos, new_val);
        } else {
            self.remove(pos);
        }
    }

    pub fn insert(&self, pos: Coord, new_val: usize) {
        let cell = &self.cell(pos);
        let old_val = cell.value.get();
        if let Some(old_val) = old_val {
            self.remove_from_occurrences(pos.row, pos.col, old_val);
            self.remove_breakages_for(pos.row, pos.col, old_val);
        } else {
            self.total_filled.update(|x| x + 1);
        }
        cell.value.set(Some(new_val));
        self.add_to_occurrences(pos.row, pos.col, new_val);
        self.mark_breakages(pos.row, pos.col, new_val);
    }

    fn init_cell(&mut self, row: usize, col: usize, val: Option<usize>) {
        if let Some(value) = val {
            self.insert(Coord { row, col }, value);
            self.cells[row][col].fixed = true;
            self.total_fixed += 1;
        }
    }

    fn mark_breakages(&self, row: usize, col: usize, val: usize) {
        let n = self.size();
        if self.in_row[val][row].get() > 1 {
            self.mark_breakages_for((0..n).map(|c| Coord { row, col: c }), val);
        }

        if self.in_col[val][col].get() > 1 {
            self.mark_breakages_for((0..n).map(|r| Coord { row: r, col }), val);
        }
    }

    fn mark_breakages_for(&self, coords: impl Iterator<Item = Coord>, val: usize) {
        for coord in coords {
            if let Some(target) = self.cells[coord.row][coord.col].value.get()
                && target == val
            {
                self.broken_coords.borrow_mut().insert(coord);
            }
        }
    }

    pub fn has_earmarks(&self, pos: Coord) -> bool {
        self.cell(pos).earmarks.iter().any(Cell::get)
    }

    pub fn is_fixed(&self, pos: Coord) -> bool {
        self.cells[pos.row][pos.col].fixed
    }

    pub fn cell_possibilies(&self, pos: Coord) -> Vec<bool> {
        (0..self.size())
            .map(|num| self.is_earmark_possible(pos, num))
            .collect()
    }

    pub fn is_earmark_possible(&self, pos: Coord, num: usize) -> bool {
        self.in_row[num][pos.row].get() == 0 && self.in_col[num][pos.col].get() == 0
    }

    fn remove_breakages_for(&self, row: usize, col: usize, val: usize) {
        let target = &Coord { row, col };
        let mut broken_coords = self.broken_coords.borrow_mut();
        if broken_coords.contains(target) {
            for aligned in &self.cells[row][col].aligned_coords {
                if let Some(target) = self.cells[aligned.row][aligned.col].value.get()
                    && target == val
                    && broken_coords.contains(aligned)
                    && self.in_row[val][aligned.row].get() <= 1
                    && self.in_col[val][aligned.col].get() <= 1
                {
                    broken_coords.remove(aligned);
                }
            }
        }
    }

    fn add_to_occurrences(&self, row: usize, col: usize, val: usize) {
        self.in_row[val][row].update(|x| x + 1);
        self.in_col[val][col].update(|x| x + 1);
    }

    fn remove_from_occurrences(&self, row: usize, col: usize, val: usize) {
        self.in_row[val][row].update(|x| x - 1);
        self.in_col[val][col].update(|x| x - 1);
    }

    pub fn is_earmark_enabled(&self, pos: Coord, num: usize) -> bool {
        self.cell(pos).earmarks[num].get()
    }

    pub fn to_json(&self, elapsed_time: f64) -> String {
        let json_board = JsonBoard::from_sudoku_board(self, elapsed_time);
        serde_json::to_string_pretty(&json_board).unwrap()
    }

    pub fn from_json(path: &str) -> Result<Self, SudokuError> {
        use SudokuError::{self as e};
        let file = std::fs::read_to_string(path)?;
        let b: JsonBoard = serde_json::from_str(file.as_str())?;
        e::true_or_error(b.version == JSON_VERSION)?;
        //-1 is for timer disabled
        e::true_or_error(b.time_elapsed > 0.0 || b.time_elapsed == -1.0)?;
        let n = b.size;
        let mut puzzle = Puzzle::empty(n).map_err(|_| e::InvalidSaveData)?;
        e::true_or_error(b.clues.len() == 4)?;
        for (side, clues) in Side::ALL.into_iter().zip(&b.clues) {
            e::true_or_error(clues.len() == n)?;
            for (i, clue) in clues.iter().enumerate() {
                puzzle.set_clue(side, i, (*clue != 0).then_some(*clue));
            }
        }
        e::true_or_error(b.cells.len() <= n * n)?;
        for cell in b.cells.iter() {
            let [row, col] = cell.position;
            e::true_or_error(row < n && col < n && cell.value <= n)?;
            if cell.fixed {
                e::true_or_error(cell.value != 0)?;
                puzzle.set_given(row, col, Some(cell.value as u8));
            }
        }
        puzzle.validate().map_err(|_| e::InvalidSaveData)?;

        let mut board = Self::new(puzzle);
        board.loaded_time = b.time_elapsed;
        //this is unused as the solver automatically calculates the difficulty
        board.difficulty_category = b.difficulty_category;
        for cell in b.cells.iter() {
            let [row, col] = cell.position;
            let pos = Coord { row, col };
            if cell.value != 0 && !cell.fixed {
                e::true_or_error(!board.is_fixed(pos))?;
                board.insert(pos, cell.value - 1);
            }
            e::true_or_error(cell.earmarks.len() <= n)?;
            for num in &cell.earmarks {
                e::true_or_error(*num >= 1 && *num <= n)?;
                e::true_or_error(!board.is_earmark_enabled(pos, num - 1))?;
                board.enable_earmark(pos, num - 1);
            }
        }
        Ok(board)
    }

    /// Parses the text format of `Puzzle::to_text`.
    pub fn from_string(s: &str) -> Result<Self, SudokuError> {
        Ok(Self::new(Puzzle::from_text(s)?))
    }

    /// The puzzle in the text format of `Puzzle::to_text`.
    pub fn fixed_to_string_pretty(&self) -> String {
        self.puzzle.to_text()
    }

    /// An identifier of the puzzle that can be used as a file name.
    pub fn fixed_to_string(&self) -> String {
        self.puzzle.to_share_code(None).replace('#', "")
    }

    /// A short name for the puzzle, used as the default file name.
    pub fn fixed_to_string_short(&self) -> String {
        let n = self.size();
        format!("{n}x{n}")
    }

    pub fn fixed_to_ascii(&self) -> String {
        self.puzzle
            .to_share_code(self.difficulty_category.to_tier())
    }

    pub fn from_ascii(s: &str) -> Result<SudokuBoard, SudokuError> {
        let (puzzle, _) = Puzzle::from_share_code(s)?;
        Ok(Self::new(puzzle))
    }

    pub fn cells_not_fixed(&self) -> HashSet<Coord> {
        let n = self.size();
        let mut ret: HashSet<Coord> = Default::default();
        for row in 0..n {
            for col in 0..n {
                if !self.cells[row][col].fixed {
                    ret.insert(Coord { row, col });
                }
            }
        }
        ret
    }
}

use serde::{Deserialize, Serialize};

const JSON_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct JsonBoard {
    version: u32,
    size: usize,
    difficulty_category: DifficultyCategory,
    time_elapsed: f64,
    /// Top, bottom, left and right clues, 0 when missing.
    clues: Vec<Vec<u8>>,
    cells: Vec<JsonCell>,
}

#[derive(Serialize, Deserialize)]
struct JsonCell {
    position: [usize; 2],
    value: usize,
    fixed: bool,
    earmarks: Vec<usize>,
}

impl JsonBoard {
    fn from_sudoku_board(board: &SudokuBoard, elapsed_time: f64) -> JsonBoard {
        let n = board.size();
        let mut cells: Vec<_> = Vec::with_capacity(n * n);
        for row in 0..n {
            for col in 0..n {
                let bcell = &board.cells[row][col];
                let earmarks = (0..n)
                    .filter(|num| bcell.earmarks[*num].get())
                    .map(|num| num + 1)
                    .collect();
                cells.push(JsonCell {
                    position: [row, col],
                    value: bcell.value.get().map_or(0, |val| val + 1),
                    fixed: bcell.fixed,
                    earmarks,
                });
            }
        }
        let clues = Side::ALL
            .iter()
            .map(|side| (0..n).map(|i| board.clue(*side, i).unwrap_or(0)).collect())
            .collect();
        JsonBoard {
            version: JSON_VERSION,
            size: n,
            difficulty_category: board.difficulty_category,
            time_elapsed: elapsed_time,
            clues,
            cells,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SudokuError {
    FileNotFound,
    InvalidPuzzle,
    MultipleSolutions,
    InvalidSaveData,
    InvalidSharedPuzzle,
}

impl fmt::Display for SudokuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SudokuError::FileNotFound => write!(f, "Save Not Found"),
            SudokuError::InvalidPuzzle => write!(f, "Invalid Puzzle"),
            SudokuError::InvalidSharedPuzzle => write!(f, "Invalid Shared Puzzle Format"),
            SudokuError::MultipleSolutions => write!(f, "Invalid Puzzle: Multiple solutions"),
            SudokuError::InvalidSaveData => write!(f, "Invalid Saved Puzzle"),
        }
    }
}

impl SudokuError {
    pub fn true_or_error(var: bool) -> Result<bool, SudokuError> {
        if var {
            Ok(var)
        } else {
            Err(SudokuError::InvalidSaveData)
        }
    }
}

impl From<std::io::Error> for SudokuError {
    fn from(_: std::io::Error) -> Self {
        SudokuError::FileNotFound
    }
}

impl From<serde_json::Error> for SudokuError {
    fn from(_: serde_json::Error) -> Self {
        SudokuError::InvalidSaveData
    }
}

impl From<PuzzleError> for SudokuError {
    fn from(e: PuzzleError) -> Self {
        match e {
            PuzzleError::ShareCode => SudokuError::InvalidSharedPuzzle,
            _ => SudokuError::InvalidPuzzle,
        }
    }
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

    fn board() -> SudokuBoard {
        let mut puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        puzzle.set_clue(Side::Top, 2, None);
        puzzle.set_given(0, 0, Some(1));
        SudokuBoard::from_generated(puzzle, &SOLUTION, DifficultyCategory::Medium)
    }

    fn pos(row: usize, col: usize) -> Coord {
        Coord { row, col }
    }

    #[test]
    fn givens_are_fixed() {
        let board = board();
        assert!(board.is_fixed(pos(0, 0)));
        assert_eq!(board.cell(pos(0, 0)).value.get(), Some(0));
        assert_eq!(board.total_fixed, 1);
        assert!(board.is_empty());
        assert_eq!(board.cell(pos(2, 1)).solution, 0);
        assert_eq!(board.cell(pos(0, 0)).aligned_coords.len(), 7);
    }

    #[test]
    fn duplicates_break_rows_and_columns() {
        let board = board();
        board.insert(pos(0, 2), 0);
        let broken = |b: &SudokuBoard| b.broken_coords.borrow().clone();
        assert_eq!(broken(&board), HashSet::from([pos(0, 0), pos(0, 2)]));
        board.insert(pos(3, 2), 0);
        assert_eq!(
            broken(&board),
            HashSet::from([pos(0, 0), pos(0, 2), pos(3, 2)])
        );
        board.remove(pos(0, 2));
        assert!(broken(&board).is_empty());
        assert!(!board.is_earmark_possible(pos(1, 2), 0));
        assert!(board.is_earmark_possible(pos(1, 1), 0));
    }

    #[test]
    fn complete() {
        let board = board();
        for row in 0..4 {
            for col in 0..4 {
                if !board.is_fixed(pos(row, col)) {
                    board.insert(pos(row, col), SOLUTION[row * 4 + col] as usize - 1);
                }
            }
        }
        assert!(board.is_complete());
        board.insert(pos(1, 1), 0);
        assert!(!board.is_complete());
        board.insert(pos(1, 1), 2);
        assert!(board.is_complete());
    }

    #[test]
    fn complete_needs_matching_clues() {
        // Swapping heights 1 and 3 in rows 0 and 3 keeps a Latin square but
        // breaks the clues.
        let puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        let board = SudokuBoard::from_generated(puzzle, &SOLUTION, DifficultyCategory::Medium);
        let mut swapped = SOLUTION;
        for i in [0, 2, 12, 14] {
            swapped[i] = 4 - swapped[i];
        }
        for row in 0..4 {
            for col in 0..4 {
                if !board.is_fixed(pos(row, col)) {
                    board.insert(pos(row, col), swapped[row * 4 + col] as usize - 1);
                }
            }
        }
        assert!(board.broken_coords.borrow().is_empty());
        assert!(!board.is_complete());
    }

    #[test]
    fn clue_conflicts() {
        let board = board();
        // Row 0 is 1 2 3 4 with clues 4 on the left and 1 on the right.
        assert_eq!(board.clue_conflict(Side::Top, 2), None);
        board.insert(pos(0, 3), 2);
        assert_eq!(board.clue_conflict(Side::Right, 0), None);
        board.insert(pos(0, 3), 3);
        board.insert(pos(0, 1), 2);
        board.insert(pos(0, 2), 1);
        assert_eq!(board.clue_conflict(Side::Left, 0), Some(ClueConflict::Full));
        assert_eq!(board.clue_conflict(Side::Right, 0), None);
    }

    #[test]
    fn json_round_trip() {
        let board = board();
        board.insert(pos(1, 1), 2);
        board.enable_earmark(pos(2, 2), 1);
        board.enable_earmark(pos(2, 2), 3);
        let path =
            std::env::temp_dir().join(format!("rascacielos-test-{}.json", std::process::id()));
        std::fs::write(&path, board.to_json(12.0)).unwrap();
        let loaded = SudokuBoard::from_json(path.to_str().unwrap());
        std::fs::remove_file(&path).unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(loaded.puzzle(), board.puzzle());
        assert_eq!(loaded.loaded_time, 12.0);
        assert_eq!(loaded.cell(pos(1, 1)).value.get(), Some(2));
        assert!(!loaded.is_fixed(pos(1, 1)));
        assert!(loaded.is_fixed(pos(0, 0)));
        assert!(loaded.is_earmark_enabled(pos(2, 2), 1));
        assert!(loaded.is_earmark_enabled(pos(2, 2), 3));
        assert!(!loaded.is_earmark_enabled(pos(2, 2), 0));
    }

    #[test]
    fn bad_json() {
        let path =
            std::env::temp_dir().join(format!("rascacielos-bad-{}.json", std::process::id()));
        let good = board().to_json(-1.0);
        for bad in [
            "{".to_string(),
            good.replace("\"version\": 1", "\"version\": 2"),
            good.replace("\"size\": 4", "\"size\": 9"),
            good.replace("\"value\": 1,", "\"value\": 5,"),
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(SudokuBoard::from_json(path.to_str().unwrap()).is_err());
        }
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            SudokuBoard::from_json("/nonexistent/rascacielos").err(),
            Some(SudokuError::FileNotFound)
        );
    }

    #[test]
    fn share_code_and_text() {
        let board = board();
        let code = board.fixed_to_ascii();
        assert!(code.starts_with("#R4m"));
        assert_eq!(
            SudokuBoard::from_ascii(&code).unwrap().puzzle(),
            board.puzzle()
        );
        let text = board.fixed_to_string_pretty();
        assert_eq!(
            SudokuBoard::from_string(&text).unwrap().puzzle(),
            board.puzzle()
        );
        assert!(!board.fixed_to_string().contains('#'));
    }
}
