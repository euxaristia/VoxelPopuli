package com.voxelpopuli.client.noise;

public class Noise {
    private static final int[] P = {
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
        142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
        203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
        74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
        220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
        132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
        186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
        59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
        70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
        178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
        241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
        176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
        128, 195, 78, 66, 215, 61, 156, 180, 151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194,
        233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234,
        75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174,
        20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83,
        111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25,
        63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188,
        159, 86, 164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147,
        118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
        213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253,
        19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193,
        238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31,
        181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
        222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180
    };

    private static float fade(float t) {
        return t * t * t * (t * (t * 6.0f - 15.0f) + 10.0f);
    }

    private static float lerp(float t, float a, float b) {
        return a + t * (b - a);
    }

    private static float grad(int hash, float x, float y, float z) {
        int h = hash & 15;
        float u = h < 8 ? x : y;
        float v = h < 4 ? y : (h == 12 || h == 14 ? x : z);
        return ((h & 1) == 0 ? u : -u) + ((h & 2) == 0 ? v : -v);
    }

    public static float noise2d(float x, float y) {
        int xi = (int) Math.floor(x);
        int yi = (int) Math.floor(y);
        
        // rem_euclid(256)
        int xf = xi % 256;
        if (xf < 0) xf += 256;
        int yf = yi % 256;
        if (yf < 0) yf += 256;

        float xd = x - (float) Math.floor(x);
        float yd = y - (float) Math.floor(y);

        float u = fade(xd);
        float v = fade(yd);

        int a = P[xf] + yf;
        int aa = P[a];
        int ab = P[a + 1];
        int b = P[(xf + 1) & 255] + yf;
        int ba = P[b];
        int bb = P[b + 1];

        return lerp(
            v,
            lerp(u, grad(P[aa], xd, yd, 0.0f), grad(P[ba], xd - 1.0f, yd, 0.0f)),
            lerp(u, grad(P[ab], xd, yd - 1.0f, 0.0f), grad(P[bb], xd - 1.0f, yd - 1.0f, 0.0f))
        );
    }

    public static float noise3d(float x, float y, float z) {
        int xi = (int) Math.floor(x);
        int yi = (int) Math.floor(y);
        int zi = (int) Math.floor(z);

        int xf = xi % 256;
        if (xf < 0) xf += 256;
        int yf = yi % 256;
        if (yf < 0) yf += 256;
        int zf = zi % 256;
        if (zf < 0) zf += 256;

        float xd = x - (float) Math.floor(x);
        float yd = y - (float) Math.floor(y);
        float zd = z - (float) Math.floor(z);

        float u = fade(xd);
        float v = fade(yd);
        float w = fade(zd);

        int a = P[xf] + yf;
        int aa = P[a] + zf;
        int ab = P[a + 1] + zf;
        int b = P[(xf + 1) & 255] + yf;
        int ba = P[b] + zf;
        int bb = P[b + 1] + zf;

        return lerp(
            w,
            lerp(
                v,
                lerp(u, grad(P[aa], xd, yd, zd), grad(P[ba], xd - 1.0f, yd, zd)),
                lerp(u, grad(P[ab], xd, yd - 1.0f, zd), grad(P[bb], xd - 1.0f, yd - 1.0f, zd))
            ),
            lerp(
                v,
                lerp(u, grad(P[aa + 1], xd, yd, zd - 1.0f), grad(P[ba + 1], xd - 1.0f, yd, zd - 1.0f)),
                lerp(u, grad(P[ab + 1], xd, yd - 1.0f, zd - 1.0f), grad(P[bb + 1], xd - 1.0f, yd - 1.0f, zd - 1.0f))
            )
        );
    }

    public static float perlin2d(float x, float y, float frequency, int octaves) {
        float total = 0.0f;
        float amplitude = 1.0f;
        float maxAmplitude = 0.0f;

        for (int i = 0; i < octaves; i++) {
            total += noise2d(x * frequency, y * frequency) * amplitude;
            maxAmplitude += amplitude;
            amplitude *= 0.5f;
            frequency *= 2.0f;
        }

        return total / maxAmplitude;
    }

    public static float perlin3d(float x, float y, float z, float frequency, int octaves) {
        float total = 0.0f;
        float amplitude = 1.0f;
        float maxAmplitude = 0.0f;

        for (int i = 0; i < octaves; i++) {
            total += noise3d(x * frequency, y * frequency, z * frequency) * amplitude;
            maxAmplitude += amplitude;
            amplitude *= 0.5f;
            frequency *= 2.0f;
        }

        return total / maxAmplitude;
    }
}
