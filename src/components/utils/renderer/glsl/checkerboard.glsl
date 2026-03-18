#version 320 es
precision highp float;

in vec2 v_tex_coords;
out vec4 color;

uniform vec2 u_canvas_size;
uniform float u_zoom;

void main() {
    float check_size = 24.0 / u_zoom;

    vec2 pos = floor(v_tex_coords * u_canvas_size / check_size);

    float pattern = mod(pos.x + pos.y, 2.0);

    vec3 color1 = vec3(0.8);
    vec3 color2 = vec3(0.9);

    vec3 final_color = mix(color1, color2, pattern);
    color = vec4(final_color, 1.0);
}
