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

use crate::G_LOG_DOMAIN;
use adw::prelude::*;
use adw::subclass::prelude::*;

use gtk::glib::{self, Object};
use gtk::{Orientation, Widget};

use crate::config;

mod imp {
    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for RascacielosGridLayout {
        const NAME: &'static str = "RascacielosGridLayout";
        type Type = super::RascacielosGridLayout;
        type ParentType = gtk::LayoutManager;
    }

    pub struct RascacielosGridLayout {
        margin_default_size: i32,
        margin_small_size: i32,
        margin_size_diff: i32,
    }

    impl Default for RascacielosGridLayout {
        fn default() -> Self {
            const DEF: i32 = 25;
            const SMALL: i32 = 10;
            const DIFF: i32 = DEF - SMALL;
            Self {
                margin_default_size: DEF,
                margin_small_size: SMALL,
                margin_size_diff: DIFF,
            }
        }
    }

    impl RascacielosGridLayout {
        fn calc_align(&self, size: i32) -> i32 {
            if size > config::MEDIUM_WINDOW_WIDTH {
                self.margin_default_size
            } else {
                let factor = RascacielosGridLayout::normalize(
                    size,
                    config::SMALL_WINDOW_WIDTH,
                    config::MEDIUM_WINDOW_WIDTH,
                );
                self.margin_small_size + (self.margin_size_diff as f64 * factor) as i32
            }
        }

        fn normalize(mut val: i32, min: i32, max: i32) -> f64 {
            val = val.clamp(min, max);
            (val - min) as f64 / (max - min) as f64
        }
    }

    impl ObjectImpl for RascacielosGridLayout {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            glib::debug!("Layout disposed");
        }
    }

    impl LayoutManagerImpl for RascacielosGridLayout {
        fn measure(&self, widget: &Widget, _: Orientation, _: i32) -> (i32, i32, i32, i32) {
            let minimum: i32;
            let natural: i32;
            let minimum_baseline = -1i32;
            let natural_baseline = -1i32;
            let child = widget.first_child().unwrap();
            if widget.is_visible() {
                let measure_width = child.measure(Orientation::Horizontal, -1);
                let measure_height = child.measure(Orientation::Vertical, -1);
                minimum = i32::max(measure_width.0, measure_height.0);
                natural = i32::max(measure_width.1, measure_height.1);
            } else {
                minimum = 0;
                natural = 0;
            }
            (minimum, natural, minimum_baseline, natural_baseline)
        }

        fn allocate(&self, widget: &Widget, width: i32, height: i32, baseline: i32) {
            let child = widget.first_child().unwrap();

            let halign = self.calc_align(width);
            let valign = self.calc_align(height);

            let child_width = i32::min(width, height) - i32::min(halign, valign) * 2;
            let child_height = child_width;

            let mut start = i32::max(halign, (width - child_width) / 2);
            let mut top = i32::max(valign, (height - child_height) / 2);

            //expand margin from top left, this is a 0-2 pixel difference
            let bottom = height - child_height - top;
            top = i32::max(top, bottom);
            let end = width - child_width - start;
            start = i32::max(start, end);

            let maximum_top_offset = 40; //align with the start menu
            top = i32::min(top, maximum_top_offset);

            let child_allocation = gtk::Allocation::new(start, top, child_width, child_height);
            child.size_allocate(&child_allocation, baseline);
        }
    }
}

glib::wrapper! {
    pub struct RascacielosGridLayout(ObjectSubclass<imp::RascacielosGridLayout>)
        @extends gtk::LayoutManager;
}

impl RascacielosGridLayout {
    pub fn new() -> Self {
        let grid_layout: Self = Object::builder().build();

        grid_layout
    }
}
