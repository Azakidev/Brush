/* canvas.rs
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

use adw::{prelude::*, subclass::prelude::*};
use glow::{Context, NativeVertexArray};
use gtk::{
    gdk,
    glib::{self, VariantTy, WeakRef, clone},
};
use libloading::Library;
use zip::result::ZipError;

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    f32::consts::PI,
    ops::{Deref, Sub},
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use crate::{
    components::{
        editor::EditorAction,
        layer_item::BrushLayerItem,
        utils::{
            canvas::draw_stroke,
            editor_state::BrushEditorState,
            renderer::{
                buffer::LayerBuffer,
                render::{render_pass, setup_gl},
                shader_manager::ShaderManager,
            },
            tools::BrushTool,
        },
    },
    data::{
        blend_modes::BrushBlendMode,
        file::{request_save, save_project},
        layer::Layer,
        project::BrushProject,
        rect::Rect,
    },
};
use strum::IntoEnumIterator;

mod imp {

    use std::time::Duration;

    use super::*;

    #[allow(dead_code)]
    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/canvas.ui")]
    pub struct BrushCanvas {
        // Template widgets
        #[template_child]
        pub canvas: TemplateChild<gtk::GLArea>,
        // Project context
        pub file_location: RefCell<Option<String>>,
        pub editor_state: OnceCell<Rc<RefCell<BrushEditorState>>>,
        pub project: RefCell<BrushProject>,
        pub buffer_cache: RefCell<HashMap<Uuid, LayerBuffer>>,
        pub layer_widget_cache: RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>,
        // Gl context
        pub gl_context: OnceCell<Context>,
        pub gl_lib: OnceCell<Library>,
        pub gl_shader_manager: OnceCell<RefCell<ShaderManager>>,
        pub gl_vao: OnceCell<NativeVertexArray>,
        pub gl_root_fbo: OnceCell<LayerBuffer>,
        // Viewport
        pub active_layer: Cell<Option<Uuid>>,
        pub zoom: Cell<f32>,
        pub position: Cell<(f64, f64)>, // Offset from screen center
        pub rotation: Cell<f32>,        // Radians
        pub mouse_pos: Cell<(f64, f64)>,

        // Stroke handling
        pub stroke_mask: Arc<RwLock<Vec<u8>>>,
        pub last_position: Cell<(f64, f64)>,
        pub last_pressure: Cell<f64>,
        // Flags
        pub should_pan: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushCanvas {
        const NAME: &'static str = "BrushCanvas";
        type Type = super::BrushCanvas;
        type ParentType = adw::Bin;

        fn new() -> Self {
            Self {
                zoom: Cell::new(1f32),
                position: Cell::new((0., 0.)),
                rotation: Cell::new(0.),
                active_layer: Cell::new(None),
                should_pan: Cell::new(false),
                ..Default::default()
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            CanvasAction::init_actions(klass);

            // Debug actions
            klass.install_action("canvas.print-state", None, move |canvas, _, _| {
                println!(
                    "Contents: {}",
                    serde_json::to_string(&canvas.imp().project).unwrap()
                )
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushCanvas {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Make shortcut controller have a managed scope so accels work without focused
            {
                let list = obj.observe_controllers();

                for i in 0..list.n_items() {
                    if let Some(controller) = list.item(i).and_downcast::<gtk::ShortcutController>()
                    {
                        controller.set_scope(gtk::ShortcutScope::Managed);
                    }
                }
            }

            obj.clear_mask();

            obj.setup_motion_controller();
            obj.setup_scroll_controller();
            obj.setup_click_controller();
            obj.setup_middle_click_drag();
            obj.setup_drag_controller();
            obj.setup_zoom_controller();
            obj.setup_rotate_controller();

            // Setup canvas
            {
                let canvas = self.canvas.get();

                canvas.connect_realize(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |area| {
                        area.make_current();

                        // 1. Create the glow context using epoxy as the loader
                        let gl_lib = unsafe {
                            libloading::Library::new("libGLESv2.so.2")
                                .or_else(|_| libloading::Library::new("libGLESv2.so"))
                                .or_else(|_| libloading::Library::new("libEGL.so.1"))
                                .expect("Could not find a valid GL/GLES library in Flatpak")
                        };

                        let gl = unsafe {
                            glow::Context::from_loader_function(|symbol| {
                                gl_lib
                                    .get::<*const std::ffi::c_void>(symbol.as_bytes())
                                    .map(|ptr| *ptr)
                                    .unwrap_or(std::ptr::null())
                            })
                        };

                        let obj = obj.obj();
                        let imp = obj.imp();

                        let _ = imp.gl_context.set(gl);
                        let _ = imp.gl_lib.set(gl_lib);

                        let gl = imp.gl_context.get().unwrap();

                        if let Some((shader_manager, vao)) = setup_gl(gl) {
                            let _ = imp.gl_shader_manager.set(RefCell::new(shader_manager));
                            let _ = imp.gl_vao.set(vao);
                        }
                    }
                ));

                let weak_self = self.downgrade();
                canvas.connect_render(move |area, _context| {
                    let Some(obj) = weak_self.upgrade() else {
                        return glib::Propagation::Proceed;
                    };

                    render_pass(&obj.obj(), area);

                    glib::Propagation::Stop
                });

                let weak_self = self.downgrade();
                canvas.connect_unrealize(move |_area| {
                    if let Some(obj) = weak_self.upgrade() {
                        let Some(gl) = obj.gl_context.get() else {
                            return;
                        };

                        let buffer_cache = obj.buffer_cache.borrow_mut();
                        buffer_cache
                            .iter()
                            .for_each(|(_uuid, buf)| unsafe { buf.destroy(gl) });

                        if let Some(root_buf) = obj.gl_root_fbo.get() {
                            unsafe {
                                root_buf.destroy(gl);
                            }
                        }

                        if let Some(shader_manager) = obj.gl_shader_manager.get() {
                            unsafe {
                                shader_manager.borrow().destroy(gl);
                            }
                        }
                    };
                });
            }

            obj.connect_realize(|c| {
                gtk::glib::spawn_future_local(clone!(
                    #[weak]
                    c,
                    async move {
                        gtk::glib::timeout_future(Duration::from_millis(20)).await;
                        c.imp().canvas.queue_draw();
                        c.zoom_to_fit();
                    }
                ));
            });
        }
    }
    impl WidgetImpl for BrushCanvas {}
    impl BinImpl for BrushCanvas {}
}

glib::wrapper! {
    pub struct BrushCanvas(ObjectSubclass<imp::BrushCanvas>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushCanvas {
    pub fn new(editor_state: Rc<RefCell<BrushEditorState>>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp()
            .editor_state
            .set(editor_state)
            .expect("Editor state already set");
        obj
    }

    pub fn from_project(
        editor_state: Rc<RefCell<BrushEditorState>>,
        project: BrushProject,
        loc: &str,
    ) -> Self {
        let obj: Self = glib::Object::new();
        // Project setup
        let first_id = project.layers.first().map(|l| l.id());
        obj.imp().project.replace(project);
        obj.imp().file_location.replace(Some(loc.to_string()));
        obj.imp().active_layer.replace(first_id);
        // Editor state
        obj.imp()
            .editor_state
            .set(editor_state)
            .expect("Editor state already set");

        obj
    }

    // Query
    pub fn project_context(&self) -> RefCell<BrushProject> {
        self.imp().project.clone()
    }

    pub fn widget_cache(&self) -> RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>> {
        self.imp().layer_widget_cache.clone()
    }

    pub fn selected_layer(&self) -> Option<Uuid> {
        self.imp().active_layer.get()
    }

    pub fn zoom(&self) -> f32 {
        self.imp().zoom.get()
    }

    pub fn rotation(&self) -> f32 {
        self.imp().rotation.get()
    }

    // Layer management
    fn new_pixel_layer(&self) {
        let id = {
            let mut project = self.imp().project.borrow_mut();

            let name = "New pixel layer".to_owned();
            let width = project.width;
            let height = project.height;

            let layer = Layer::new_pixel(name, width, height);

            let id = layer.id();

            self.push_layer(&mut project, layer);
            self.update_tree(&mut project);

            id
        };
        let _ = self.activate_action("editor.activate-layer", Some(&id.to_string().to_variant()));
    }

    fn new_group_layer(&self) {
        let id = {
            let mut project = self.imp().project.borrow_mut();
            let name = "New Group".to_owned();
            let layer = Layer::new_group(name);
            let id = layer.id();

            self.push_layer(&mut project, layer);
            self.update_tree(&mut project);
            id
        };
        let _ = self.activate_action("editor.activate-layer", Some(&id.to_string().to_variant()));
    }

    fn push_layer(&self, project: &mut BrushProject, layer: Layer) {
        if let Some(active_id) = self.imp().active_layer.get()
            && let Some(active_layer) = project.find_layer_mut(active_id)
        {
            // If the active layer has children, append it to the layer
            if let Some(children) = active_layer.children() {
                let idx = children
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);

                active_layer.append(idx, layer);
                // If the parent of the active layer has children, append it to the parent
            } else if let Some(parent) = project.find_parent_mut(active_id)
                && let Some(children) = parent.children()
            {
                let idx = children
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);
                parent.append(idx, layer);
                // If if doesn't have a parent, append it to the project in position
            } else {
                let idx = project
                    .layers
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);
                project.layers.insert(idx, layer);
            }
            //If there's no active layer, push it to the beginning
        } else {
            project.layers.push(layer);
        }
    }

    fn update_tree(&self, project: &mut BrushProject) {
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(id) = self.imp().active_layer.get() {
            project.remove_stale_widgets(id, &mut widget_cache);
        }

        self.imp().canvas.queue_draw();
    }

    fn remove_layer(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
        let mut widget_cache = imp.layer_widget_cache.borrow_mut();
        let mut buffer_cache = imp.buffer_cache.borrow_mut();

        if let Some(active_layer) = imp.active_layer.get() {
            // If the active layer's parent...
            // Is a group...
            if let Some(parent) = project.find_parent(active_layer)
                && let Some(children) = parent.children()
            {
                // That will still have children after removal
                if children.len() - 1 != 0 {
                    // Select the next children
                    let idx = children
                        .iter()
                        .position(|l| l.id() == active_layer)
                        .unwrap_or(0);
                    let idx = if children.len() == idx + 1 {
                        idx - 1
                    } else {
                        idx + 1
                    };
                    if let Some(layer) = children.get(idx) {
                        imp.active_layer.set(Some(layer.id()));
                    }
                } else {
                    // Otherwise, select the parent
                    imp.active_layer.set(Some(parent.id()));
                }
                // If it doesn't have a parent
            } else
            // And the parent has other layers after removal
            if project.layers.len() - 1 != 0 {
                // Select the next one
                let idx = project
                    .layers
                    .iter()
                    .position(|l| l.id() == active_layer)
                    .unwrap_or(0);
                let idx = if project.layers.len() == idx + 1 {
                    idx - 1
                } else {
                    idx + 1
                };
                if let Some(layer) = project.layers.get(idx) {
                    imp.active_layer.set(Some(layer.id()));
                }
            } else {
                // Otherwise, there's no layer left and the active layer should be None
                imp.active_layer.set(None);
            }

            // Remove layer and caches
            project.remove_stale_widgets(active_layer, &mut widget_cache);
            project.remove_layer(active_layer);
            buffer_cache.remove(&active_layer);
        }

        self.imp().canvas.queue_draw();
    }

    fn move_layer_up(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        let mut buf_cache = self.imp().buffer_cache.borrow_mut();

        if let Some(active_id) = self.imp().active_layer.get()
            && let Some(layer) = project.clone().find_layer(active_id)
        {
            // Has a parent
            if let Some(parent) = project.clone().find_parent(active_id)
                && let Some(children) = parent.children()
            {
                let idx = children
                    .iter()
                    .position(|l| l.id() == active_id)
                    .unwrap_or(children.len());
                // If it ain't the first one in the parent
                if idx != 0 {
                    // Move it up by 1
                    // And, if the previous layer is a group
                    if let Some(previous) = children.get(idx - 1)
                        && let Some(previous_children) = previous.children()
                    {
                        project.move_layer(
                            layer,
                            previous_children.len(),
                            Some(parent.id()),
                            Some(previous.id()),
                            &mut buf_cache,
                            &mut widget_cache,
                        )
                    } else {
                        project.move_layer(
                            layer,
                            idx - 1,
                            Some(parent.id()),
                            Some(parent.id()),
                            &mut buf_cache,
                            &mut widget_cache,
                        );
                    }
                // If it is
                } else {
                    // Bump it up a level
                    // Grandparent found
                    if let Some(grandparent) = project.clone().find_parent(parent.id())
                        && let Some(children) = grandparent.children()
                    {
                        let idx = children
                            .iter()
                            .position(|l| l.id() == parent.id())
                            .unwrap_or(children.len());
                        project.move_layer(
                            layer,
                            idx,
                            Some(parent.id()),
                            Some(grandparent.id()),
                            &mut buf_cache,
                            &mut widget_cache,
                        );
                    } else {
                        // At project root
                        let idx = project
                            .layers
                            .iter()
                            .position(|l| l.id() == parent.id())
                            .unwrap_or(children.len());
                        project.move_layer(
                            layer,
                            idx,
                            Some(parent.id()),
                            None,
                            &mut buf_cache,
                            &mut widget_cache,
                        )
                    }
                }
            // At project root
            } else {
                let idx = project
                    .layers
                    .iter()
                    .position(|l| l.id() == active_id)
                    .unwrap_or(project.layers.len());
                if idx != 0
                    && let Some(previous) = project.clone().layers.get(idx - 1)
                {
                    if let Some(previous_children) = previous.children() {
                        project.move_layer(
                            layer,
                            previous_children.len(),
                            None,
                            Some(previous.id()),
                            &mut buf_cache,
                            &mut widget_cache,
                        )
                    } else {
                        project.move_layer(
                            layer,
                            idx - 1,
                            None,
                            None,
                            &mut buf_cache,
                            &mut widget_cache,
                        );
                    }
                }
            }
        }
        self.imp().canvas.queue_draw();
    }

    fn move_layer_down(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        let mut buf_cache = self.imp().buffer_cache.borrow_mut();

        if let Some(active_id) = self.imp().active_layer.get()
            && let Some(active_layer) = project.clone().find_layer(active_id)
        {
            // Has a parent
            if let Some(parent) = project.clone().find_parent(active_id)
                && let Some(children) = parent.children()
            {
                let idx = children
                    .iter()
                    .position(|l| l.id() == active_id)
                    .unwrap_or(children.len());
                // If it ain't the first one in the parent
                if idx != children.len() - 1 {
                    // Move it up by 1
                    // And, if the previous layer is a group
                    if let Some(next) = children.get(idx + 1) {
                        if next.children().is_some() {
                            project.move_layer(
                                active_layer,
                                0,
                                Some(parent.id()),
                                Some(next.id()),
                                &mut buf_cache,
                                &mut widget_cache,
                            )
                        } else {
                            project.move_layer(
                                active_layer,
                                idx + 1,
                                Some(parent.id()),
                                Some(parent.id()),
                                &mut buf_cache,
                                &mut widget_cache,
                            );
                        }
                    // If it is
                    } else {
                        // Bump it up a level
                        // Grandparent found
                        if let Some(grandparent) = project.clone().find_parent(parent.id())
                            && let Some(children) = grandparent.children()
                        {
                            let idx = children
                                .iter()
                                .position(|l| l.id() == parent.id())
                                .unwrap_or(children.len());
                            project.move_layer(
                                active_layer,
                                idx + 1,
                                Some(parent.id()),
                                Some(grandparent.id()),
                                &mut buf_cache,
                                &mut widget_cache,
                            );
                        } else {
                            // At project root
                            let idx = project
                                .layers
                                .iter()
                                .position(|l| l.id() == parent.id())
                                .unwrap_or(children.len());
                            project.move_layer(
                                active_layer,
                                idx + 1,
                                Some(parent.id()),
                                None,
                                &mut buf_cache,
                                &mut widget_cache,
                            )
                        }
                    }
                }
            // At project root
            } else {
                let idx = project
                    .layers
                    .iter()
                    .position(|l| l.id() == active_id)
                    .unwrap_or(project.layers.len());
                if idx != project.layers.len()
                    && let Some(next) = project.clone().layers.get(idx + 1)
                {
                    if next.children().is_some() {
                        project.move_layer(
                            active_layer,
                            0,
                            None,
                            Some(next.id()),
                            &mut buf_cache,
                            &mut widget_cache,
                        )
                    } else {
                        project.move_layer(
                            active_layer,
                            idx + 1,
                            None,
                            None,
                            &mut buf_cache,
                            &mut widget_cache,
                        );
                    }
                }
            }
        }
        self.imp().canvas.queue_draw();
    }

    fn rename_layer(&self, uuid: Uuid, new_name: String) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        project.rename_layer(uuid, new_name);

        project.remove_stale_widgets(uuid, &mut widget_cache);
    }

    fn set_layer_opacity(&self, opacity: f32) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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

    fn set_layer_blend(&self, blend_mode: BrushBlendMode) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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

    fn toggle_visible(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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

    fn toggle_alpha_clip(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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
    fn toggle_alpha_lock(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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
    fn toggle_passthrough(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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
    fn toggle_lock(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
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

    fn save_project(&self, project: BrushProject) {
        let imp = self.imp();

        if let Some(loc) = imp.file_location.borrow().clone() {
            // File string set
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    let pixels = obj
                        .get_composite(project.width as i32, project.height as i32)
                        .unwrap();

                    let result = gtk::gio::spawn_blocking(move || {
                        save_project(Path::new(loc.as_str()), project, pixels.as_slice())
                    })
                    .await
                    .expect("Failed to finish save");

                    obj.save_feedback(result);
                }
            ));
        } else {
            // Location not set, prompt user
            println!("File not saved yet, prompting");
            self.save_project_as(project, true);
        }
    }

    fn save_project_as(&self, project: BrushProject, swap_to: bool) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            project,
            async move {
                if let Ok(file) = request_save().await {
                    if swap_to {
                        let path = file.as_path().to_str().unwrap().to_owned();
                        let new_name = file.as_path().file_name().unwrap().to_str().unwrap();

                        let _ = obj.activate_action(
                            &EditorAction::RenameTab,
                            Some(&new_name.to_variant()),
                        );
                        obj.imp().file_location.replace(Some(path));
                    }
                    obj.save_project(project);
                }
            }
        ));
    }

    fn get_composite(&self, width: i32, height: i32) -> Option<Vec<u8>> {
        let imp = self.imp();

        if let Some(root) = imp.gl_root_fbo.get()
            && let Some(gl) = imp.gl_context.get()
            && let Some(shader_manager) = imp.gl_shader_manager.get()
        {
            unsafe {
                return capture_oklab_to_srgb_png(
                    gl,
                    root.texture,
                    width,
                    height,
                    &mut shader_manager.borrow_mut(),
                );
            }
        }
        None
    }

    // Create and show a popup in the toast overlay via action
    // If OK, it should be a simple confirmation
    // If Err, it should be a toast with a simple error and a button to copy the error output
    fn save_feedback(&self, result: Result<(), ZipError>) {
        match result {
            // TODO: The actual toasts
            Ok(_) => {}
            Err(e) => {
                eprintln!("{e}");
            }
        }
    }

    // Viewport control
    fn zoom_by(&self, factor: f32) {
        let new_zoom = (self.imp().zoom.get() + factor).clamp(0.1, 10f32);
        self.imp().zoom.set(new_zoom);
        self.imp().canvas.queue_draw();
    }

    fn zoom_to(&self, zoom: f32) {
        self.imp().zoom.set(zoom.clamp(0.1, 10f32));
        self.imp().canvas.queue_draw();
    }

    fn move_by(&self, dx: f64, dy: f64) {
        let (x, y) = self.imp().position.get();
        let zoom = self.zoom() as f64;

        self.imp().position.set((x + (dx * zoom), y + (dy * zoom)));
        self.imp().canvas.queue_draw();
    }

    fn move_to(&self, x: f64, y: f64) {
        self.imp().position.set((x, y));
        self.imp().canvas.queue_draw();
    }

    fn rotate_by(&self, radians: f32) {
        let new_rot = (self.imp().rotation.get() + radians) % (PI * 2f32);
        self.imp().rotation.set(new_rot);
        self.imp().canvas.queue_draw();
    }

    fn rotate_to(&self, radians: f32) {
        self.imp().rotation.set(radians);
        self.imp().canvas.queue_draw();
    }

    fn zoom_to_fit(&self) {
        let imp = self.imp();
        let (canvas_width, canvas_height) = (
            imp.project.borrow().width as f32,
            imp.project.borrow().height as f32,
        );
        let (viewport_width, viewport_height) = (self.width() as f32, self.height() as f32);

        let scale_x = viewport_width / canvas_width;
        let scale_y = viewport_height / canvas_height;

        let scale = scale_x.min(scale_y);

        self.zoom_to(scale);
        self.move_to(0., 0.);
        imp.canvas.get().queue_draw();
    }

    fn setup_rotate_controller(&self) {
        let controller = gtk::GestureRotate::new();

        let start_rotate = Rc::new(Cell::new(0f32));
        let should_rotate = Rc::new(Cell::new(false));

        controller.connect_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            start_rotate,
            #[weak]
            should_rotate,
            move |_, _| {
                let rotation = obj.imp().rotation.get();
                start_rotate.set(rotation);
                should_rotate.set(false);
            }
        ));

        controller.connect_angle_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            start_rotate,
            #[strong]
            should_rotate,
            move |controller, _, _| {
                let orig_rot = start_rotate.get();
                let threshold = PI / 20f32;

                let angle = controller.angle_delta() as f32;

                if angle.abs() > threshold {
                    should_rotate.set(true)
                }

                let final_angle = obj.rotation() + angle;

                if (final_angle).abs() < threshold {
                    should_rotate.set(false);
                    obj.rotate_to(0f32);
                }

                if should_rotate.get() {
                    obj.rotate_to(orig_rot + angle);
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        self.add_controller(controller);
    }

    fn setup_zoom_controller(&self) {
        let controller = gtk::GestureZoom::new();

        let start_zoom = Rc::new(Cell::new(0.));
        let start_pos = Rc::new(Cell::new((0f64, 0f64)));
        let start_drag = Rc::new(Cell::new((0f64, 0f64)));

        controller.connect_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            start_zoom,
            #[weak]
            start_pos,
            #[weak]
            start_drag,
            move |gesture, _| {
                let imp = obj.imp();

                start_zoom.set(imp.zoom.get());
                start_pos.set(imp.position.get());

                if let Some((x, y)) = gesture.bounding_box_center() {
                    start_drag.set((x, y));
                }
            }
        ));

        controller.connect_scale_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            start_zoom,
            #[strong]
            start_pos,
            #[strong]
            start_drag,
            move |gesture, zoom| {
                let orig_zoom = start_zoom.get();
                let new_zoom = orig_zoom * zoom as f32;

                obj.zoom_to(new_zoom);

                if let Some((center_x, center_y)) = gesture.bounding_box_center() {
                    let (old_x, old_y) = start_drag.get();
                    let (canvas_old_x, canvas_old_y) = start_pos.get();

                    let dx = center_x - old_x;
                    let dy = center_y - old_y;

                    let new_x = canvas_old_x + dx * zoom as f64;
                    let new_y = canvas_old_y + dy * zoom as f64;

                    obj.move_to(new_x, new_y);
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        self.add_controller(controller);
    }

    fn setup_click_controller(&self) {
        let controller = gtk::GestureClick::new();

        controller.connect_pressed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |gesture, _, _x, _y| {
                obj.clear_mask();

                if let Some(state) = obj.imp().editor_state.get() {
                    let mask = obj.imp().stroke_mask.clone();

                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {} // NO OP
                        BrushTool::Brush => {
                            if let Some(event) = gesture.last_event(None) {
                                let pressure = event
                                    .axis(gdk::AxisUse::Pressure)
                                    .unwrap_or(1.0)
                                    .clamp(0f64, 1f64);
                                let _x_tilt = event
                                    .axis(gdk::AxisUse::Xtilt)
                                    .unwrap_or(0.0)
                                    .clamp(-1f64, 1f64);
                                let _y_tilt = event
                                    .axis(gdk::AxisUse::Ytilt)
                                    .unwrap_or(0.0)
                                    .clamp(-1f64, 1f64);
                                glib::spawn_future_local(glib::clone!(
                                    #[weak (rename_to = c)]
                                    obj,
                                    #[strong]
                                    state,
                                    #[strong]
                                    mask,
                                    #[strong]
                                    pressure,
                                    async move {
                                        let mut project = c.imp().project.borrow_mut();
                                        let a_id = c.imp().active_layer.get();
                                        let screen = (c.width() as f32, c.height() as f32);
                                        let position = c.imp().position.get();
                                        let zoom = c.zoom();
                                        let rotation = c.rotation();
                                        let cp = c.imp().mouse_pos.get();
                                        let lp = c.imp().last_position.get();
                                        let l_pressure = c.imp().last_pressure.get();

                                        draw_stroke(
                                            &mut project,
                                            a_id,
                                            &state,
                                            mask,
                                            pressure,
                                            l_pressure,
                                            cp,
                                            lp,
                                            screen,
                                            position,
                                            zoom,
                                            rotation,
                                        )
                                        .await;

                                        c.imp().last_position.replace(c.imp().mouse_pos.get());
                                        c.imp().last_pressure.set(pressure);
                                    }
                                ));
                            }
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        controller.connect_released(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _, _, _| {
                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {} // NO OP
                        BrushTool::Brush => {
                            obj.update_layer();
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }
            }
        ));

        self.add_controller(controller);
    }

    fn setup_middle_click_drag(&self) {
        let controller = gtk::GestureDrag::new();
        controller.set_button(2); // Middle-click only

        let start_pos = Rc::new(Cell::new((0., 0.)));

        controller.connect_drag_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            start_pos,
            move |_, _, _| {
                let pos = obj.imp().position.get();
                start_pos.set(pos);
            }
        ));
        controller.connect_drag_update(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            start_pos,
            move |_, offset_x, offset_y| {
                let (orig_x, orig_y) = start_pos.get();
                obj.move_to(orig_x + offset_x, orig_y + offset_y)
            }
        ));

        self.add_controller(controller);
    }

    fn setup_drag_controller(&self) {
        let controller = gtk::GestureDrag::new();

        let start_pos = Rc::new(Cell::new((0., 0.)));

        controller.connect_drag_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            start_pos,
            move |gesture, _x, _y| {
                let pos = obj.imp().position.get();
                start_pos.set(pos);

                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {} // No op
                        BrushTool::Brush => {
                            obj.clear_mask();

                            obj.imp().last_position.replace(obj.imp().mouse_pos.get());
                            if let Some(event) = gesture.last_event(None) {
                                let pressure = event
                                    .axis(gdk::AxisUse::Pressure)
                                    .unwrap_or(1.)
                                    .clamp(0., 1.);
                                let _x_tilt =
                                    event.axis(gdk::AxisUse::Xtilt).unwrap_or(0.).clamp(-1., 1.);
                                let _y_tilt =
                                    event.axis(gdk::AxisUse::Ytilt).unwrap_or(0.).clamp(-1., 1.);

                                obj.imp().last_pressure.set(pressure);
                            }
                        }
                        _ => {}
                    }
                }
            }
        ));

        controller.connect_drag_update(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            start_pos,
            move |gesture, offset_x, offset_y| {
                let (orig_x, orig_y) = start_pos.get();

                if let Some(state) = obj.imp().editor_state.get() {
                    let mask = obj.imp().stroke_mask.clone();

                    let state = state.borrow();

                    let tool = if obj.imp().should_pan.get() {
                        BrushTool::Move
                    } else {
                        *state.tool.borrow()
                    };

                    match tool {
                        BrushTool::Move => obj.move_to(orig_x + offset_x, orig_y + offset_y),
                        BrushTool::Brush => {
                            if let Some(event) = gesture.last_event(None) {
                                let pressure = event.axis(gdk::AxisUse::Pressure).unwrap_or(1.);
                                let _x_tilt = event.axis(gdk::AxisUse::Xtilt).unwrap_or(0.);
                                let _y_tilt = event.axis(gdk::AxisUse::Ytilt).unwrap_or(0.);
                                glib::spawn_future_local(glib::clone!(
                                    #[weak (rename_to = c)]
                                    obj,
                                    #[strong]
                                    state,
                                    #[strong]
                                    mask,
                                    #[strong]
                                    pressure,
                                    async move {
                                        let mut project = c.imp().project.borrow_mut();
                                        let a_id = c.imp().active_layer.get();
                                        let screen = (c.width() as f32, c.height() as f32);
                                        let position = c.imp().position.get();
                                        let zoom = c.zoom();
                                        let rotation = c.rotation();
                                        let cp = c.imp().mouse_pos.get();
                                        let lp = c.imp().last_position.get();
                                        let l_pressure = c.imp().last_pressure.get();

                                        draw_stroke(
                                            &mut project,
                                            a_id,
                                            &state,
                                            mask,
                                            pressure,
                                            l_pressure,
                                            cp,
                                            lp,
                                            screen,
                                            position,
                                            zoom,
                                            rotation,
                                        )
                                        .await;

                                        c.imp().last_position.replace(c.imp().mouse_pos.get());
                                        c.imp().last_pressure.set(pressure);
                                    }
                                ));
                            }
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        controller.connect_drag_end(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _, _| {
                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {} // No-op
                        BrushTool::Brush => {
                            obj.update_layer();
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        self.add_controller(controller);
    }

    fn setup_motion_controller(&self) {
        let motion = gtk::EventControllerMotion::new();
        let weak_self = self.downgrade();

        motion.connect_motion(move |_, x, y| {
            if let Some(obj) = weak_self.upgrade() {
                obj.imp().mouse_pos.set((x, y));
            }
        });
        self.add_controller(motion);
    }

    fn setup_scroll_controller(&self) {
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);

        let weak_self = self.downgrade();

        scroll.connect_scroll(move |_controller, _dx, dy| {
            let Some(obj) = weak_self.upgrade() else {
                return glib::Propagation::Proceed;
            };

            let imp = obj.imp();

            let (win_w, win_h) = (obj.width() as f64, obj.height() as f64);

            let (mouse_x, mouse_y) = imp.mouse_pos.get();

            let old_zoom = imp.zoom.get() as f64;
            let (old_x, old_y) = imp.position.get();

            let zoom_mult = if dy < 0.0 { 1.1 } else { 0.9 };
            let zoom = (old_zoom * zoom_mult).clamp(0.001, 100.0);

            let factor = zoom / old_zoom;

            let new_x = mouse_x - win_w / 2.0 - factor * (mouse_x - win_w / 2.0 - old_x);
            let new_y = mouse_y - win_h / 2.0 - factor * (mouse_y - win_h / 2.0 - old_y);

            obj.zoom_to(zoom as f32);
            obj.move_to(new_x, new_y);
            obj.imp().canvas.queue_draw();

            glib::Propagation::Stop
        });

        self.add_controller(scroll);
    }

    fn update_layer(&self) {
        let mut project = self.imp().project.borrow_mut();

        if let Some(acive_id) = self.imp().active_layer.get()
            && let Some(layer) = project.find_layer_mut(acive_id)
        {
            layer.set_dirty(true);
            layer.set_dirty_rect(Some(Rect {
                x: 0,
                y: 0,
                w: layer.width() as i32,
                h: layer.height() as i32,
            }));
        }
    }

    fn clear_layer(&self) {
        let mut project = self.imp().project.borrow_mut();

        if let Some(acive_id) = self.imp().active_layer.get()
            && let Some(layer) = project.find_layer_mut(acive_id)
        {
            layer.clear();
            layer.set_dirty(true);
            layer.set_dirty_rect(Some(Rect {
                x: 0,
                y: 0,
                w: layer.width() as i32,
                h: layer.height() as i32,
            }));
        }

        self.imp().canvas.queue_draw();
    }

    fn clear_mask(&self) {
        let project = self.imp().project.borrow();
        let size = project.width * project.height;

        let mut mask = self.imp().stroke_mask.write().unwrap();
        *mask = vec![0; size as usize];
    }
}

unsafe fn capture_oklab_to_srgb_png(
    gl: &glow::Context,
    root_fbo_texture: glow::Texture,
    width: i32,
    height: i32,
    shader_manager: &mut ShaderManager, // Adjust based on your actual struct name
) -> Option<Vec<u8>> {
    unsafe {
        use glow::HasContext;

        let read_fbo = gl.create_framebuffer().ok()?;
        let read_tex = gl.create_texture().ok()?;

        gl.bind_texture(glow::TEXTURE_2D, Some(read_tex));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width,
            height,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(read_fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(read_tex),
            0,
        );

        gl.viewport(0, 0, width, height);

        shader_manager.oklab2srgb.bind(gl);

        // Identity Matrix
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        if let Some(loc) = shader_manager.oklab2srgb.get_uniform(gl, "u_mvp") {
            gl.uniform_matrix_4_f32_slice(Some(&loc), false, &identity);
        }

        // Already flipped in render, no need to flip again
        if let Some(loc) = shader_manager.oklab2srgb.get_uniform(gl, "u_flip_y") {
            gl.uniform_1_f32(Some(&loc), 0.0);
        }

        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(root_fbo_texture));

        let vao = gl.create_vertex_array().ok()?;
        let vbo = gl.create_buffer().ok()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

        // Full screen quad
        let vertices: [f32; 24] = [
            -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, -1.0, 1.0, 0.0, 1.0,
            1.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0,
        ];
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&vertices),
            glow::STATIC_DRAW,
        );

        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

        gl.draw_arrays(glow::TRIANGLES, 0, 6);
        gl.finish();

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        gl.read_pixels(
            0,
            0,
            width,
            height,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );

        // 8. Cleanup
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.bind_vertex_array(None);
        gl.delete_vertex_array(vao);
        gl.delete_buffer(vbo);
        gl.delete_framebuffer(read_fbo);
        gl.delete_texture(read_tex);

        Some(pixels)
    }
}

#[derive(strum::Display, strum::EnumIter, strum::AsRefStr)]
pub enum CanvasAction {
    // Layer management
    #[strum(to_string = "canvas.new-pixel")]
    NewPixel,
    #[strum(to_string = "canvas.new-group")]
    NewGroup,
    #[strum(to_string = "canvas.rename-layer")]
    RenameLayer,
    #[strum(to_string = "canvas.delete-layer")]
    DeleteLayer,
    #[strum(to_string = "canvas.clear-layer")]
    ClearLayer,
    #[strum(to_string = "canvas.move-layer-up")]
    MoveLayerUp,
    #[strum(to_string = "canvas.move-layer-down")]
    MoveLayerDown,
    // Project management
    #[strum(to_string = "canvas.save")]
    SaveProject,
    #[strum(to_string = "canvas.save-as")]
    SaveProjectAs,
    #[strum(to_string = "canvas.export-as")]
    ExportProjectAs,
    // Viewport navigation
    #[strum(to_string = "canvas.zoom-in")]
    ZoomIn,
    #[strum(to_string = "canvas.zoom-out")]
    ZoomOut,
    #[strum(to_string = "canvas.zoom-to-fit")]
    ZoomToFit,
    #[strum(to_string = "canvas.pan-up")]
    PanUp,
    #[strum(to_string = "canvas.pan-down")]
    PanDown,
    #[strum(to_string = "canvas.pan-left")]
    PanLeft,
    #[strum(to_string = "canvas.pan-up")]
    PanRight,
    #[strum(to_string = "canvas.rotate-right")]
    RotateCW,
    #[strum(to_string = "canvas.rotate-left")]
    RotateCCW,
    #[strum(to_string = "canvas.rotate-reset")]
    RotateTo0,
    // Layer modification
    #[strum(to_string = "canvas.set-layer-opacity")]
    SetLayerOpacity,
    #[strum(to_string = "canvas.set-layer-blend")]
    SetLayerBlendMode,
    #[strum(to_string = "canvas.toggle-lock")]
    ToggleLock,
    #[strum(to_string = "canvas.toggle-visible")]
    ToggleVisible,
    #[strum(to_string = "canvas.toggle-alpha-clip")]
    ToggleAlphaClip,
    #[strum(to_string = "canvas.toggle-alpha-lock")]
    ToggleAlphaLock,
    #[strum(to_string = "canvas.toggle-passthrough")]
    TogglePassthrough,
}

impl Deref for CanvasAction {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl CanvasAction {
    fn init_actions(klass: &mut <imp::BrushCanvas as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                CanvasAction::NewPixel => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.new_pixel_layer();
                    });
                }
                CanvasAction::NewGroup => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.new_group_layer();
                    });
                }
                CanvasAction::RenameLayer => {
                    klass.install_action(&action, Some(VariantTy::STRING), move |c, _, arg| {
                        if let Some(var) = arg {
                            let value = var.to_string(); // 'Name'
                            let name = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                            if let Some(active_layer) = c.imp().active_layer.get() {
                                c.rename_layer(active_layer, name.to_string());
                            }
                        }
                    });
                }
                CanvasAction::DeleteLayer => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.remove_layer();
                    });
                }
                CanvasAction::ClearLayer => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.clear_layer();
                    });

                    klass.add_binding_action(
                        gdk::Key::Delete,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::MoveLayerUp => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.move_layer_up();
                    });
                }
                CanvasAction::MoveLayerDown => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.move_layer_down();
                    });
                }
                // Project handling
                CanvasAction::SaveProject => {
                    klass.install_action(&action, None, |c, _, _| {
                        let project = c.imp().project.borrow().clone();
                        c.save_project(project);
                    });

                    klass.add_binding_action(gdk::Key::S, gdk::ModifierType::CONTROL_MASK, &action);
                }
                CanvasAction::SaveProjectAs => {
                    klass.install_action(&action, None, |c, _, _| {
                        let project = c.imp().project.borrow().clone();
                        c.save_project_as(project, true);
                    });

                    klass.add_binding_action(
                        gdk::Key::S,
                        gdk::ModifierType::SHIFT_MASK.intersection(gdk::ModifierType::CONTROL_MASK),
                        &action,
                    );
                }
                CanvasAction::ExportProjectAs => {}
                // Viewport control
                CanvasAction::ZoomIn => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.zoom_by(0.05f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::plus,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );

                    klass.add_binding_action(
                        gdk::Key::equal,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::ZoomOut => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.zoom_by(-0.05f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::minus,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::ZoomToFit => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.zoom_to_fit();
                    });

                    klass.add_binding_action(
                        gdk::Key::Home,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::PanUp => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(0., 60.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Up,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::PanDown => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(0., -60.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Down,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::PanLeft => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(60., 0.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Left,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::PanRight => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(-60., 0.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Right,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                CanvasAction::RotateCW => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_by(PI / 5f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::bracketright,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }
                CanvasAction::RotateCCW => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_by(PI / -5f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::bracketleft,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }
                CanvasAction::RotateTo0 => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_to(0f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::Home,
                        gdk::ModifierType::SHIFT_MASK,
                        &action,
                    );
                }
                CanvasAction::SetLayerOpacity => {
                    klass.install_action(&action, Some(VariantTy::DOUBLE), |c, _, arg| {
                        if let Some(var) = arg
                            && let Some(val) = var.get::<f64>()
                        {
                            c.set_layer_opacity(val as f32);
                        }
                    });
                }
                CanvasAction::SetLayerBlendMode => {
                    klass.install_action(&action, Some(VariantTy::UINT32), |c, _, arg| {
                        if let Some(var) = arg
                            && let Some(val) = var.get::<u32>()
                            && let Some(blend_mode) =
                                BrushBlendMode::iter().take(val as usize + 1).next_back()
                        {
                            c.set_layer_blend(blend_mode);
                        }
                    });
                }
                CanvasAction::ToggleVisible => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_visible();
                    });
                }
                CanvasAction::ToggleLock => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_lock();
                    });
                }
                CanvasAction::ToggleAlphaClip => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_alpha_clip();
                    });
                }
                CanvasAction::ToggleAlphaLock => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_alpha_lock();
                    });
                }
                CanvasAction::TogglePassthrough => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_passthrough();
                    });
                }
            }
        }
    }
}

