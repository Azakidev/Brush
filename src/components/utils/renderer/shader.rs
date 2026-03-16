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

pub const VERT_SRC: &str = r#"
#version 320 es
precision highp float;
precision highp int;

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_coords;

uniform mat4 u_mvp;
out vec2 v_tex_coords;

void main() {
    v_tex_coords = tex_coords;
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
}
"#;

pub const FRAG_SRC: &str = r#"
#version 320 es
precision highp float;
precision highp int;

in vec2 v_tex_coords;
out vec4 color;

uniform sampler2D u_texture;

void main() {
    color = texture(u_texture, v_tex_coords);
}
"#;

pub unsafe fn compile_shader(gl: &glow::Context, shader_type: u32, source: &str) -> glow::Shader {
    use glow::HasContext;
    let shader = gl.create_shader(shader_type).expect("Cannot create shader");
    gl.shader_source(shader, source);
    gl.compile_shader(shader);

    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        panic!("Shader Compile Error ({:?}): {}", shader_type, log);
    }
    shader
}
