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

//! Prints puzzles. Each puzzle is drawn with a `gtk::Snapshot` and the
//! resulting render node is drawn into the print context.

use gtk::glib;
use gtk::gsk::{self, PathBuilder, Stroke};
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{PrintContext, PrintOperationResult, TextDirection, gdk, graphene};

use adw::prelude::{AdwDialogExt, AlertDialogExt};

use crate::lib::board::SudokuBoard;
use crate::lib::puzzle::Side;

use std::cell::OnceCell;

/// The width of the clue band, in cells.
const CLUE_BAND: f32 = 0.6;

const BLACK: gdk::RGBA = gdk::RGBA::BLACK;
const GREY: gdk::RGBA = gdk::RGBA::new(0.3, 0.3, 0.3, 1.0);

mod imp {
    use super::*;

    pub struct SudokuPrinter {
        pub boards: OnceCell<Vec<SudokuBoard>>,
        pub print_operation: gtk::PrintOperation,
        pub margin: i32,
        pub sudokus_count: OnceCell<i32>,
        pub sudokus_per_page: OnceCell<i32>,
    }

    impl SudokuPrinter {
        pub fn print_sudoku(&self, window: &gtk::Window) -> PrintOperationResult {
            let result = self
                .print_operation
                .run(gtk::PrintOperationAction::PrintDialog, Some(window));
            use gettextrs::gettext;
            match result {
                Ok(print_op) => print_op,
                Err(e) => {
                    let mut title = gettext("Error printing file:");
                    title = format!("{}\n{}", title, e.message());
                    let dialog = adw::AlertDialog::builder().title(title).build();
                    dialog.add_response("close", &gettext("_Close"));
                    dialog.present(Some(window));
                    PrintOperationResult::Error
                }
            }
        }

        fn layout(context: &pango::Context, text: &str, font: &str) -> pango::Layout {
            let layout = pango::Layout::new(context);
            layout.set_font_description(Some(&pango::FontDescription::from_string(font)));
            layout.set_text(text);
            layout
        }

