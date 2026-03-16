/* welcome.rs
 *
 * Copyright 2026 FatDawlf
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::subclass::prelude::*;
use gtk::glib;
mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/welcome.ui")]
    pub struct BrushWelcome {
        // Template widgets
        #[template_child]
        open_editor: TemplateChild<gtk::Button>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushWelcome {
        const NAME: &'static str = "BrushWelcome";
        type Type = super::BrushWelcome;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushWelcome {}
    impl WidgetImpl for BrushWelcome {}
    impl BoxImpl for BrushWelcome {}
}

glib::wrapper! {
    pub struct BrushWelcome(ObjectSubclass<imp::BrushWelcome>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushWelcome {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}

