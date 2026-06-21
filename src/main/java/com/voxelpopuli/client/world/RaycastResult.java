package com.voxelpopuli.client.world;

import com.voxelpopuli.client.block.BlockType;

public class RaycastResult {
    public final boolean hit;
    public final int x;
    public final int y;
    public final int z;
    public final int nx;
    public final int ny;
    public final int nz;
    public final BlockType block;

    public RaycastResult(boolean hit, int x, int y, int z, int nx, int ny, int nz, BlockType block) {
        this.hit = hit;
        this.x = x;
        this.y = y;
        this.z = z;
        this.nx = nx;
        this.ny = ny;
        this.nz = nz;
        this.block = block;
    }
}
