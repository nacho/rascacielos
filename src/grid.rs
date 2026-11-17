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

use adw::prelude::*;
use adw::subclass::prelude::*;

use gtk::gio;
use gtk::gio::SimpleAction;
use gtk::glib;
use gtk::{
    EventControllerFocus,
    gio::SimpleActionGroup,
    glib::{Propagation, Variant, WeakRef, closure_local},
};

use crate::lib::backend::SudokuBackend;
use crate::lib::enums::{CompatValue, Coord, SudokuDirection};
use crate::lib::game::{StackAction, SudokuGame};
use crate::lib::utils::{new_shortcut, new_shortcut_with_args};

use crate::cell::SudokuCell;
use crate::connect_cb_action;
use crate::connect_cb_pref;
use crate::lib::puzzle::MAX_SIZE;
use crate::picker_popover::SudokuPickerPopover;

use std::cell::{Cell, OnceCell};
use std::pin::Pin;
use std::time::Duration;

mod imp {

    use std::array::from_fn;

    use gtk::{Allocation, CssProvider, Orientation, Separator, glib::SourceId};

    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for SudokuGrid {
        const NAME: &'static str = "SudokuGrid";
        type Type = super::SudokuGrid;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("sudoku-grid");
            new_shortcut(klass, "grid.escape", "Escape");

            new_shortcut_with_args(klass, "grid.move-up", "w", SudokuDirection::Up);
            new_shortcut_with_args(klass, "grid.move-down", "s", SudokuDirection::Down);
            new_shortcut_with_args(klass, "grid.move-left", "a", SudokuDirection::Left);
            new_shortcut_with_args(klass, "grid.move-right", "d", SudokuDirection::Right);
        }
    }

    /// The cells are created for the largest grid and the ones outside the
    /// current puzzle are hidden.
    pub struct SudokuGrid {
        pub cells: [[SudokuCell; MAX_SIZE]; MAX_SIZE],
        pub cell_horizontal_separators: [Separator; MAX_SIZE - 1],
        pub cell_vertical_separators: [Separator; MAX_SIZE - 1],
        pub size: Cell<usize>,
        pub focus_controller: WeakRef<EventControllerFocus>,
        pub selected_row: Cell<usize>,
        pub selected_col: Cell<usize>,
        pub picker_popover: SudokuPickerPopover,
        pub backend: OnceCell<SudokuBackend>,
        pub completed_css_provider: CssProvider,
        pub completed_source_id: Cell<Option<SourceId>>,
    }

    impl Default for SudokuGrid {
        fn default() -> Self {
            let cells: [[SudokuCell; MAX_SIZE]; MAX_SIZE] =
                std::array::from_fn(|row| std::array::from_fn(|col| SudokuCell::new(row, col)));

            let cell_horizontal_separators: [Separator; MAX_SIZE - 1] = from_fn(|_| {
                glib::Object::builder()
                    .property("css-name", "cell-separator")
                    .property("orientation", Orientation::Horizontal)
                    .build()
            });
            let cell_vertical_separators: [Separator; MAX_SIZE - 1] = from_fn(|_| {
                glib::Object::builder()
                    .property("css-name", "cell-separator")
                    .property("orientation", Orientation::Vertical)
                    .build()
            });

            Self {
                cells,
                cell_horizontal_separators,
                cell_vertical_separators,
                size: Cell::new(MAX_SIZE),
                completed_source_id: Default::default(),
                completed_css_provider: Default::default(),
                selected_row: Cell::new(0),
                selected_col: Cell::new(0),
                focus_controller: Default::default(),
                picker_popover: SudokuPickerPopover::new_unitialized(),
                backend: Default::default(),
            }
        }
    }

    impl SudokuGrid {
        pub fn selected_cell(&self) -> &SudokuCell {
            &self.cells[self.selected_row.get()][self.selected_col.get()]
        }

        fn update_selected(&self, cell: &SudokuCell) {
            let old_pos = self.selected_pos();
            if self.selected_cell() != cell {
                self.selected_cell().unselect();
                self.selected_row.set(cell.pos().row);
                self.selected_col.set(cell.pos().col);
                self.picker_popover.popdown();
            }

            //handles quiet select
            if self.selected_cell().accented() {
                self.update_highlighter(old_pos);
            }
        }

        fn move_cb(&self, variant: &Variant) {
            let dir = SudokuDirection::from_variant(variant);
            self.obj().child_focus(dir.unwrap().into());
        }

        fn setup_actions(&self) {
            let action_group = SimpleActionGroup::new();

            self.create_move_action("move-up", &action_group);
            self.create_move_action("move-down", &action_group);
            self.create_move_action("move-left", &action_group);
            self.create_move_action("move-right", &action_group);

            let action = SimpleAction::new("escape", None);
            connect_cb_action!(self.obj(), action, unselect, action_group);

            self.obj().insert_action_group("grid", Some(&action_group));
        }

        pub fn backend(&self) -> &SudokuBackend {
            self.backend.get().unwrap()
        }

        pub fn selected_pos(&self) -> Coord {
            Coord {
                row: self.selected_row.get(),
                col: self.selected_col.get(),
            }
        }

        fn update_warnings(&self) {
            let n = self.size.get();
            for row in &self.cells[..n] {
                for cell in &row[..n] {
                    cell.update_value_warnings();
                    cell.update_all_earmark_warnings();
                }
            }
        }

        /// Shows the cells and separators of the current puzzle, hides the
        /// rest and loads the values of the new game.
        fn update_size(&self) {
            let n = self.backend().game_size();
            self.size.set(n);
            for (row, cells) in self.cells.iter().enumerate() {
                for (col, cell) in cells.iter().enumerate() {
                    let visible = row < n && col < n;
                    cell.set_child_visible(visible);
                    if !visible {
                        continue;
                    }
                    cell.update_visibility();
                    //for rounded corners
                    let last = n - 1;
                    for (class, corner) in [
                        ("top-left", (0, 0)),
                        ("top-right", (0, last)),
                        ("bottom-left", (last, 0)),
                        ("bottom-right", (last, last)),
                    ] {
                        if (row, col) == corner {
                            cell.add_css_class(class);
                        }
                    }
                }
            }
            for (i, separator) in self
                .cell_horizontal_separators
                .iter()
                .chain(&self.cell_vertical_separators)
                .enumerate()
            {
                separator.set_child_visible(i % (MAX_SIZE - 1) < n - 1);
            }
            self.selected_row.set(n / 2);
            self.selected_col.set(n / 2);
            self.obj().queue_resize();
        }

        fn create_move_action(&self, name: &str, group: &SimpleActionGroup) {
            let action = gio::SimpleAction::new(name, Some(&i32::static_variant_type()));
            action.connect_activate(glib::clone!(
                #[weak(rename_to = grid)]
                self,
                move |_, variant| grid.move_cb(variant.unwrap())
            ));
            group.add_action(&action);
        }

        pub fn add_game_hooks(&self) {
            let binding = self.backend().game();
            let game = binding.as_ref().unwrap();
            game.connect_closure(
                "earmark-changed",
                false,
                closure_local! {
                    #[weak(rename_to = grid)] self,
                    move |game : SudokuGame, row: i32, col: i32, num: i32, enabled: bool| {
                        let cell = &grid.cells[row as usize][col as usize];
                        let action = game.current_action();
                        if action.is_single_earmarks_change() {
                            cell.select();
                            cell.set_earmark_visibility(num as usize, enabled);
                        }
                        else {
                            if !enabled && action == StackAction::InsertAndDisableAlignedEarmarks {
                                cell.imp().earmark_play_hide_animation (num as usize);
                            }
                            else {
                                cell.set_earmark_visibility(num as usize, enabled);
                            }
                        }

                        cell.update_earmark_warnings(num as usize);
                    }
                },
            );

            game.connect_closure("value-changed", false, closure_local!{
                #[weak(rename_to = grid)] self,
                move |_ : glib::Object, row: i32, col: i32, old_val: CompatValue, new_val: CompatValue| {
                    let cell = &grid.cells[row as usize][col as usize];
                    cell.select ();
                    cell.update_value_visibility(old_val.into(), new_val.into());
                    grid.update_value_highlighter(
                        Coord {row: row as usize, col: col as usize},
                        old_val.into(),
                        new_val.into()
                    );
                    let n = grid.size.get();
                    for row in 0..n {
                        for col in 0..n {
                            grid.cells[row][col].update_value_warnings();
                        }
                    }
                }
            });

            game.connect_closure(
                "paused",
                false,
                closure_local! {
                    #[weak(rename_to = grid)] self,
                    move |_ : glib::Object, paused: bool| {
                        grid.obj().set_can_focus(!paused);
                        if paused {
                            grid.obj().unselect();
                        }
                        else {
                            grid.grab_focus();
                        }
                    }
                },
            );
        }

        fn update_highlighter(&self, old_pos: Coord) {
            self.set_cell_highlighter(old_pos, false);
            self.set_cell_highlighter(self.selected_pos(), true);
        }

        fn update_value_highlighter(
            &self,
            old_pos: Coord,
            old_val: Option<usize>,
            new_val: Option<usize>,
        ) {
            if !self.backend().pref_highlight_numbers() {
                return;
            }

            //undo and redo jump the position
            if old_pos != self.selected_pos() {
                let cell = &self.cells[old_pos.row][old_pos.col];
                if old_val == new_val {
                    cell.highlight_value(false);
                } else if new_val.is_some() {
                    cell.highlight_value(true);
                }
                return;
            }

            let n = self.size.get();
            for row in 0..n {
                for col in 0..n {
                    let cell = &self.cells[row][col];
                    if *cell == *self.selected_cell() {
                        continue;
                    }

                    if let Some(value) = self.backend().game_value(Coord { row, col }) {
                        if let Some(old_val) = old_val
                            && old_val == value
                        {
                            cell.highlight_value(false);
                        } else if let Some(new_val) = new_val
                            && new_val == value
                        {
                            cell.highlight_value(true);
                        }
                    } else {
                        if let Some(old_val) = old_val {
                            cell.highlight_earmark(old_val, false);
                        }
                        if let Some(new_val) = new_val {
                            cell.highlight_earmark(new_val, true);
                        }
                    }
                }
            }
        }

        pub fn set_cell_highlighter(&self, pos: Coord, enabled: bool) {
            let target_cell = &self.cells[pos.row][pos.col];

            let backend = self.backend();
            let n = self.size.get();
            for row in 0..n {
                for col in 0..n {
                    let cell = &self.cells[row][col];
                    if *cell == *target_cell {
                        continue;
                    }

                    if backend.pref_highlight_numbers()
                        && let Some(target_value) = target_cell.imp().value()
                    {
                        let cell_value = cell.imp().value();
                        if let Some(cell_value) = cell_value
                            && cell_value == target_value
                        {
                            cell.highlight_value(enabled);
                        } else {
                            cell.highlight_earmark(target_value, enabled);
                        }
                    }

                    if !backend.game_fixed(Coord { row, col })
                        && backend.pref_highlight_row_column()
                        && (pos.row == row || pos.col == col)
                    {
                        cell.highlight_coord(enabled);
                    }
                }
            }
        }

        pub fn init(&self, backend: &SudokuBackend) {
            self.backend
                .set(backend.clone())
                .expect("Grid is already initialized");
            for row in &self.cells {
                for cell in row {
                    cell.init(backend);
                }
            }
            self.update_size();

            self.picker_popover.init(backend);

            backend.connect_closure(
                "game-changed",
                false,
                glib::closure_local!(
                    #[weak(rename_to = grid)]
                    self,
                    move |_: SudokuBackend| {
                        for row in &grid.cells {
                            for cell in row {
                                cell.add_game_hooks();
                            }
                        }
                        grid.update_size();
                        grid.update_warnings();
                        grid.add_game_hooks();

                        grid.obj().set_can_focus(true);
                        grid.obj().set_can_target(true);
                        grid.grab_focus();
                    }
                ),
            );

            backend.connect_pref_zoom_notify(glib::clone!(
                #[weak(rename_to = grid_layout)]
                self,
                move |_| grid_layout.obj().queue_resize()
            ));

            connect_cb_pref!(
                self,
                connect_pref_duplicate_warnings_notify,
                update_warnings
            );
            connect_cb_pref!(self, connect_pref_solution_warnings_notify, update_warnings);
            connect_cb_pref!(self, connect_pref_earmark_warnings_notify, update_warnings);

            self.add_game_hooks();

            self.update_warnings();
        }

        pub fn set_font_size(&self, height: i32) {
            let n = self.size.get();
            for row in &self.cells[..n] {
                for cell in &row[..n] {
                    let grid_padding = 10; //for size consistency with <= v50, remove with gtk5
                    let height = (height - grid_padding) / n as i32;
                    cell.set_font_sizes(height);
                }
            }
        }
    }

    impl ObjectImpl for SudokuGrid {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_accessible_role(gtk::AccessibleRole::Grid);

            let focus_controller = EventControllerFocus::new();
            self.focus_controller.set(Some(&focus_controller));
            self.focus_controller
                .upgrade()
                .unwrap()
                .connect_leave(glib::clone!(
                    #[weak(rename_to = grid)]
                    self,
                    move |_| {
                        let window: gtk::Window =
                            grid.obj().root().and_downcast::<gtk::Window>().unwrap();
                        if window.is_active() && grid.backend().game_exists() {
                            grid.obj().unselect();
                        }
                    }
                ));

            self.obj()
                .add_controller(self.focus_controller.upgrade().unwrap());
            self.setup_actions();

            for row in &self.cells {
                for cell in row {
                    cell.connect_closure(
                        "selected",
                        false,
                        closure_local!(
                            #[weak(rename_to = grid)]
                            self,
                            move |grid_cell: SudokuCell| {
                                grid.update_selected(&grid_cell);
                            }
                        ),
                    );

                    cell.connect_closure(
                        "grab-picker",
                        false,
                        closure_local!(
                            #[weak(rename_to = grid)]
                            self,
                            move |grid_cell: SudokuCell| {
                                grid_cell.set_picker(&grid.picker_popover);
                            }
                        ),
                    );
                    cell.set_parent(&*self.obj());
                }
            }

            for separator in &self.cell_horizontal_separators {
                separator.set_parent(&*self.obj());
            }
            for separator in &self.cell_vertical_separators {
                separator.set_parent(&*self.obj());
            }
        }

        fn dispose(&self) {
            for row in &self.cells {
                for cell in row {
                    cell.unparent();
                }
            }

            for separator in &self.cell_horizontal_separators {
                separator.unparent();
            }
            for separator in &self.cell_vertical_separators {
                separator.unparent();
            }
            println!("Grid disposed");
        }
    }

    impl WidgetImpl for SudokuGrid {
        fn focus(&self, direction_type: gtk::DirectionType) -> bool {
            use gtk::DirectionType::*;
            match direction_type {
                //this lets us control the focus when it comes from the headerbar and gtk/adwaita
                TabForward | TabBackward => {
                    if !self.focus_controller.upgrade().unwrap().contains_focus() {
                        self.grab_focus()
                    } else {
                        Propagation::Proceed.into() //propagate the event so that the focus moves to the headerbar
                    }
                }
                Up | Down | Left | Right => {
                    let last = self.size.get() - 1;
                    let (row, col) = (self.selected_row.get(), self.selected_col.get());
                    let step_back = |i: usize| if i == 0 { last } else { i - 1 };
                    let step_forward = |i: usize| if i == last { 0 } else { i + 1 };
                    let (row, col) = match direction_type {
                        Up => (step_back(row), col),
                        Down => (step_forward(row), col),
                        Left => (row, step_back(col)),
                        _ => (row, step_forward(col)),
                    };
                    self.cells[row][col].select()
                }
                _ => panic!("Failed to parse direction"),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.set_font_size(height);

            const CELL_SEPARATOR_SIZE: i32 = 1;
            let n = self.size.get();

            /// The start and length of each cell along one axis.
            fn spans(length: i32, n: usize) -> Vec<(i32, i32)> {
                let adjusted = length - CELL_SEPARATOR_SIZE * (n as i32 - 1);
                let cell = adjusted / n as i32;
                let mut remaining = adjusted % n as i32;
                let mut pos = 0;
                (0..n)
                    .map(|_| {
                        let size = if remaining > 0 {
                            remaining -= 1;
                            cell + 1
                        } else {
                            cell
                        };
                        let start = pos;
                        pos += size + CELL_SEPARATOR_SIZE;
                        (start, size)
                    })
                    .collect()
            }

            let cell_xs = spans(width, n);
            let cell_ys = spans(height, n);

            for row in 0..n {
                for col in 0..n {
                    let allocation = Allocation::new(
                        cell_xs[col].0,
                        cell_ys[row].0,
                        cell_xs[col].1,
                        cell_ys[row].1,
                    );
                    self.cells[row][col].size_allocate(&allocation, baseline);
                }
            }

            for i in 0..n - 1 {
                let (x, w) = cell_xs[i];
                let vertical_allocation = Allocation::new(x + w, 0, CELL_SEPARATOR_SIZE, height);
                self.cell_vertical_separators[i].size_allocate(&vertical_allocation, baseline);

                let (y, h) = cell_ys[i];
                let horizontal_allocation = Allocation::new(0, y + h, width, CELL_SEPARATOR_SIZE);
                self.cell_horizontal_separators[i].size_allocate(&horizontal_allocation, baseline);
            }
        }

        fn grab_focus(&self) -> bool {
            if self.backend().keyboard_pressed_recently() {
                self.selected_cell().select()
            } else {
                self.selected_cell().quiet_select()
            }
        }
    }
}

