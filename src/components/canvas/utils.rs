/* canvas.rs
 *
 * Copyright 2026 FatDawlf
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use color::{AlphaColor, Oklab};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::{
    components::utils::{editor_state::BrushEditorState, renderer::shader_manager::ShaderManager},
    data::project::BrushProject,
};

pub async fn draw_stroke(
    project: &mut BrushProject,
    a_id: Option<Uuid>,
    state: &BrushEditorState,
    mask: Arc<RwLock<Vec<u8>>>,
    current_pressure: f64,
    last_pressure: f64,
    // Screen and canvas state
    current_point: (f64, f64),
    last_point: (f64, f64),
    screen: (f32, f32),
    s_pos: (f64, f64),
    zoom: f32,
    rotation: f32,
) {
    let mut mask = mask.write().unwrap();

    // Brush parameters
    let base_size = state.brush_size.borrow();
    let base_opacity = state.brush_opacity.borrow();
    let erase_mode = state.erase_mode.borrow();

    let color = state.primary_color.borrow().with_alpha(*base_opacity);
    let oklab: AlphaColor<Oklab> = color.convert();

    // Brush coordinates
    let cp = screen_to_canvas(project, current_point, screen, s_pos, zoom, rotation);
    let lp = screen_to_canvas(project, last_point, screen, s_pos, zoom, rotation);

    let interpolation_factor = if last_pressure < 0.3 {
        (0.1 * (3. * last_pressure)).clamp(0.05, 0.1)
    } else {
        0.1
    };

    if let Some(active_id) = a_id {
        if project.is_layer_in_lock(active_id) {
            // Don't draw if locked or hidden
            return;
        }

        // TODO: Brush engine
        if let Some(layer) = project.find_layer_mut(active_id) {
            let points = interpolate_stroke(
                cp,
                lp,
                *base_size as f64,
                current_pressure,
                last_pressure,
                interpolation_factor,
            );

            let p_len = points.len();

            for (x, y, p) in points {
                let dynamic_size = (*base_size as f64 * p).clamp(1f64, 1000f64);
                let should_par = dynamic_size > 150. || p_len > 10;

                layer.draw_brush_dab(
                    &mut mask,
                    (x as i32, y as i32),
                    dynamic_size as i32,
                    oklab,
                    *erase_mode,
                    should_par,
                );
            }
        }
    }
}

pub fn screen_to_canvas(
    project: &BrushProject,
    (x, y): (f64, f64),
    (sw, sh): (f32, f32),
    (px, py): (f64, f64),
    zoom: f32,
    rotation: f32,
) -> (f64, f64) {
    let canv_w = project.width as f32;
    let canv_h = project.height as f32;

    let view =
        glam::Mat4::from_translation(glam::vec3(sw / 2.0 + px as f32, sh / 2.0 + py as f32, 0.0))
            * glam::Mat4::from_rotation_z(rotation)
            * glam::Mat4::from_scale(glam::vec3(zoom, zoom, 1.0))
            * glam::Mat4::from_translation(glam::vec3(-canv_w / 2.0, -canv_h / 2.0, 0.0));

    let inv_view = view.inverse();

    let point = glam::vec4(x as f32, y as f32, 0.0, 1.0);
    let result = inv_view * point;

    (result.x as f64, result.y as f64)
}

fn interpolate_stroke(
    new_pos: (f64, f64),
    last_pos: (f64, f64),
    brush_radius: f64,
    new_pressure: f64,
    last_pressure: f64,
    spacing_ratio: f64, // e.g., 0.1 for 10% spacing
) -> Vec<(f64, f64, f64)> {
    let dx = new_pos.0 - last_pos.0;
    let dy = new_pos.1 - last_pos.1;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance < f64::EPSILON {
        return vec![(new_pos.0, new_pos.1, new_pressure)];
    }

    let step_size = (brush_radius * 2f64) * spacing_ratio;
    let mut points = Vec::new();

    let mut traveled = 0f64;
    while traveled < distance {
        let t = traveled / distance;

        let x = last_pos.0 + dx * t;
        let y = last_pos.1 + dy * t;
        let p = last_pressure + (new_pressure - last_pressure) * t;

        points.push((x, y, p));
        traveled += step_size;
    }
    points
}

pub unsafe fn capture_oklab_to_srgb_png(
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
