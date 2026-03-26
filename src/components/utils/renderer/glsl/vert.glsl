#version 320 es
precision highp float;
precision highp int;

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_coords;

uniform mat4 u_mvp;
uniform float u_flip_y; // 1.0 to flip, 0.0 for normal

out vec2 v_tex_coords;

void main() {
    v_tex_coords = vec2(tex_coords.x, abs(u_flip_y - tex_coords.y));
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
}

