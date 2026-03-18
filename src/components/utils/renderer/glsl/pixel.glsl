#version 320 es
precision highp float;
precision highp int;

in vec2 v_tex_coords;
out vec4 color;

uniform sampler2D u_texture;
uniform float u_opacity;

void main() {
    vec4 texture = texture(u_texture, v_tex_coords);

    texture.a * u_opacity; // Apply alpha

    color = texture;
}

