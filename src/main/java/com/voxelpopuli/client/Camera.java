package com.voxelpopuli.client;

import org.joml.Matrix4f;
import org.joml.Vector3f;

public class Camera {
    public final Vector3f position = new Vector3f(0.0f, 130.0f, 0.0f);
    public float yaw = (float) Math.PI;
    public float pitch = 0.0f;

    public Vector3f getLookDirection() {
        float cosPitch = (float) Math.cos(pitch);
        float sinPitch = (float) Math.sin(pitch);
        float cosYaw = (float) Math.cos(yaw);
        float sinYaw = (float) Math.sin(yaw);
        return new Vector3f(cosPitch * sinYaw, sinPitch, cosPitch * cosYaw).normalize();
    }

    public Vector3f getForward() {
        float sinYaw = (float) Math.sin(yaw);
        float cosYaw = (float) Math.cos(yaw);
        return new Vector3f(sinYaw, 0.0f, cosYaw).normalize();
    }

    public Vector3f getRight() {
        float sinYaw = (float) Math.sin(yaw);
        float cosYaw = (float) Math.cos(yaw);
        // Cross product of forward (sinYaw, 0, cosYaw) and up (0, 1, 0) is (-cosYaw, 0, sinYaw)
        return new Vector3f(-cosYaw, 0.0f, sinYaw).normalize();
    }

    public Matrix4f getViewMatrix() {
        Vector3f lookDir = getLookDirection();
        Vector3f target = new Vector3f(position).add(lookDir);
        return new Matrix4f().lookAt(position, target, new Vector3f(0.0f, 1.0f, 0.0f));
    }

    public Matrix4f getProjectionMatrix(float fov, float aspect, float near, float far) {
        return new Matrix4f().perspective((float) Math.toRadians(fov), aspect, near, far);
    }
}
