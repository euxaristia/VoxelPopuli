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
    // Simple wave animation for water (alpha 240/255 ~= 0.94)
    if (vertexColor.a > 0.940 && vertexColor.a < 0.942) {
        pos.y += sin(uTime * 1.5 + vertexPosition.x * 0.8 + vertexPosition.z * 0.8) * 0.08 - 0.05;
    }

    fragPos = (uModel * vec4(pos, 1.0)).xyz;
    fragNormal = normalize((uModel * vec4(vertexNormal, 0.0)).xyz);

    gl_Position = uMVP * (uModel * vec4(pos, 1.0));
}
