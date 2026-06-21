package com.voxelpopuli.client.world;

import org.joml.Matrix4f;
import org.joml.Vector3f;

public class Frustum {
    public final float[][] planes = new float[6][4];

    public static Frustum fromMatrix(Matrix4f m) {
        Frustum f = new Frustum();

        // Right
        f.planes[0][0] = m.m03() - m.m00();
        f.planes[0][1] = m.m13() - m.m10();
        f.planes[0][2] = m.m23() - m.m20();
        f.planes[0][3] = m.m33() - m.m30();

        // Left
        f.planes[1][0] = m.m03() + m.m00();
        f.planes[1][1] = m.m13() + m.m10();
        f.planes[1][2] = m.m23() + m.m20();
        f.planes[1][3] = m.m33() + m.m30();

        // Bottom
        f.planes[2][0] = m.m03() + m.m01();
        f.planes[2][1] = m.m13() + m.m11();
        f.planes[2][2] = m.m23() + m.m21();
        f.planes[2][3] = m.m33() + m.m31();

        // Top
        f.planes[3][0] = m.m03() - m.m01();
        f.planes[3][1] = m.m13() - m.m11();
        f.planes[3][2] = m.m23() - m.m21();
        f.planes[3][3] = m.m33() - m.m31();

        // Far
        f.planes[4][0] = m.m03() - m.m02();
        f.planes[4][1] = m.m13() - m.m12();
        f.planes[4][2] = m.m23() - m.m22();
        f.planes[4][3] = m.m33() - m.m32();

        // Near
        f.planes[5][0] = m.m03() + m.m02();
        f.planes[5][1] = m.m13() + m.m12();
        f.planes[5][2] = m.m23() + m.m22();
        f.planes[5][3] = m.m33() + m.m32();

        // Normalize
        for (int i = 0; i < 6; i++) {
            float length = (float) Math.sqrt(f.planes[i][0] * f.planes[i][0] +
                                            f.planes[i][1] * f.planes[i][1] +
                                            f.planes[i][2] * f.planes[i][2]);
            if (length > 0.0f) {
                f.planes[i][0] /= length;
                f.planes[i][1] /= length;
                f.planes[i][2] /= length;
                f.planes[i][3] /= length;
            }
        }

        return f;
    }

    public boolean isBoxVisible(Vector3f min, Vector3f max) {
        for (int i = 0; i < 6; i++) {
            float px = (planes[i][0] > 0.0f) ? max.x : min.x;
            float py = (planes[i][1] > 0.0f) ? max.y : min.y;
            float pz = (planes[i][2] > 0.0f) ? max.z : min.z;
            if (planes[i][0] * px + planes[i][1] * py + planes[i][2] * pz + planes[i][3] <= 0.0f) {
                return false;
            }
        }
        return true;
    }
}
