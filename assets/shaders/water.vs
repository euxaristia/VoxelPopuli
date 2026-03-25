#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 2) in vec3 vertexNormal;
layout(location = 3) in vec4 vertexColor;

uniform mat4 uMVP;
uniform mat4 uModel;
uniform float uTime;

out vec4 fragColor;
out vec2 fragTexCoord;
out vec3 fragPos;
out vec3 fragNormal;

void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;

    vec3 pos = vertexPosition;
    // Slight vertex animation for waves
    float wave = sin(uTime * 2.5 + pos.x * 1.5 + pos.z * 1.5) * 0.05;
    pos.y += wave;

    fragPos = (uModel * vec4(pos, 1.0)).xyz;
    fragNormal = normalize((uModel * vec4(vertexNormal, 0.0)).xyz);

    gl_Position = uMVP * vec4(pos, 1.0);
}
