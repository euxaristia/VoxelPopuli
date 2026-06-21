package com.voxelpopuli.client;

import com.voxelpopuli.client.block.BlockType;
import com.voxelpopuli.client.item.ToolProperties;
import com.voxelpopuli.client.world.BlockPos;
import com.voxelpopuli.client.world.RaycastResult;
import com.voxelpopuli.client.world.World;
import org.joml.Vector3f;

public class MiningState {
    public BlockPos target = null;
    public float progress = 0.0f;
    public float totalTime = 0.0f;

    public void reset() {
        target = null;
        progress = 0.0f;
        totalTime = 0.0f;
    }

    public Integer getCrackStage() {
        if (target != null && totalTime > 0.0f) {
            float frac = progress / totalTime;
            if (frac < 0.0f) frac = 0.0f;
            if (frac > 0.999f) frac = 0.999f;
            return (int) (frac * 10.0f);
        }
        return null;
    }

    public MinedBlock update(World world, Vector3f eyePos, Vector3f lookDir, BlockType heldItem, float dt) {
        RaycastResult res = world.raycast(eyePos, lookDir, 8.0f);
        if (!res.hit) {
            reset();
            return null;
        }

        BlockType block = world.getBlock(res.x, res.y, res.z);
        if (block == BlockType.AIR || block == BlockType.WATER || block == BlockType.BEDROCK) {
            reset();
            return null;
        }

        BlockPos targetPos = new BlockPos(res.x, res.y, res.z);
        if (target == null || !target.equals(targetPos)) {
            target = targetPos;
            progress = 0.0f;
            totalTime = ToolProperties.breakingTime(block, heldItem);
        }

        progress += dt;

        if (progress >= totalTime) {
            BlockType drop = ToolProperties.getDrop(block, heldItem);
            int count = ToolProperties.getDropCount(block, heldItem);
            MinedBlock result = new MinedBlock(res.x, res.y, res.z, block, drop, count);
            reset();
            return result;
        }

        return null;
    }
}
