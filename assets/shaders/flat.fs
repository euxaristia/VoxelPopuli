#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
out vec4 finalColor;
uniform int uBodyType; // 0=flat, 1=sun, 2=moon
void main() {
    if (uBodyType > 0) {
        vec2 uv = fragTexCoord * 8.0;
        ivec2 iuv = ivec2(floor(uv));
        if (iuv.x < 0 || iuv.x > 7 || iuv.y < 0 || iuv.y > 7) discard;

        int shape[8] = int[](60, 126, 255, 255, 255, 255, 126, 60);
        int rowMask = shape[7 - iuv.y];
        if ((rowMask & (1 << (7 - iuv.x))) == 0) discard;

        vec4 color = fragColor;
        if (uBodyType == 2) {
            // Craters on the moon
            // 0, 0, 10, 4, 24, 24, 68, 0 roughly mapping to darker craters on MC moon
            int craters[8] = int[](0, 20, 36, 64, 8, 16, 2, 0);
            int craterMask = craters[7 - iuv.y];
            if ((craterMask & (1 << (7 - iuv.x))) != 0) {
                color.rgb *= 0.65;
            }
        }
        finalColor = color;
    } else {
        finalColor = fragColor;
    }
}
