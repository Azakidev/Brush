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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glow::{Context, HasContext, NativeVertexArray, PixelUnpackData, Program};
use gtk::glib::{self, clone, Properties};
use libloading::Library;
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use uuid::Uuid;

use crate::components::utils::renderer::shader::{compile_shader, FRAG_SRC, VERT_SRC};
use crate::data::file::BrushProject;
use crate::data::layer::Layer;

mod imp {

    use super::*;

    #[allow(dead_code)]
    #[derive(Default, Debug, Properties, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/editor-content.ui")]
    #[properties(wrapper_type= super::BrushEditorContent)]
    pub struct BrushEditorContent {
        // Template widgets
        #[template_child]
        pub canvas: TemplateChild<gtk::GLArea>,
        // Properties
        #[property(get, set)]
        pub active_tool: RefCell<String>,
        // Project context
        pub context: RefCell<BrushProject>,
        pub texture_cache: RefCell<HashMap<Uuid, glow::Texture>>,
        // Gl context
        pub gl_context: OnceCell<Context>,
        pub gl_lib: OnceCell<Library>,
        pub gl_program: OnceCell<Program>,
        pub gl_vao: OnceCell<NativeVertexArray>,
        // Viewport
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

            klass.install_action("canvas.zoom-out", None, move |content, _, _| {
                content.zoom_by(-0.05f32);
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushEditorContent {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_motion_controller();
            obj.setup_scroll_controller();
            obj.setup_drag_controller();

            // Layer setup test
            {
                obj.add_pixel_layer();
                let layer = self
                    .context
                    .borrow()
                    .layers
                    .first()
                    .unwrap()
                    .id()
                    .to_string();

                obj.rename_layer(layer, "Waow".to_owned());
            }

            // Setup default values
            {
                self.zoom.set(1.0);
                self.position.set((0.0, 0.0));
                self.rotation.set(0.0);
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
    pub fn new() -> Self {
        glib::Object::new()
    }

    unsafe fn setup_program(&self) {
        let gl = self.imp().gl_context.get().unwrap();

        let vs = compile_shader(gl, glow::VERTEX_SHADER, VERT_SRC);
        let fs = compile_shader(gl, glow::FRAGMENT_SHADER, FRAG_SRC);
        let program = gl.create_program().expect("Cannot create program");

        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            panic!("Link Error: {}", gl.get_program_info_log(program));
        }

        // [x, y, u, v]
        let vertices: [f32; 16] = [
            0.0, 0.0, 0.0, 0.0, // Top Left
            1.0, 0.0, 1.0, 0.0, // Top Right
            0.0, 1.0, 0.0, 1.0, // Bottom Left
            1.0, 1.0, 1.0, 1.0, // Bottom Right
        ];

        let vao = gl.create_vertex_array().expect("Cannot create VAO");
        gl.bind_vertex_array(Some(vao));

        let vbo = gl.create_buffer().expect("Cannot create VBO");
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&vertices),
            glow::STATIC_DRAW,
        );

        // Position attribute
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);

        // TexCoords attribute
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

        // Store these in your RefCell/OnceCell in the Imp
        let _ = self.imp().gl_program.set(program);
        let _ = self.imp().gl_vao.set(vao);
    }

    fn do_render(&self, area: &gtk::GLArea) -> glib::Propagation {
        let imp = self.imp();
        let context = self.imp().context.borrow();

        // OpenGL Context
        let Some(gl) = imp.gl_context.get() else {
            return glib::Propagation::Proceed;
        };
        let Some(program) = imp.gl_program.get() else {
            return glib::Propagation::Proceed;
        };
        let Some(vao) = imp.gl_vao.get() else {
            return glib::Propagation::Proceed;
        };

        // Viewport parameters
        let (win_w, win_h) = (area.width() as f32, area.height() as f32);
        let (canvas_w, canvas_h) = (context.width as f32, context.height as f32);
        let zoom = imp.zoom.get();
        let (pos_x, pos_y) = imp.position.get();
        let rotation = imp.rotation.get();

        let Some(layer) = context.layers.first() else {
            return glib::Propagation::Proceed;
        };

        let Some(pixel_data) = layer.pixel_data() else {
            return glib::Propagation::Proceed;
        };

        let Some(tex_handle) =
            self.prepare_texture(gl, layer.id(), context.width, context.height, &pixel_data)
        else {
            return glib::Propagation::Proceed;
        };

        unsafe {
            use glow::HasContext;

            gl.viewport(0, 0, win_w as i32, win_h as i32);
            gl.clear_color(0.1, 0.1, 0.1, 1.0); // Dark background
            gl.disable(glow::DEPTH_TEST);
            gl.clear(glow::COLOR_BUFFER_BIT);

            let projection = glam::Mat4::orthographic_lh(0.0, win_w, win_h, 0.0, -1.0, 1.0);

            // Transformation Stack:
            // a) Start at screen center + user offset
            // b) Rotate the whole view
            // c) Apply Zoom
            // d) Move so the Quad's Top-Left is the local origin
            // e) Scale to the actual pixel size of the canvas
            let transform = glam::Mat4::from_translation(glam::vec3(
                win_w / 2.0 + pos_x,
                win_h / 2.0 + pos_y,
                0.0,
            )) * glam::Mat4::from_rotation_z(rotation)
                * glam::Mat4::from_scale(glam::vec3(zoom, zoom, 1.0))
                * glam::Mat4::from_translation(glam::vec3(-canvas_w / 2.0, -canvas_h / 2.0, 0.0))
                * glam::Mat4::from_scale(glam::vec3(canvas_w, canvas_h, 1.0));

            let mvp = projection * transform;

            // 6. DRAWING
            gl.use_program(Some(*program));

            if !gl.get_program_link_status(*program) {
                let log = gl.get_program_info_log(*program);
                panic!("Shader Link Error: {}", log); // This will at least give you a log!
            }

            // Upload the matrix
            if let Some(loc) = gl.get_uniform_location(*program, "u_mvp") {
                gl.uniform_matrix_4_f32_slice(Some(&loc), false, &mvp.to_cols_array());
            } else {
                eprintln!("Warning: Uniform u_mvp not found!");
            }

            // Bind Texture
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex_handle));
            let u_tex = gl.get_uniform_location(*program, "u_texture");
            gl.uniform_1_i32(u_tex.as_ref(), 0);

