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
    blend_modes::BrushBlendMode,
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

    pub fn blend_mode(&self) -> BrushBlendMode {
        match self {
            Layer::Pixel(inner) => inner.parameters.blend_mode,
            Layer::Group(inner) => inner.parameters.blend_mode,
            Layer::Fill(inner) => inner.parameters.blend_mode,
            _ => BrushBlendMode::default(),
        }
    }

    pub fn set_blend_mode(&mut self, blend_mode: BrushBlendMode) {
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

    pub fn draw_brush_dab(
        &mut self,
        (x, y): (i32, i32),
        radius: i32,
        color: AlphaColor<Oklab>,
        erase_mode: bool,
        mask: &mut [u8],
    ) {
        // Convert global canvas coordinates to layer-local coordinates
        let local_x = x - self.x();
        let local_y = y - self.y();
        let (width, height) = (self.width() as i32, self.height() as i32);

        let alpha_lock = self.alpha_lock();

        for dy in -radius..radius {
            for dx in -radius..radius {
                if dx * dx + dy * dy <= radius * radius {
                    let px = local_x + dx;
                    let py = local_y + dy;

                    // Bounds check: only draw if inside this specific layer's dimensions
                    if px >= 0 && px < width && py >= 0 && py < height {
                        let idx = ((py * width + px) * 4) as usize;

                        if should_edit_pixel(mask, px, py, width)
                            && let Some(data) = self.pixel_data_mut()
                        {
                            let mut orig_color = [0f32; 4];
                            orig_color.copy_from_slice(&data[idx..idx + 4]);
                            let orig_color: AlphaColor<Oklab> = AlphaColor::new(orig_color);
                            if erase_mode {
                                let alpha =
                                    (orig_color.components[3] - color.components[3]).max(0f32);
                                let final_color = orig_color.with_alpha(alpha);

                                data[idx..idx + 4].copy_from_slice(&final_color.components);
                            } else {
                                // TODO: Sample strength of brush from brush engine
                                paint_pixel(
                                    &mut data[idx..idx + 4],
                                    color.components,
                                    1f32,
                                    alpha_lock,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.set_dirty(true);
    }
}

fn paint_pixel(canvas_rgba: &mut [f32], brush_rgba: [f32; 4], strength: f32, alpha_lock: bool) {
    let src_a = brush_rgba[3] * strength;
    if src_a <= 0.0 {
        return;
    }

    let src_r = brush_rgba[0] * src_a;
    let src_g = brush_rgba[1] * src_a;
    let src_b = brush_rgba[2] * src_a;

    let dst_r = canvas_rgba[0];
    let dst_g = canvas_rgba[1];
    let dst_b = canvas_rgba[2];
    let dst_a = canvas_rgba[3];

    let dst_pre_r = dst_r * dst_a;
    let dst_pre_g = dst_g * dst_a;
    let dst_pre_b = dst_b * dst_a;

    let out_a = if alpha_lock {
        dst_a
    } else {
        src_a + dst_a * (1.0 - src_a)
    };

    let out_pre_r = src_r + dst_pre_r * (1.0 - src_a);
    let out_pre_g = src_g + dst_pre_g * (1.0 - src_a);
    let out_pre_b = src_b + dst_pre_b * (1.0 - src_a);

    if out_a > f32::EPSILON {
        canvas_rgba[0] = out_pre_r / out_a;
        canvas_rgba[1] = out_pre_g / out_a;
        canvas_rgba[2] = out_pre_b / out_a;
        canvas_rgba[3] = out_a;
    } else {
        canvas_rgba.copy_from_slice(&[0.0, 0.0, 0.0, 0.0]);
    }
}

fn should_edit_pixel(mask: &mut [u8], x: i32, y: i32, width: i32) -> bool {
    let idx = y * width + x;
    if mask[idx as usize] == 0 {
        mask[idx as usize] = 1; // Mark as touched
        true
    } else {
        false
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrushLayer<T, D>
where
    T: LayerParameter,
    D: LayerData,
{
    pub id: String,
    pub name: String,

    pub filters: Vec<Layer>,
    pub parameters: T,
    pub data: D,

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
    pub blend_mode: BrushBlendMode,
}

impl Default for NodeLayerParameters {
    fn default() -> Self {
        Self {
            opacity: 1f32,
            visible: true,
            blend_mode: BrushBlendMode::default(),
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
        blend_mode: BrushBlendMode,
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
