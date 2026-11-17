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

//! The clues around the grid: one per row and column on each side, lined
//! up with the cells.

use adw::prelude::*;
use adw::subclass::prelude::*;

use gtk::glib::{self, closure_local};
use gtk::{Allocation, graphene::Point};

use crate::cell::SudokuCell;
use crate::clue::SudokuClue;
use crate::connect_cb_pref;
use crate::grid::SudokuGrid;
use crate::lib::backend::SudokuBackend;
use crate::lib::enums::CompatValue;
use crate::lib::puzzle::{ClueConflict, MAX_SIZE, Side};

use std::cell::OnceCell;

/// The width of the clue band, in cells.
pub const CLUE_BAND: f64 = 1.0;

mod imp {
    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for SudokuClueFrame {
        const NAME: &'static str = "SudokuClueFrame";
        type Type = super::SudokuClueFrame;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("sudoku-clues");
        }
    }

    pub struct SudokuClueFrame {
        pub grid: OnceCell<SudokuGrid>,
        /// Indexed by `Side::index()`, then by row or column.
        pub labels: [[SudokuClue; MAX_SIZE]; 4],
        pub backend: OnceCell<SudokuBackend>,
    }

    impl Default for SudokuClueFrame {
        fn default() -> Self {
            let labels = Side::ALL.map(|side| std::array::from_fn(|_| SudokuClue::new(side)));
            Self {
                grid: Default::default(),
                labels,
                backend: Default::default(),
            }
        }
    }

    impl SudokuClueFrame {
        pub fn backend(&self) -> &SudokuBackend {
            self.backend.get().unwrap()
        }

        pub fn label(&self, side: Side, index: usize) -> &SudokuClue {
            &self.labels[side.index()][index]
        }

        /// Shows the clues of the current game and marks the ones the
        /// entered values contradict.
        pub fn update_clues(&self) {
            let backend = self.backend();
            let n = backend.game_size();
            for side in Side::ALL {
                for index in 0..MAX_SIZE {
                    let label = self.label(side, index);
                    let clue = (index < n)
                        .then(|| backend.game_clue(side, index))
                        .flatten();
                    label.set_child_visible(index < n);
                    label.set_clue(clue);
                    if clue.is_none() {
                        label.remove_css_class("error");
                        continue;
                    }
                    let error = match backend.game_clue_conflict(side, index) {
                        Some(ClueConflict::Full) => backend.pref_duplicate_warnings(),
                        Some(ClueConflict::Early) => backend.pref_clue_warnings(),
                        None => false,
                    };
                    if error {
                        label.add_css_class("error");
                    } else {
                        label.remove_css_class("error");
                    }
                }
            }
            self.obj().queue_allocate();
        }

        fn add_game_hooks(&self) {
            let binding = self.backend().game();
            let game = binding.as_ref().unwrap();
            game.connect_closure(
                "value-changed",
                false,
                closure_local!(
                    #[weak(rename_to = frame)]
                    self,
                    move |_: glib::Object, _: i32, _: i32, _: CompatValue, _: CompatValue| {
                        frame.update_clues();
                    }
                ),
            );
        }

        pub fn init(&self, backend: &SudokuBackend) {
            self.backend
                .set(backend.clone())
                .expect("Clue frame is already initialized");

            backend.connect_closure(
                "game-changed",
                false,
                closure_local!(
                    #[weak(rename_to = frame)]
                    self,
                    move |_: SudokuBackend| {
                        frame.add_game_hooks();
                        frame.update_clues();
                    }
                ),
            );
            connect_cb_pref!(self, connect_pref_duplicate_warnings_notify, update_clues);
            connect_cb_pref!(self, connect_pref_clue_warnings_notify, update_clues);

            self.add_game_hooks();
            self.update_clues();
        }

        /// The bounds of `cell` in frame coordinates.
        fn cell_bounds(&self, cell: &SudokuCell) -> Allocation {
            let origin = cell
                .compute_point(&*self.obj(), &Point::new(0.0, 0.0))
                .unwrap_or(Point::new(0.0, 0.0));
            Allocation::new(
                origin.x() as i32,
                origin.y() as i32,
                cell.width(),
                cell.height(),
            )
        }
    }

    impl ObjectImpl for SudokuClueFrame {
        fn constructed(&self) {
            self.parent_constructed();
            for side in &self.labels {
                for label in side {
                    label.set_parent(&*self.obj());
                }
            }
        }

        fn dispose(&self) {
            if let Some(grid) = self.grid.get() {
                grid.unparent();
            }
            for side in &self.labels {
                for label in side {
                    label.unparent();
                }
            }
        }
    }

    impl WidgetImpl for SudokuClueFrame {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(grid) = self.grid.get() else {
                return;
            };
            let n = match self.backend.get() {
                Some(backend) if backend.game_exists() => backend.game_size(),
                _ => MAX_SIZE,
            };
            let band = (width.min(height) as f64 * CLUE_BAND / (n as f64 + 2.0 * CLUE_BAND)) as i32;
            grid.size_allocate(
                &Allocation::new(band, band, width - 2 * band, height - 2 * band),
                baseline,
            );

            let font_size = (band as f64 * 0.45) as i32;
            for i in 0..n {
                let column = self.cell_bounds(&grid.imp().cells[0][i]);
                let row = self.cell_bounds(&grid.imp().cells[i][0]);
                let allocations = [
                    (
                        Side::Top,
                        Allocation::new(column.x(), 0, column.width(), band),
                    ),
                    (
                        Side::Bottom,
                        Allocation::new(column.x(), height - band, column.width(), band),
                    ),
                    (Side::Left, Allocation::new(0, row.y(), band, row.height())),
                    (
                        Side::Right,
                        Allocation::new(width - band, row.y(), band, row.height()),
                    ),
                ];
                for (side, allocation) in allocations {
                    let label = self.label(side, i);
                    label.set_font_size(font_size);
                    label.size_allocate(&allocation, baseline);
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct SudokuClueFrame(ObjectSubclass<imp::SudokuClueFrame>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SudokuClueFrame {
    /// A frame around `grid`.
    pub fn new(grid: &SudokuGrid) -> Self {
        let frame: Self = glib::Object::builder().build();
        grid.insert_after(&frame, gtk::Widget::NONE);
        frame.imp().grid.set(grid.clone()).unwrap();
        frame
    }

    pub fn init(&self, backend: &SudokuBackend) {
        self.imp().init(backend);
    }
}