glib::wrapper! {
    pub struct SudokuGrid(ObjectSubclass<imp::SudokuGrid>)
        @extends gtk::Widget, gtk::Grid,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SudokuGrid {
    pub fn new_unitialized() -> Self {
        let grid: Self = glib::Object::builder().build();

        grid
    }

    pub fn init(&self, backend: &SudokuBackend) {
        self.imp().init(backend);
    }

    pub fn cancel_completed_animation(&self) {
        if let Some(animation) = self.imp().completed_source_id.take() {
            animation.remove();
        }
    }

    pub fn init_completed_animation(&self) {
        self.imp()
            .completed_source_id
            .set(Some(glib::timeout_add_local_once(
                Duration::from_millis(800),
                glib::clone!(
                    #[weak(rename_to = grid)]
                    self,
                    move || {
                        let play_order = grid.imp().backend().game_play_order();
                        grid.completed_animation(std::pin::Pin::new(play_order), 0);
                    }
                ),
            )));
    }

    pub fn completed_animation(&self, v: Pin<Vec<Coord>>, current: usize) {
        let duration = {
            match current {
                0 => 500,
                1 => 400,
                2..3 => 300,
                3..7 => 200,
                7..15 => 100,
                15.. => 50,
            }
        };

        let pos = v[current];
        let s = {
            ":root {--sudoku-completed-duration: ".to_owned() + &duration.to_string() + "ms" + ";}"
        };

        self.imp().completed_css_provider.load_from_string(&s);
        self.imp().cells[pos.row][pos.col].add_css_class("completed");
        if current + 1 < v.len() {
            self.imp()
                .completed_source_id
                .set(Some(glib::timeout_add_local_once(
                    Duration::from_millis(duration),
                    glib::clone!(
                        #[weak(rename_to = grid)]
                        self,
                        move || grid.completed_animation(v, current + 1)
                    ),
                )));
        } else {
            self.finish_completed_animation();
        }
    }

    pub fn finish_completed_animation(&self) {
        self.imp()
            .completed_source_id
            .set(Some(glib::timeout_add_local_once(
                Duration::from_millis(800),
                glib::clone!(
                    #[weak(rename_to = grid)]
                    self,
                    move || {
                        grid.imp().completed_source_id.set(None);
                        grid.activate_action("game-view.completed-dialog", None)
                            .unwrap();
                    }
                ),
            )));
    }

    pub fn unselect(&self) {
        self.imp().selected_cell().unselect();
        self.imp().picker_popover.popdown();
        self.imp()
            .set_cell_highlighter(self.imp().selected_pos(), false);
    }

    pub fn set_quiet_select(&self) {
        self.imp().selected_cell().set_accented(false);
    }
}
