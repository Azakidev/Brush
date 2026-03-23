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
                render::{render_pass, setup_gl},
                shader_manager::ShaderManager,
            },
            tools::BrushTool,
        },
    },
    data::{layer::Layer, project::BrushProject},
};

mod imp {

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
        pub texture_cache: RefCell<HashMap<Uuid, glow::Texture>>,
        pub layer_widgets: RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>,
        // Gl context
        pub gl_context: OnceCell<Context>,
        pub gl_lib: OnceCell<Library>,
        pub gl_shader_manager: OnceCell<RefCell<ShaderManager>>,
        pub gl_vao: OnceCell<NativeVertexArray>,
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

            klass.install_action("canvas.zoom-in", None, |content, _, _| {
                content.zoom_by(0.05f32);
            });

            klass.install_action("canvas.new-pixel", None, |content, _, _| {
                let layer = content.new_pixel_layer();
                content.push_layer(layer);
            });

            klass.install_action("canvas.new-group", None, |content, _, _| {
                let layer = content.new_group_layer();
                content.push_layer(layer);
            });

            klass.install_action("canvas.remove-layer", None, |content, _, _| {
                content.remove_layer();
            });

            klass.install_action("canvas.move-layer-up", None, |content, _, _| {
                content.move_layer_up();
            });

            klass.install_action("canvas.move-layer-down", None, |content, _, _| {
                content.move_layer_down();
            });

            klass.install_action(
                "canvas.rename-layer",
                Some(VariantTy::STRING),
                move |content, _, arg| {
                    if let Some(var) = arg {
                        let value = var.to_string(); // 'Name'
                        let name = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                        if let Some(active_layer) = content.imp().active_layer.get() {
                            content.rename_layer(active_layer, name.to_string());
                        }
                    }
                },
            );

            klass.install_action("canvas.zoom-out", None, move |content, _, _| {
                content.zoom_by(-0.05f32);
            });

            klass.install_action("canvas.zoom-to-fit", None, move |content, _, _| {
                content.zoom_to_fit();
            });

            klass.install_action("canvas.rotate-right", None, move |content, _, _| {
                content.rotate_by(PI / 5f32);
            });

            klass.install_action("canvas.rotate-left", None, move |content, _, _| {
                content.rotate_by(PI / -5f32);
            });

            klass.install_action("canvas.rotate-reset", None, move |content, _, _| {
                content.rotate_to(0f32);
            });

            klass.install_action("canvas.print-state", None, move |content, _, _| {
                println!(
                    "Contents: {}",
                    serde_json::to_string(&content.imp().project).unwrap()
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

                        let _ = obj.obj().imp().gl_context.set(gl);
                        let _ = obj.obj().imp().gl_lib.set(gl_lib);

                        unsafe {
                            obj.obj().setup_program();
                        }
                    }
                ));

                let weak_self = self.downgrade();

                canvas.connect_render(move |area, _context| {
                    let Some(obj) = weak_self.upgrade() else {
                        return glib::Propagation::Proceed;
                    };

                    obj.obj().do_render(area)
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
    pub fn project_context(&self) -> Rc<RefCell<BrushProject>> {
        Rc::new(self.imp().project.clone())
    }

    pub fn widget_cache(&self) -> Rc<RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>> {
        Rc::new(self.imp().layer_widgets.clone())
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

    // GL stuff
    unsafe fn setup_program(&self) {
        let gl = self.imp().gl_context.get().unwrap();

        if let Some((shader_manager, vao)) = setup_gl(gl) {
            let _ = self
                .imp()
                .gl_shader_manager
                .set(RefCell::new(shader_manager));
            let _ = self.imp().gl_vao.set(vao);
        }
    }

    fn do_render(&self, area: &gtk::GLArea) -> glib::Propagation {
        render_pass(&self, area)
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
        let _ = self.activate_action(
            "editor.activate-layer",
            Some(&layer.id().to_string().to_variant()),
        );
    }

    pub fn remove_layer(&self) {
        let imp = self.imp();
        let mut project = imp.project.borrow_mut();
        let mut widget_cache = imp.layer_widgets.borrow_mut();
        let mut texture_cache = imp.texture_cache.borrow_mut();

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

            project.remove_layer(active_layer);
            texture_cache.remove(&active_layer);
            widget_cache.remove(&active_layer);
        }
    }

    pub fn move_layer_up(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widgets.borrow_mut();

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
                                        &mut widget_cache,
                                    )
                                } else {
                                    project.move_layer(
                                        layer,
                                        idx - 1,
                                        Some(parent.id()),
                                        Some(parent.id()),
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
                                    &mut widget_cache,
                                )
                            } else {
                                project.move_layer(layer, idx - 1, None, None, &mut widget_cache);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn move_layer_down(&self) {
        let mut project = self.imp().project.borrow_mut();
        let mut widget_cache = self.imp().layer_widgets.borrow_mut();

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
                                        &mut widget_cache,
                                    )
                                } else {
                                    project.move_layer(
                                        layer,
                                        idx + 1,
                                        Some(parent.id()),
                                        Some(parent.id()),
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
                                    &mut widget_cache,
                                )
                            } else {
                                project.move_layer(layer, idx + 1, None, None, &mut widget_cache);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn rename_layer(&self, uuid: Uuid, new_name: String) {
        let mut project = self.imp().project.borrow_mut();
        project.rename_layer(uuid, new_name);
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
                        BrushTool::Move => obj.move_to(orig_x + offset_x as f32, orig_y + offset_y as f32),
                        BrushTool::Brush => {
                            if let Some(event) = gesture.last_event(None) {
                                let pressure = event.axis(gdk::AxisUse::Pressure).unwrap_or(1.0);
                                let x_tilt = event.axis(gdk::AxisUse::Xtilt);
                                let y_tilt = event.axis(gdk::AxisUse::Ytilt);
                                println!("Pressure: {:?}", pressure);
                                println!("X tilt: {:?}", x_tilt);
                                println!("Y tilt: {:?}", y_tilt);
                            }
                        },
                        _ => unimplemented!()
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

    pub fn screen_to_canvas(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let imp = self.imp();
        let (win_w, win_h) = (self.width() as f32, self.height() as f32);
        let project = imp.project.borrow();

        let zoom = imp.zoom.get();
        let (pos_x, pos_y) = imp.position.get();
        let (canv_w, canv_h) = (project.width as f32, project.height as f32);

        // 1. Relative to screen center
        let mut x = screen_x - (win_w / 2.0);
        let mut y = screen_y - (win_h / 2.0);

        // 2. Undo the position and zoom
        x = (x - pos_x) / zoom;
        y = (y - pos_y) / zoom;

        // 3. Undo the Top-Left origin shift
        x += canv_w / 2.0;
        y += canv_h / 2.0;

        (x, y)
    }
}
