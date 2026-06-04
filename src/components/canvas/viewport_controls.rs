/* viewport_controls.rs
 *
 * Copyright 2026 FatDawlf
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::{prelude::*, subclass::prelude::*};
use std::f32::consts::PI;

use crate::components::canvas::widget::BrushCanvas;

impl BrushCanvas {
    pub fn zoom_by(&self, factor: f32) {
        let new_zoom = (self.imp().zoom.get() + factor).clamp(0.1, 10f32);
        self.imp().zoom.set(new_zoom);
        self.imp().canvas.queue_draw();
    }

    pub fn zoom_to(&self, zoom: f32) {
        self.imp().zoom.set(zoom.clamp(0.1, 10f32));
        self.imp().canvas.queue_draw();
    }

    pub fn move_by(&self, dx: f64, dy: f64) {
        let (x, y) = self.imp().position.get();
        let zoom = self.zoom() as f64;

        self.imp().position.set((x + (dx * zoom), y + (dy * zoom)));
        self.imp().canvas.queue_draw();
    }

    pub fn move_to(&self, x: f64, y: f64) {
        self.imp().position.set((x, y));
        self.imp().canvas.queue_draw();
    }

    pub fn rotate_by(&self, radians: f32) {
        let new_rot = (self.imp().rotation.get() + radians) % (PI * 2f32);
        self.imp().rotation.set(new_rot);
        self.imp().canvas.queue_draw();
    }

    pub fn rotate_to(&self, radians: f32) {
        self.imp().rotation.set(radians);
        self.imp().canvas.queue_draw();
    }

    pub fn zoom_to_fit(&self) {
        let imp = self.imp();
        let project = imp.project.read().unwrap();

        let (canvas_width, canvas_height) = (project.width as f32, project.height as f32);
        let (viewport_width, viewport_height) = (self.width() as f32, self.height() as f32);

        let scale_x = viewport_width / canvas_width;
        let scale_y = viewport_height / canvas_height;

        let scale = scale_x.min(scale_y);

        self.zoom_to(scale);
        self.move_to(0., 0.);
        imp.canvas.get().queue_draw();
    }
}
