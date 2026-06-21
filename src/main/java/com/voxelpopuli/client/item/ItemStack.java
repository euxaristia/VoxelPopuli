package com.voxelpopuli.client.item;

import com.voxelpopuli.client.block.BlockType;

public class ItemStack {
    public final BlockType block;
    public int count;
    public Integer durability;

    public ItemStack(BlockType block, int count) {
        this.block = block;
        this.count = count;
        ToolProperties tp = ToolProperties.getProperties(block);
        if (tp != null) {
            this.durability = tp.durability;
        } else {
            this.durability = null;
        }
    }

    public ItemStack(BlockType block, int count, Integer durability) {
        this.block = block;
        this.count = count;
        this.durability = durability;
    }

    public int getMaxStackSize() {
        return block.getMaxStackSize();
    }

    @Override
    public ItemStack clone() {
        return new ItemStack(this.block, this.count, this.durability);
    }
}
