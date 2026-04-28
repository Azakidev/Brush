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
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::sync::{Arc, RwLock};

use color::{AlphaColor, Oklab};
use uuid::Uuid;

use crate::{components::utils::editor_state::BrushEditorState, data::project::BrushProject};

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
