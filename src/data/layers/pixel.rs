/* pixel.rs
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

use serde::{Deserialize, Serialize};

use crate::data::layer::LayerData;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PixelData {
    #[serde(skip_serializing)]
    pub pixels: Vec<u8>,
    pub color_space: String, // TODO: Replace with enum

    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl LayerData for PixelData {}

#[allow(dead_code)]
impl PixelData {
    pub fn new(pixels: Vec<u8>, color_space: String, width: u32, height: u32) -> Self {
        Self {
            pixels,
            color_space,

            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let len = (width * height * 4) as usize;
        self.pixels.resize(len, 0u8);
    }
}
