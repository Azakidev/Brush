#version 320 es
precision highp float;
precision highp int;

uniform sampler2D u_SrcTexture;

in vec2 v_tex_coords;
out vec4 color;

vec3 oklab_to_linear_rgb(vec3 c) {
    // 1. Oklab to LMS
    float l_ = c.x + 0.3963377774f * c.y + 0.2158037573f * c.z;
    float m_ = c.x - 0.1055613458f * c.y - 0.0638541728f * c.z;
    float s_ = c.x - 0.0894841775f * c.y - 1.2914855480f * c.z;

    float l = l_ * l_ * l_;
    float m = m_ * m_ * m_;
    float s = s_ * s_ * s_;

    // 2. LMS to Linear sRGB
    return vec3(
        +4.0767416621f * l - 3.3077115913f * m + 0.2309699292f * s,
        -1.2684380046f * l + 2.6097574011f * m - 0.3413193965f * s,
        -0.0041960863f * l - 0.7034186147f * m + 1.7076147010f * s
    );
}

void main() {
    vec4 oklab = texture(u_SrcTexture, v_tex_coords);
    vec3 srgb = oklab_to_linear_rgb(oklab.rgb);
    color = vec4(srgb, oklab.a);
}
