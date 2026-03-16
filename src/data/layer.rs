/* layer.rs
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
use uuid::Uuid;

use crate::data::{
    blend_modes::BlendMode,
    layers::{
        filter::{Filter, FilterData},
        group::GroupData,
        pixel::PixelData,
    },
};

#[allow(dead_code)]
pub trait LayerParameter {
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
}
pub trait LayerData {}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Layer {
    Group(BrushLayer<NodeLayerParameters, GroupData>),
    Pixel(BrushLayer<NodeLayerParameters, PixelData>),
    Filter(BrushLayer<FilterLayerParameters, FilterData>),
}

impl Layer {
    pub fn new_pixel(name: String, width: u32, height: u32) -> Self {
        let params = NodeLayerParameters::default();
        let mut data = PixelData::new(Vec::new(), "OkLab".to_owned(), width, height);
        data.resize(width, height);

        Layer::Pixel(BrushLayer::new(name, params, data))
    }

    pub fn new_group(name: String) -> Self {
        let params = NodeLayerParameters::default();
        let data = GroupData::new();

        Layer::Group(BrushLayer::new(name, params, data))
    }

    pub fn id(&self) -> Uuid {
        match self {
            Layer::Pixel(inner) => Uuid::parse_str(&inner.id).unwrap(),
            Layer::Group(inner) => Uuid::parse_str(&inner.id).unwrap(),
            Layer::Filter(inner) => Uuid::parse_str(&inner.id).unwrap(),
        }
    }

    pub fn pixel_data(&self) -> Option<Vec<u8>> {
        match self {
            Layer::Pixel(inner) => Some(inner.data.pixels.clone()),
            _ => None,
        }
    }

    pub fn children(&self) -> Option<&Vec<Layer>> {
        match self {
            Layer::Group(inner) => Some(&inner.data.layers),
            _ => None,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match self {
            Layer::Group(inner) => Some(&mut inner.data.layers),
            _ => None,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Layer::Pixel(inner) => &mut inner.name,
            Layer::Group(inner) => &mut inner.name,
            Layer::Filter(inner) => &mut inner.name,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        match self {
            Layer::Pixel(inner) => {
                inner.data.width = new_width;
                inner.data.height = new_height;
                inner.data.resize(new_width, new_height);
            },
            _ => {},
        }
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrushLayer<T, D>
where
    T: LayerParameter,
    D: LayerData,
{
    id: String,
    name: String,

    filters: Vec<Filter>,
    parameters: T,
    data: D,
}

#[allow(dead_code)]
impl<T, D> BrushLayer<T, D>
where
    T: LayerParameter,
    D: LayerData,
{
    pub fn new(name: String, parameters: T, data: D) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            filters: Vec::new(),
            parameters,
            data,
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.parameters.set_visible(!self.parameters.is_visible());
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeLayerParameters {
    opacity: u8,
    blend_mode: BlendMode, // TODO: Replace with enum
    visible: bool,
    lock: bool,
    alpha_clip: bool,
    alpha_lock: bool,
}

impl Default for NodeLayerParameters {
    fn default() -> Self {
        Self {
            opacity: 100,
            blend_mode: BlendMode::default(),
            visible: true,
            lock: false,
            alpha_clip: false,
            alpha_lock: false,
        }
    }
}

impl LayerParameter for NodeLayerParameters {
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[allow(dead_code)]
impl NodeLayerParameters {
    fn new(
        opacity: u8,
        blend_mode: BlendMode,
        visible: bool,
        lock: bool,
        alpha_clip: bool,
        alpha_lock: bool,
    ) -> Self {
        Self {
            opacity: opacity.max(100),
            blend_mode,
            visible,
            lock,
            alpha_clip,
            alpha_lock,
        }
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterLayerParameters {
    visible: bool,
    lock: bool,
}

impl LayerParameter for FilterLayerParameters {
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RefLayerParameters {
    opacity: u8,
    visible: bool,
}

impl LayerParameter for RefLayerParameters {
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
