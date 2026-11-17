/* -*- Mode: rust; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/*
 * Copyright © 2026 Johan GAY
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

use fastrand::Rng;
use gtk::glib;
use gtk::glib::SourceId;
use gtk::glib::{Properties, subclass::Signal};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::lib::board::SudokuBoard;
use crate::lib::board::SudokuError;
use crate::lib::enums::CompatValue;
use crate::lib::enums::Coord;
use crate::lib::enums::DifficultyCategory;
use crate::lib::generator::{self, Generated};
use crate::lib::puzzle::Tier;
use crate::lib::solver;

use std::cell::{Cell, OnceCell};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use std::{cell::RefCell, sync::OnceLock};

mod imp {

    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::SudokuGame)]
    pub struct SudokuGame {
        pub timer: Cell<Option<SystemTime>>,
        pub stored_time: Cell<f64>,
        pub paused: Cell<bool>,
        pub timer_source_id: Cell<Option<SourceId>>,
        pub board: OnceCell<SudokuBoard>,
        pub stack_head_index: Cell<Option<usize>>,
        pub current_action: Cell<Option<StackAction>>,
        pub stack: RefCell<Vec<StackItem>>,
    }

    impl SudokuGame {
        pub fn add_value_step(
            &self,
            item: &mut StackItem,
            pos: Coord,
            old_val: Option<usize>,
            new_val: Option<usize>,
        ) {
            let value_step = ValueChange {
                pos,
                old_val,
                new_val,
            };
            item.value_changes.push(value_step);

            self.obj().board().set_value(pos, new_val);
            self.value_changed(pos, CompatValue::from(old_val), CompatValue::from(new_val));
        }

        pub fn add_earmark_step(
            &self,
            item: &mut StackItem,
            pos: Coord,
            num: usize,
            enabled: bool,
        ) {
            let earmark_step = EarmarkChange { pos, num, enabled };
            item.earmark_changes.push(earmark_step);
            match enabled {
                true => self.obj().board().enable_earmark(pos, num),
                false => self.obj().board().disable_earmark(pos, num),
            }
            self.earmark_changed(pos, num, enabled);
        }

        pub fn disable_all_earmarks_helper(&self, pos: Coord, item: &mut StackItem) {
            for num in 0..self.obj().board().size() {
                if self.obj().board().is_earmark_enabled(pos, num) {
                    self.add_earmark_step(&mut *item, pos, num, false);
                }
            }
        }

        pub fn value_changed(&self, pos: Coord, old_val: CompatValue, new_val: CompatValue) {
            self.obj().emit_by_name::<()>(
                "value-changed",
                &[&(pos.row as i32), &(pos.col as i32), &old_val.0, &new_val.0],
            );
        }

        pub fn completed(&self) {
            self.obj().emit_by_name::<()>("completed", &[]);
        }

        pub fn paused(&self, paused: bool) {
            self.obj().emit_by_name::<()>("paused", &[&paused]);
        }

        pub fn earmark_changed(&self, pos: Coord, num: usize, enabled: bool) {
            self.obj().emit_by_name::<()>(
                "earmark-changed",
                &[
                    &(pos.row as i32),
                    &(pos.col as i32),
                    &(num as i32),
                    &enabled,
                ],
            );
        }

        //clears all redo actions
        pub fn truncate_stack(&self) {
            if let Some(stack_head_index) = self.stack_head_index.get() {
                self.stack.borrow_mut().truncate(stack_head_index + 1);
            } else {
                self.stack.borrow_mut().clear();
            }
            self.stack_head_index.set(Some(self.stack.borrow().len()));
        }

        pub fn start_clock_tick(&self) {
            self.timer_source_id
                .set(Some(glib::timeout_add_seconds_local(
                    1,
                    glib::clone!(
                        #[weak(rename_to = obj)]
                        self.obj(),
                        #[upgrade_or]
                        glib::ControlFlow::Break,
                        move || {
                            obj.emit_by_name::<()>("tick", &[]);
                            glib::ControlFlow::Continue
                        }
                    ),
                )));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SudokuGame {
        const NAME: &'static str = "SudokuGame";
        type Type = super::SudokuGame;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SudokuGame {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("action-completed")
                        .param_types([StackAction::static_type()])
                        .build(),
                    Signal::builder("tick").build(),
                    Signal::builder("completed").build(),
                    Signal::builder("paused")
                        .param_types([bool::static_type()])
                        .build(),
                    //pos, value, enabled
                    Signal::builder("earmark-changed")
                        .param_types([
                            i32::static_type(),
                            i32::static_type(),
                            i32::static_type(),
                            bool::static_type(),
                        ])
                        .build(),
                    //pos, old_val, new_val
                    Signal::builder("value-changed")
                        .param_types([
                            i32::static_type(),
                            i32::static_type(),
                            i32::static_type(),
                            i32::static_type(),
                        ])
                        .build(),
                ]
            })
        }
        fn constructed(&self) {}
    }

    impl StackAction {
        pub fn is_single_value_change(&self) -> bool {
            use StackAction::*;
            matches!(self, Insert | Remove | InsertAndDisableAlignedEarmarks)
        }

        pub fn is_single_earmarks_change(&self) -> bool {
            use StackAction::*;
            matches!(self, EnableEarmark | DisableEarmark | DisableAllEarmarks)
        }
    }
}

macro_rules! new_action {
    ($obj: expr, $action_type: expr) => {{
        $obj.imp().current_action.set(Some($action_type));
        $obj.imp().truncate_stack();
        StackItem::new($action_type)
    }};
}

macro_rules! finish_action {
    ($obj: expr, $item: expr) => {
        $obj.imp().current_action.set(None);
        let action_type = $item.action_type;
        $obj.imp().stack.borrow_mut().push($item);
        $obj.emit_by_name::<()>("action-completed", &[&action_type]);
    };
}

glib::wrapper! {
    pub struct SudokuGame(ObjectSubclass<imp::SudokuGame>);
}

impl SudokuGame {
    pub fn new_from_fn<F: Fn(&str) -> Result<SudokuBoard, SudokuError>>(
        func: F,
        path: &str,
    ) -> Result<Self, SudokuError> {
        let game: Self = glib::Object::new();
        let mut board = func(path)?;
        if let Some(error) = SudokuGame::solve(&mut board) {
            return Err(error);
        }
        game.imp().board.set(board).unwrap();
        game.start_clock();
        Ok(game)
    }

    pub fn new_from_json(path: &str) -> Result<Self, SudokuError> {
        Self::new_from_fn(SudokuBoard::from_json, path)
    }

    pub fn new_from_string(s: &str) -> Result<Self, SudokuError> {
        Self::new_from_fn(SudokuBoard::from_string, s)
    }

    pub fn new_from_ascii(s: &str) -> Result<Self, SudokuError> {
        Self::new_from_fn(SudokuBoard::from_ascii, s)
    }

    /// Generates `count` puzzles of size `n` and `difficulty` on every
    /// processor. Blocks until they are found.
    pub fn generate(n: usize, difficulty: DifficultyCategory, count: usize) -> Vec<Generated> {
        let tier = difficulty.to_tier().unwrap_or(Tier::Easy);
        let time = SystemTime::now();
        let found: Mutex<Vec<Generated>> = Default::default();
        let cancel = AtomicBool::new(false);

        std::thread::scope(|scope| {
            for _ in 0..std::thread::available_parallelism().map_or(1, |n| n.get()) {
                scope.spawn(|| {
                    let mut rng = Rng::new();
                    while let Some(result) = generator::generate(n, tier, &mut rng, &cancel) {
                        let mut found = found.lock().unwrap();
                        if found.len() < count {
                            found.push(result);
                        }
                        if found.len() >= count {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    }
                });
            }
        });

        println!(
            "Generated {count} {n}×{n} puzzles in {}ms",
            time.elapsed().unwrap_or_default().as_millis()
        );
        found.into_inner().unwrap()
    }

    pub fn new_from_generated(generated: Generated, difficulty: DifficultyCategory) -> Self {
        let (puzzle, solution) = generated;
        let board = SudokuBoard::from_generated(puzzle, &solution, difficulty);
        let game: Self = glib::Object::new();
        game.imp().board.set(board).unwrap();
        game.start_clock();
        game
    }

    /// Finds the solution of `board` and grades it. Fails when the puzzle
    /// has no solution or more than one.
    pub fn solve(board: &mut SudokuBoard) -> Option<SudokuError> {
        if let Some((solution, tier)) = solver::solve(board.puzzle(), Tier::VeryHard) {
            board.set_solution(&solution);
            board.difficulty_category = DifficultyCategory::from_tier(tier);
            return None;
        }

        // Too hard for the solver's techniques, check it with a search.
        let solutions = solver::find_solutions(board.puzzle(), 2);
        match solutions.as_slice() {
            [] => Some(SudokuError::InvalidPuzzle),
            [solution] => {
                board.set_solution(solution);
                board.difficulty_category = DifficultyCategory::Custom;
                None
            }
            _ => Some(SudokuError::MultipleSolutions),
        }
    }

    pub fn board(&self) -> &SudokuBoard {
        self.imp().board.get().unwrap()
    }

    pub fn toggle_pause(&self) {
        let paused = self.imp().paused.get();
        if paused {
            self.start_clock();
        } else {
            self.stop_clock();
        }
        self.imp().paused(!paused);
    }

    pub fn start_clock(&self) {
        if self.imp().timer.get().is_none() {
            self.imp().timer.set(Some(SystemTime::now()));
            self.imp().start_clock_tick();
            self.imp().paused.set(false);
        }
    }

    pub fn total_time_played(&self) -> f64 {
        let imp = self.imp();
        let mut accumulated_time = imp.stored_time.get();
        if let Some(timer) = imp.timer.get()
            && let Ok(elapsed_time) = timer.elapsed()
        {
            accumulated_time += elapsed_time.as_secs() as f64;
        }
        accumulated_time += imp.board.get().unwrap().loaded_time;
        accumulated_time
    }

    pub fn stop_clock(&self) {
        if let Some(timer) = self.imp().timer.get() {
            if let Some(source_id) = self.imp().timer_source_id.take() {
                SourceId::remove(source_id);
            }
            if let Ok(elapsed_time) = timer.elapsed() {
                self.imp()
                    .stored_time
                    .update(|x| x + elapsed_time.as_secs_f64());
                self.imp().timer.set(None);
                self.imp().paused.set(true);
            }
        }
    }

    pub fn difficulty(&self) -> DifficultyCategory {
        self.board().difficulty_category
    }

    pub fn is_undostack_null(&self) -> bool {
        self.imp().stack_head_index.get().is_none()
    }

    pub fn is_redostack_null(&self) -> bool {
        if let Some(stack_head_index) = self.imp().stack_head_index.get() {
            stack_head_index == self.imp().stack.borrow().len() - 1
        } else {
            self.imp().stack.borrow().is_empty()
        }
    }

    pub fn enable_earmark(&self, pos: Coord, num: usize) {
        let imp = self.imp();
        let mut item = new_action!(self, StackAction::EnableEarmark);
        imp.add_earmark_step(&mut item, pos, num, true);
        finish_action!(self, item);
    }

    pub fn enable_all_earmark_possibilities(&self) {
        let imp = self.imp();
        let stack_head_index_copy = self.imp().stack_head_index.get();

        let mut item = new_action!(self, StackAction::EnableAllEarmarkPossibilities);

        let n = self.board().size();
        for row in 0..n {
            for col in 0..n {
                let pos = Coord { row, col };
                let cell = &self.board().cells[row][col];
                if cell.value.get().is_some() {
                    continue;
                }

                let possibilities = self.board().cell_possibilies(pos);
                for num in 0..n {
                    if !cell.earmarks[num].get() && possibilities[num] {
                        imp.add_earmark_step(&mut item, pos, num, true);
                    }
                }
            }
        }

        if !item.earmark_changes.is_empty() {
            finish_action!(self, item);
        } else {
            self.imp().stack_head_index.set(stack_head_index_copy);
        }
    }

    pub fn disable_earmark(&self, pos: Coord, num: usize) {
        let imp = self.imp();
        let mut item = new_action!(self, StackAction::DisableEarmark);
        imp.add_earmark_step(&mut item, pos, num, false);
        finish_action!(self, item);
    }

    pub fn disable_all_earmarks(&self, pos: Coord) {
        let imp = self.imp();
        let mut item = new_action!(self, StackAction::DisableAllEarmarks);

        imp.disable_all_earmarks_helper(pos, &mut item);

        finish_action!(self, item);
    }

    pub fn insert_and_disable_aligned_earmarks(&self, pos: Coord, new_val: usize) {
        let imp = self.imp();
        let old_val = self.board().cell(pos).value.get();

        let mut item = new_action!(self, StackAction::InsertAndDisableAlignedEarmarks);

        for new_pos in self.board().cell(pos).aligned_coords.clone() {
            if pos == new_pos {
                continue;
            }

            if self.board().is_earmark_enabled(new_pos, new_val) {
                imp.add_earmark_step(&mut item, new_pos, new_val, false);
            }
        }

        if self.board().has_earmarks(pos) {
            imp.disable_all_earmarks_helper(pos, &mut item);
        }

        imp.add_value_step(&mut item, pos, old_val, Some(new_val));

        finish_action!(self, item);

        if self.board().is_complete() {
            imp.completed();
        }
    }

    pub fn insert(&self, pos: Coord, new_val: usize) {
        let imp = self.imp();
        let old_val = self.board().cell(pos).value.get();

        let mut item = new_action!(self, StackAction::Insert);

        if self.board().has_earmarks(pos) {
            imp.disable_all_earmarks_helper(pos, &mut item);
        }

        imp.add_value_step(&mut item, pos, old_val, Some(new_val));

        finish_action!(self, item);

        if self.board().is_complete() {
            imp.completed();
        }
    }

    pub fn remove(&self, pos: Coord) {
        let imp = self.imp();
        let old_val = self.board().cell(pos).value.get();

        let mut item = new_action!(self, StackAction::Remove);
        imp.add_value_step(&mut item, pos, old_val, None);

        finish_action!(self, item);
    }

    pub fn reset(&self) {
        let imp = self.imp();
        let mut item = new_action!(self, StackAction::Reset);

        let n = self.board().size();
        for row in 0..n {
            for col in 0..n {
                let cell = &self.board().cells[row][col];
                if cell.fixed {
                    continue;
                }
                let pos = Coord { row, col };

                if let Some(value) = cell.value.get() {
                    imp.add_value_step(&mut item, pos, Some(value), None);
                } else if self.board().has_earmarks(pos) {
                    imp.disable_all_earmarks_helper(pos, &mut item);
                }
            }
        }

        finish_action!(self, item);
    }

    pub fn current_action(&self) -> StackAction {
        self.imp().current_action.get().unwrap()
    }

    fn unique(v: Vec<Coord>) -> Vec<Coord> {
        let mut ret = Vec::new();
        let mut hashset = HashSet::<Coord>::new();
        for coord in v {
            if hashset.insert(coord) {
                ret.push(coord);
            }
        }
        ret
    }

    pub fn play_order(&self) -> Vec<Coord> {
        let stack = self.imp().stack.borrow();
        let mut play_order: Vec<Coord> = Default::default();
        for item in stack.iter() {
            for value_change in &item.value_changes {
                play_order.push(value_change.pos);
            }
        }
        let mut play_order: Vec<Coord> = Self::unique(play_order);
        play_order.reverse();

        let n = self.board().size();
        if play_order.len() == n * n - self.board().total_fixed {
            return play_order;
        }

        //this is in case the game was loaded we generate a random
        //order for already filled in cells
        let mut randomized_save_order: Vec<Coord> = Vec::new();
        let cells_not_fixed = self.board().cells_not_fixed();
        for coord in cells_not_fixed {
            if !play_order.contains(&coord) {
                randomized_save_order.push(coord);
            }
        }

        let mut rng = Rng::new();
        rng.shuffle(&mut randomized_save_order);
        play_order.append(&mut randomized_save_order);

        play_order
    }

    pub fn undo(&self) {
        let stack_head_index = self.imp().stack_head_index.get().unwrap();
        let changes = &self.imp().stack.borrow()[stack_head_index];
        let imp = self.imp();
        imp.current_action.set(Some(changes.action_type));

        for vc in changes.value_changes.iter() {
            self.board().set_value(vc.pos, vc.old_val);
            imp.value_changed(
                vc.pos,
                CompatValue::from(vc.new_val),
                CompatValue::from(vc.old_val),
            );
        }

        for ec in changes.earmark_changes.iter() {
            if ec.enabled {
                self.board().disable_earmark(ec.pos, ec.num);
            } else {
                self.board().enable_earmark(ec.pos, ec.num);
            }
            imp.earmark_changed(ec.pos, ec.num, !ec.enabled);
        }

        if stack_head_index == 0 {
            imp.stack_head_index.set(None);
        } else {
            imp.stack_head_index.set(Some(stack_head_index - 1));
        }

        self.imp().current_action.set(None);
        self.emit_by_name::<()>("action-completed", &[&changes.action_type]);
    }

    pub fn redo(&self) {
        let index_to_redo = {
            if let Some(stack_head_index) = self.imp().stack_head_index.get() {
                stack_head_index + 1
            } else {
                0
            }
        };
        let imp = self.imp();
        let changes = &imp.stack.borrow()[index_to_redo];
        imp.current_action.set(Some(changes.action_type));

        for vc in changes.value_changes.iter() {
            self.board().set_value(vc.pos, vc.new_val);
            imp.value_changed(
                vc.pos,
                CompatValue::from(vc.old_val),
                CompatValue::from(vc.new_val),
            );
        }

        for ec in changes.earmark_changes.iter() {
            if ec.enabled {
                self.board().enable_earmark(ec.pos, ec.num);
            } else {
                self.board().disable_earmark(ec.pos, ec.num);
            }
            imp.earmark_changed(ec.pos, ec.num, ec.enabled);
        }

        imp.stack_head_index.set(Some(index_to_redo));
        self.imp().current_action.set(None);
        self.emit_by_name::<()>("action-completed", &[&changes.action_type]);
    }

    //for testing purposes
    pub fn _auto_complete(&self) {
        let n = self.board().size();
        for row in 0..n {
            for col in 0..n {
                let cell = &self.board().cells[row][col];
                if cell.value.get().is_none_or(|value| value != cell.solution) {
                    self.insert(Coord { row, col }, cell.solution);
                }
            }
        }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "StackAction")]
pub enum StackAction {
    #[default]
    None,
    Insert,
    Remove,
    EnableEarmark,
    DisableEarmark,
    DisableAllEarmarks,
    InsertAndDisableAlignedEarmarks,
    EnableAllEarmarkPossibilities,
    Reset,
}

#[derive(Clone, Copy)]
struct EarmarkChange {
    pos: Coord,
    num: usize,
    enabled: bool,
}

#[derive(Clone, Copy)]
struct ValueChange {
    pos: Coord,
    old_val: Option<usize>,
    new_val: Option<usize>,
}

#[derive(Clone)]
pub struct StackItem {
    value_changes: Vec<ValueChange>,
    earmark_changes: Vec<EarmarkChange>,
    action_type: StackAction,
}

impl StackItem {
    fn new(action_type: StackAction) -> Self {
        Self {
            value_changes: Default::default(),
            earmark_changes: Default::default(),
            action_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::puzzle::{Puzzle, Side};

    const SOLUTION: [u8; 16] = [
        1, 2, 3, 4, //
        2, 3, 4, 1, //
        4, 1, 2, 3, //
        3, 4, 1, 2, //
    ];

    fn game() -> SudokuGame {
        let mut puzzle = Puzzle::from_solution(4, &SOLUTION).unwrap();
        puzzle.set_clue(Side::Top, 0, None);
        SudokuGame::new_from_ascii(&puzzle.to_share_code(None)).unwrap()
    }

    fn pos(row: usize, col: usize) -> Coord {
        Coord { row, col }
    }

    // Games start a clock on the default main context, which only one
    // thread can own, so all the game tests run from a single test.
    #[test]
    fn games() {
        imported_puzzles_are_solved_and_graded();
        undo_and_redo();
        autoclean_clears_aligned_earmarks();
        possibilities();
    }

    fn imported_puzzles_are_solved_and_graded() {
        let game = game();
        assert_eq!(game.board().cell(pos(2, 1)).solution, 0);
        assert_ne!(game.difficulty(), DifficultyCategory::Unknown);
        let several = Puzzle::empty(4).unwrap().to_share_code(None);
        assert_eq!(
            SudokuGame::new_from_ascii(&several).err(),
            Some(SudokuError::MultipleSolutions)
        );
    }

    fn undo_and_redo() {
        let game = game();
        let value = |game: &SudokuGame| game.board().cell(pos(1, 1)).value.get();
        game.insert(pos(1, 1), 2);
        game.enable_earmark(pos(0, 0), 3);
        game.insert(pos(1, 1), 1);
        assert_eq!(value(&game), Some(1));
        game.undo();
        assert_eq!(value(&game), Some(2));
        assert!(game.board().is_earmark_enabled(pos(0, 0), 3));
        game.undo();
        assert!(!game.board().is_earmark_enabled(pos(0, 0), 3));
        game.undo();
        assert_eq!(value(&game), None);
        assert!(game.is_undostack_null());
        game.redo();
        assert_eq!(value(&game), Some(2));
        game.redo();
        game.redo();
        assert_eq!(value(&game), Some(1));
        assert!(game.is_redostack_null());
    }

    fn autoclean_clears_aligned_earmarks() {
        let game = game();
        game.enable_earmark(pos(1, 3), 2);
        game.enable_earmark(pos(3, 1), 2);
        game.enable_earmark(pos(3, 3), 2);
        game.insert_and_disable_aligned_earmarks(pos(1, 1), 2);
        assert!(!game.board().is_earmark_enabled(pos(1, 3), 2));
        assert!(!game.board().is_earmark_enabled(pos(3, 1), 2));
        assert!(game.board().is_earmark_enabled(pos(3, 3), 2));
    }

    fn possibilities() {
        let game = game();
        game.insert(pos(1, 1), 2);
        game.enable_all_earmark_possibilities();
        assert!(!game.board().is_earmark_enabled(pos(1, 2), 2));
        assert!(game.board().is_earmark_enabled(pos(2, 2), 2));
        assert!(!game.board().has_earmarks(pos(1, 1)));
    }
}
