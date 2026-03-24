/* render.rs
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

use std::collections::HashMap;

use adw::subclass::prelude::ObjectSubclassIsExt;
use glow::{HasContext, NativeVertexArray};
use gtk::{glib, prelude::WidgetExt};
use uuid::Uuid;

use crate::{
    components::{
        canvas::BrushCanvas,
        utils::renderer::{buffer::LayerBuffer, shader_manager::ShaderManager},
    },
    data::layer::Layer,
};

pub fn setup_gl(gl: &glow::Context) -> Option<(ShaderManager, NativeVertexArray)> {
    unsafe {
        let manager = ShaderManager::new(gl);

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

        // Texture coordinates attribute
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

        return Some((manager, vao));
    }
}

pub fn render_pass(canvas: &BrushCanvas, area: &gtk::GLArea) -> glib::Propagation {
    let imp = canvas.imp();
    let mut project = imp.project.borrow_mut();

    // OpenGL Context
    let Some(gl) = imp.gl_context.get() else {
        return glib::Propagation::Proceed;
    };
    let Some(shaders) = imp.gl_shader_manager.get() else {
        return glib::Propagation::Proceed;
    };
    let Some(vao) = imp.gl_vao.get() else {
        return glib::Propagation::Proceed;
    };

    let mut shaders = shaders.borrow_mut();
    let mut cache = imp.texture_cache.borrow_mut();

    // Viewport parameters
    let (win_w, win_h) = (area.width() as f32, area.height() as f32);
    let (canvas_w, canvas_h) = (project.width as f32, project.height as f32);
    let zoom = imp.zoom.get();
    let (pos_x, pos_y) = imp.position.get();
    let rotation = imp.rotation.get();

    unsafe {
        use glow::HasContext;

        // Clear
        gl.viewport(0, 0, win_w as i32, win_h as i32);

        gl.clear_color(0.1, 0.1, 0.1, 1.0); // Dark background
        gl.clear(glow::COLOR_BUFFER_BIT);

        // Projection matrix
        let projection = glam::Mat4::orthographic_lh(0.0, win_w, win_h, 0.0, -1.0, 1.0);

        // Transformation Stack:
        // a) Start at screen center + user offset
        // b) Rotate the whole view
        // c) Apply Zoom
        // d) Move so the Quad's Top-Left is the local origin
        // e) Scale to the actual pixel size of the canvas
        let transform =
            glam::Mat4::from_translation(glam::vec3(win_w / 2.0 + pos_x, win_h / 2.0 + pos_y, 0.0))
                * glam::Mat4::from_rotation_z(rotation)
                * glam::Mat4::from_scale(glam::vec3(zoom, zoom, 1.0))
                * glam::Mat4::from_translation(glam::vec3(-canvas_w / 2.0, -canvas_h / 2.0, 0.0))
                * glam::Mat4::from_scale(glam::vec3(canvas_w, canvas_h, 1.0));

        let mvp = projection * transform;

        // Background
        shaders.background.bind(gl);
        if let Some(loc) = shaders.background.get_uniform(gl, "u_mvp") {
            gl.uniform_matrix_4_f32_slice(Some(&loc), false, &mvp.to_cols_array());
        }
        if let Some(loc) = shaders.background.get_uniform(gl, "u_canvas_size") {
            gl.uniform_2_f32(Some(&loc), project.width as f32, project.height as f32);
        }
        if let Some(loc) = shaders.background.get_uniform(gl, "u_zoom") {
            gl.uniform_1_f32(Some(&loc), zoom);
        }

        gl.bind_vertex_array(Some(*vao));
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        // Layers
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

        render_layer_tree(
            &mut cache,
            gl,
            &mut project.layers,
            &mut shaders,
            &mvp.to_cols_array(),
            win_w as i32,
            win_h as i32,
            canvas_w as f32,
            canvas_h as f32,
        );

        // Clean up state
        gl.disable(glow::BLEND);
        gl.bind_vertex_array(None);
        gl.use_program(None);

        gl.flush();
    }

    glib::Propagation::Proceed
}

unsafe fn render_layer_tree(
    cache: &mut HashMap<Uuid, LayerBuffer>,
    gl: &glow::Context,
    layers: &mut [Layer],
    shaders: &mut ShaderManager,
    mvp: &[f32; 16],
    win_w: i32,
    win_h: i32,
    canvas_w: f32,
    canvas_h: f32,
) {
    let tree: Vec<&mut Layer> = layers.iter_mut().rev().collect();
    for layer in tree {
        if !layer.visible() {
            continue;
        }

        match layer {
            Layer::Pixel(_) => {
                let buffer = get_or_create_buffer(cache, gl, &layer);
                if layer.is_dirty() {
                    gl.bind_texture(glow::TEXTURE_2D, Some(buffer.texture));

                    unsafe {
                        gl.tex_sub_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            0,
                            0,
                            layer.width() as i32,
                            layer.height() as i32,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(layer.pixel_data()),
                        );
                    }
                    layer.set_dirty(false); // Reset the flag
                }
                composite_buffer(gl, &buffer, &layer, shaders, mvp, canvas_w, canvas_h);
            }
            Layer::Group(_) => {
                let (gx, gy) = (layer.x(), layer.y());
                let (gw, gh) = (layer.width() as i32, layer.height() as i32);
                let group_buffer = get_or_create_buffer(cache, gl, layer);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(group_buffer.framebuffer));
                gl.viewport(0, 0, gw, gh);
                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(glow::COLOR_BUFFER_BIT);

                let local_mvp =
                    glam::Mat4::from_translation(glam::vec3(-gx as f32, -gy as f32, 0.0));

                if let Some(children) = layer.children_mut() {
                    render_layer_tree(
                        cache,
                        gl,
                        children,
                        shaders,
                        &local_mvp.to_cols_array(),
                        win_w,
                        win_h,
                        canvas_w,
                        canvas_h
                    );
                } else {
                    continue; // Don't continue with this layer if somehow a group fails to get
                              // its children, should be unreachable but you're never sure
                }

                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                gl.viewport(0, 0, win_w as i32, win_h as i32);

                composite_buffer(gl, &group_buffer, layer, shaders, mvp, canvas_w, canvas_h);
            }
            Layer::Fill(_) => {
                todo!()
            }
            _ => unreachable!(), // If layer is something else, ignore for now
        }
    }
}

pub fn get_or_create_buffer(
    cache: &mut HashMap<Uuid, LayerBuffer>,
    gl: &glow::Context,
    layer: &Layer,
) -> LayerBuffer {
    // If dimensions changed, we must reallocate to avoid stretching
    let needs_realloc = cache
        .get(&layer.id())
        .map(|b| b.width != layer.width() || b.height != layer.height())
        .unwrap_or(false);

    if needs_realloc {
        cache.remove(&layer.id());
    }

    let buffer = cache.entry(layer.id()).or_insert_with(|| unsafe {
        LayerBuffer::new(
            gl,
            layer.x(),
            layer.y(),
            layer.width(),
            layer.height(),
            layer.pixel_data(),
        )
    });

    return buffer.clone();
}

pub unsafe fn composite_buffer(
    gl: &glow::Context,
    buffer: &LayerBuffer,
    layer: &Layer,
    shaders: &mut ShaderManager,
    global_mvp: &[f32; 16],
    canvas_w: f32,
    canvas_h: f32,
) {
    shaders.layer.bind(gl);

    let model = glam::Mat4::from_translation(glam::vec3(layer.x() as f32, layer.y() as f32, 0.0))
        * glam::Mat4::from_scale(glam::vec3(
            layer.width() as f32 / canvas_w,
            layer.height() as f32 / canvas_h,
            1.0,
        ));

    let final_mvp = glam::Mat4::from_cols_array(global_mvp) * model;

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_mvp") {
        gl.uniform_matrix_4_f32_slice(Some(&loc), false, &final_mvp.to_cols_array());
    }

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_opacity") {
        gl.uniform_1_f32(Some(&loc), layer.opacity());
    }

    gl.bind_texture(glow::TEXTURE_2D, Some(buffer.texture));
    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
}
