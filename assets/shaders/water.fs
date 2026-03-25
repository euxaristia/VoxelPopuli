#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
in vec3 fragPos;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec3 sunDir;
uniform vec3 viewPos;
uniform float uTime;
uniform vec4 skyCol;

out vec4 finalColor;

void main() {
    vec4 texelColor = texture(texture0, fragTexCoord);

    vec3 N = normalize(fragNormal);
    vec3 V = normalize(viewPos - fragPos);
    vec3 L = normalize(sunDir);
    vec3 R = reflect(-L, N);

    // Ambient + Diffuse
    float diff = max(dot(N, L), 0.0);
    vec3 diffuse = texelColor.rgb * fragColor.rgb * (diff * 0.7 + 0.3);

    // Specular (Sparkle/Highlights)
    float spec = pow(max(dot(V, R), 0.0), 32.0);
    vec3 specular = vec3(1.0) * spec * 0.6;

    // Fresnel-ish transparency
    float fresnel = pow(1.0 - max(dot(N, V), 0.0), 3.0);
    float alpha = mix(fragColor.a, 1.0, fresnel * 0.5);

    // Simple shore foam / depth effect simulation
    vec3 color = mix(diffuse, vec3(0.1, 0.4, 0.8), 0.2) + specular;

    finalColor = vec4(color, alpha);
}
