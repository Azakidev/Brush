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
use glow::{HasContext, NativeTexture, NativeVertexArray, PixelUnpackData};
use gtk::{glib, prelude::WidgetExt};
use uuid::Uuid;

use crate::{
    components::{
        editor_content::BrushEditorContent, utils::renderer::shader_manager::ShaderManager,
    },
    data::layer::Layer,
};

pub fn setup_gl(gl: &glow::Context) -> Option<(ShaderManager, NativeVertexArray)> {
    unsafe {
        let manager = ShaderManager::new(gl);

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

        // Texture coordinates attribute
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

        return Some((manager, vao));
    }
}

pub fn render_pass(canvas: &BrushEditorContent, area: &gtk::GLArea) -> glib::Propagation {
    let imp = canvas.imp();
    let project = imp.project.borrow();

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

        render_layer_tree(&mut cache, gl, &project.layers, &mut shaders, &mvp.to_cols_array());

        // Clean up state
        gl.disable(glow::BLEND);
        gl.bind_vertex_array(None);
        gl.use_program(None);

        gl.flush();
    }

    glib::Propagation::Proceed
}

unsafe fn render_layer_tree(
    cache: &mut HashMap<Uuid, NativeTexture>,
    gl: &glow::Context,
    layers: &[Layer],
    shaders: &mut ShaderManager,
    mvp: &[f32; 16],
) {
    let tree: Vec<&Layer> = layers.iter().rev().collect();
    for layer in tree {
        if !layer.visible() {
            continue;
        }

        match layer {
            Layer::Pixel(_) => {
                draw_pixel_layer(cache, gl, layer, shaders, mvp);
            }
            Layer::Group(_) => {
                let children = layer
                    .children()
                    .expect("Failed to get children from group layer");
                render_layer_tree(cache, gl, children, shaders, mvp);
            }
            Layer::Fill(_) => {
                todo!()
            }
            _ => unreachable!(), // If layer is something else, ignore for now
        }
    }
}

unsafe fn draw_pixel_layer(
    cache: &mut HashMap<Uuid, NativeTexture>,
    gl: &glow::Context,
    layer: &Layer,
    shaders: &mut ShaderManager,
    mvp: &[f32; 16],
) {
    shaders.layer.bind(gl);
    if let Some(mvp_loc) = shaders.layer.get_uniform(gl, "u_mvp") {
        gl.uniform_matrix_4_f32_slice(Some(&mvp_loc), false, mvp);
    }

    if let Some(texture) = prepare_texture(
        cache,
        gl,
        layer.id(),
        layer.width(),
        layer.height(),
        &layer.pixel_data().unwrap(),
    ) {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    }

    if let Some(alpha_loc) = shaders.layer.get_uniform(gl, "u_opacity") {
        gl.uniform_1_f32(Some(&alpha_loc), layer.opacity());
    }

    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
}

fn prepare_texture(
    cache: &mut HashMap<Uuid, NativeTexture>,
    gl: &glow::Context,
    layer_id: uuid::Uuid,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Option<glow::Texture> {

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