            // Bind VAO and Draw the Quad
            gl.bind_vertex_array(Some(*vao));
            gl.disable(glow::CULL_FACE);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            // Clean up state
            gl.bind_vertex_array(None);
            gl.use_program(None);

            gl.flush();
        }

        glib::Propagation::Proceed
    }

    pub fn prepare_texture(
        &self,
        gl: &glow::Context,
        layer_id: uuid::Uuid,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Option<glow::Texture> {
        let mut cache = self.imp().texture_cache.borrow_mut();

        if let Some(&tex) = cache.get(&layer_id) {
            return Some(tex);
        }

        let expected_size = (width * height * 4) as usize;
        if pixels.len() != expected_size {
            eprintln!(
                "CRITICAL: Buffer size mismatch! Expected {}, got {}",
                expected_size,
                pixels.len()
            );
            return None;
        }

        unsafe {
            use glow::HasContext;
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            let tex = gl.create_texture().expect("Failed to create texture");

            gl.bind_texture(glow::TEXTURE_2D, Some(tex));

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

            // CLAMP_TO_EDGE prevents a "seam" at the edges of the canvas
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            // Upload the raw pixel data
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32, // Internal format
                width as i32,
                height as i32,
                0,                   // Border (must be 0)
                glow::RGBA,          // Format of source data
                glow::UNSIGNED_BYTE, // Type of source data
                PixelUnpackData::Slice(Some(pixels)),
            );

            // 3. Store in cache for the next frame
            cache.insert(layer_id, tex);
            Some(tex)
        }
    }

    pub fn add_pixel_layer(&self) {
        let mut context = self.imp().context.borrow_mut();

        let name = "New pixel layer".to_owned();
        let width = context.width;
        let height = context.height;

        let layer = Layer::new_pixel(name, width, height);

        context.layers.push(layer);
    }

    pub fn rename_layer(&self, uuid: String, new_name: String) {
        let mut context = self.imp().context.borrow_mut();
        context.rename_layer(&uuid, new_name);
    }

    // Viewport control
    pub fn zoom_by(&self, factor: f32) {
        let new_zoom = (self.imp().zoom.get() + factor).max(0.1);
        self.imp().zoom.set(new_zoom);
        self.imp().canvas.queue_draw();
    }

    pub fn zoom_to(&self, zoom: f32) {
        self.imp().zoom.set(zoom);
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
        let new_rot = self.imp().rotation.get() + radians;
        self.imp().rotation.set(new_rot);
        self.imp().canvas.queue_draw();
    }

    pub fn rotate_to(&self, radians: f32) {
        self.imp().rotation.set(radians);
        self.imp().canvas.queue_draw();
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

                let tool = obj.imp().active_tool.borrow();

                if tool.contains("move") {
                    obj.imp()
                        .position
                        .set((orig_x + offset_x as f32, orig_y + offset_y as f32));
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
            let zoom = (old_zoom * zoom_mult).clamp(0.01, 100.0);

            let actual_factor = zoom / old_zoom;

            let new_x = mouse_x - win_w / 2.0 - actual_factor * (mouse_x - win_w / 2.0 - old_x);
            let new_y = mouse_y - win_h / 2.0 - actual_factor * (mouse_y - win_h / 2.0 - old_y);

            imp.zoom.set(zoom);
            imp.position.set((new_x, new_y));

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
