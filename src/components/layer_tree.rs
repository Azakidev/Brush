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

use crate::data::blend_modes::BrushBlendMode;
use adw::{
    prelude::{RangeExt, ToVariant, WidgetExt},
    subclass::prelude::*,
};
use gtk::{
    TemplateChild,
    glib::{self, clone},
};
use strum::VariantNames;
use std::{cell::Cell, rc::Rc};

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
        #[template_child]
        pub blend_mode: TemplateChild<gtk::DropDown>,

        // Flags
        pub should_update: Rc<Cell<bool>>,
        pub compositing_enabled: Rc<Cell<bool>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushLayerTree {
        const NAME: &'static str = "BrushLayerTree";
        type Type = super::BrushLayerTree;
        type ParentType = gtk::Box;

        fn new() -> Self {
            Self {
                should_update: Rc::new(Cell::new(true)),
                compositing_enabled: Rc::new(Cell::new(true)),
                ..Default::default()
            }
        }

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

            obj.prepare_dropdown();
            obj.connect_opacity();
            obj.connect_dropdown();
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

    fn connect_opacity(&self) {
        self.imp().layer_opacity.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |s| {
                let val = s.value();
                let should_update = obj.imp().should_update.get();
                if should_update {
                    let _ = s.activate_action("editor.set-layer-opacity", Some(&val.to_variant()));
                }
            }
        ));
    }

    fn prepare_dropdown(&self) {
        let modes = BrushBlendMode::VARIANTS;

        let list = gtk::StringList::new(modes);

        self.imp().blend_mode.set_model(Some(&list));
    }

    fn connect_dropdown(&self) {
        self.imp().blend_mode.connect_selected_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |d| {
                let val = d.selected();
                let should_update = obj.imp().should_update.get();
                if should_update {
                    let _ = d.activate_action("editor.set-layer-blend", Some(&val.to_variant()));
                }
            }
        ));
    }
}
