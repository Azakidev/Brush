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
}

impl LayerData for GroupData {}

impl GroupData {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }
    pub fn append(&mut self, index: usize, layer: Layer) {
        self.layers.insert(index, layer);
    }
}
