package com.voxelpopuli.client;

import com.voxelpopuli.client.block.BlockType;

public record MinedBlock(int x, int y, int z, BlockType block, BlockType drop, int dropCount) {}
