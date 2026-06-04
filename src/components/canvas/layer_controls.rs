/* viewport_controls.rs
 *
 * Copyright 2026 FatDawlf
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::{glib::WeakRef, prelude::*, subclass::prelude::*};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    components::{canvas::widget::BrushCanvas, layer_item::BrushLayerItem},
    data::blend_modes::BrushBlendMode,
};

impl BrushCanvas {
    pub fn rename_layer(
        &self,
        uuid: Uuid,
        new_name: String,
        cache: &mut HashMap<Uuid, WeakRef<BrushLayerItem>>,
    ) {
        let mut project = self.imp().project.write().unwrap();

        project.rename_layer(uuid, new_name);
        project.remove_stale_widgets(uuid, cache);
    }

    pub fn set_layer_opacity(&self, opacity: f32) {
        let imp = self.imp();
        let mut project = self.imp().project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = self.imp().active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_opacity(opacity);

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn set_layer_blend(&self, blend_mode: BrushBlendMode) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = self.imp().active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_blend_mode(blend_mode);

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn toggle_visible(&self) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_visible(!active_layer.visible());

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn toggle_alpha_clip(&self) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_alpha_clip(!active_layer.alpha_clip());

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn toggle_alpha_lock(&self) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_alpha_lock(!active_layer.alpha_lock());

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn toggle_passthrough(&self) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_passthrough(!active_layer.passthrough());

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }

    pub fn toggle_lock(&self) {
        let imp = self.imp();
        let mut project = imp.project.write().unwrap();
        let widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            active_layer.set_lock(!active_layer.lock());

            if let Some(w) = widget_cache.get(&active_id)
                && let Some(i) = w.upgrade()
            {
                i.update(Some(active_id), active_layer);
            }
        }
        imp.canvas.queue_draw();
    }
}
