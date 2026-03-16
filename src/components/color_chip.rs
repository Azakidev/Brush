/* color_chip.rs
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

use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::gsk::RoundedRect;
use gtk::{gdk, glib, graphene};

mod imp {
    use super::*;

    #[allow(dead_code)]
    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::BrushColorChip)]
    pub struct BrushColorChip {
        #[property(get, set = Self::set_color, explicit_notify)]
        pub color: RefCell<Option<gdk::RGBA>>,
    }

    impl Default for BrushColorChip {
        fn default() -> Self {
            BrushColorChip {
                color: RefCell::new(Some(gdk::RGBA::BLACK)),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushColorChip {
        const NAME: &'static str = "BrushColorChip";
        type Type = super::BrushColorChip;
        type ParentType = gtk::Widget; // Base widget type

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("colorChip");
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for BrushColorChip {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_valign(gtk::Align::Center);
            obj.set_halign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for BrushColorChip {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(color) = *self.color.borrow() {
                let width = self.obj().width() as f32;
                let height = self.obj().height() as f32;

                let rect = graphene::Rect::new(0.0, 0.0, width, height);

                let radius = 4f32;

                let radius = radius.min(width / 2.0).min(height / 2.0);
                let rounded_rect = RoundedRect::from_rect(rect, radius);

                snapshot.push_rounded_clip(&rounded_rect);
                snapshot.append_color(&color, &rect);
                snapshot.pop();
            }
        }
    }

    impl BrushColorChip {
        fn set_color(&self, value: Option<gdk::RGBA>) {
            if *self.color.borrow() != value {
                self.color.replace(value);
                self.obj().notify_color(); // Notify GObject system of change
                self.obj().queue_draw(); // Trigger the redraw
            }
        }
    }
}

glib::wrapper! {
    pub struct BrushColorChip(ObjectSubclass<imp::BrushColorChip>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[allow(dead_code)]
impl BrushColorChip {
    fn new() -> Self {
        glib::Object::new()
    }
}
