/* color_selector.rs
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

use adw::{
    prelude::{RangeExt, WidgetExt},
    subclass::prelude::*,
};
use color::{Hsl, OpaqueColor};
use gtk::{
    CssProvider, TemplateChild,
    gdk::Display,
    glib::{self, Properties, clone, object::ObjectExt},
};
use std::cell::RefCell;

use crate::components::utils::color::Hsv;

mod imp {

    use std::{cell::Cell, rc::Rc};

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::BrushColorSelector)]
    #[template(resource = "/art/FatDawlf/Brush/color-selector.ui")]
    pub struct BrushColorSelector {
        // Sliders
        #[template_child]
        pub hue_slider: TemplateChild<gtk::Scale>,
        #[template_child]
        pub saturation_slider: TemplateChild<gtk::Scale>,
        #[template_child]
        pub value_slider: TemplateChild<gtk::Scale>,

        // Spin Buttons
        #[template_child]
        pub hue_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub saturation_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub value_label: TemplateChild<gtk::SpinButton>,

        // Adjustments
        #[template_child]
        pub hue_a: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub saturation_a: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub value_a: TemplateChild<gtk::Adjustment>,

        #[property(get, set)]
        pub h: RefCell<f32>,
        #[property(get, set)]
        pub s: RefCell<f32>,
        #[property(get, set)]
        pub v: RefCell<f32>,

        // Flags
        pub should_update: Rc<Cell<bool>>,
        pub css_provider: RefCell<CssProvider>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushColorSelector {
        const NAME: &'static str = "BrushColorSelector";
        type Type = super::BrushColorSelector;
        type ParentType = gtk::Box;

        fn new() -> Self {
            Self {
                h: RefCell::new(0f32),
                s: RefCell::new(0f32),
                v: RefCell::new(0f32),
                should_update: Rc::new(Cell::new(true)),
                css_provider: RefCell::new(CssProvider::new()),
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

    impl ObjectImpl for BrushColorSelector {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.link_sliders();
            obj.setup_css_provider();
            obj.update_properties();
        }
    }
    impl WidgetImpl for BrushColorSelector {}
    impl BoxImpl for BrushColorSelector {}
}

glib::wrapper! {
    pub struct BrushColorSelector(ObjectSubclass<imp::BrushColorSelector>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushColorSelector {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn link_sliders(&self) {
        let imp = self.imp();

        let hs = &imp.hue_slider;
        let hl = &imp.hue_label;
        let ha = &imp.hue_a;

        hs.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |s| {
                let val = s.value();
                let imp = obj.imp();
                let label = &imp.hue_label;

                label.set_value(val);
                if obj.imp().should_update.get() {
                    let _ = obj.activate_action("editor.set-color", None);
                    obj.update_properties();
                }
            }
        ));

        hl.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |l| {
                let imp = obj.imp();
                let slider = &imp.hue_slider;
                let val = l.value();

                slider.set_value(val);
            }
        ));

        self.bind_property("h", &ha.get(), "value")
            .bidirectional()
            .sync_create()
            .build();

        let ss = &imp.saturation_slider;
        let sl = &imp.saturation_label;
        let sa = &imp.saturation_a;

        ss.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |s| {
                let val = s.value();
                let imp = obj.imp();
                let label = &imp.saturation_label;

                label.set_value(val);
                if obj.imp().should_update.get() {
                    let _ = obj.activate_action("editor.set-color", None);
                    obj.update_properties();
                }
            }
        ));

        sl.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |l| {
                let imp = obj.imp();
                let slider = &imp.saturation_slider;
                let val = l.value();

                slider.set_value(val);
            }
        ));

        self.bind_property("s", &sa.get(), "value")
            .bidirectional()
            .sync_create()
            .build();

        let vs = &imp.value_slider;
        let vl = &imp.value_label;
        let va = &imp.value_a;

        vs.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |s| {
                let val = s.value();
                let imp = obj.imp();
                let label = &imp.value_label;

                label.set_value(val);
                if obj.imp().should_update.get() {
                    let _ = obj.activate_action("editor.set-color", None);
                    obj.update_properties();
                }
            }
        ));

        vl.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |l| {
                let imp = obj.imp();
                let slider = &imp.value_slider;
                let val = l.value();

                slider.set_value(val);
            }
        ));

        self.bind_property("v", &va.get(), "value")
            .bidirectional()
            .sync_create()
            .build();
    }

    pub fn color(&self) -> OpaqueColor<Hsv> {
        let hsl: OpaqueColor<Hsv> = color::OpaqueColor::new([self.h(), self.s(), self.v()]);

        hsl
    }

    pub fn set_color(&self, color: &OpaqueColor<Hsv>) {
        let [h, s, v] = color.components;

        self.imp().should_update.set(false);
        self.set_h(h);
        self.set_s(s);
        self.set_v(v);
        self.imp().should_update.set(true);

        self.update_properties();
    }

    fn setup_css_provider(&self) {
        let provider = self.imp().css_provider.borrow();

        gtk::style_context_add_provider_for_display(
            &Display::default().unwrap(),
            &provider.clone(),
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn update_properties(&self) {
        let provider = self.imp().css_provider.borrow();

        let hsv: OpaqueColor<Hsv> = OpaqueColor::new([self.h(), self.s(), self.v()]);

        let sat = make_saturation_gradient(hsv);
        let val = make_value_gradient(hsv);

        provider.load_from_string(&format!(
            ":root {{ --sat_gradient: {}; --val_gradient: {}; }}",
            sat, val
        ));
    }
}

fn make_saturation_gradient(hsv: OpaqueColor<Hsv>) -> String {
    let [h, _, hsv_v] = hsv.components;

    let full_hsv: OpaqueColor<Hsv> = OpaqueColor::new([h, 100f32, hsv_v]);
    let full_hsl: OpaqueColor<Hsl> = full_hsv.convert();

    let [_, _, l] = full_hsl.components;

    format!(
        "linear-gradient(to right, \
         hsl({h}, 0%, {hsv_v}%), \
         hsl({h}, 100%, {l}%))"
    )
}

fn make_value_gradient(hsv: OpaqueColor<Hsv>) -> String {
    let [h, hsv_s, _] = hsv.components;

    let full_hsv: OpaqueColor<Hsv> = OpaqueColor::new([h, hsv_s, 100f32]);
    let full_hsl: OpaqueColor<Hsl> = full_hsv.convert();

    let [_, s, l] = full_hsl.components;

    format!(
        "linear-gradient(to right, \
         #000, \
         hsl({h}, {s}%, {l}%))",
    )
}
