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

use std::{collections::HashMap, num::NonZero};

use adw::subclass::prelude::ObjectSubclassIsExt;
use glow::{HasContext, NativeFramebuffer, NativeVertexArray};
use gtk::{glib, prelude::WidgetExt};
use uuid::Uuid;

use crate::{
    components::{
        canvas::BrushCanvas,
        utils::renderer::{buffer::LayerBuffer, shader_manager::ShaderManager, utils::clean_unused_buffers},
    },
    data::{layer::Layer, project::BrushProject},
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
    let mut cache = imp.buffer_cache.borrow_mut();

    // Viewport parameters
    let (win_w, win_h) = (area.width() as f32, area.height() as f32);
    let (pw, ph) = (project.width as i32, project.height as i32);
    let zoom = imp.zoom.get();

    unsafe {
        use glow::HasContext;

        // Cleanup deleted buffers
        clean_unused_buffers(gl, &mut cache, &project);

        // Save default FBO
        let default_fbo_id = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) as u32;
        let default_fbo = NativeFramebuffer {
            0: NonZero::new(default_fbo_id).expect("default_fbo shouldn't be 0"),
        };

        gl.bind_vertex_array(Some(*vao));

        // Create root FBO and clear it
        let root_fbo = get_or_create_root_buffer(gl, canvas, &project);

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(root_fbo.framebuffer));
        gl.viewport(0, 0, pw, ph);

        gl.clear_color(0.0, 0.0, 0.0, 0.0);
        gl.clear(glow::COLOR_BUFFER_BIT);

        // Render layers to FBO
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::BLEND_SRC_ALPHA);

        let root_mvp = glam::Mat4::orthographic_lh(0.0, pw as f32, ph as f32, 0.0, -1.0, 1.0);

        render_layer_tree(
            &mut cache,
            gl,
            &mut project.layers,
            &mut shaders,
            &root_mvp,
            root_fbo.framebuffer,
            pw as i32,
            ph as i32,
        );

        // Composite to camera
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(default_fbo));
        gl.viewport(0, 0, win_w as i32, win_h as i32);

        gl.clear_color(0.1, 0.1, 0.1, 1.0); // Dark background
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::DEPTH_TEST);

        let camera_mvp = calculate_global_mvp(canvas, area, &project);

        // Background
        draw_checkerboard(
            gl,
            &mut shaders,
            project.width as f32,
            project.height as f32,
            zoom,
            &camera_mvp,
        );

        composite_root_buffer(gl, &root_fbo, &mut shaders, &camera_mvp);

        // Clean up state
        gl.disable(glow::BLEND);
        gl.bind_vertex_array(None);

        gl.flush();
    }

    glib::Propagation::Proceed
}

unsafe fn render_layer_tree(
    cache: &mut HashMap<Uuid, LayerBuffer>,
    gl: &glow::Context,
    layers: &mut [Layer],
    shaders: &mut ShaderManager,
    parent_mvp: &glam::Mat4,
    parent_fbo: NativeFramebuffer,
    parent_w: i32,
    parent_h: i32,
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

                composite_buffer(gl, &buffer, &layer, shaders, parent_mvp);

                // gl.bind_framebuffer(glow::FRAMEBUFFER, parent_fbo);
            }
            Layer::Group(_) => {
                layer.resize_group();

                let (gw, gh) = (layer.width() as i32, layer.height() as i32);
                let (gx, gy) = (layer.x() as f32, layer.y() as f32);

                if gw <= 0 || gh <= 0 {
                    continue;
                } // Skip empty groups

                let group_buffer = get_or_create_buffer(cache, gl, layer);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(group_buffer.framebuffer));
                gl.viewport(0, 0, gw, gh);
                gl.clear_color(0.0, 0.0, 0.0, 0.0); // Transparent background!
                gl.clear(glow::COLOR_BUFFER_BIT);

                let group_proj =
                    glam::Mat4::orthographic_lh(0.0, gw as f32, gh as f32, 0.0, -1.0, 1.0);

                // Shift children so (gx, gy) becomes (0,0) inside the FBO
                let group_view = glam::Mat4::from_translation(glam::vec3(gx, gy, 0.0));
                let group_mvp = group_proj * group_view;

                if let Some(children) = layer.children_mut() {
                    render_layer_tree(
                        cache,
                        gl,
                        children,
                        shaders,
                        &group_mvp,
                        group_buffer.framebuffer,
                        gw,
                        gh,
                    );
                }

                // debug_buffer_contents(gl, layer);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(parent_fbo));
                gl.viewport(0, 0, parent_w, parent_h);

                composite_buffer(gl, &group_buffer, layer, shaders, parent_mvp);
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
    let needs_realloc = cache
        .get(&layer.id())
        .map(|b| b.width != layer.width() || b.height != layer.height())
        .unwrap_or(false);

    if needs_realloc {
        if let Some(old_buf) = cache.remove(&layer.id()) {
            unsafe {
                gl.delete_framebuffer(old_buf.framebuffer);
                gl.delete_texture(old_buf.texture);
            }
        }
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
    active_mvp: &glam::Mat4,
) {
    shaders.layer.bind(gl);

    let (x, y) = (layer.x() as f32, layer.y() as f32);
    let (w, h) = (buffer.width as f32, buffer.height as f32);

    let model = glam::Mat4::from_translation(glam::vec3(x, y, 0.0))
        * glam::Mat4::from_scale(glam::vec3(w, h, 1.0));

    let final_mvp = *active_mvp * model;

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_mvp") {
        gl.uniform_matrix_4_f32_slice(Some(&loc), false, &final_mvp.to_cols_array());
    }

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_opacity") {
        gl.uniform_1_f32(Some(&loc), layer.opacity());
    }
    if let Some(loc) = shaders.layer.get_uniform(gl, "u_flip_y") {
        gl.uniform_1_f32(Some(&loc), 1.0); // Static textures usually don't need flipping
    }

    gl.enable(glow::BLEND);
    gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
    gl.blend_equation(glow::FUNC_ADD);

    gl.bind_texture(glow::TEXTURE_2D, Some(buffer.texture));
    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
}

