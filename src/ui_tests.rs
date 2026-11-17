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

//! Smoke tests that start the application and open every window and
//! dialog. They need a display and a session bus, so they are ignored by
//! default. Run them from a desktop session with:
//!
//! ```sh
//! GSETTINGS_BACKEND=memory cargo test -- --ignored --test-threads=1
//! ```

use adw::prelude::*;
use gtk::glib;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::RascacielosApplication;

/// Runs the application, activates `actions` one after the other on the
/// main window once it is shown, then quits. Panics if the window never
/// appears or an action does not exist.
fn run_app_with_actions(actions: &'static [&'static str]) {
    crate::register_resources();
    let app = RascacielosApplication::new();
    let ran = Rc::new(Cell::new(false));

    app.connect_activate(glib::clone!(
        #[strong]
        ran,
        move |app| {
            // The window is created by the class handler, which runs after
            // this one, so wait for it.
            glib::timeout_add_local_once(
                Duration::from_millis(300),
                glib::clone!(
                    #[strong]
                    app,
                    #[strong]
                    ran,
                    move || {
                        let window = app.active_window().expect("no main window");
                        for action in actions {
                            window
                                .activate_action(action, None)
                                .unwrap_or_else(|_| panic!("unknown action {action}"));
                        }
                        ran.set(true);
                        glib::timeout_add_local_once(Duration::from_millis(500), move || {
                            app.quit()
                        });
                    }
                ),
            );
        }
    ));

    let status = app.run_with_args::<&str>(&[]);
    assert_eq!(status, glib::ExitCode::SUCCESS);
    assert!(ran.get(), "the main window was never shown");
}

#[test]
#[ignore = "needs a display and a session bus"]
fn open_every_dialog() {
    run_app_with_actions(&[
        "app.preferences-dialog",
        "app.print-generator-dialog",
        "app.shortcuts-dialog",
        "app.about-dialog",
    ]);
}

/// Prints puzzles of the smallest and largest sizes, one and four per page,
/// to PDF files in the temporary directory so their layout can be checked.
#[test]
#[ignore = "needs a display"]
fn print_to_pdf() {
    use crate::lib::board::RascacielosBoard;
    use crate::lib::enums::DifficultyCategory;
    use crate::lib::game::RascacielosGame;
    use crate::printer::RascacielosPrinter;

    gtk::init().unwrap();
    for size in [4, 7] {
        for per_page in [1, 4] {
            let boards = RascacielosGame::generate(size, DifficultyCategory::Hard, 4)
                .into_iter()
                .map(|(puzzle, solution)| {
                    RascacielosBoard::from_generated(puzzle, &solution, DifficultyCategory::Hard)
                })
                .collect();
            let path =
                std::env::temp_dir().join(format!("rascacielos-{size}x{size}-{per_page}.pdf"));
            let printer = RascacielosPrinter::new(boards, per_page);
            assert_eq!(
                printer.export_pdf(path.to_str().unwrap()),
                gtk::PrintOperationResult::Apply
            );
            assert!(std::fs::metadata(&path).unwrap().len() > 0);
            println!("{}", path.display());
        }
    }
}
