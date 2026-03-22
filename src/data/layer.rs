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
    layer_types::{
        fill::{FillLayerData, FillLayerParameters}, filter::{FilterData}, group::GroupData, pixel::PixelData
    },
};

pub trait LayerParameter {
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
}
pub trait LayerData {}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Layer {
    Group(BrushLayer<NodeLayerParameters, GroupData>),
    Pixel(BrushLayer<NodeLayerParameters, PixelData>),
    Fill(BrushLayer<FillLayerParameters, FillLayerData>),
    Filter(BrushLayer<FilterLayerParameters, FilterData>),
}

impl Layer {
    pub fn new_pixel(name: String, width: u32, height: u32) -> Self {
        let mut pixels: Vec<u8> = Vec::new();
        pixels.resize((width * height * 4) as usize,  0u8);

        let params = NodeLayerParameters::default();
        let data = PixelData::new(pixels, "OkLab".to_owned(), width, height);

        Layer::Pixel(BrushLayer::new(name, params, data))
    }

    pub fn new_group(name: String) -> Self {
        let params = NodeLayerParameters::default();
        let data = GroupData::new();

        Layer::Group(BrushLayer::new(name, params, data))
    }

    pub fn append(&mut self, index: usize, layer: Layer) {
        match self {
            Layer::Group(inner) => inner.data.layers.insert(index, layer),
            _ => {}
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            Layer::Pixel(inner) => Uuid::parse_str(&inner.id).unwrap(),
            Layer::Group(inner) => Uuid::parse_str(&inner.id).unwrap(),
            Layer::Fill(inner) => Uuid::parse_str(&inner.id).unwrap(),
            Layer::Filter(inner) => Uuid::parse_str(&inner.id).unwrap(),
        }
    }
    
    pub fn visible(&self) -> bool {
        match self {
            Layer::Pixel(inner) => inner.parameters.is_visible(),
            Layer::Group(inner) => inner.parameters.is_visible(),
            Layer::Fill(inner) => inner.parameters.is_visible(),
            Layer::Filter(inner) => inner.parameters.is_visible(),
        }
    }
    
    pub fn opacity(&self) -> f32 {
        match self {
            Layer::Pixel(inner) => inner.parameters.opacity,
            Layer::Group(inner) => inner.parameters.opacity,
            Layer::Fill(inner) => inner.parameters.opacity,
            _ => 1f32,
        }
    }

    // TODO: Measure group layers too maybe
    pub fn width(&self) -> u32 {
        match self {
            Layer::Pixel(inner) => inner.data.width,
            _ => 0,
        }
    }
    
    // TODO: Measure group layers too maybe
    pub fn height(&self) -> u32 {
        match self {
            Layer::Pixel(inner) => inner.data.height,
            _ => 0,
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

    pub fn remove_child(&mut self, child: &Layer) {
        match child {
            Layer::Filter(_) => {
                match self {
                    Layer::Pixel(inner) => {
                        if let Some(idx) = inner.filters.iter().position(|l| l.id() == child.id()) {
                            inner.filters.remove(idx);
                        }
                    }
                    Layer::Group(inner) => {
                        if let Some(idx) = inner.filters.iter().position(|l| l.id() == child.id()) {
                            inner.filters.remove(idx);
                        }
                    }
                    Layer::Fill(inner) => {
                        if let Some(idx) = inner.filters.iter().position(|l| l.id() == child.id()) {
                            inner.filters.remove(idx);
                        }
                    }
                    _ => unreachable!() // Filters can't have filters
                }
            }
            _ => {
                match self {
                    Layer::Group(inner) => {
                        if let Some(idx) = inner.data.layers.iter().position(|l| l.id() == child.id()) {
                            inner.data.layers.remove(idx);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn blend_mode(&self) -> &BlendMode {
        match self {
            Layer::Pixel(inner) => &inner.parameters.blend_mode,
            Layer::Group(inner) => &inner.parameters.blend_mode,
            Layer::Fill(inner) => &inner.parameters.blend_mode,
            _ => {&BlendMode::Normal}
        }
    }
    
    pub fn name(&self) -> &str {
        match self {
            Layer::Pixel(inner) => &inner.name,
            Layer::Group(inner) => &inner.name,
            Layer::Fill(inner) => &inner.name,
            Layer::Filter(inner) => &inner.name,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Layer::Pixel(inner) => &mut inner.name,
            Layer::Group(inner) => &mut inner.name,
            Layer::Fill(inner) => &mut inner.name,
            Layer::Filter(inner) => &mut inner.name,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        match self {
            Layer::Pixel(inner) => {
                inner.data.width = new_width;
                inner.data.height = new_height;
                inner.data.resize(new_width, new_height);
            }
            _ => {}
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

    filters: Vec<Layer>,
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
    opacity: f32,
    visible: bool,
    lock: bool,
    alpha_clip: bool,
    alpha_lock: bool,
    pub blend_mode: BlendMode,
}

impl Default for NodeLayerParameters {
    fn default() -> Self {
        Self {
            opacity: 1f32,
            visible: true,
            blend_mode: BlendMode::default(),
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
        opacity: f32,
        blend_mode: BlendMode,
        visible: bool,
        lock: bool,
        alpha_clip: bool,
        alpha_lock: bool,
    ) -> Self {
        Self {
            opacity: opacity.max(1f32),
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
