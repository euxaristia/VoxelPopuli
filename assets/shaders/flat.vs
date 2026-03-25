#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 3) in vec4 vertexColor;
uniform mat4 uMVP;
out vec4 fragColor;
out vec2 fragTexCoord;
void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;
    gl_Position = uMVP * vec4(vertexPosition, 1.0);
}
