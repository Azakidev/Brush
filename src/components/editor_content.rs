/* editor.rs
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
    glib::{self, clone},
};
use libloading::Library;
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    f32::consts::PI,
    rc::Rc,
};
use uuid::Uuid;

use crate::{
    components::utils::{
        editor_state::BrushEditorState,
        renderer::render::{render_pass, setup_gl},
        tools::BrushTool,
    },
    data::{project::BrushProject, layer::Layer},
};

mod imp {

    use crate::components::utils::renderer::shader_manager::ShaderManager;

    use super::*;

    #[allow(dead_code)]
    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/editor-content.ui")]
    pub struct BrushEditorContent {
        // Template widgets
        #[template_child]
        pub canvas: TemplateChild<gtk::GLArea>,
        // Project context
        pub editor_state: OnceCell<Rc<RefCell<BrushEditorState>>>,
        pub context: RefCell<BrushProject>,
        pub texture_cache: RefCell<HashMap<Uuid, glow::Texture>>,
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
    impl ObjectSubclass for BrushEditorContent {
        const NAME: &'static str = "BrushEditorContent";
        type Type = super::BrushEditorContent;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("canvas.zoom-in", None, move |content, _, _| {
                content.zoom_by(0.05f32);
            });

            klass.install_action("canvas.new_pixel", None, move |content, _, _| {
                let layer = content.new_pixel_layer();
                content.push_layer(layer);
            });

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
                println!("Contents: {:?}", content.imp().context)
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushEditorContent {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_accels_controller();
            obj.setup_motion_controller();
            obj.setup_scroll_controller();
            obj.setup_drag_controller();
            obj.setup_pinch_controller();
            obj.setup_rotate_controller();

            // Setup default values
            {
                self.zoom.set(1.0);
                self.position.set((0.0, 0.0));
                self.rotation.set(0.0);
                self.active_layer.set(None);
            }

            // Layer setup test
            let layer = obj.new_pixel_layer();
            obj.push_layer(layer);

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
                        // If the widget was destroyed, we must still return Propagation
                        return glib::Propagation::Proceed;
                    };

                    obj.obj().do_render(area)
                });
            }
        }
    }
    impl WidgetImpl for BrushEditorContent {}
    impl BinImpl for BrushEditorContent {}
}

glib::wrapper! {
    pub struct BrushEditorContent(ObjectSubclass<imp::BrushEditorContent>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BrushEditorContent {
    pub fn new(editor_state: Rc<RefCell<BrushEditorState>>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp()
            .editor_state
            .set(editor_state)
            .expect("Editor state already set");
        obj
    }

    pub fn project_context(&self) -> Rc<RefCell<BrushProject>> {
        Rc::new(self.imp().context.clone())
    }

    pub fn zoom(&self) -> f32 {
        self.imp().zoom.get()
    }

    pub fn rotation(&self) -> f32 {
        self.imp().rotation.get()
    }

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

    pub fn new_pixel_layer(&self) -> Layer {
        let context = self.imp().context.borrow_mut();

        let name = "New pixel layer".to_owned();
        let width = context.width;
        let height = context.height;

        Layer::new_pixel(name, width, height)
    }

    pub fn push_layer(&self, layer: Layer) {
        let mut context = self.imp().context.borrow_mut();

        if let Some(active_layer) = self.imp().active_layer.get() {
            if let Some(active_layer) =
                context.clone().find_layer_mut(&active_layer.to_string())
            {
                if let Some(parent) = context.find_parent(&active_layer) {
                    if let Some(children) = parent.children() {
                        let idx = children
                            .iter()
                            .position(|r| r.id() == active_layer.id())
                            .unwrap_or(0);
                        parent.append(idx, layer.clone());
                    }
                } else {
                    let idx = context
                        .layers
                        .iter()
                        .position(|r| r.id() == active_layer.id())
                        .unwrap_or(0);
                    context.layers.insert(idx, layer.clone());
                }
            }
        } else {
            context.layers.push(layer.clone());
        }
        self.imp().active_layer.set(Some(layer.id()));
    }

    pub fn rename_layer(&self, uuid: String, new_name: String) {
        let mut context = self.imp().context.borrow_mut();
        context.rename_layer(&uuid, new_name);
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
            imp.context.borrow().width as f32,
            imp.context.borrow().height as f32,
        );
        let (viewport_width, viewport_height) = (self.width() as f32, self.height() as f32);

        let scale_x = viewport_width / canvas_width;
        let scale_y = viewport_height / canvas_height;

        let scale = scale_x.min(scale_y);

        self.zoom_to(scale);
        self.move_to(0f32, 0f32);
        imp.canvas.get().queue_draw();
    }

    pub fn setup_rotate_controller(&self) {
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

                let angle = controller.angle_delta() as f32;

                if angle.abs() > PI / 20f32 {
                    should_rotate.set(true)
                }

                if should_rotate.get() {
                    obj.rotate_to(orig_rot + angle);

                    obj.imp().canvas.queue_draw();
                }
            }
        ));

        self.add_controller(controller);
    }

    pub fn setup_pinch_controller(&self) {
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

    pub fn setup_drag_controller(&self) {
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
            move |_, offset_x, offset_y| {
                let (orig_x, orig_y) = start_pos.get();

                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();

                    if state.tool.borrow().eq(&BrushTool::Move) {
                        obj.move_to(orig_x + offset_x as f32, orig_y + offset_y as f32);
                    }
                }

                obj.imp().canvas.queue_draw();
            }
        ));

        self.add_controller(drag);
    }

    pub fn setup_motion_controller(&self) {
        let motion = gtk::EventControllerMotion::new();
        let weak_self = self.downgrade();

        motion.connect_motion(move |_, x, y| {
            if let Some(obj) = weak_self.upgrade() {
                obj.imp().mouse_pos.set((x as f32, y as f32));
            }
        });
        self.add_controller(motion);
    }

    pub fn setup_accels_controller(&self) {
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

    pub fn setup_scroll_controller(&self) {
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
        let project = imp.context.borrow();

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
