/* fill.rs
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

use crate::data::{
    blend_modes::BlendMode,
    layer::{LayerData, LayerParameter},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FillLayerType {
    Solid,
    Gradient,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FillLayerData {
    pub fill_type: FillLayerType,
    pub color: Option<u8>,
    pub gradient: Option<Vec<(f32, u8)>>, // Position, color
}

impl LayerData for FillLayerData {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FillLayerParameters {
    pub opacity: f32,
    pub visible: bool,
    pub alpha_clip: bool,
    pub alpha_lock: bool,
    pub blend_mode: BlendMode,
}

impl LayerParameter for FillLayerParameters {
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
