/* group.rs
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

use crate::data::layer::{Layer, LayerData};

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupData {
    pub layers: Vec<Layer>,

    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl LayerData for GroupData {}

impl GroupData {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),

            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    pub fn calculate_group_bounds(&mut self) {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for child in &self.layers {
            if !child.visible() {
                continue;
            }

            // Note: If child is a group, this needs to be called recursively
            let (cx, cy, cw, ch) = (
                child.x(),
                child.y(),
                child.width() as i32,
                child.height() as i32,
            );
            min_x = min_x.min(cx);
            min_y = min_y.min(cy);
            max_x = max_x.max(cx + cw);
            max_y = max_y.max(cy + ch);
        }

        if min_x == i32::MAX {
            self.x = 0;
            self.y = 0;
            self.width = 0;
            self.height = 0;
        } else {
            self.x = min_x;
            self.y = min_y;
            self.width = (max_x - min_x) as u32;
            self.height = (max_y - min_y) as u32;
        }

        println!("Group layer resized");
        println!("X: {}", self.x);
        println!("Y: {}", self.y);
        println!("Width: {}", self.width);
        println!("Height: {}", self.height);
    }
}
