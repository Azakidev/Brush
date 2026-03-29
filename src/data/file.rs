/* file.rs
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

use std::path::Path;

use uuid::Uuid;

use crate::data::{layer::Layer, project::BrushProject};

#[allow(dead_code)]
pub fn save_project(_path: &Path, project: &BrushProject) {
    // Walk through each layer and save it
    save_layers(&project.layers);
    // Save the main project structure
    if let Ok(_structure) = serde_json::to_string(project) {
        todo!();
    }
    // TODO Generate a preview
    // TODO Commit the file
}

#[allow(dead_code)]
fn save_layers(layers: &Vec<Layer>) {
    for layer in layers {
        match layer {
            // Save children if it's a group
            Layer::Group(_) => {
                if let Some(children) = layer.children() {
                    save_layers(children);
                }
            }
            // Save data if it's a pixel layer
            Layer::Pixel(_) => {
                if let Some(data) = layer.pixel_data() {
                    save_pixel_data(layer.id(), data);
                }
            }
            // Do nothing if it's a data only layer
            _ => {}
        }
    }
}

#[allow(dead_code)]
fn save_pixel_data(_id: Uuid, _pixels: &Vec<f32>) {
    todo!();
}
