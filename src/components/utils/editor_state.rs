/* editor_state.rs
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
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::cell::RefCell;

use color::OpaqueColor;
use strum::IntoEnumIterator;

use crate::components::utils::{color::Hsv, tools::BrushTool};

#[derive(Debug, PartialEq, Clone)]
#[allow(dead_code)]
pub struct BrushEditorState {
    pub tool: RefCell<BrushTool>,
    // Colors
    pub primary_color: RefCell<OpaqueColor<Hsv>>,
    pub secondary_color: RefCell<OpaqueColor<Hsv>>,
    // Brush
    pub brush_opacity: RefCell<f32>,
    pub brush_size: RefCell<u32>,
}

impl Default for BrushEditorState {
    fn default() -> Self {
        BrushEditorState {
            tool: RefCell::new(BrushTool::Brush),

            primary_color: RefCell::new(OpaqueColor::BLACK),
            secondary_color: RefCell::new(OpaqueColor::WHITE),

            brush_opacity: RefCell::new(1f32),
            brush_size: RefCell::new(64),
        }
    }
}

impl BrushEditorState {
    pub fn swap_colors(&self) {
        self.primary_color.swap(&self.secondary_color);
    }

    pub fn set_color(&self, primary: OpaqueColor<Hsv>) {
        self.primary_color.replace(primary.convert());
    }

    pub fn set_tool(&self, tool: &str) {
        if let Some(tool) = BrushTool::iter().find(|t| t.to_string() == tool) {
            self.tool.replace(tool);
        }
    }

    pub fn set_brush_opacity(&self, value: f32) {
        self.brush_opacity.replace(value);
    }

    pub fn set_brush_size(&self, value: u32) {
        self.brush_size.replace(value);
    }
}
