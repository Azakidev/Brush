/* utils.rs
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

use glow::HasContext;
use uuid::Uuid;

use crate::{
    components::utils::renderer::buffer::LayerBuffer,
    data::{layer::Layer, project::BrushProject},
};

pub unsafe fn clean_unused_buffers(
    gl: &glow::Context,
    cache: &mut HashMap<Uuid, LayerBuffer>,
    project: &BrushProject,
) {
    let mut active_ids = std::collections::HashSet::new();
    collect_ids(&project.layers, &mut active_ids);

    cache.retain(|id, buffer| {
        if !active_ids.contains(id) {
            buffer.destroy(gl);
            false
        } else {
            true
        }
    });
}

fn collect_ids(layers: &[Layer], ids: &mut std::collections::HashSet<Uuid>) {
    for layer in layers {
        ids.insert(layer.id());
        if let Some(children) = layer.children() {
            collect_ids(children, ids);
        }
    }
}

#[allow(dead_code)]
pub fn debug_matrix(label: &str, m: &glam::Mat4) {
    let cols = m.to_cols_array_2d();
    println!("--- {} ---", label);
    for row in 0..4 {
        println!(
            "[{:7.2}, {:7.2}, {:7.2}, {:7.2}]",
            cols[0][row], cols[1][row], cols[2][row], cols[3][row]
        );
    }
}

#[allow(dead_code)]
pub unsafe fn debug_buffer_contents(gl: &glow::Context, layer: &Layer) {
    let (w, h) = (layer.width(), layer.height());
    let mut pixels = vec![0u8; (w * h * 4) as usize];

    gl.read_pixels(
        0,
        0,
        w as i32,
        h as i32,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelPackData::Slice(Some(&mut pixels)),
    );

    let has_data = pixels.iter().any(|&x| x > 0);
    let sum: u64 = pixels.iter().map(|&x| x as u64).sum();

    println!("--- FBO Diagnostic for Group {} ---", layer.id());
    println!("Dimensions: {}x{}", w, h);
    println!("Has non-zero pixels: {}", has_data);
    println!(
        "Byte Sum: {} (If 0, the texture is totally empty/transparent)",
        sum
    );
}
