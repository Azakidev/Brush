/* shader.rs
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
use std::collections::HashMap;

#[derive(Debug)]
pub struct ShaderProgram {
    program: glow::Program,
    uniforms: HashMap<String, glow::UniformLocation>,
}

impl ShaderProgram {
    pub unsafe fn new(gl: &glow::Context, v_src: &str, f_src: &str) -> Self {
        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let vs = compile_shader(gl, glow::VERTEX_SHADER, v_src);
            let fs = compile_shader(gl, glow::FRAGMENT_SHADER, f_src);

            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);

            // Check link status as we discussed before
            if !gl.get_program_link_status(program) {
                panic!("Link Error: {}", gl.get_program_info_log(program));
            }

            Self {
                program,
                uniforms: HashMap::new(),
            }
        }
    }

    pub unsafe fn get_uniform(
        &mut self,
        gl: &glow::Context,
        name: &str,
    ) -> Option<glow::UniformLocation> {
        if let Some(loc) = self.uniforms.get(name) {
            return Some(*loc);
        }

        unsafe {
            let loc = gl.get_uniform_location(self.program, name);

            if let Some(l) = loc {
                self.uniforms.insert(name.to_string(), l);
            }

            loc
        }
    }

    pub unsafe fn bind(&self, gl: &glow::Context) {
        unsafe {
            gl.use_program(Some(self.program));
        }
    }

    pub unsafe fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
        }
    }
}

unsafe fn compile_shader(gl: &glow::Context, shader_type: u32, source: &str) -> glow::Shader {
    unsafe {
        let shader = gl.create_shader(shader_type).expect("Cannot create shader");
        gl.shader_source(shader, source);
        gl.compile_shader(shader);

        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            panic!("Shader Compile Error ({:?}): {}", shader_type, log);
        }
        shader
    }
}
