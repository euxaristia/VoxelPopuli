#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 3) in vec4 vertexColor;
uniform vec2 uScreenSize;
out vec4 fragColor;
void main() {
    vec2 pos = (vertexPosition.xy / uScreenSize) * 2.0 - 1.0;
    gl_Position = vec4(pos.x, -pos.y, 0.0, 1.0);
    fragColor = vertexColor;
}
