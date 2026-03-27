#version 320 es
precision highp float;
precision highp int;

in vec2 v_tex_coords;
out vec4 color;

uniform sampler2D u_texture;
uniform float u_opacity;

void main() {
    vec4 tex_color = texture(u_texture, v_tex_coords);

    float final_alpha = tex_color.a * u_opacity;

    color = vec4(tex_color.rgb * final_alpha, final_alpha);
}
