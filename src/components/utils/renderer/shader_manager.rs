/* shader_manager.rs
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
use struct_iterable::Iterable;

use crate::components::utils::renderer::shader::ShaderProgram;

const VERT: &str = include_str!("./glsl/vert.glsl");
const PIXEL_FRAG: &str = include_str!("./glsl/pixel.glsl");
const BG_FRAG: &str = include_str!("./glsl/checkerboard.glsl");
const OKLAB_TO_SRG: &str = include_str!("./glsl/oklab2srgb.glsl");

#[derive(Debug, Iterable)]
pub struct ShaderManager {
    pub background: ShaderProgram,
    pub layer: ShaderProgram,
    pub oklab2srgb: ShaderProgram
}

impl ShaderManager {
    pub unsafe fn new(gl: &glow::Context) -> Self {
        unsafe {
            Self {
                background: ShaderProgram::new(gl, VERT, BG_FRAG),
                layer: ShaderProgram::new(gl, VERT, PIXEL_FRAG),
                oklab2srgb: ShaderProgram::new(gl, VERT, OKLAB_TO_SRG),
            }
        }
    }

    pub unsafe fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.use_program(None);

            for (_name, shader) in self.iter() {
                if let Some(program) = shader.downcast_ref::<ShaderProgram>() {
                    program.destroy(gl);
                }
            }
        }
    }
}
