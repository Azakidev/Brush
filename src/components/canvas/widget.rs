/* widget.rs
 *
 * Copyright 2026 FatDawlf
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::{
    gdk,
    glib::{self, VariantTy, WeakRef, clone},
    prelude::*,
    subclass::prelude::*,
};
use glow::{Context, HasContext, NativeVertexArray};
use libloading::Library;
use zip::result::ZipError;

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    f32::consts::PI,
    ops::Deref,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
    time::Duration,
};
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    components::{
        canvas::utils::{capture_oklab_to_srgb_png, draw_stroke},
        editor::EditorAction,
        layer_item::BrushLayerItem,
        utils::{
            editor_state::BrushEditorState,
            renderer::{
                buffer::LayerBuffer,
                frame_update::FrameUpdate,
                render::{get_or_create_buffer, get_or_create_root_buffer, render_pass, setup_gl},
                shader_manager::ShaderManager,
            },
            tools::BrushTool,
        },
        window::WindowActions,
    },
    data::{
        blend_modes::BrushBlendMode,
        file::{request_save, save_image, save_project},
        layer::Layer,
        project::BrushProject,
        rect::Rect,
    },
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

        pub project: Arc<RwLock<BrushProject>>,
        pub buffer_cache: RefCell<HashMap<Uuid, LayerBuffer>>,
        pub layer_widget_cache: RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>,

        // Gl context
        pub gl_context: OnceCell<Context>,
        pub gl_lib: OnceCell<Library>,
        pub gl_shader_manager: OnceCell<RefCell<ShaderManager>>,
        pub gl_vao: OnceCell<NativeVertexArray>,
        pub gl_root_fbo: OnceCell<LayerBuffer>,

        // pub gl_t_buffers: Arc<TripleBuffer>,

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
                let project = canvas.imp().project.read().unwrap().clone();
                println!("Contents: {}", serde_json::to_string(&project).unwrap())
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

            // Init the painting mask by clearing it
            obj.clear_mask();

            // Setup controllers
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

                        let project = imp.project.read().unwrap().clone();
                        let _root_fbo = unsafe { get_or_create_root_buffer(gl, &obj, &project) };

                        if let Some((shader_manager, vao)) = setup_gl(gl) {
                            let _ = imp.gl_shader_manager.set(RefCell::new(shader_manager));
                            let _ = imp.gl_vao.set(vao);
                        }
                    }
                ));

                let weak_self = obj.downgrade();
                canvas.connect_render(move |area, _context| {
                    let Some(obj) = weak_self.upgrade() else {
                        return glib::Propagation::Proceed;
                    };

                    let imp = obj.imp();

                    let Ok(project) = imp.project.read() else {
                        return glib::Propagation::Proceed;
                    };

                    let gl = imp.gl_context.get().unwrap();
                    let shaders = imp.gl_shader_manager.get().unwrap();
                    let vao = imp.gl_vao.get().unwrap();
                    let root_fbo = unsafe { get_or_create_root_buffer(gl, &obj, &project) };

                    let mut cache = imp.buffer_cache.borrow_mut();
                    let mut shaders = shaders.borrow_mut();

                    let win = (area.width() as f32, area.height() as f32);

                    render_pass(
                        gl,
                        *vao,
                        root_fbo,
                        &mut cache,
                        &mut shaders,
                        &project,
                        win,
                        imp.position.get(),
                        imp.zoom.get(),
                        imp.rotation.get(),
                    );

                    glib::Propagation::Proceed
                });
            }

            obj.connect_realize(|c| {
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    c,
                    async move {
                        gtk::glib::timeout_future(Duration::from_millis(20)).await;
                        c.imp().canvas.queue_render();
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
        let imp = obj.imp();

        // Project setup
        let first_id = project.layers.first().map(|l| l.id());

        *imp.project.write().unwrap() = project;
        imp.file_location.replace(Some(loc.to_string()));
        imp.active_layer.replace(first_id);
        // Editor state
        imp.editor_state
            .set(editor_state)
            .expect("Editor state already set");

        obj
    }

    // Query
    pub fn project_context(&self) -> BrushProject {
        self.imp().project.read().unwrap().clone()
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
            let mut project = self.imp().project.write().unwrap();

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
            let mut project = self.imp().project.write().unwrap();

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
            if let Some(children) = active_layer.children() {
                // If the active layer has children, append it to the layer
                let idx = children
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);

                active_layer.append(idx, layer);
            } else if let Some(parent) = project.find_parent_mut(active_id)
                && let Some(children) = parent.children()
            {
                // If the parent of the active layer has children, append it to the parent
                let idx = children
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);
                parent.append(idx, layer);
            } else {
                // If if doesn't have a parent, append it to the project in position
                let idx = project
                    .layers
                    .iter()
                    .position(|r| r.id() == active_id)
                    .unwrap_or(0);
                project.layers.insert(idx, layer);
            }
        } else {
            //If there's no active layer, push it to the beginning
            project.layers.push(layer);
        }
    }

    fn update_tree(&self, project: &mut BrushProject) {
        let mut widget_cache = self.imp().layer_widget_cache.borrow_mut();

        if let Some(id) = self.imp().active_layer.get() {
            project.remove_stale_widgets(id, &mut widget_cache);
        }

        self.imp().canvas.queue_render();
    }

    fn remove_layer(&self) {
        let imp = self.imp();

        let mut project = imp.project.write().unwrap();

        let mut widget_cache = imp.layer_widget_cache.borrow_mut();
        let mut buffer_cache = imp.buffer_cache.borrow_mut();

        if let Some(active_layer) = imp.active_layer.get() {
            // If the active layer's parent…
            // Is a group…
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

        self.imp().canvas.queue_render();
    }

    fn move_layer_up(&self) {
        let mut project = self.imp().project.write().unwrap();

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
        self.imp().canvas.queue_render();
    }

    fn move_layer_down(&self) {
        let mut project = self.imp().project.write().unwrap();

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
        self.imp().canvas.queue_render();
    }

    fn save_project(&self, project: BrushProject, location: Option<String>) {
        let imp = self.imp();

        let save_loc = location.or(imp.file_location.borrow().clone());

        if let Some(loc) = save_loc {
            // File string set
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    let pixels = obj
                        .get_composite(project.width as i32, project.height as i32)
                        .unwrap();

                    let feedback_loc = loc.clone();

                    let result = gtk::gio::spawn_blocking(move || {
                        let path = Path::new(loc.as_str());
                        if loc.as_str().contains(".bsh") {
                            save_project(path, project, pixels.as_slice())
                        } else {
                            save_image(path, project, pixels.as_slice())
                        }
                    })
                    .await
                    .expect("Failed to finish save");

                    obj.save_feedback(result, Some(feedback_loc));
                }
            ));
        } else {
            // Location not set, prompt user
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
                if let Ok(file) = request_save(!swap_to).await {
                    let path = file.as_path().to_str().unwrap().to_owned();
                    let new_name = file.as_path().file_name().unwrap().to_str().unwrap();

                    if swap_to {
                        let _ = obj.activate_action(
                            &EditorAction::RenameTab,
                            Some(&new_name.to_variant()),
                        );
                        obj.imp().file_location.replace(Some(path.clone()));
                    }

                    obj.save_project(project, Some(path));
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
    fn save_feedback(&self, result: Result<(), ZipError>, location: Option<String>) {
        match result {
            // TODO: The actual toasts
            Ok(_) => {
                if let Some(loc) = location {
                    let _ =
                        self.activate_action(&WindowActions::ShowToast, Some(&loc.to_variant()));
                }
            }
            Err(e) => {
                eprintln!("{e}");
            }
        }
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

                obj.imp().canvas.queue_render();
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

                    let new_x = canvas_old_x + dx * zoom;
                    let new_y = canvas_old_y + dy * zoom;

                    obj.move_to(new_x, new_y);
                }

                obj.imp().canvas.queue_render();
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
                                obj.dispatch_stroke_worker(pressure);
                            }
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }
            }
        ));

        // Reserved for future expansion
        controller.connect_released(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _, _, _| {
                if let Some(state) = obj.imp().editor_state.get() {
                    let state = state.borrow();
                    let tool = state.tool.borrow();

                    match *tool {
                        _ => {} // NO OP
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
                                obj.dispatch_stroke_worker(pressure);
                            }
                        }
                        _ => {
                            println!("Tool not implemented!")
                        }
                    }
                }
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
                        _ => {} // NO OP
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
            let zoom = (old_zoom * zoom_mult).clamp(0.1, 10.);

            if zoom != old_zoom {
                let factor = zoom / old_zoom;

                let new_x = mouse_x - win_w / 2.0 - factor * (mouse_x - win_w / 2.0 - old_x);
                let new_y = mouse_y - win_h / 2.0 - factor * (mouse_y - win_h / 2.0 - old_y);

                obj.zoom_to(zoom as f32);
                obj.move_to(new_x, new_y);

                obj.imp().canvas.queue_render();
            }

            glib::Propagation::Stop
        });

        self.add_controller(scroll);
    }

    fn clear_layer(&self) {
        let imp = self.imp();

        let gl = imp.gl_context.get().unwrap();
        let mut project = imp.project.write().unwrap();
        let mut cache = imp.buffer_cache.borrow_mut();

        if let Some(active_id) = imp.active_layer.get()
            && let Some(layer) = project.find_layer_mut(active_id)
        {
            layer.clear();
            unsafe {
                let buffer = get_or_create_buffer(&mut cache, gl, layer);
                let root_fbo = imp.gl_root_fbo.get().expect("Root FBO should exist");

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(buffer.framebuffer));
                gl.viewport(0, 0, layer.width() as i32, layer.height() as i32);

                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(root_fbo.framebuffer));
            }
        }
        imp.canvas.queue_render();
    }

    fn clear_mask(&self) {
        let project = self.imp().project.read().unwrap();
        let size = project.width * project.height;

        let mut mask = self.imp().stroke_mask.write().unwrap();
        *mask = vec![0; size as usize];
    }

    pub unsafe fn sync_layer_to_gpu(
        &self,
        gl: &glow::Context,
        cache: &mut HashMap<Uuid, LayerBuffer>,
        layer_id: Uuid,
        rect: Rect,
    ) {
        let mut project = self.imp().project.write().unwrap();
        let canvas = &self.imp().canvas;

        if let Some(layer) = project.find_layer_mut(layer_id) {
            let buffer = get_or_create_buffer(cache, gl, layer);

            if let Some(pixels) = layer.pixel_data() {
                unsafe {
                    gl.bind_texture(glow::TEXTURE_2D, Some(buffer.texture));

                    // Set byte-row alignment to match the master layer bounds
                    gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, layer.width() as i32);
                    gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, rect.x);
                    gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, rect.y);

                    let bytes = bytemuck::cast_slice(pixels);

                    gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        glow::RGBA,
                        glow::FLOAT,
                        glow::PixelUnpackData::Slice(Some(bytes)),
                    );

                    // Clear stride rule to keep composition clean
                    gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
                    gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
                    gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);

                    // Reset the dirty
                    layer.set_dirty(false);
                    layer.set_dirty_rect(None);
                }
                canvas.queue_render();
            }
        }
    }

    pub fn dispatch_stroke_worker(&self, pressure: f64) {
        let imp = self.imp();

        let Some(active_layer_id) = imp.active_layer.get() else {
            return;
        };
        let Some(editor_state_rc) = imp.editor_state.get() else {
            return;
        };

        // Capture thread-local snapshots synchronously on the main thread
        let editor_state_snapshot = editor_state_rc.borrow().clone();
        let screen = (self.width() as f32, self.height() as f32);
        let position = imp.position.get();
        let zoom = self.zoom();
        let rotation = self.rotation();
        let cp = imp.mouse_pos.get();
        let lp = imp.last_position.get();
        let l_pressure = imp.last_pressure.get();

        // Clone atomic pointers for safe cross-thread sharing
        let project_arc = Arc::clone(&imp.project);
        let mask_arc = Arc::clone(&imp.stroke_mask);

        // Save current positions into history immediately on the main thread
        imp.last_position.replace(cp);
        imp.last_pressure.set(pressure);

        let (sender, receiver) = async_channel::bounded::<FrameUpdate>(1);

        glib::spawn_future_local(clone!(
            #[weak(rename_to = canvas)]
            self,
            async move {
                let imp = canvas.imp();

                while let Ok(update) = receiver.recv().await {
                    let gl = imp.gl_context.get().unwrap();
                    let mut cache = imp.buffer_cache.borrow_mut();

                    unsafe {
                        canvas.sync_layer_to_gpu(
                            gl,
                            &mut cache,
                            update.dirty_layer_id,
                            update.rect,
                        );
                    }
                }
            }
        ));

        rayon::spawn(move || {
            if let Ok(mut project) = project_arc.try_write() {
                // Execute stroke math out-of-thread
                futures::executor::block_on(draw_stroke(
                    &mut *project,
                    Some(active_layer_id),
                    &editor_state_snapshot,
                    mask_arc,
                    pressure,
                    l_pressure,
                    cp,
                    lp,
                    screen,
                    position,
                    zoom,
                    rotation,
                ));

                if let Some(layer) = project.find_layer(active_layer_id) {
                    let final_update = FrameUpdate {
                        dirty_layer_id: active_layer_id,
                        rect: layer.dirty_rect().unwrap_or_default(),
                    };

                    // Ship the data across the bridge. This automatically alerts the main thread.
                    let _ = futures::executor::block_on(sender.send(final_update).into_future());
                }
            }
        });
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
                Self::NewPixel => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.new_pixel_layer();
                    });
                }
                Self::NewGroup => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.new_group_layer();
                    });
                }
                Self::RenameLayer => {
                    klass.install_action(&action, None, move |c, _, _| {
                        let mut cache = c.imp().layer_widget_cache.borrow_mut();

                        if let Some(active) = c.imp().active_layer.get()
                            && let Some(widget) = cache.get(&active)
                            && let Some(item) = widget.upgrade()
                        {
                            let name = item.imp().rename_entry.text().to_string();
                            c.rename_layer(active, name, &mut cache);
                        }
                    });
                }
                Self::DeleteLayer => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.remove_layer();
                    });
                }
                Self::ClearLayer => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.clear_layer();
                    });

                    klass.add_binding_action(
                        gdk::Key::Delete,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::MoveLayerUp => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.move_layer_up();
                    });
                }
                Self::MoveLayerDown => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.move_layer_down();
                    });
                }
                // Project handling
                Self::SaveProject => {
                    klass.install_action(&action, None, |c, _, _| {
                        let project = c.imp().project.read().unwrap().clone();
                        c.save_project(project, None);
                    });

                    klass.add_binding_action(gdk::Key::S, gdk::ModifierType::CONTROL_MASK, &action);
                }
                Self::SaveProjectAs => {
                    klass.install_action(&action, None, |c, _, _| {
                        let project = c.imp().project.read().unwrap().clone();
                        c.save_project_as(project, true);
                    });

                    klass.add_binding_action(
                        gdk::Key::S,
                        gdk::ModifierType::SHIFT_MASK.union(gdk::ModifierType::CONTROL_MASK),
                        &action,
                    );
                }
                Self::ExportProjectAs => {
                    klass.install_action(&action, None, |c, _, _| {
                        let project = c.imp().project.read().unwrap().clone();
                        c.save_project_as(project, false);
                    });

                    klass.add_binding_action(
                        gdk::Key::E,
                        gdk::ModifierType::SHIFT_MASK.union(gdk::ModifierType::CONTROL_MASK),
                        &action,
                    );
                }
                // Viewport control
                Self::ZoomIn => {
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
                Self::ZoomOut => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.zoom_by(-0.05f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::minus,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::ZoomToFit => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.zoom_to_fit();
                    });

                    klass.add_binding_action(
                        gdk::Key::Home,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::PanUp => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(0., 60.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Up,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::PanDown => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(0., -60.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Down,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::PanLeft => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(60., 0.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Left,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::PanRight => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.move_by(-60., 0.);
                    });

                    klass.add_binding_action(
                        gdk::Key::Right,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::RotateCW => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_by(PI / 5f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::bracketright,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }
                Self::RotateCCW => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_by(PI / -5f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::bracketleft,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }
                Self::RotateTo0 => {
                    klass.install_action(&action, None, move |c, _, _| {
                        c.rotate_to(0f32);
                    });

                    klass.add_binding_action(
                        gdk::Key::Home,
                        gdk::ModifierType::SHIFT_MASK,
                        &action,
                    );
                }
                Self::SetLayerOpacity => {
                    klass.install_action(&action, Some(VariantTy::DOUBLE), |c, _, arg| {
                        if let Some(var) = arg
                            && let Some(val) = var.get::<f64>()
                        {
                            c.set_layer_opacity(val as f32);
                        }
                    });
                }
                Self::SetLayerBlendMode => {
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
                Self::ToggleVisible => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_visible();
                    });
                }
                Self::ToggleLock => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_lock();
                    });
                }
                Self::ToggleAlphaClip => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_alpha_clip();
                    });
                }
                Self::ToggleAlphaLock => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_alpha_lock();
                    });
                }
                Self::TogglePassthrough => {
                    klass.install_action(&action, None, |c, _, _| {
                        c.toggle_passthrough();
                    });
                }
            }
        }
    }
}