pub unsafe fn composite_root_buffer(
    gl: &glow::Context,
    buffer: &LayerBuffer,
    shaders: &mut ShaderManager,
    active_mvp: &glam::Mat4,
) {
    shaders.layer.bind(gl);

    let (w, h) = (buffer.width as f32, buffer.height as f32);

    let model = glam::Mat4::from_translation(glam::vec3(0.0, 0.0, 0.0))
        * glam::Mat4::from_scale(glam::vec3(w, h, 1.0));

    let final_mvp = *active_mvp * model;

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_mvp") {
        gl.uniform_matrix_4_f32_slice(Some(&loc), false, &final_mvp.to_cols_array());
    }

    if let Some(loc) = shaders.layer.get_uniform(gl, "u_opacity") {
        gl.uniform_1_f32(Some(&loc), 1.0);
    }
    if let Some(loc) = shaders.layer.get_uniform(gl, "u_flip_y") {
        gl.uniform_1_f32(Some(&loc), 0.0); // Static textures usually don't need flipping
    }

    gl.enable(glow::BLEND);
    gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
    gl.blend_equation(glow::FUNC_ADD);

    gl.bind_texture(glow::TEXTURE_2D, Some(buffer.texture));
    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
}

fn calculate_global_mvp(
    canvas: &BrushCanvas,
    area: &gtk::GLArea,
    project: &BrushProject,
) -> glam::Mat4 {
    let imp = canvas.imp();

    let (win_w, win_h) = (area.width() as f32, area.height() as f32);
    let (canvas_w, canvas_h) = (project.width as f32, project.height as f32);

    let (pos_x, pos_y) = imp.position.get();
    let zoom = imp.zoom.get();
    let rotation = imp.rotation.get();

    let projection = glam::Mat4::orthographic_lh(0.0, win_w, win_h, 0.0, -1.0, 1.0);

    let view =
        glam::Mat4::from_translation(glam::vec3(win_w / 2.0 + pos_x, win_h / 2.0 + pos_y, 0.0))
            * glam::Mat4::from_rotation_z(rotation)
            * glam::Mat4::from_scale(glam::vec3(zoom, zoom, 1.0))
            * glam::Mat4::from_translation(glam::vec3(-canvas_w / 2.0, -canvas_h / 2.0, 0.0));

    projection * view
}

pub unsafe fn draw_checkerboard(
    gl: &glow::Context,
    shaders: &mut ShaderManager,
    canvas_w: f32,
    canvas_h: f32,
    zoom: f32,
    mvp: &glam::Mat4,
) {
    shaders.background.bind(gl);

    let model = glam::Mat4::from_scale(glam::vec3(canvas_w, canvas_h, 1.0));

    let final_mvp = *mvp * model;

    if let Some(loc) = shaders.background.get_uniform(gl, "u_mvp") {
        gl.uniform_matrix_4_f32_slice(Some(&loc), false, &final_mvp.to_cols_array());
    }
    if let Some(loc) = shaders.background.get_uniform(gl, "u_canvas_size") {
        gl.uniform_2_f32(Some(&loc), canvas_w as f32, canvas_h as f32);
    }
    if let Some(loc) = shaders.background.get_uniform(gl, "u_zoom") {
        gl.uniform_1_f32(Some(&loc), zoom);
    }
    if let Some(loc) = shaders.layer.get_uniform(gl, "u_flip_y") {
        gl.uniform_1_f32(Some(&loc), 0.0); // Static textures usually don't need flipping
    }

    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
}

unsafe fn get_or_create_root_buffer<'a>(
    gl: &glow::Context,
    canvas: &'a BrushCanvas,
    project: &BrushProject,
) -> &'a LayerBuffer {
    let imp = canvas.imp();
    let root_fbo = &imp.gl_root_fbo;

    if root_fbo.get().is_none() {
        let fbo = LayerBuffer::new(gl, 0, 0, project.width, project.height, None);
        imp.gl_root_fbo
            .set(fbo)
            .expect("Root FBO already set, this shouldn't happen, just checked it's empty");
    }

    imp.gl_root_fbo.get().unwrap()
}
