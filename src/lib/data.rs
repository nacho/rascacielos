/* -*- Mode: rust; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/*
 * Copyright © 2026 Johan GAY
 * Copyright © 2026 Ignacio Casal Quinteiro
 *
 * This file is part of GNOME Sudoku.
 *
 * GNOME Sudoku is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * GNOME Sudoku is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with GNOME Sudoku. If not, see <http://www.gnu.org/licenses/>.
 */

use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::Write,
};

use serde::{Deserialize, Serialize};

use crate::lib::{backend::DEFAULT_SIZE, board::SudokuError, enums::DifficultyCategory};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct SudokuData {
    #[serde(default)]
    highscores: RefCell<Vec<Highscore>>,
    #[serde(default)]
    pub print_data: RefCell<PrintData>,
    #[serde(default)]
    pub start_button_selected: Cell<DifficultyCategory>,
    #[serde(default = "default_size")]
    pub size_selected: Cell<usize>,
    #[serde(skip)]
    data_file: String,
}

/// The best time for puzzles of a size and difficulty.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
struct Highscore {
    size: usize,
    difficulty: DifficultyCategory,
    time: f64,
}

fn default_size() -> Cell<usize> {
    Cell::new(DEFAULT_SIZE)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrintData {
    pub number_of_puzzles: f64,
    pub number_of_puzzles_per_page: f64,
    pub difficulty: DifficultyCategory,
    #[serde(default = "default_size_value")]
    pub size: usize,
}

fn default_size_value() -> usize {
    DEFAULT_SIZE
}

impl Default for PrintData {
    fn default() -> Self {
        Self {
            number_of_puzzles: 2.0,
            number_of_puzzles_per_page: 1.0,
            difficulty: DifficultyCategory::Easy,
            size: DEFAULT_SIZE,
        }
    }
}

impl SudokuData {
    pub fn new(path: String) -> Self {
        if let Ok(str) = std::fs::read_to_string(&path)
            && let Ok(mut data) = Self::load(&str)
        {
            data.data_file = path;
            data
        } else {
            Self {
                highscores: Default::default(),
                data_file: path,
                print_data: Default::default(),
                start_button_selected: Cell::new(DifficultyCategory::Easy),
                size_selected: default_size(),
            }
        }
    }

    fn load(str: &str) -> Result<Self, SudokuError> {
        let data: Self = serde_json::from_str(str)?;
        Ok(data)
    }

    pub fn save(&self) -> bool {
        if let Ok(mut file) = File::create(&self.data_file) {
            let str = serde_json::to_string_pretty(&self).unwrap();
            file.write_all(&str.into_bytes()).expect("Error saving");
            true
        } else {
            println!("ERROR OPENING FILE");
            false
        }
    }

    /// The best time for puzzles of `size` and `difficulty`, in seconds.
    pub fn highscore(&self, size: usize, difficulty: DifficultyCategory) -> Option<f64> {
        self.highscores
            .borrow()
            .iter()
            .find(|h| h.size == size && h.difficulty == difficulty)
            .map(|h| h.time)
    }

    /// Records `time` if it beats the best time for `size` and
    /// `difficulty`. Returns whether it did.
    pub fn record_time(&self, size: usize, difficulty: DifficultyCategory, time: f64) -> bool {
        let mut highscores = self.highscores.borrow_mut();
        match highscores
            .iter_mut()
            .find(|h| h.size == size && h.difficulty == difficulty)
        {
            Some(highscore) if highscore.time <= time => false,
            Some(highscore) => {
                highscore.time = time;
                true
            }
            None => {
                highscores.push(Highscore {
                    size,
                    difficulty,
                    time,
                });
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highscores_per_size_and_difficulty() {
        use DifficultyCategory::*;
        let data = SudokuData::new("/nonexistent/rascacielos-data".to_string());
        assert_eq!(data.highscore(5, Easy), None);
        assert!(data.record_time(5, Easy, 60.0));
        assert!(data.record_time(7, Easy, 300.0));
        assert!(data.record_time(5, Hard, 200.0));
        assert!(!data.record_time(5, Easy, 61.0));
        assert!(data.record_time(5, Easy, 59.0));
        assert_eq!(data.highscore(5, Easy), Some(59.0));
        assert_eq!(data.highscore(7, Easy), Some(300.0));
        assert_eq!(data.highscore(5, Hard), Some(200.0));
        assert_eq!(data.highscore(6, Easy), None);
    }

    #[test]
    fn json_round_trip() {
        use DifficultyCategory::*;
        let data = SudokuData::new("/nonexistent/rascacielos-data".to_string());
        data.record_time(6, Medium, 120.0);
        data.size_selected.set(6);
        let loaded = SudokuData::load(&serde_json::to_string(&data).unwrap()).unwrap();
        assert_eq!(loaded.highscore(6, Medium), Some(120.0));
        assert_eq!(loaded.size_selected.get(), 6);
        // Data saved before sizes existed falls back to the default size.
        let old = SudokuData::load("{}").unwrap();
        assert_eq!(old.size_selected.get(), DEFAULT_SIZE);
        assert_eq!(old.print_data.borrow().size, DEFAULT_SIZE);
    }
}
