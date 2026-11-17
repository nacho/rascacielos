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
// The boards are small fixed-size grids, indexing them by row and column
// reads better than iterator chains.
#![allow(clippy::needless_range_loop)]

mod lib {
    pub mod backend;
    pub mod board;
    pub mod data;
    pub mod enums;
    pub mod game;
    #[macro_use]
    pub mod utils;
    // Not used by the game yet, the sudoku board is replaced later.
    #[allow(dead_code)]
    pub mod generator;
    #[allow(dead_code)]
    pub mod puzzle;
    #[allow(dead_code)]
    pub mod solver;
}

mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}
mod cell;
mod clue_frame;
mod earmark_picker;
mod game_view;
mod grid;
mod grid_layout;
mod menu_button;
mod picker_popover;
mod preferences_dialog;
mod print_generator_dialog;
mod printer;
mod start_view;
#[cfg(test)]
mod ui_tests;
mod value_picker;
mod window;

use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::lib::backend::SudokuBackend;
use gettextrs::{LocaleCategory, setlocale};
use gtk::gio;
use gtk::gio::ApplicationFlags;
use gtk::glib::{self, Char, OptionArg, OptionFlags};

mod imp {

    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for RascacielosApplication {
        const NAME: &'static str = "RascacielosApplication";
        type Type = super::RascacielosApplication;
        type ParentType = adw::Application;
    }

    pub struct RascacielosApplication {
        pub backend: SudokuBackend,
    }

    impl Default for RascacielosApplication {
        fn default() -> Self {
            Self {
                backend: SudokuBackend::new(),
            }
        }
    }

    impl ApplicationImpl for RascacielosApplication {
        fn startup(&self) {
            self.parent_startup();
            self.setup_actions();
        }

        fn activate(&self) {
            let app = self.obj();
            let window = window::SudokuWindow::new(app.as_ref(), &self.backend);
            window.present();
        }

        fn shutdown(&self) {
            for window in self.obj().windows() {
                window.close();
            }
            self.parent_shutdown();
        }

        fn handle_local_options(
            &self,
            options: &glib::VariantDict,
        ) -> std::ops::ControlFlow<glib::ExitCode> {
            if options.contains("version") {
                glib::g_print!("rascacielos {}\n", config::VERSION);
                return std::ops::ControlFlow::Break(glib::ExitCode::SUCCESS);
            }

            std::ops::ControlFlow::Continue(())
        }
    }

    impl RascacielosApplication {
        fn setup_actions(&self) {
            let actions = [create_cb_entry!(self.obj(), "quit", quit)];

            self.obj().add_action_entries(actions);
            self.obj()
                .set_accels_for_action("app.quit", &["<Primary>Q"]);
            self.obj()
                .set_accels_for_action("app.preferences-dialog", &["<Primary>comma"]);
        }
    }

    impl ObjectImpl for RascacielosApplication {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            println!("app disposed");
        }
    }
    impl GtkApplicationImpl for RascacielosApplication {}
    impl AdwApplicationImpl for RascacielosApplication {}
}

glib::wrapper! {
    pub struct RascacielosApplication(ObjectSubclass<imp::RascacielosApplication>)
    @extends adw::Application, gtk::Application, gio::Application,
    @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for RascacielosApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl RascacielosApplication {
    pub fn new() -> Self {
        let obj: RascacielosApplication = glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("flags", ApplicationFlags::FLAGS_NONE)
            .property("resource-base-path", "/io/github/nacho/rascacielos")
            .build();

        use gettextrs::gettext;
        obj.add_main_option(
            "version",
            Char('v' as i8),
            OptionFlags::NONE,
            OptionArg::None,
            /* Help string for command line --version flag */
            &gettext("Show release version"),
            None,
        );

        obj
    }
}

fn register_resources() {
    gio::resources_register_include!("resources.gresource").expect("failed to load resources");
}

fn main() -> glib::ExitCode {
    use config::*;
    setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to bind the text domain codeset");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    register_resources();

    let app = RascacielosApplication::new();
    app.run()
}