        /// Draws `text` centred on (`x`, `y`).
        fn centered_text(
            snapshot: &gtk::Snapshot,
            context: &pango::Context,
            text: &str,
            font: &str,
            color: &gdk::RGBA,
            x: f32,
            y: f32,
        ) {
            let layout = Self::layout(context, text, font);
            let (width, height) = layout.pixel_size();
            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                x - width as f32 / 2.0,
                y - height as f32 / 2.0,
            ));
            snapshot.append_layout(&layout, color);
            snapshot.restore();
        }

        pub fn draw_page_cb(&self, context: &PrintContext, page_nr: i32) {
            let width = context.width();
            let height = context.height();
            let pango_context = context.create_pango_context();
            let label_font = "Sans Bold 9";
            let label_height = Self::layout(&pango_context, "Ww", label_font)
                .pixel_size()
                .1 as f64;

            let (best_square_size, n_across, n_down) =
                self.fit_squares_in_rectangle(width, height, label_height, self.margin);
            let margin_x = (width - best_square_size * n_across as f64) / (n_across as f64 + 1.0);
            let margin_y = (height - best_square_size * n_down as f64) / (n_down as f64 + 1.0);
            let spp = *self.sudokus_per_page.get().unwrap();
            let start = page_nr * spp;
            let end = i32::min(start + spp, self.boards.get().unwrap().len() as i32);
            let sudokus_on_page = &self.boards.get().unwrap()[start as usize..end as usize];

            let snapshot = gtk::Snapshot::new();
            for (index, sudoku) in sudokus_on_page.iter().enumerate() {
                let index = index as i32;
                let left = margin_x + (index % n_across) as f64 * (best_square_size + margin_x);
                let top = margin_y
                    + label_height
                    + (index / n_across) as f64 * (best_square_size + margin_y + label_height);
                let n = sudoku.size();
                let label = format!(
                    "{n}×{n} · {}",
                    sudoku.difficulty_category.to_translated_string()
                );
                Self::centered_text(
                    &snapshot,
                    &pango_context,
                    &label,
                    label_font,
                    &BLACK,
                    (left + best_square_size / 2.0) as f32,
                    (top - label_height / 2.0) as f32,
                );

                snapshot.save();
                snapshot.translate(&graphene::Point::new(left as f32, top as f32));
                Self::draw_sudoku(&snapshot, &pango_context, sudoku, best_square_size as f32);
                snapshot.restore();
            }

            if let Some(node) = snapshot.to_node() {
                node.draw(&context.cairo_context());
            }
        }

        fn fit_squares_in_rectangle(
            &self,
            width: f64,
            height: f64,
            label_height: f64,
            margin: i32,
        ) -> (f64, i32, i32) {
            let n = self.sudokus_per_page.get().unwrap();
            let mut across = 1;
            let mut down = *n;
            let mut n_across = 1;

            let mut best_square_size = 0.0;
            loop {
                if n_across > *n {
                    break;
                }
                let n_down = (n + n_across - 1) / n_across;
                let mut across_size = width - ((n_across + 1) * margin) as f64;
                across_size /= n_across as f64;

                let mut down_size =
                    height - ((n_down + 1) * margin) as f64 - n_down as f64 * label_height;
                down_size /= n_down as f64;

                let square_size = f64::min(across_size, down_size);
                if square_size > best_square_size {
                    best_square_size = square_size;
                    across = n_across;
                    down = n_down;
                }

                n_across += 1;
            }

            (best_square_size, across, down)
        }

        /// Draws `board` with its clues in a square of side `size` at the
        /// origin.
        fn draw_sudoku(
            snapshot: &gtk::Snapshot,
            context: &pango::Context,
            board: &SudokuBoard,
            size: f32,
        ) {
            let n = board.size();
            let cell = size / (n as f32 + 2.0 * CLUE_BAND);
            let band = cell * CLUE_BAND;
            let grid = cell * n as f32;
            let thin = size / 500.0;
            let thick = thin * 5.0;

            snapshot.append_color(
                &gdk::RGBA::WHITE,
                &graphene::Rect::new(band, band, grid, grid),
            );

            let lines = PathBuilder::new();
            for i in 1..n {
                let offset = band + i as f32 * cell;
                lines.move_to(band, offset);
                lines.line_to(band + grid, offset);
                lines.move_to(offset, band);
                lines.line_to(offset, band + grid);
            }
            snapshot.append_stroke(&lines.to_path(), &Stroke::new(thin), &BLACK);

            let border = PathBuilder::new();
            border.add_rect(&graphene::Rect::new(band, band, grid, grid));
            let stroke = Stroke::new(thick);
            stroke.set_line_join(gsk::LineJoin::Miter);
            snapshot.append_stroke(&border.to_path(), &stroke, &BLACK);

            let center = |i: usize| band + (i as f32 + 0.5) * cell;
            let value_font = format!("Sans {}px", (cell * 0.5) as i32);
            let user_font = format!("Sans Italic {}px", (cell * 0.5) as i32);
            let invert = gtk::Widget::default_direction() == TextDirection::Rtl;
            for row in 0..n {
                for col in 0..n {
                    let real_col = if invert { n - col - 1 } else { col };
                    let board_cell = &board.cells[row][real_col];
                    let Some(value) = board_cell.value.get() else {
                        continue;
                    };
                    let (font, color) = if board_cell.fixed {
                        (&value_font, &BLACK)
                    } else {
                        (&user_font, &GREY)
                    };
                    Self::centered_text(
                        snapshot,
                        context,
                        &(value + 1).to_string(),
                        font,
                        color,
                        center(col),
                        center(row),
                    );
                }
            }

            let clue_font = format!("Sans Bold {}px", (band * 0.7) as i32);
            for i in 0..n {
                let real_i = if invert { n - i - 1 } else { i };
                let clues = [
                    (board.clue(Side::Top, real_i), center(i), band / 2.0),
                    (
                        board.clue(Side::Bottom, real_i),
                        center(i),
                        size - band / 2.0,
                    ),
                    (board.clue(Side::Left, i), band / 2.0, center(i)),
                    (board.clue(Side::Right, i), size - band / 2.0, center(i)),
                ];
                let clues = if invert {
                    // Left and right swap places when the grid is mirrored.
                    [
                        clues[0],
                        clues[1],
                        (clues[3].0, clues[2].1, clues[2].2),
                        (clues[2].0, clues[3].1, clues[3].2),
                    ]
                } else {
                    clues
                };
                for (clue, x, y) in clues {
                    if let Some(clue) = clue {
                        Self::centered_text(
                            snapshot,
                            context,
                            &clue.to_string(),
                            &clue_font,
                            &BLACK,
                            x,
                            y,
                        );
                    }
                }
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SudokuPrinter {
        const NAME: &'static str = "SudokuPrinter";
        type Type = super::SudokuPrinter;
    }

    impl ObjectImpl for SudokuPrinter {
        fn constructed(&self) {
            self.parent_constructed();

            self.print_operation.connect_begin_print(glib::clone!(
                #[weak(rename_to = printer)]
                self,
                move |_, _| {
                    let sudokus = printer.sudokus_count.get().unwrap();
                    let spp = printer.sudokus_per_page.get().unwrap();
                    let mut pages = sudokus / spp;
                    while pages * spp < *sudokus {
                        pages += 1;
                    }
                    printer.print_operation.set_n_pages(pages);
                }
            ));

            self.print_operation.connect_draw_page(glib::clone!(
                #[weak(rename_to = printer)]
                self,
                move |_, context, page_nr| printer.draw_page_cb(context, page_nr)
            ));
        }
    }

    impl Default for SudokuPrinter {
        fn default() -> Self {
            Self {
                margin: 25,
                sudokus_count: Default::default(),
                print_operation: Default::default(),
                boards: Default::default(),
                sudokus_per_page: Default::default(),
            }
        }
    }
}

glib::wrapper! {
    pub struct SudokuPrinter(ObjectSubclass<imp::SudokuPrinter>);
}

impl SudokuPrinter {
    pub fn new(boards: Vec<SudokuBoard>, sudokus_per_page: i32) -> Self {
        let printer: Self = glib::Object::builder().build();
        printer
            .imp()
            .sudokus_count
            .set(boards.len() as i32)
            .unwrap();
        printer
            .imp()
            .sudokus_per_page
            .set(sudokus_per_page)
            .unwrap();
        printer.imp().boards.set(boards).unwrap();
        printer
    }

    /// Prints to a PDF file without showing the print dialog.
    #[cfg(test)]
    pub fn export_pdf(&self, path: &str) -> PrintOperationResult {
        let operation = &self.imp().print_operation;
        operation.set_export_filename(path);
        operation
            .run(gtk::PrintOperationAction::Export, None::<&gtk::Window>)
            .unwrap_or(PrintOperationResult::Error)
    }
}
