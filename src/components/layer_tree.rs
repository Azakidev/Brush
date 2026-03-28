/* layer_tree.rs
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
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::subclass::prelude::*;
use gtk::{TemplateChild, glib};

mod imp {


    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/layer-tree.ui")]
    pub struct BrushLayerTree {
        // Widgets
        #[template_child]
        pub tree: TemplateChild<gtk::Box>,
        #[template_child]
        pub layer_opacity: TemplateChild<gtk::Scale>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushLayerTree {
        const NAME: &'static str = "BrushLayerTree";
        type Type = super::BrushLayerTree;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushLayerTree {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let layer_slider = obj.imp().layer_opacity;

            self.layer_opacity.get().connect_value_changed(
                    move |s| {
                        let val = s.value();
                    let _ = s.activate_action(
                        "editor.set-layer-opacity",
                        Some(&val.to_variant()),
                    );
                    }
            );
        }
    }
    impl WidgetImpl for BrushLayerTree {}
    impl BoxImpl for BrushLayerTree {}
}

glib::wrapper! {
    pub struct BrushLayerTree(ObjectSubclass<imp::BrushLayerTree>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushLayerTree {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
