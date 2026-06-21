package com.voxelpopuli.client.renderer;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL30;
import java.nio.ByteBuffer;

public class Texture2D {
    private int id;
    private int width;
    private int height;

    public Texture2D(ByteBuffer data, int width, int height) {
        this.width = width;
        this.height = height;
        this.id = GL11.glGenTextures();
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, id);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_S, GL11.GL_REPEAT);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_T, GL11.GL_REPEAT);

        GL11.glTexImage2D(
            GL11.GL_TEXTURE_2D,
            0,
            GL11.GL_RGBA,
            width,
            height,
            0,
            GL11.GL_RGBA,
            GL11.GL_UNSIGNED_BYTE,
            data
        );
        GL30.glGenerateMipmap(GL11.GL_TEXTURE_2D);
    }

    public Texture2D(byte[] data, int width, int height) {
        this.width = width;
        this.height = height;
        this.id = GL11.glGenTextures();
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, id);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_S, GL11.GL_REPEAT);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_T, GL11.GL_REPEAT);

        ByteBuffer buf = org.lwjgl.system.MemoryUtil.memAlloc(data.length);
        buf.put(data).flip();
        GL11.glTexImage2D(
            GL11.GL_TEXTURE_2D,
            0,
            GL11.GL_RGBA,
            width,
            height,
            0,
            GL11.GL_RGBA,
            GL11.GL_UNSIGNED_BYTE,
            buf
        );
        GL30.glGenerateMipmap(GL11.GL_TEXTURE_2D);
        org.lwjgl.system.MemoryUtil.memFree(buf);
    }

    public static Texture2D fromFile(String path) {
        try {
            java.io.File file = new java.io.File(path);
            if (!file.exists()) {
                throw new java.io.FileNotFoundException("Texture file not found: " + path);
            }
            java.awt.image.BufferedImage img = javax.imageio.ImageIO.read(file);
            int width = img.getWidth();
            int height = img.getHeight();
            int[] pixels = new int[width * height];
            img.getRGB(0, 0, width, height, pixels, 0, width);

            ByteBuffer buffer = org.lwjgl.system.MemoryUtil.memAlloc(width * height * 4);
            for (int y = 0; y < height; y++) {
                for (int x = 0; x < width; x++) {
                    int pixel = pixels[y * width + x];
                    buffer.put((byte) ((pixel >> 16) & 0xFF)); // Red
                    buffer.put((byte) ((pixel >> 8) & 0xFF));  // Green
                    buffer.put((byte) (pixel & 0xFF));         // Blue
                    buffer.put((byte) ((pixel >> 24) & 0xFF)); // Alpha
                }
            }
            buffer.flip();
            Texture2D tex = new Texture2D(buffer, width, height);
            org.lwjgl.system.MemoryUtil.memFree(buffer);
            return tex;
        } catch (java.io.IOException e) {
            throw new RuntimeException("Failed to load texture: " + path, e);
        }
    }

    public void update(int x, int y, int w, int h, ByteBuffer data) {
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, this.id);
        GL11.glTexSubImage2D(
            GL11.GL_TEXTURE_2D,
            0,
            x,
            y,
            w,
            h,
            GL11.GL_RGBA,
            GL11.GL_UNSIGNED_BYTE,
            data
        );
    }

    public void bind(int slot) {
        GL13.glActiveTexture(GL13.GL_TEXTURE0 + slot);
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, this.id);
    }

    public void cleanup() {
        GL11.glDeleteTextures(id);
    }

    public int getId() { return id; }
    public int getWidth() { return width; }
    public int getHeight() { return height; }
}
