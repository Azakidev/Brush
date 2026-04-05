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

use color::{AlphaColor, Oklab};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::{
    blend_modes::BlendMode,
    layer_types::{
        fill::{FillLayerData, FillLayerParameters},
        filter::FilterData,
        group::GroupData,
        pixel::PixelData,
    },
};

pub trait LayerParameter {
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_lock(&self) -> bool;
    fn set_lock(&mut self, lock: bool);
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
        let mut pixels: Vec<f32> = Vec::new();
        pixels.resize((width * height * 4) as usize, 0f32);

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
        if let Layer::Group(inner) = self {
            inner.data.layers.insert(index, layer);
            self.resize_group();
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

    pub fn set_visible(&mut self, visible: bool) {
        match self {
            Layer::Pixel(inner) => inner.parameters.set_visible(visible),
            Layer::Group(inner) => inner.parameters.set_visible(visible),
            Layer::Fill(inner) => inner.parameters.set_visible(visible),
            Layer::Filter(inner) => inner.parameters.set_visible(visible),
        }
    }

    pub fn lock(&self) -> bool {
        match self {
            Layer::Pixel(inner) => inner.parameters.is_lock(),
            Layer::Group(inner) => inner.parameters.is_lock(),
            Layer::Fill(inner) => inner.parameters.is_lock(),
            Layer::Filter(inner) => inner.parameters.is_lock(),
        }
    }

    pub fn set_lock(&mut self, lock: bool) {
        match self {
            Layer::Pixel(inner) => inner.parameters.set_lock(lock),
            Layer::Group(inner) => inner.parameters.set_lock(lock),
            Layer::Fill(inner) => inner.parameters.set_lock(lock),
            Layer::Filter(inner) => inner.parameters.set_lock(lock),
        }
    }

    pub fn alpha_clip(&self) -> bool {
        match self {
            Layer::Pixel(inner) => inner.parameters.alpha_clip(),
            Layer::Group(inner) => inner.parameters.alpha_clip(),
            Layer::Fill(inner) => inner.parameters.alpha_clip(),
            Layer::Filter(_) => false,
        }
    }

    pub fn set_alpha_clip(&mut self, lock: bool) {
        match self {
            Layer::Pixel(inner) => inner.parameters.set_alpha_clip(lock),
            Layer::Group(inner) => inner.parameters.set_alpha_clip(lock),
            Layer::Fill(inner) => inner.parameters.set_alpha_clip(lock),
            Layer::Filter(_) => {}
        }
    }

    pub fn alpha_lock(&self) -> bool {
        match self {
            Layer::Pixel(inner) => inner.parameters.alpha_lock(),
            Layer::Group(inner) => inner.parameters.alpha_lock(),
            Layer::Fill(inner) => inner.parameters.alpha_lock(),
            Layer::Filter(_) => false,
        }
    }

    pub fn set_alpha_lock(&mut self, lock: bool) {
        match self {
            Layer::Pixel(inner) => inner.parameters.set_alpha_lock(lock),
            Layer::Group(inner) => inner.parameters.set_alpha_lock(lock),
            Layer::Fill(inner) => inner.parameters.set_alpha_lock(lock),
            Layer::Filter(_) => {}
        }
    }

    pub fn passthrough(&self) -> bool {
        if let Layer::Group(inner) = self {
            return inner.parameters.passthrough();
        }
        false
    }

    pub fn set_passthrough(&mut self, passthrough: bool) {
        if let Layer::Group(inner) = self {
            inner.parameters.set_passthrough(passthrough);
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

    pub fn set_opacity(&mut self, opacity: f32) {
        match self {
            Layer::Pixel(inner) => inner.parameters.opacity = opacity,
            Layer::Group(inner) => inner.parameters.opacity = opacity,
            Layer::Fill(inner) => inner.parameters.opacity = opacity,
            _ => {}
        }
    }

    pub fn blend_mode(&self) -> BlendMode {
        match self {
            Layer::Pixel(inner) => inner.parameters.blend_mode,
            Layer::Group(inner) => inner.parameters.blend_mode,
            Layer::Fill(inner) => inner.parameters.blend_mode,
            _ => BlendMode::default(),
        }
    }

    pub fn set_blend_mode(&mut self, blend_mode: BlendMode) {
        match self {
            Layer::Pixel(inner) => inner.parameters.blend_mode = blend_mode,
            Layer::Group(inner) => inner.parameters.blend_mode = blend_mode,
            Layer::Fill(inner) => inner.parameters.blend_mode = blend_mode,
            _ => {}
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            Layer::Pixel(inner) => inner.data.width,
            Layer::Group(inner) => inner.data.width,
            _ => 0,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Layer::Pixel(inner) => inner.data.height,
            Layer::Group(inner) => inner.data.height,
            _ => 0,
        }
    }

    pub fn x(&self) -> i32 {
        match self {
            Layer::Pixel(inner) => inner.data.x,
            Layer::Group(inner) => inner.data.x,
            _ => 0,
        }
    }

    pub fn y(&self) -> i32 {
        match self {
            Layer::Pixel(inner) => inner.data.y,
            Layer::Group(inner) => inner.data.y,
            _ => 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            Layer::Pixel(inner) => inner.is_dirty,
            Layer::Group(inner) => inner.is_dirty,
            Layer::Fill(inner) => inner.is_dirty,
            Layer::Filter(inner) => inner.is_dirty,
        }
    }

    pub fn set_dirty(&mut self, flag: bool) {
        match self {
            Layer::Pixel(inner) => inner.is_dirty = flag,
            Layer::Group(inner) => inner.is_dirty = flag,
            Layer::Fill(inner) => inner.is_dirty = flag,
            Layer::Filter(inner) => inner.is_dirty = flag,
        }
    }

    pub fn pixel_data(&self) -> Option<&Vec<f32>> {
        if let Layer::Pixel(inner) = self {
            return Some(&inner.data.pixels);
        }
        None
    }

    pub fn pixel_data_mut(&mut self) -> Option<&mut Vec<f32>> {
        if let Layer::Pixel(inner) = self {
            return Some(&mut inner.data.pixels);
        }
        None
    }

    pub fn children(&self) -> Option<&Vec<Layer>> {
        if let Layer::Group(inner) = self {
            return Some(&inner.data.layers);
        }
        None
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
                    _ => unreachable!(), // Filters can't have filters
                }
            }
            _ => {
                if let Layer::Group(inner) = self
                    && let Some(idx) = inner.data.layers.iter().position(|l| l.id() == child.id())
                {
                    inner.data.layers.remove(idx);
                    self.resize_group();
                }
            }
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
        if let Layer::Pixel(inner) = self {
            inner.data.width = new_width;
            inner.data.height = new_height;
            inner.data.resize(new_width, new_height);
        }
    }

    pub fn resize_group(&mut self) {
        if let Layer::Group(inner) = self {
            inner.data.calculate_group_bounds();
        }
    }

    pub fn draw_brush_dab(&mut self, x: i32, y: i32, radius: i32, color: AlphaColor<Oklab>) {
        // Convert global canvas coordinates to layer-local coordinates
        let local_x = x - self.x();
        let local_y = y - self.y();
        let (width, height) = (self.width() as i32, self.height() as i32);

        for dy in -radius..radius {
            for dx in -radius..radius {
                if dx * dx + dy * dy <= radius * radius {
                    let px = local_x + dx;
                    let py = local_y + dy;

                    // Bounds check: only draw if inside this specific layer's dimensions
                    if px >= 0 && px < width && py >= 0 && py < height {
                        let idx = ((py * width + px) * 4) as usize;
                        if let Some(data) = self.pixel_data_mut() {
                            data[idx..idx + 4].copy_from_slice(&color.components);
                        }
                    }
                }
            }
        }
        self.set_dirty(true);
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

    // Flags
    #[serde(skip_serializing)]
    is_dirty: bool,
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
            is_dirty: true,
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.parameters.set_visible(!self.parameters.is_visible());
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NodeLayerParameters {
    opacity: f32,
    visible: bool,
    lock: bool,
    alpha_clip: bool,
    alpha_lock: bool,
    passthrough: bool,
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
            passthrough: false,
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
    fn is_lock(&self) -> bool {
        self.lock
    }
    fn set_lock(&mut self, lock: bool) {
        self.lock = lock;
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
        passthrough: bool,
    ) -> Self {
        Self {
            opacity: opacity.clamp(0f32, 1f32),
            blend_mode,
            visible,
            lock,
            alpha_clip,
            alpha_lock,
            passthrough,
        }
    }

    fn alpha_clip(&self) -> bool {
        self.alpha_clip
    }
    fn set_alpha_clip(&mut self, alpha_clip: bool) {
        self.alpha_clip = alpha_clip;
    }
    fn alpha_lock(&self) -> bool {
        self.alpha_lock
    }
    fn set_alpha_lock(&mut self, alpha_lock: bool) {
        self.alpha_lock = alpha_lock;
    }
    fn passthrough(&self) -> bool {
        self.passthrough
    }
    fn set_passthrough(&mut self, passthrough: bool) {
        self.passthrough = passthrough;
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
    fn is_lock(&self) -> bool {
        self.lock
    }
    fn set_lock(&mut self, lock: bool) {
        self.lock = lock;
    }
}
