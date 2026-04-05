/* color_wheel.rs
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

use adw::{prelude::*, subclass::prelude::*};
use color::OpaqueColor;
use gtk::{
    Snapshot,
    gdk::RGBA,
    glib::{self, Properties, clone},
    graphene::{Point, Rect},
    gsk::{self, ColorStop, RoundedRect},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::components::utils::color::{Hsv, to_rgba};

const TRACK_WIDTH: f32 = 20f32;
const TRIANGLE_GAP: f32 = 4f32;

#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
enum ColorWheelDragState {
    H,
    SV,
}

mod imp {

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::BrushColorWheel)]
    pub struct BrushColorWheel {
        pub mouse_pos: Cell<(f32, f32)>,

        #[property(get, set)]
        pub h: RefCell<f32>,
        #[property(get, set)]
        pub s: RefCell<f32>,
        #[property(get, set)]
        pub v: RefCell<f32>,

        // Flags
        pub should_update: Rc<Cell<bool>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushColorWheel {
        const NAME: &'static str = "BrushColorWheel";
        type Type = super::BrushColorWheel;
        type ParentType = gtk::Widget;

        fn new() -> Self {
            Self {
                h: RefCell::new(0f32),
                s: RefCell::new(0f32),
                v: RefCell::new(0f32),
                should_update: Rc::new(Cell::new(true)),
                ..Default::default()
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for BrushColorWheel {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);

            self.obj().queue_draw();
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_motion_controller();
            obj.setup_drag();
            obj.setup_click();
        }
    }

    impl WidgetImpl for BrushColorWheel {
        fn snapshot(&self, snapshot: &Snapshot) {
            self.obj().draw_wheel(snapshot);
            self.obj().draw_triangle(snapshot);
            // Coordinates
            let hue = self.obj().coodinates_from_hue();
            self.obj().draw_indicator(snapshot, hue, 8f32);
            let sv = self.obj().coodinates_from_sv();
            self.obj().draw_indicator(snapshot, sv, 5f32);
        }
    }
}

glib::wrapper! {
    pub struct BrushColorWheel(ObjectSubclass<imp::BrushColorWheel>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushColorWheel {
    fn draw_wheel(&self, snapshot: &Snapshot) {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let min = w.min(h);

        let center = Point::new(w / 2f32, h / 2f32);
        let bounds = Rect::new((w - min) / 2f32, (h - min) / 2f32, min, min);

        let stops = self.color_stops();

        let builder = gsk::PathBuilder::new();
        builder.add_circle(&center, min / 2f32);
        builder.add_circle(&center, min / 2f32 - TRACK_WIDTH);
        let path = builder.to_path();

        snapshot.push_fill(&path, gsk::FillRule::EvenOdd);

        snapshot.append_conic_gradient(&bounds, &center, 90f32, &stops);

        snapshot.pop();
    }

    fn draw_triangle(&self, snapshot: &Snapshot) {
        let hsv_max = [self.h(), 100f32, 100f32];
        let color: OpaqueColor<Hsv> = OpaqueColor::new(hsv_max);
        let rgba = to_rgba(&color);

        let (w, h) = (self.width() as f32, self.height() as f32);

        let min = w.min(h) - (TRACK_WIDTH + TRIANGLE_GAP) * 2f32;
        let bounds = Rect::new((w - min) / 2f32, (h - min) / 2f32, min, min);

        let [pb, pw, ph] = self.triange_points();
        let (pbx, pby) = pb;
        let (pwx, pwy) = pw;
        let (phx, phy) = ph;

        let builder = gsk::PathBuilder::new();
        builder.move_to(pbx, pby);
        builder.line_to(pwx, pwy);
        builder.line_to(phx, phy);
        builder.close();

        let path = builder.to_path();

        snapshot.push_fill(&path, gsk::FillRule::Winding);

        snapshot.append_linear_gradient(
            &bounds,
            &Point::new(phx, phy),
            &Point::new(pwx, pwy),
            &[
                gsk::ColorStop::new(0.0, rgba),
                gsk::ColorStop::new(1.0, RGBA::WHITE),
            ],
        );

        snapshot.append_linear_gradient(
            &bounds,
            &Point::new(pwx, pwy),
            &Point::new(pbx, pby),
            &[
                gsk::ColorStop::new(0.0, RGBA::TRANSPARENT),
                gsk::ColorStop::new(1.0, RGBA::BLACK),
            ],
        );

        snapshot.pop();
    }

    fn draw_indicator(&self, snapshot: &Snapshot, (x, y): (f32, f32), radius: f32) {
        let border = (radius / 6f32).min(1f32);

        let rect = Rect::new(x - radius, y - radius, radius * 2f32, radius * 2f32);
        let circle = RoundedRect::from_rect(rect, radius);

        snapshot.append_border(&circle, &[border; 4], &[RGBA::BLACK; 4]);

        let inner_rect = Rect::new(
            x - (radius - border),
            y - (radius - border),
            (radius - border) * 2f32,
            (radius - border) * 2f32,
        );
        let inner_circle = RoundedRect::from_rect(inner_rect, radius);
        snapshot.append_border(&inner_circle, &[border * 2f32; 4], &[RGBA::WHITE; 4]);
    }

    fn color_stops(&self) -> [ColorStop; 7] {
        [
            ColorStop::new(0.00, RGBA::new(1f32, 0f32, 0f32, 1f32)),
            ColorStop::new(0.16, RGBA::new(1f32, 1f32, 0f32, 1f32)),
            ColorStop::new(0.33, RGBA::new(0f32, 1f32, 0f32, 1f32)),
            ColorStop::new(0.50, RGBA::new(0f32, 1f32, 1f32, 1f32)),
            ColorStop::new(0.66, RGBA::new(0f32, 0f32, 1f32, 1f32)),
            ColorStop::new(0.83, RGBA::new(1f32, 0f32, 1f32, 1f32)),
            ColorStop::new(1.00, RGBA::new(1f32, 0f32, 0f32, 1f32)),
        ]
    }

    fn setup_click(&self) {
        let controller = gtk::GestureClick::new();

        controller.connect_pressed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _, _, _| {
                let (x, y) = obj.imp().mouse_pos.get();

                if obj.test_wheel((x, y)) {
                    let hue = obj.to_wheel_angle((x, y));
                    obj.set_h(hue);
                }

                if obj.test_triangle((x, y)) {
                    let (w1, w2, w3) = obj.to_triangle_coords((x, y));

                    let s = (w3 / (w2 + w3)) * 100f32;
                    let v = 100f32 - (w1 * 100f32);

                    obj.set_s(s.clamp(0f32, 100f32));
                    obj.set_v(v.clamp(0f32, 100f32));
                }
            }
        ));

        self.add_controller(controller);
    }

    fn setup_drag(&self) {
        let controller = gtk::GestureDrag::new();

        let state: Rc<Cell<Option<ColorWheelDragState>>> = Rc::new(Cell::new(None));

        controller.connect_drag_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            state,
            move |_, _x, _y| {
                let (x, y) = obj.imp().mouse_pos.get();

                if obj.test_wheel((x, y)) {
                    state.set(Some(ColorWheelDragState::H));
                }

                if obj.test_triangle((x, y)) {
                    state.set(Some(ColorWheelDragState::SV));
                }
            }
        ));

        controller.connect_drag_end(clone!(
            #[weak]
            state,
            move |_, _x, _y| {
                state.set(None);
            }
        ));

        controller.connect_drag_update(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            state,
            move |_, _x, _y| {
                let (x, y) = obj.imp().mouse_pos.get();

                if let Some(state) = state.get() {
                    match state {
                        ColorWheelDragState::H => {
                            let hue = obj.to_wheel_angle((x, y));
                            obj.set_h(hue);
                        }
                        ColorWheelDragState::SV => {
                            let (w1, w2, w3) = obj.to_triangle_coords((x, y));

                            let s = (w3 / (w2 + w3)) * 100f32;
                            let v = 100f32 - (w1 * 100f32);

                            if v > f32::EPSILON {
                                obj.set_s(s.clamp(0f32, 100f32));
                            }
                            obj.set_v(v.clamp(0f32, 100f32));
                        }
                    }
                }
            }
        ));

        self.add_controller(controller);
    }

    fn setup_motion_controller(&self) {
        let motion = gtk::EventControllerMotion::new();
        let weak_self = self.downgrade();

        motion.connect_motion(move |_, x, y| {
            if let Some(obj) = weak_self.upgrade() {
                obj.imp().mouse_pos.set((x as f32, y as f32));
            }
        });
        self.add_controller(motion);
    }

    fn triange_points(&self) -> [(f32, f32); 3] {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let (cx, cy) = (w / 2f32, h / 2f32);

        let min = w.min(h) - (TRACK_WIDTH + TRIANGLE_GAP) * 2f32;
        let r = min / 2f32;

        let pb = (cx, cy - r); // Top (Black)
        let pw = (cx - (r * 0.866), cy + (r * 0.5)); // Bottom Left (White)
        let ph = (cx + (r * 0.866), cy + (r * 0.5)); // Bottom Right (Pure Hue)

        [pb, pw, ph]
    }

    fn to_wheel_angle(&self, p: (f32, f32)) -> f32 {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let (cx, cy) = (w / 2f32, h / 2f32);
        let (x, y) = p;

        let dx = x - cx;
        let dy = y - cy;

        let mut rad = dy.atan2(dx);

        if rad < 0f32 {
            rad += 2f32 * std::f32::consts::PI;
        }

        rad.to_degrees()
    }

    fn coodinates_from_hue(&self) -> (f32, f32) {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let (cx, cy) = (w / 2f32, h / 2f32);
        let min = w.min(h);

        let hue = self.h().to_radians();

        let r = (min / 2f32) - (TRACK_WIDTH / 2f32);

        let x = cx + r * hue.cos();
        let y = cy + r * hue.sin();

        (x, y)
    }

    fn to_triangle_coords(&self, p: (f32, f32)) -> (f32, f32, f32) {
        let (px, py) = p;
        let [v1, v2, v3] = self.triange_points();

        let (x1, y1) = v1;
        let (x2, y2) = v2;
        let (x3, y3) = v3;

        let denominator = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3);

        // Guard against division by zero for degenerate triangles
        if denominator.abs() < f32::EPSILON {
            return (0f32, 0f32, 0f32);
        }

        let w1 = ((y2 - y3) * (px - x3) + (x3 - x2) * (py - y3)) / denominator;
        let w2 = ((y3 - y1) * (px - x3) + (x1 - x3) * (py - y3)) / denominator;
        let w3 = 1.0 - w1 - w2;

        (w1, w2, w3)
    }

    fn coodinates_from_sv(&self) -> (f32, f32) {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let (s, v) = (self.s() / 100f32, self.v() / 100f32);

        let (cx, _cy) = (w / 2f32, h / 2f32);
        let [(_pbx, pby), (_pwx, pwy), _ph] = self.triange_points();

        let y_top = pby;
        let y_bottom = pwy;

        let y = y_top + (y_bottom - y_top) * v;

        let x_left_at_y =
            cx - (y - y_top) * (30f32.to_radians().cos() / (1.0 + 30f32.to_radians().sin()));
        let x_right_at_y =
            cx + (y - y_top) * (30f32.to_radians().cos() / (1.0 + 30f32.to_radians().sin()));

        let x = x_left_at_y + (x_right_at_y - x_left_at_y) * s;

        (x, y)
    }

    fn test_wheel(&self, p: (f32, f32)) -> bool {
        let (w, h) = (self.width() as f32, self.height() as f32);

        let (cx, cy) = (w / 2f32, h / 2f32);
        let (x, y) = p;

        let center = Point::new(cx, cy);
        let pointer = Point::new(x, y);

        let (distance, _, _) = pointer.distance(&center);

        let major = w.min(h) / 2f32;
        let minor = (w.min(h) - (TRACK_WIDTH + TRIANGLE_GAP) * 2f32) / 2f32;

        minor < distance && distance < major
    }

    fn test_triangle(&self, p: (f32, f32)) -> bool {
        let (w1, w2, w3) = self.to_triangle_coords(p);

        w1 >= 0.0 && w2 >= 0.0 && w3 >= 0.0
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

        self.queue_draw();
    }
}
