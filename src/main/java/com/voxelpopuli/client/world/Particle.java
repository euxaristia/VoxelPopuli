package com.voxelpopuli.client.world;

import org.joml.Vector3f;

public class Particle {
    public final Vector3f position;
    public final Vector3f velocity;
    public float life;
    public final float maxLife;
    public final float scale;

    public Particle(Vector3f position, Vector3f velocity, float life, float maxLife, float scale) {
        this.position = new Vector3f(position);
        this.velocity = new Vector3f(velocity);
        this.life = life;
        this.maxLife = maxLife;
        this.scale = scale;
    }
}
