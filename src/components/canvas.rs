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
    glib::{self, clone, VariantTy, WeakRef},
};
use libloading::Library;

use glow::HasContext;
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    f32::consts::PI,
    ops::Sub,
    rc::Rc,
};
use uuid::Uuid;

use crate::{
    components::{
        layer_item::BrushLayerItem,
        utils::{
            editor_state::BrushEditorState,
            renderer::{
                buffer::LayerBuffer,
                render::{render_pass, setup_gl},
                shader_manager::ShaderManager,
            },
            tools::BrushTool,
        },
    },
    data::{layer::Layer, project::BrushProject},
};

mod imp {

    use gtk::glib::WeakRef;

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
        pub position: Cell<(f32, f32)>, // Offset from screen center
        pub rotation: Cell<f32>,        // Radians
        pub mouse_pos: Cell<(f32, f32)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushCanvas {
        const NAME: &'static str = "BrushCanvas";
        type Type = super::BrushCanvas;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("canvas.zoom-in", None, |canvas, _, _| {
                canvas.zoom_by(0.05f32);
            });

            klass.install_action("canvas.new-pixel", None, |canvas, _, _| {
                let layer = canvas.new_pixel_layer();
                canvas.push_layer(layer);
            });

            klass.install_action("canvas.new-group", None, |canvas, _, _| {
                let layer = canvas.new_group_layer();
                canvas.push_layer(layer);
            });

            klass.install_action("canvas.remove-layer", None, |canvas, _, _| {
                canvas.remove_layer();
            });

            klass.install_action("canvas.move-layer-up", None, |canvas, _, _| {
                canvas.move_layer_up();
            });

            klass.install_action("canvas.move-layer-down", None, |canvas, _, _| {
                canvas.move_layer_down();
            });

            klass.install_action(
                "canvas.rename-layer",
                Some(VariantTy::STRING),
                move |canvas, _, arg| {
                    if let Some(var) = arg {
                        let value = var.to_string(); // 'Name'
                        let name = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                        if let Some(active_layer) = canvas.imp().active_layer.get() {
                            canvas.rename_layer(active_layer, name.to_string());
                        }
                    }
                },
            );

            klass.install_action(
                "canvas.set-layer-opacity",
                Some(VariantTy::DOUBLE),
                |canvas, _, arg| {
                    if let Some(var) = arg {
                        if let Some(val) = var.get::<f64>() {
                            canvas.set_layer_opacity(val as f32);
                        }
                    }
                },
            );

            klass.install_action("canvas.zoom-out", None, move |canvas, _, _| {
                canvas.zoom_by(-0.05f32);
            });

            klass.install_action("canvas.zoom-to-fit", None, move |canvas, _, _| {
                canvas.zoom_to_fit();
            });

            klass.install_action("canvas.pan-up", None, move |canvas, _, _| {
                canvas.move_by(0f32, -20f32);
            });

            klass.install_action("canvas.pan-down", None, move |canvas, _, _| {
                canvas.move_by(0f32, 20f32);
            });

            klass.install_action("canvas.pan-right", None, move |canvas, _, _| {
                canvas.move_by(20f32, 0f32);
            });

            klass.install_action("canvas.pan-left", None, move |canvas, _, _| {
                canvas.move_by(-20f32, 0f32);
            });

            klass.install_action("canvas.rotate-right", None, move |canvas, _, _| {
                canvas.rotate_by(PI / 5f32);
            });

            klass.install_action("canvas.rotate-left", None, move |canvas, _, _| {
                canvas.rotate_by(PI / -5f32);
            });

            klass.install_action("canvas.rotate-reset", None, move |canvas, _, _| {
                canvas.rotate_to(0f32);
            });

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

            obj.setup_accels_controller();
            obj.setup_motion_controller();
            obj.setup_scroll_controller();
            obj.setup_drag_controller();
            obj.setup_zoom_controller();
            obj.setup_rotate_controller();

            // Setup default values
            {
                self.zoom.set(1.0);
                self.position.set((0.0, 0.0));
                self.rotation.set(0.0);
                self.active_layer.set(None);
            }

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

                    render_pass(&obj.obj(), area)
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

                        if let Some(vao) = obj.gl_vao.get() {
                            unsafe {
                                gl.delete_vertex_array(*vao);
                            }
                        }
                    };
                });
            }
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
    pub fn new_pixel_layer(&self) -> Layer {
        let context = self.imp().project.borrow_mut();

        let name = "New pixel layer".to_owned();
        let width = context.width;
        let height = context.height;

        Layer::new_pixel(name, width, height)
    }

    pub fn new_group_layer(&self) -> Layer {
        let name = "New Group".to_owned();
        Layer::new_group(name)
    }

    pub fn push_layer(&self, layer: Layer) {
        {
            let mut project = self.imp().project.borrow_mut();

            if let Some(active_id) = self.imp().active_layer.get() {
                if let Some(active_layer) = project.clone().find_layer_mut(active_id) {
                    // If the active layer has children, append it to the layer
                    if let Some(children) = active_layer.children() {
                        let idx = children
                            .iter()
                            .position(|r| r.id() == active_layer.id())
                            .unwrap_or(0);

                        if let Some(a_layer) = project.find_layer_mut(active_layer.id()) {
                            a_layer.append(idx, layer.clone());
                        }
                        // If the parent of the active layer has children, append it to the parent
                    } else if let Some(parent) = project.find_parent_mut(active_id) {
                        if let Some(children) = parent.children() {
                            let idx = children
                                .iter()
                                .position(|r| r.id() == active_layer.id())
                                .unwrap_or(0);
                            parent.append(idx, layer.clone());
                        }
                        // If if doesn't have a parent, append it to the project in position
                    } else {
                        let idx = project
                            .layers
                            .iter()
                            .position(|r| r.id() == active_layer.id())
                            .unwrap_or(0);
                        project.layers.insert(idx, layer.clone());
                    }
                }
                //If there's no active layer, push it to the beginning
            } else {
                project.layers.push(layer.clone());
            }
        }

        let project = self.imp().project.borrow();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        project.remove_stale_widgets(layer.id(), &mut widget_cache);

        let _ = self.activate_action(
            "editor.activate-layer",
            Some(&layer.id().to_string().to_variant()),
        );

        self.imp().canvas.queue_draw();
    }

    pub fn remove_layer(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
        let mut widget_cache = imp.layer_widget_cache.borrow_mut();
        let mut buffer_cache = imp.buffer_cache.borrow_mut();

        if let Some(active_layer) = imp.active_layer.get() {
            // If the active layer's parent...
            if let Some(parent) = project.find_parent(active_layer) {
                // Is a group...
                if let Some(children) = parent.children() {
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
                }
                // If it doesn't have a parent
            } else {
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
            }
            // Remove layer and caches
            project.remove_stale_widgets(active_layer, &mut widget_cache);
            project.remove_layer(active_layer);
            buffer_cache.remove(&active_layer);
        }

        self.imp().canvas.queue_draw();
    }

    pub fn move_layer_up(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        let mut buf_cache = self.imp().buffer_cache.borrow_mut();

        if let Some(active) = self.imp().active_layer.get() {
            if let Some(layer) = project.clone().find_layer(active) {
                // Has a parent
                if let Some(parent) = project.clone().find_parent(active) {
                    if let Some(children) = parent.children() {
                        let idx = children
                            .iter()
                            .position(|l| l.id() == active)
                            .unwrap_or(children.len());
                        // If it ain't the first one in the parent
                        if idx != 0 {
                            // Move it up by 1
                            // And, if the previous layer is a group
                            if let Some(previous) = children.get(idx - 1) {
                                if let Some(previous_children) = previous.children() {
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
                            }
                        // If it is
                        } else {
                            // Bump it up a level
                            // Grandparent found
                            if let Some(grandparent) = project.clone().find_parent(parent.id()) {
                                if let Some(children) = grandparent.children() {
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
                                }
                            } else {
                                // At project root
                                let idx = project
                                    .clone()
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
                    }
                // At project root
                } else {
                    let idx = project
                        .layers
                        .iter()
                        .position(|l| l.id() == active)
                        .unwrap_or(project.layers.len());
                    if idx != 0 {
                        if let Some(previous) = project.clone().layers.get(idx - 1) {
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
            }
        }
        self.imp().canvas.queue_draw();
    }

    pub fn move_layer_down(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        let mut buf_cache = self.imp().buffer_cache.borrow_mut();

        if let Some(active) = self.imp().active_layer.get() {
            if let Some(layer) = project.clone().find_layer(active) {
                // Has a parent
                if let Some(parent) = project.clone().find_parent(active) {
                    if let Some(children) = parent.children() {
                        let idx = children
                            .iter()
                            .position(|l| l.id() == active)
                            .unwrap_or(children.len());
                        // If it ain't the first one in the parent
                        if idx != children.len() - 1 {
                            // Move it up by 1
                            // And, if the previous layer is a group
                            if let Some(next) = children.get(idx + 1) {
                                if next.children().is_some() {
                                    project.move_layer(
                                        layer,
                                        0,
                                        Some(parent.id()),
                                        Some(next.id()),
                                        &mut buf_cache,
                                        &mut widget_cache,
                                    )
                                } else {
                                    project.move_layer(
                                        layer,
                                        idx + 1,
                                        Some(parent.id()),
                                        Some(parent.id()),
                                        &mut buf_cache,
                                        &mut widget_cache,
                                    );
                                }
                            }
                        // If it is
                        } else {
                            // Bump it up a level
                            // Grandparent found
                            if let Some(grandparent) = project.clone().find_parent(parent.id()) {
                                if let Some(children) = grandparent.children() {
                                    let idx = children
                                        .iter()
                                        .position(|l| l.id() == parent.id())
                                        .unwrap_or(children.len());
                                    project.move_layer(
                                        layer,
                                        idx + 1,
                                        Some(parent.id()),
                                        Some(grandparent.id()),
                                        &mut buf_cache,
                                        &mut widget_cache,
                                    );
                                }
                            } else {
                                // At project root
                                let idx = project
                                    .clone()
                                    .layers
                                    .iter()
                                    .position(|l| l.id() == parent.id())
                                    .unwrap_or(children.len());
                                project.move_layer(
                                    layer,
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
                        .position(|l| l.id() == active)
                        .unwrap_or(project.layers.len());
                    if idx != project.layers.len() {
                        if let Some(next) = project.clone().layers.get(idx + 1) {
                            if next.children().is_some() {
                                project.move_layer(
                                    layer,
                                    0,
                                    None,
                                    Some(next.id()),
                                    &mut buf_cache,
                                    &mut widget_cache,
                                )
                            } else {
                                project.move_layer(
                                    layer,
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
            }
        }
        self.imp().canvas.queue_draw();
    }

    pub fn rename_layer(&self, uuid: Uuid, new_name: String) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();
        project.rename_layer(uuid, new_name);

        project.remove_stale_widgets(uuid, &mut widget_cache);
    }

    pub fn set_layer_opacity(&self, opacity: f32) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(active_id) = self.imp().active_layer.get() {
            if let Some(active_layer) = project.find_layer_mut(active_id) {
                active_layer.set_opacity(opacity);
            }
            project.remove_stale_widgets(active_id, &mut widget_cache);
        }
        imp.canvas.queue_draw();
    }

    // Viewport control
    pub fn zoom_by(&self, factor: f32) {
        let new_zoom = (self.imp().zoom.get() + factor).clamp(0.1, 10f32);
        self.imp().zoom.set(new_zoom);
        self.imp().canvas.queue_draw();
    }

    pub fn zoom_to(&self, zoom: f32) {
        self.imp().zoom.set(zoom.clamp(0.1, 10f32));
        self.imp().canvas.queue_draw();
    }

    pub fn move_by(&self, dx: f32, dy: f32) {
        let (x, y) = self.imp().position.get();
        self.imp().position.set((x + dx, y + dy));
        self.imp().canvas.queue_draw();
    }

    pub fn move_to(&self, x: f32, y: f32) {
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
        let (canvas_width, canvas_height) = (
            imp.project.borrow().width as f32,
            imp.project.borrow().height as f32,
        );
        let (viewport_width, viewport_height) = (self.width() as f32, self.height() as f32);

        println!("Canvas size: {}, {}", canvas_width, canvas_height);
        println!("Viewport size: {}, {}", viewport_width, viewport_height);

        let scale_x = viewport_width / canvas_width;
        let scale_y = viewport_height / canvas_height;

        let scale = scale_x.min(scale_y);

        self.zoom_to(scale);
        self.move_to(0f32, 0f32);
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

        let start_zoom = Rc::new(Cell::new(0f32));
        let start_pos = Rc::new(Cell::new((0f32, 0f32)));
        let start_drag = Rc::new(Cell::new((0f32, 0f32)));

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
                    start_drag.set((x as f32, y as f32));
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

                    let dx = center_x as f32 - old_x;
                    let dy = center_y as f32 - old_y;

                    let new_x = canvas_old_x + dx * zoom as f32;
                    let new_y = canvas_old_y + dy * zoom as f32;

                    obj.move_to(new_x, new_y);
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        self.add_controller(controller);
    }

    fn setup_drag_controller(&self) {
        let drag = gtk::GestureDrag::new();

        let start_pos = Rc::new(Cell::new((0f32, 0f32)));

        drag.connect_drag_begin(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            start_pos,
            move |_, _x, _y| {
                let pos = obj.imp().position.get();
                start_pos.set(pos);

                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {}  // No op
                        BrushTool::Brush => {} // TODO: Initial draw
                        _ => {}
                    }
                }
            }
        ));

        drag.connect_drag_update(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            start_pos,
            move |gesture, offset_x, offset_y| {
                let (orig_x, orig_y) = start_pos.get();

                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        BrushTool::Move => {
                            obj.move_to(orig_x + offset_x as f32, orig_y + offset_y as f32)
                        }
                        BrushTool::Brush => {
                            if let Some(event) = gesture.last_event(None) {
                                let pressure = event.axis(gdk::AxisUse::Pressure).unwrap_or(1.0);
                                let _x_tilt = event.axis(gdk::AxisUse::Xtilt).unwrap_or(0.0);
                                let _y_tilt = event.axis(gdk::AxisUse::Ytilt).unwrap_or(0.0);

                                obj.draw_stroke(pressure);
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

        self.add_controller(drag);
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

    fn setup_accels_controller(&self) {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::minus,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.zoom-out")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::equal,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.zoom-in")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Home,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.zoom-to-fit")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::bracketleft,
                gdk::ModifierType::CONTROL_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.rotate-left")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::bracketright,
                gdk::ModifierType::CONTROL_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.rotate-right")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Home,
                gdk::ModifierType::SHIFT_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.rotate-reset")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Up,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.pan-up")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Down,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.pan-down")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Right,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.pan-right")),
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Left,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("canvas.pan-left")),
        ));

        self.add_controller(controller);
    }

    fn setup_scroll_controller(&self) {
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);

        let weak_self = self.downgrade();

        scroll.connect_scroll(move |_controller, _dx, dy| {
            let Some(obj) = weak_self.upgrade() else {
                return glib::Propagation::Proceed;
            };

            let imp = obj.imp();

            let (win_w, win_h) = (obj.width() as f32, obj.height() as f32);

            let (mouse_x, mouse_y) = imp.mouse_pos.get();

            let old_zoom = imp.zoom.get();
            let (old_x, old_y) = imp.position.get();

            let zoom_mult = if dy < 0.0 { 1.1 } else { 0.9 };
            let zoom = (old_zoom * zoom_mult).clamp(0.001, 100.0);

            let factor = zoom / old_zoom;

            let new_x = mouse_x - win_w / 2.0 - factor * (mouse_x - win_w / 2.0 - old_x);
            let new_y = mouse_y - win_h / 2.0 - factor * (mouse_y - win_h / 2.0 - old_y);

            obj.zoom_to(zoom);
            obj.move_to(new_x, new_y);
            obj.imp().canvas.queue_draw();

            glib::Propagation::Stop
        });

        self.add_controller(scroll);
    }

    pub fn screen_to_canvas(
        &self,
        project: &BrushProject,
        screen_x: f64,
        screen_y: f64,
    ) -> (f64, f64) {
        let imp = self.imp();

        let win_w = self.width() as f32;
        let win_h = self.height() as f32;
        let canv_w = project.width as f32;
        let canv_h = project.height as f32;

        let zoom = imp.zoom.get();
        let rotation = imp.rotation.get(); // In radians
        let (pos_x, pos_y) = imp.position.get();

        let view =
            glam::Mat4::from_translation(glam::vec3(win_w / 2.0 + pos_x, win_h / 2.0 + pos_y, 0.0))
                * glam::Mat4::from_rotation_z(rotation)
                * glam::Mat4::from_scale(glam::vec3(zoom, zoom, 1.0))
                * glam::Mat4::from_translation(glam::vec3(-canv_w / 2.0, -canv_h / 2.0, 0.0));

        let inv_view = view.inverse();

        let point = glam::vec4(screen_x as f32, screen_y as f32, 0.0, 1.0);
        let result = inv_view * point;

        (result.x as f64, result.y as f64)
    }

    fn draw_stroke(&self, pressure: f64) {
        let mut project = self.imp().project.borrow_mut();
        let state = self.imp().editor_state.get().unwrap().borrow();

        // Brush parameters
        let base_size = state.brush_size.borrow();
        let base_opacity = state.brush_opacity.borrow();

        let color = state.primary_color.borrow().with_alpha(*base_opacity);

        // Brush coordinates
        let (px, py) = self.imp().mouse_pos.get();
        let (cx, cy) = self.screen_to_canvas(&project, px as f64, py as f64);

        if let Some(active_id) = self.imp().active_layer.get() {
            if let Some(layer) = project.find_layer_mut(active_id) {
                // TODO: Brush engine
                let dynamic_size = (*base_size as f64 * pressure).clamp(1f64, 1000f64) as i32;

                layer.draw_brush_dab(cx as i32, cy as i32, dynamic_size, color);

                self.imp().canvas.queue_render();
            }
        }
    }
}
