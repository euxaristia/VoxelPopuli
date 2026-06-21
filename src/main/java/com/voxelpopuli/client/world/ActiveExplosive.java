package com.voxelpopuli.client.world;

import com.voxelpopuli.client.block.BlockType;
import org.joml.Vector3f;

public class ActiveExplosive {
    public final Vector3f position;
    public float fuse;
    public final BlockType blockType;

    public ActiveExplosive(Vector3f position, float fuse, BlockType blockType) {
        this.position = new Vector3f(position);
        this.fuse = fuse;
        this.blockType = blockType;
    }
}
