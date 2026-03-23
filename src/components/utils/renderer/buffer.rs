/* buffer.rs
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

use glow::HasContext;

pub struct LayerBuffer {
    pub texture: glow::Texture,
    pub framebuffer: glow::Framebuffer,
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl LayerBuffer {
    pub unsafe fn new(gl: &glow::Context, width: i32, height: i32) -> Self {
        let texture = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::RGBA as i32, width, height,
            0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None),
        );

        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0
        );

        Self { texture, framebuffer, width, height, offset_x: 0, offset_y: 0 }
    }
}
