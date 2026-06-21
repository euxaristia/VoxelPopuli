package com.voxelpopuli.client.renderer;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL30;

public class RenderTexture2D {
    private int fbo;
    private int rbo;
    private Texture2D texture;

    public RenderTexture2D(int width, int height) {
        fbo = GL30.glGenFramebuffers();
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);

        texture = new Texture2D(new byte[width * height * 4], width, height);

        GL30.glFramebufferTexture2D(
            GL30.GL_FRAMEBUFFER,
            GL30.GL_COLOR_ATTACHMENT0,
            GL11.GL_TEXTURE_2D,
            texture.getId(),
            0
        );

        rbo = GL30.glGenRenderbuffers();
        GL30.glBindRenderbuffer(GL30.GL_RENDERBUFFER, rbo);
        GL30.glRenderbufferStorage(
            GL30.GL_RENDERBUFFER,
            GL30.GL_DEPTH24_STENCIL8,
            width,
            height
        );
        GL30.glFramebufferRenderbuffer(
            GL30.GL_FRAMEBUFFER,
            GL30.GL_DEPTH_STENCIL_ATTACHMENT,
            GL30.GL_RENDERBUFFER,
            rbo
        );

        if (GL30.glCheckFramebufferStatus(GL30.GL_FRAMEBUFFER) != GL30.GL_FRAMEBUFFER_COMPLETE) {
            throw new RuntimeException("Framebuffer is not complete!");
        }

        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, 0);
    }

    public void bind() {
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);
        GL11.glViewport(0, 0, texture.getWidth(), texture.getHeight());
    }

    public void unbind() {
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, 0);
    }

    public void cleanup() {
        GL30.glDeleteFramebuffers(fbo);
        GL30.glDeleteRenderbuffers(rbo);
        texture.cleanup();
    }

    public Texture2D getTexture() { return texture; }
}
