/* layer_item.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{
    glib::{self, clone, WeakRef},
    TemplateChild,
};
use std::{cell::OnceCell, collections::HashMap, ops::Mul};
use uuid::Uuid;

use crate::data::{blend_modes::BlendMode, layer::Layer};

mod imp {

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/layer-item.ui")]
    pub struct BrushLayerItem {
        #[template_child]
        pub container: TemplateChild<adw::Bin>,
        // Information
        #[template_child]
        pub layer_name: TemplateChild<gtk::Label>,
        #[template_child]
        pub opacity: TemplateChild<gtk::Label>,
        #[template_child]
        pub blend_mode: TemplateChild<gtk::Label>,
        #[template_child]
        pub icon: TemplateChild<gtk::Image>,

        // Children
        #[template_child]
        pub children_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub filters: TemplateChild<gtk::Box>,
        #[template_child]
        pub children: TemplateChild<gtk::Box>,

        //Buttons
        #[template_child]
        pub revealer_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub visible_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub lock_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub alpha_clip_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub alpha_lock_toggle: TemplateChild<gtk::ToggleButton>,

        // References
        pub layer: OnceCell<Uuid>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushLayerItem {
        const NAME: &'static str = "BrushLayerItem";
        type Type = super::BrushLayerItem;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("layer.toggle-revealer", None, |item, _, _| {
                let imp = item.imp();
                let revealer = &imp.children_revealer;
                revealer.set_reveal_child(!revealer.reveals_child());
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushLayerItem {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_click();
            obj.bind_revealer();
        }
    }
    impl WidgetImpl for BrushLayerItem {}
    impl BoxImpl for BrushLayerItem {}
}

glib::wrapper! {
    pub struct BrushLayerItem(ObjectSubclass<imp::BrushLayerItem>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushLayerItem {
    pub fn new(
        layer: &Layer,
        selected_layer: &Option<Uuid>,
        cache: &mut HashMap<Uuid, WeakRef<BrushLayerItem>>,
    ) -> Self {
        // Return cached widget
        if let Some(weak_widget) = cache.get(&layer.id()) {
            if let Some(widget) = weak_widget.upgrade() {
                return widget;
            }
        }

        let obj: BrushLayerItem = glib::Object::new();
        let imp = obj.imp();

        imp.layer.set(layer.id()).expect("ID already set");

        if let Some(selection) = selected_layer {
            obj.toggle_selected(selection);
        }
        obj.set_name(layer.name());
        obj.set_opacity(layer.opacity());
        obj.set_blend_mode(layer.blend_mode());
        obj.setup_visibility(layer);

        if let Some(children) = layer.children() {
            for child in children {
                let child_widget = BrushLayerItem::new(child, selected_layer, cache);
                imp.children.append(&child_widget);
            }
        }

        cache.insert(layer.id(), obj.downgrade());

        obj
    }

    pub fn reveal(&self) {
        self.imp().revealer_toggle.set_active(true);
    }

    fn bind_revealer(&self) {
        let imp = self.imp();
        let toggle = &imp.revealer_toggle;
        let revealer = &imp.children_revealer.get();

        toggle
            .bind_property("active", revealer, "reveal-child")
            .sync_create()
            .bidirectional()
            .build();
    }

    fn setup_visibility(&self, layer: &Layer) {
        let imp = self.imp();

        match layer {
            Layer::Pixel(_inner) => {
                // Pixel layers only show their filters (if any)
                imp.children.set_visible(false);
                imp.icon.set_icon_name(Some("folder-documents-symbolic"));
            }
            Layer::Group(_inner) => {
                imp.icon.set_icon_name(Some("folder-open-symbolic"));
                imp.alpha_lock_toggle.set_visible(false);
            }
            Layer::Fill(_inner) => {
                imp.children.set_visible(false);
                imp.alpha_lock_toggle.set_visible(false);
                imp.icon.set_icon_name(Some("fill-tool-symbolic"));
            }
            Layer::Filter(_inner) => {
                imp.alpha_clip_toggle.set_visible(false);
                imp.alpha_lock_toggle.set_visible(false);
                imp.lock_toggle.set_visible(false);
                // Filters have no children
                imp.children_revealer.set_reveal_child(false);
            }
        }
    }

    fn setup_click(&self) {
        let controller = gtk::GestureClick::new();

        controller.connect_pressed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _, _, _| {
                let imp = obj.imp();

                if let Some(uuid) = imp.layer.get() {
                    let _ = obj.activate_action(
                        "editor.activate-layer",
                        Some(&uuid.to_string().to_variant()),
                    );
                }
            }
        ));

        self.imp().container.add_controller(controller);
    }

    fn toggle_selected(&self, selected_layer: &Uuid) {
        let imp = self.imp();
        let container = imp.container.get();

        if let Some(id) = imp.layer.get() {
            if id == selected_layer {
                container.add_css_class("layer_selected");
            } else {
                container.remove_css_class("layer_selected");
            }
        }
    }

    fn set_name(&self, name: &str) {
        let imp = self.imp();
        let label = imp.layer_name.get();

        label.set_label(name);
    }

    fn set_opacity(&self, opacity: f32) {
        let imp = self.imp();

        let opacity = opacity.mul(100f32).floor().to_string() + "%";

        if opacity == "100%" {
            imp.opacity.set_visible(false);
        } else {
            imp.opacity.set_label(&opacity);
            imp.opacity.set_visible(true);
        }
    }

    fn set_blend_mode(&self, blend_mode: &BlendMode) {
        let imp = self.imp();

        if blend_mode == &BlendMode::Normal {
            imp.blend_mode.set_visible(false);
        } else {
            imp.blend_mode.set_visible(true);
            imp.blend_mode.set_label(blend_mode.name());
        }
    }

    pub fn update(&self, selected_layer: &Uuid, layer: &Layer) {
        self.toggle_selected(selected_layer);
        self.set_name(layer.name());
        self.set_opacity(layer.opacity());
        self.set_blend_mode(layer.blend_mode());
    }
}
