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

//! A clue on the edge of the grid, drawn as a box with a tip pointing into
//! the grid, like an arrow showing the direction to look in.

use adw::prelude::*;
use adw::subclass::prelude::*;

use gtk::glib;
use gtk::gsk::{self, FillRule, PathBuilder, Stroke};
use gtk::{Allocation, graphene, pango::AttrSize};

use crate::lib::puzzle::Side;

use std::cell::Cell;

/// How far the tip sticks out, as a fraction of the clue's depth.
const TIP: f32 = 0.3;
/// The space between neighbouring clues, in units of the clue size.
const GAP: f32 = 0.06;

/// The outline of a clue on `side` filling `rect`, with its tip towards
/// the grid.
pub fn clue_path(side: Side, rect: &graphene::Rect) -> gsk::Path {
    let (x, y, w, h) = (rect.x(), rect.y(), rect.width(), rect.height());
    // Work along the clue's row or column (u) and towards the grid (v).
    let (along, depth) = match side {
        Side::Top | Side::Bottom => (w, h),
        Side::Left | Side::Right => (h, w),
    };
    let gap = along.min(depth) * GAP;
    let tip = depth * TIP;
    let point = |u: f32, v: f32| match side {
        Side::Top => (x + u, y + v),
        Side::Bottom => (x + u, y + h - v),
        Side::Left => (x + v, y + u),
        Side::Right => (x + w - v, y + u),
    };
    let corners = [
        point(gap, gap),
        point(along - gap, gap),
        point(along - gap, depth - gap - tip),
        point(along / 2.0, depth - gap),
        point(gap, depth - gap - tip),
    ];

    let builder = PathBuilder::new();
    builder.move_to(corners[0].0, corners[0].1);
    for (px, py) in &corners[1..] {
        builder.line_to(*px, *py);
    }
    builder.close();
    builder.to_path()
}

/// The part of `rect` outside the tip, where the clue number goes.
pub fn clue_box(side: Side, rect: &graphene::Rect) -> graphene::Rect {
    let (x, y, w, h) = (rect.x(), rect.y(), rect.width(), rect.height());
    match side {
        Side::Top => graphene::Rect::new(x, y, w, h * (1.0 - TIP)),
        Side::Bottom => graphene::Rect::new(x, y + h * TIP, w, h * (1.0 - TIP)),
        Side::Left => graphene::Rect::new(x, y, w * (1.0 - TIP), h),
        Side::Right => graphene::Rect::new(x + w * TIP, y, w * (1.0 - TIP), h),
    }
}

mod imp {
    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for RascacielosClue {
        const NAME: &'static str = "RascacielosClue";
        type Type = super::RascacielosClue;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("rascacielos-clue");
            klass.set_accessible_role(gtk::AccessibleRole::Label);
        }
    }

    pub struct RascacielosClue {
        pub label: gtk::Label,
        pub side: Cell<Side>,
    }

    impl Default for RascacielosClue {
        fn default() -> Self {
            let label = gtk::Label::new(None);
            label.add_css_class("numeric");
            Self {
                label,
                side: Cell::new(Side::Top),
            }
        }
    }

    impl ObjectImpl for RascacielosClue {
        fn constructed(&self) {
            self.parent_constructed();
            self.label.set_parent(&*self.obj());
        }

        fn dispose(&self) {
            self.label.unparent();
        }
    }

    impl WidgetImpl for RascacielosClue {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
            let area = clue_box(self.side.get(), &rect);
            self.label.size_allocate(
                &Allocation::new(
                    area.x() as i32,
                    area.y() as i32,
                    area.width() as i32,
                    area.height() as i32,
                ),
                baseline,
            );
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if self.label.label().is_empty() {
                return;
            }
            let widget = self.obj();
            let rect = graphene::Rect::new(0.0, 0.0, widget.width() as f32, widget.height() as f32);
            let path = clue_path(self.side.get(), &rect);
            let color = widget.color();
            snapshot.append_fill(&path, FillRule::Winding, &color.with_alpha(0.12));
            snapshot.append_stroke(&path, &Stroke::new(1.5), &color.with_alpha(0.7));
            widget.snapshot_child(&self.label, snapshot);
        }
    }
}

glib::wrapper! {
    pub struct RascacielosClue(ObjectSubclass<imp::RascacielosClue>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RascacielosClue {
    pub fn new(side: Side) -> Self {
        let clue: Self = glib::Object::builder().build();
        clue.imp().side.set(side);
        clue
    }

    /// Shows `clue`, or nothing when the clue is missing.
    pub fn set_clue(&self, clue: Option<u8>) {
        let text = clue.map_or(String::new(), |c| c.to_string());
        self.imp().label.set_label(&text);
        self.update_property(&[gtk::accessible::Property::Label(&text)]);
        self.queue_draw();
    }

    pub fn set_font_size(&self, font_size: i32) {
        let label = &self.imp().label;
        let attr_list = label.attributes().unwrap_or_default();
        attr_list.change(AttrSize::new_size_absolute(font_size * gtk::pango::SCALE));
        label.set_attributes(Some(&attr_list));
    }
}
