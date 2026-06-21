package com.voxelpopuli.client.renderer;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL30;

public class Mesh {
    private int vao;
    private int[] vbo = new int[4];
    private int vertexCount;

    public Mesh(float[] vertices, float[] texcoords, float[] normals, byte[] colors) {
        vertexCount = vertices.length / 3;

        vao = GL30.glGenVertexArrays();
        GL15.glGenBuffers(vbo);

        GL30.glBindVertexArray(vao);

        // Vertices
        GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, vbo[0]);
        GL15.glBufferData(GL15.GL_ARRAY_BUFFER, vertices, GL15.GL_STATIC_DRAW);
        GL20.glVertexAttribPointer(0, 3, GL11.GL_FLOAT, false, 0, 0);
        GL20.glEnableVertexAttribArray(0);

        // Texcoords
        if (texcoords != null && texcoords.length > 0) {
            GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, vbo[1]);
            GL15.glBufferData(GL15.GL_ARRAY_BUFFER, texcoords, GL15.GL_STATIC_DRAW);
            GL20.glVertexAttribPointer(1, 2, GL11.GL_FLOAT, false, 0, 0);
            GL20.glEnableVertexAttribArray(1);
        }

        // Normals
        if (normals != null && normals.length > 0) {
            GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, vbo[2]);
            GL15.glBufferData(GL15.GL_ARRAY_BUFFER, normals, GL15.GL_STATIC_DRAW);
            GL20.glVertexAttribPointer(2, 3, GL11.GL_FLOAT, false, 0, 0);
            GL20.glEnableVertexAttribArray(2);
        }

        // Colors
        if (colors != null && colors.length > 0) {
            GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, vbo[3]);
            java.nio.ByteBuffer colorBuffer = org.lwjgl.BufferUtils.createByteBuffer(colors.length);
            colorBuffer.put(colors).flip();
            GL15.glBufferData(GL15.GL_ARRAY_BUFFER, colorBuffer, GL15.GL_STATIC_DRAW);
            GL20.glVertexAttribPointer(3, 4, GL11.GL_UNSIGNED_BYTE, true, 0, 0);
            GL20.glEnableVertexAttribArray(3);
        }

        GL30.glBindVertexArray(0);
    }

    public void draw() {
        if (vertexCount == 0) {
            return;
        }
        GL30.glBindVertexArray(vao);
        GL11.glDrawArrays(GL11.GL_TRIANGLES, 0, vertexCount);
        GL30.glBindVertexArray(0);
    }

    public void cleanup() {
        GL15.glDeleteBuffers(vbo);
        GL30.glDeleteVertexArrays(vao);
    }

    public int getVertexCount() {
        return vertexCount;
    }
}
