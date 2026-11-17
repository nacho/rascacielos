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

use adw::prelude::*;
use adw::subclass::prelude::*;

use gtk::CompositeTemplate;
use gtk::PrintOperationResult;
use gtk::gio;
use gtk::glib;
use gtk::glib::subclass::InitializingObject;

use crate::lib::backend::RascacielosBackend;
use crate::lib::board::RascacielosBoard;
use crate::lib::enums::DifficultyCategory;
use crate::lib::game::RascacielosGame;
use crate::lib::generator;
use crate::lib::puzzle::{MIN_SIZE, Tier};
use crate::menu_button::RascacielosMenuButton;
use crate::{game, printer::RascacielosPrinter};

use std::cell::OnceCell;

mod imp {
    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for RascacielosPrintGeneratorDialog {
        const NAME: &'static str = "RascacielosPrintGeneratorDialog";
        type Type = super::RascacielosPrintGeneratorDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            RascacielosMenuButton::ensure_type();
            obj.init_template();
        }
    }

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/nacho/rascacielos/ui/print-generator-dialog.ui")]
    pub struct RascacielosPrintGeneratorDialog {
        pub backend: OnceCell<RascacielosBackend>,
        #[template_child]
        pub puzzles_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub puzzles_per_page_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub size_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub difficulty_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub print_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub print_current_puzzle_row: TemplateChild<adw::SwitchRow>,
    }

    #[gtk::template_callbacks]
    impl RascacielosPrintGeneratorDialog {
        #[template_callback]
        fn print_button_pressed(&self, _: &gtk::Button) {
            let mut puzzles_to_generate = self.puzzles_row.adjustment().value() as u8;
            let puzzles_per_page = self.puzzles_per_page_row.adjustment().value() as i32;
            let size = self.selected_size();
            let difficulty_category = self.selected_difficulty();
            let mut boards: Vec<RascacielosBoard> = Default::default();

            if self.print_current_puzzle_row.is_active() {
                boards.push(game!(self.backend.get().unwrap()).board().clone());
                puzzles_to_generate -= 1;
                if puzzles_to_generate == 0 {
                    self.send_to_printer(boards, puzzles_per_page);
                    return;
                }
            }

            // Generating many large puzzles takes a while, keep the main
            // loop running meanwhile.
            self.print_button.set_sensitive(false);
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                async move {
                    let generated = gio::spawn_blocking(move || {
                        RascacielosGame::generate(
                            size,
                            difficulty_category,
                            puzzles_to_generate as usize,
                        )
                    })
                    .await
                    .expect("The puzzle generator failed");
                    boards.extend(generated.into_iter().map(|(puzzle, solution)| {
                        RascacielosBoard::from_generated(puzzle, &solution, difficulty_category)
                    }));
                    dialog.print_button.set_sensitive(true);
                    dialog.send_to_printer(boards, puzzles_per_page);
                }
            ));
        }

        fn selected_size(&self) -> usize {
            MIN_SIZE + self.size_row.selected() as usize
        }

        fn selected_difficulty(&self) -> DifficultyCategory {
            (self.difficulty_row.selected() + 1).into()
        }

        /// Moves the difficulty to Hard when the size cannot be Very Hard.
        fn check_difficulty(&self) {
            let tier = self.selected_difficulty().to_tier().unwrap_or(Tier::Easy);
            if !generator::available(self.selected_size(), tier) {
                self.difficulty_row
                    .set_selected(DifficultyCategory::Hard as u32 - 1);
            }
        }

        pub fn init(&self, backend: &RascacielosBackend) {
            self.backend
                .set(backend.clone())
                .expect("Print Generator Dialog is already initialized");
            self.print_current_puzzle_row
                .set_visible(backend.game_exists());
            let p_otions = backend.print_options();
            self.puzzles_row.set_value(p_otions.0);
            self.puzzles_per_page_row.set_value(p_otions.1);
            self.difficulty_row
                .set_selected((p_otions.2 as i32 - 1) as u32);
            self.size_row
                .set_selected(p_otions.3.saturating_sub(MIN_SIZE) as u32);

            for row in [&*self.size_row, &*self.difficulty_row] {
                row.connect_selected_notify(glib::clone!(
                    #[weak(rename_to = dialog)]
                    self,
                    move |_| dialog.check_difficulty()
                ));
            }
            self.check_difficulty();
        }

        fn send_to_printer(&self, boards: Vec<RascacielosBoard>, puzzles_per_page: i32) {
            let printer = RascacielosPrinter::new(boards, puzzles_per_page);
            let window: gtk::Window = self.obj().root().and_downcast::<gtk::Window>().unwrap();

            if printer.imp().print_puzzles(&window) == PrintOperationResult::Apply {
                for board in printer.imp().boards.get().unwrap() {
                    self.backend.get().unwrap().save_printed_board(board);
                }
            }
        }
    }

    impl ObjectImpl for RascacielosPrintGeneratorDialog {
        fn constructed(&self) {
            self.parent_constructed();

            //makes sure the number of puzzles to print is a factor of the puzzles per page
            self.puzzles_per_page_row
                .adjustment()
                .connect_value_changed(glib::clone!(
                    #[weak(rename_to = dialog)]
                    self,
                    move |adjustment| {
                        let per_page = adjustment.value();
                        dialog.puzzles_row.adjustment().set_step_increment(per_page);
                        dialog
                            .puzzles_row
                            .adjustment()
                            .set_page_increment(per_page * 5.0);
                        dialog.puzzles_row.adjustment().set_lower(per_page);
                        if dialog.puzzles_row.adjustment().value() as i64 % per_page as i64 != 0 {
                            dialog.puzzles_row.adjustment().set_value(
                                per_page - dialog.puzzles_row.adjustment().value() % per_page,
                            );
                        }
                    }
                ));
        }

        fn dispose(&self) {
            let backend = &self.backend.get().unwrap();
            backend.set_print_options(
                self.puzzles_row.value(),
                self.puzzles_per_page_row.value(),
                self.selected_difficulty(),
                self.selected_size(),
            );
        }
    }

    impl WidgetImpl for RascacielosPrintGeneratorDialog {}
    impl AdwDialogImpl for RascacielosPrintGeneratorDialog {}
}

glib::wrapper! {
    pub struct RascacielosPrintGeneratorDialog(ObjectSubclass<imp::RascacielosPrintGeneratorDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl RascacielosPrintGeneratorDialog {
    pub fn new(backend: &RascacielosBackend) -> Self {
        let dialog: Self = glib::Object::builder().build();

        dialog.imp().init(backend);
        dialog
    }
}
