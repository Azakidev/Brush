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
    prelude::{RangeExt, ToVariant, WidgetExt},
    subclass::prelude::*,
};
use color::{Hsl, Oklab, OpaqueColor};
use gtk::{
    glib::{self, clone, object::ObjectExt, value::ToValue, Properties, Variant, VariantTy},
    TemplateChild,
};

mod imp {

    use std::cell::RefCell;

    use color::{Hsl, Oklab, OpaqueColor};

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::BrushColorSelector)]
    #[template(resource = "/art/FatDawlf/Brush/color-selector.ui")]
    pub struct BrushColorSelector {
        // HSV Sliders
        #[template_child]
        pub hue_slider: TemplateChild<gtk::Scale>,
        #[template_child]
        pub saturation_slider: TemplateChild<gtk::Scale>,
        #[template_child]
        pub value_slider: TemplateChild<gtk::Scale>,

        // HSV Labels
        #[template_child]
        pub hue_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub saturation_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub value_label: TemplateChild<gtk::SpinButton>,
        // HSV adjustments
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
            Self::derived_set_property(&self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(&self, id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.link_sliders();
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
                let _ = obj.activate_action("editor.set-color", None);
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
        let sl = &imp.saturation_slider;
        let sa = &imp.saturation_a;

        ss.connect_value_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |s| {
                let val = s.value();
                let imp = obj.imp();
                let label = &imp.saturation_label;

                label.set_value(val);
                let _ = obj.activate_action("editor.set-color", None);
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
                let _ = obj.activate_action("editor.set-color", None);
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

    pub fn color(&self) -> OpaqueColor<Oklab> {
        let hsl: OpaqueColor<Hsl> = color::OpaqueColor::new([self.h(), self.s(), self.v()]);
        let oklab: OpaqueColor<Oklab> = hsl.convert();

        oklab
    }
}
