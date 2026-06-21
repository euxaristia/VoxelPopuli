package com.voxelpopuli.client.renderer;

import org.lwjgl.opengl.GL20;
import org.joml.Matrix4f;
import org.joml.Vector2f;
import org.joml.Vector3f;
import org.joml.Vector4f;
import org.lwjgl.system.MemoryStack;
import java.nio.FloatBuffer;

public class Shader {
    private int id;

    public Shader(String vertexSource, String fragmentSource) {
        int vertexShader = compileShader(GL20.GL_VERTEX_SHADER, vertexSource);
        int fragmentShader = compileShader(GL20.GL_FRAGMENT_SHADER, fragmentSource);

        id = GL20.glCreateProgram();
        GL20.glAttachShader(id, vertexShader);
        GL20.glAttachShader(id, fragmentShader);
        GL20.glLinkProgram(id);

        int linked = GL20.glGetProgrami(id, GL20.GL_LINK_STATUS);
        if (linked == GL20.GL_FALSE) {
            String log = GL20.glGetProgramInfoLog(id);
            throw new RuntimeException("Shader program linking failed: " + log);
        }

        GL20.glDeleteShader(vertexShader);
        GL20.glDeleteShader(fragmentShader);
    }

    private static int compileShader(int type, String source) {
        int shader = GL20.glCreateShader(type);
        GL20.glShaderSource(shader, source);
        GL20.glCompileShader(shader);

        int compiled = GL20.glGetShaderi(shader, GL20.GL_COMPILE_STATUS);
        if (compiled == GL20.GL_FALSE) {
            String log = GL20.glGetShaderInfoLog(shader);
            throw new RuntimeException("Shader compilation failed (" + (type == GL20.GL_VERTEX_SHADER ? "VERTEX" : "FRAGMENT") + "): " + log);
        }
        return shader;
    }

    public static String loadSource(String path) {
        try {
            java.io.File file = new java.io.File(path);
            if (!file.exists()) {
                throw new java.io.FileNotFoundException("Shader source not found: " + path);
            }
            return java.nio.file.Files.readString(file.toPath());
        } catch (java.io.IOException e) {
            throw new RuntimeException("Failed to read shader file: " + path, e);
        }
    }

    public void bind() {
        GL20.glUseProgram(id);
    }

    public static void unbind() {
        GL20.glUseProgram(0);
    }

    public int getUniformLocation(String name) {
        return GL20.glGetUniformLocation(id, name);
    }

    public void setInt(int location, int value) {
        GL20.glUniform1i(location, value);
    }

    public void setFloat(int location, float value) {
        GL20.glUniform1f(location, value);
    }

    public void setVec2(int location, Vector2f value) {
        GL20.glUniform2f(location, value.x, value.y);
    }

    public void setVec3(int location, Vector3f value) {
        GL20.glUniform3f(location, value.x, value.y, value.z);
    }

    public void setVec4(int location, Vector4f value) {
        GL20.glUniform4f(location, value.x, value.y, value.z, value.w);
    }

    public void setMat4(int location, Matrix4f value) {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            FloatBuffer fb = stack.mallocFloat(16);
            value.get(fb);
            GL20.glUniformMatrix4fv(location, false, fb);
        }
    }

    public void cleanup() {
        GL20.glDeleteProgram(id);
    }

    public int getId() { return id; }
}
