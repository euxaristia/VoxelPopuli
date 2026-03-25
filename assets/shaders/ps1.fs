#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
in vec3 fragPos;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec4 colDiffuse;
uniform vec3 sunDir;
uniform vec3 viewPos;
uniform float time;
uniform vec4 skyCol;

out vec4 finalColor;

void main() {
    vec4 texelColor = texture(texture0, fragTexCoord);
    if (texelColor.a < 0.1) discard;

    // Global ambient based on sun position (time of day)
    float sunY = max(0.0, sunDir.y);
    float timeLight = 0.15 + (sunY * 0.85); // 0.15 at night, 1.0 at noon

    vec3 baseColor = texelColor.rgb * fragColor.rgb * colDiffuse.rgb;
    vec3 ambient = texelColor.rgb * 0.12; // minimum ambient so caves aren't pure black
    vec3 color = max(baseColor * timeLight, ambient);

    color *= mix(vec3(1.0, 0.9, 0.8), vec3(1.0, 1.0, 1.05), sunY); // slight tinting

    // Final color output without artificial quantization
    vec4 c = vec4(color, texelColor.a * fragColor.a * colDiffuse.a);
    finalColor = c;
}
