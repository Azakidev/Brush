/* mod.rs
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

use std::any::TypeId;

use color::{ColorSpace, ColorSpaceLayout, Hsl, OpaqueColor, Rgba8, Srgb};

#[derive(Clone, Copy, Debug)]
pub struct Hsv;

impl ColorSpace for Hsv {
    const TAG: Option<color::ColorSpaceTag> = None;

    const LAYOUT: color::ColorSpaceLayout = ColorSpaceLayout::HueFirst;

    const WHITE_COMPONENTS: [f32; 3] = [0f32, 0f32, 100f32];

    fn to_linear_srgb(src: [f32; 3]) -> [f32; 3] {
        hsv_to_srgb(src)
    }

    fn from_linear_srgb(src: [f32; 3]) -> [f32; 3] {
        srgb_to_hsv(src)
    }

    fn scale_chroma([h, s, l]: [f32; 3], scale: f32) -> [f32; 3] {
        [h, s * scale, l]
    }

    fn convert<TargetCS: ColorSpace>(src: [f32; 3]) -> [f32; 3] {
        if TypeId::of::<Self>() == TypeId::of::<TargetCS>() {
            src
        } else if TypeId::of::<TargetCS>() == TypeId::of::<Srgb>() {
            hsv_to_srgb(src)
        } else if TypeId::of::<TargetCS>() == TypeId::of::<Hsl>() {
            hsv_to_hsl(src)
        } else {
            let lin_rgb = Self::to_linear_srgb(src);
            TargetCS::from_linear_srgb(lin_rgb)
        }
    }

    fn clip(src: [f32; 3]) -> [f32; 3] {
        let [h, s, v] = src;

        [h, s.max(0f32), v.clamp(0f32, 100f32)]
    }
}

fn hsv_to_srgb([h, s, v]: [f32; 3]) -> [f32; 3] {
    let s = (s * 0.01).clamp(0f32, 1f32);
    let v = (v * 0.01).clamp(0f32, 1f32);

    // Standardize hue to [0, 360)
    let h_prime = h.rem_euclid(360f32);
    let c = v * s;
    let x = c * (1f32 - ((h_prime / 60f32).rem_euclid(2f32) - 1f32).abs());
    let m = v - c;

    let (r_temp, g_temp, b_temp) = match h_prime {
        hp if hp < 60f32 => (c, x, 0f32),
        hp if hp < 120f32 => (x, c, 0f32),
        hp if hp < 180f32 => (0f32, c, x),
        hp if hp < 240f32 => (0f32, x, c),
        hp if hp < 300f32 => (x, 0f32, c),
        _ => (c, 0f32, x),
    };

    [r_temp + m, g_temp + m, b_temp + m]
}

// FIXME: Not converting properly from hex codes
fn srgb_to_hsv(src: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = src;

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let v = max;
    let s = if max == 0f32 { 0f32 } else { delta / max };

    const EPSILON: f32 = 1e-6;
    let mut h = if delta > EPSILON {
        0f32
    } else if max == r {
        60f32 * (((g - b) / delta).rem_euclid(6f32))
    } else if max == g {
        60f32 * (((b - r) / delta) + 2f32)
    } else {
        60f32 * (((r - g) / delta) + 4f32)
    };

    // Ensure hue is positive
    if h < 0f32 {
        h += 360f32;
    }

    [h, s * 100f32, v * 100f32]
}

fn hsv_to_hsl(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s_v, v] = hsv;
    let s_v = s_v * 0.01;
    let v = v * 0.01;

    let l = v * (1.0 - s_v / 2.0);

    let s_l = if l == 0.0 || l == 1.0 {
        0.0
    } else {
        (v - l) / l.min(1.0 - l)
    };

    [h, s_l * 100f32, l * 100f32]
}

pub fn hsl_to_hsv([h, s_l, l]: [f32; 3]) -> [f32; 3] {
    let s_l = s_l * 0.01;
    let l = l * 0.01;

    let v = l + s_l * l.min(1.0 - l);

    let s_v = if v == 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };

    [h, s_v* 100f32, v * 100f32]
}

pub fn to_rgba(hsv: &OpaqueColor<Hsv>) -> gtk::gdk::RGBA {
    let srgb: Rgba8 = hsv.to_rgba8();

    gtk::gdk::RGBA::new(
        srgb.r as f32 / 255.0,
        srgb.g as f32 / 255.0,
        srgb.b as f32 / 255.0,
        1.0,
    )
}
