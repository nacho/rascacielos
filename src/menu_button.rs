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
use gtk::glib;
use gtk::glib::subclass::InitializingObject;

mod imp {
    use super::*;

    #[glib::object_subclass]
    impl ObjectSubclass for RascacielosMenuButton {
        const NAME: &'static str = "RascacielosMenuButton";
        type Type = super::RascacielosMenuButton;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/nacho/rascacielos/ui/menu-button.ui")]
    pub struct RascacielosMenuButton {
        #[template_child]
        pub menu_fullscreen_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub menu_fullscreen_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub menu_unfullscreen_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub main_menu: TemplateChild<gtk::Popover>,
    }

    impl RascacielosMenuButton {
        fn set_fullscreen_button(&self, fullscreen: bool) {
            if !fullscreen {
                self.menu_fullscreen_stack
                    .set_visible_child(&*self.menu_fullscreen_button);
            } else {
                self.menu_fullscreen_stack
                    .set_visible_child(&*self.menu_unfullscreen_button);
            }
        }
    }

    impl ObjectImpl for RascacielosMenuButton {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for RascacielosMenuButton {
        fn realize(&self) {
            self.parent_realize();
            let window: gtk::Window = self.obj().root().and_downcast::<gtk::Window>().unwrap();
            self.set_fullscreen_button(window.is_fullscreen());
            window.connect_fullscreened_notify(glib::clone!(
                #[weak(rename_to = menu_button)]
                self,
                move |window| menu_button.set_fullscreen_button(window.is_fullscreen())
            ));
        }
    }
    impl BinImpl for RascacielosMenuButton {}
}

glib::wrapper! {
    pub struct RascacielosMenuButton(ObjectSubclass<imp::RascacielosMenuButton>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RascacielosMenuButton {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
