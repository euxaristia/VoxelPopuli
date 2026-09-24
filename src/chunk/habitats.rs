//! Habitat decoration for generator 2. The legacy generator remains unchanged.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tree {
    Oak,
    Birch,
    Cherry,
    Mangrove,
}

impl Tree {
    fn blocks(self) -> (BlockType, BlockType) {
        use BlockType::*;
        match self {
            Self::Oak => (OakLog, OakLeaves),
            Self::Birch => (BirchLog, BirchLeaves),
            Self::Cherry => (CherryLog, CherryLeaves),
            Self::Mangrove => (MangroveLog, MangroveLeaves),
        }
    }
}

fn tree_kind(biome: Biome, roll: u32) -> Option<Tree> {
    match biome {
        Biome::Forest | Biome::FlowerForest => Some(if roll % 5 == 0 {
            Tree::Birch
        } else {
            Tree::Oak
        }),
        Biome::BirchForest => Some(Tree::Birch),
        Biome::CherryGrove => Some(Tree::Cherry),
        Biome::MangroveSwamp => Some(Tree::Mangrove),
        Biome::Meadow | Biome::SunflowerPlains if roll % 8 == 0 => Some(if roll & 8 == 0 {
            Tree::Birch
        } else {
            Tree::Oak
        }),
        _ => None,
    }
}

fn ground(chunk: &Chunk, x: usize, z: usize) -> Option<usize> {
    (1..CHUNK_HEIGHT - 15)
        .rev()
        .find(|&y| {
            let block = chunk.blocks[x][y][z];
            block != BlockType::Air && block != BlockType::Water
        })
        .filter(|&y| {
            matches!(
                chunk.blocks[x][y][z],
                BlockType::Grass | BlockType::Mud | BlockType::Dirt
            )
        })
}

fn grow(chunk: &mut Chunk, x: usize, y: usize, z: usize, tree: Tree, roll: u32) -> bool {
    if tree != Tree::Mangrove && chunk.blocks[x][y + 1][z] != BlockType::Air {
        return false;
    }
    if tree == Tree::Mangrove && chunk.blocks[x][y + 4][z] == BlockType::Water {
        return false;
    }
    let (log, leaves) = tree.blocks();
    let height = match tree {
        Tree::Birch => 5 + roll as usize % 3,
        Tree::Mangrove => 7 + roll as usize % 3,
        _ => 4 + roll as usize % 3,
    };
    let crown = y + height;
    let radius = if matches!(tree, Tree::Cherry | Tree::Mangrove) {
        3
    } else {
        2
    };
    if x < radius
        || z < radius
        || x + radius >= CHUNK_WIDTH
        || z + radius >= CHUNK_DEPTH
        || crown + 2 >= CHUNK_HEIGHT
    {
        return false;
    }
    // Reject obstructed crowns instead of overwriting a neighboring tree or structure.
    for xx in x - radius..=x + radius {
        for zz in z - radius..=z + radius {
            for yy in y + 1..=crown + 1 {
                if !matches!(chunk.blocks[xx][yy][zz], BlockType::Air | BlockType::Water) {
                    return false;
                }
            }
        }
    }
    let root_height = if tree == Tree::Mangrove { 3 } else { 1 };
    for yy in y + root_height..=crown {
        chunk.set_local(x as i32, yy as i32, z as i32, log);
    }
    if tree == Tree::Mangrove {
        for yy in y + 1..y + 3 {
            chunk.set_local(x as i32, yy as i32, z as i32, BlockType::MangroveRoots);
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for step in 1..=2 {
                let xx = (x as i32 + dx * step) as usize;
                let zz = (z as i32 + dz * step) as usize;
                for yy in y + 1..=y + 3 - step as usize {
                    chunk.set_local(xx as i32, yy as i32, zz as i32, BlockType::MangroveRoots);
                }
            }
        }
    }
    for dy in -1i32..=1 {
        let r = if dy == 1 { radius - 1 } else { radius };
        for dx in -(r as i32)..=r as i32 {
            for dz in -(r as i32)..=r as i32 {
                if dx.abs() == r as i32 && dz.abs() == r as i32 {
                    continue;
                }
                let p = (
                    (x as i32 + dx) as usize,
                    (crown as i32 + dy) as usize,
                    (z as i32 + dz) as usize,
                );
                if chunk.blocks[p.0][p.1][p.2] == BlockType::Air {
                    chunk.blocks[p.0][p.1][p.2] = leaves;
                }
            }
        }
    }
    if tree == Tree::Cherry {
        // Two spreading branches support the broad pink canopy.
        for dx in [-2i32, -1, 1, 2] {
            chunk.blocks[(x as i32 + dx) as usize][crown - 1][z] = log;
        }
    }
    true
}

fn attach_nest(
    chunk: &mut Chunk,
    x: usize,
    y: usize,
    z: usize,
    tree: Tree,
    roll: u32,
    biome: Biome,
) {
    // Habitat-specific probabilities are engine choices; public samples do not expose vanilla feature rolls.
    let denominator = match biome {
        Biome::Meadow => 1,
        Biome::FlowerForest => 20,
        Biome::Forest | Biome::BirchForest => 500,
        _ => 20,
    };
    if roll % denominator != 0 {
        return;
    }
    let (_, leaves) = tree.blocks();
    for yy in y + 2..(y + 12).min(CHUNK_HEIGHT - 1) {
        // South-facing entrance (+Z), directly below canopy, clear exit.
        if chunk.blocks[x + 1][yy + 1][z] == leaves
            && chunk.blocks[x + 1][yy][z] == BlockType::Air
            && chunk.blocks[x + 1][yy][z + 1] == BlockType::Air
        {
            chunk.blocks[x + 1][yy][z] = BlockType::BeeNest;
            return;
        }
    }
}

pub(super) fn decorate(chunk: &mut Chunk) {
    let biomes: [[Biome; CHUNK_DEPTH]; CHUNK_WIDTH] = std::array::from_fn(|x| {
        std::array::from_fn(|z| {
            biome_at(
                (chunk.x * 16 + x as i32) as f32,
                (chunk.z * 16 + z as i32) as f32,
                chunk.seed,
            )
        })
    });
    // Surface replacement uses per-column biomes, including shores under shallow water.
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_DEPTH {
            let biome = biomes[x][z];
            if biome == Biome::MangroveSwamp {
                for y in (1..CHUNK_HEIGHT).rev() {
                    let b = chunk.blocks[x][y][z];
                    if matches!(b, BlockType::Air | BlockType::Water) {
                        continue;
                    }
                    if matches!(
                        b,
                        BlockType::Grass | BlockType::Dirt | BlockType::Sand | BlockType::Mud
                    ) {
                        chunk.blocks[x][y][z] = BlockType::Mud;
                        if chunk.blocks[x][y - 1][z] == BlockType::Dirt {
                            chunk.blocks[x][y - 1][z] = BlockType::Mud;
                        }
                    }
                    break;
                }
            }
        }
    }
    let mut rng = ChunkRng::new(chunk.seed, chunk.x, chunk.z, 0x48414249544154);
    let attempts = match biomes[8][8] {
        Biome::Meadow => 1,
        Biome::SunflowerPlains => 2,
        _ => 24,
    };
    for _ in 0..attempts {
        let x = rng.range_usize(3, CHUNK_WIDTH - 3);
        let z = rng.range_usize(3, CHUNK_DEPTH - 3);
        let biome = biomes[x][z];
        let Some(tree) = tree_kind(biome, rng.next_u32()) else {
            continue;
        };
        let Some(y) = ground(chunk, x, z) else {
            continue;
        };
        let roll = rng.next_u32();
        if grow(chunk, x, y, z, tree, roll) {
            attach_nest(chunk, x, y, z, tree, rng.next_u32(), biome);
        }
    }
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_DEPTH {
            let wx = chunk.x * 16 + x as i32;
            let wz = chunk.z * 16 + z as i32;
            let biome = biomes[x][z];
            let roll = chunk_hash(chunk.seed, wx, wz, 0x464c4f57);
            let divisor = match biome {
                Biome::FlowerForest | Biome::Meadow => 4,
                Biome::CherryGrove => 5,
                Biome::SunflowerPlains => 9,
                _ => 40,
            };
            if !biome.is_bee_habitat() || roll % divisor != 0 {
                continue;
            }
            let Some(y) = ground(chunk, x, z) else {
                continue;
            };
            if chunk.blocks[x][y][z] != BlockType::Grass
                || chunk.blocks[x][y + 1][z] != BlockType::Air
            {
                continue;
            }
            let flower = match biome {
                Biome::SunflowerPlains => BlockType::Sunflower,
                Biome::CherryGrove => BlockType::PinkPetals,
                _ => [
                    BlockType::Poppy,
                    BlockType::Dandelion,
                    BlockType::Cornflower,
                    BlockType::Allium,
                    BlockType::OxeyeDaisy,
                ][(roll / divisor) as usize % 5],
            };
            if flower == BlockType::Sunflower {
                if chunk.blocks[x][y + 2][z] != BlockType::Air {
                    continue;
                }
                chunk.blocks[x][y + 2][z] = BlockType::SunflowerTop;
            }
            chunk.blocks[x][y + 1][z] = flower;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mangrove_roots_and_trunk_clear_replaced_water_levels() {
        let mut chunk = Chunk::new(0, 0, 42);
        chunk.set_local(8, 100, 8, BlockType::Mud);
        for x in 5..=11 {
            for z in 5..=11 {
                for y in 101..=103 {
                    chunk.set_local(x, y, z, BlockType::Water);
                }
            }
        }
        assert!(grow(&mut chunk, 8, 100, 8, Tree::Mangrove, 0));
        let mut replaced = 0;
        for x in 5..=11 {
            for z in 5..=11 {
                for y in 101..=103 {
                    let solid = matches!(
                        chunk.blocks[x][y][z],
                        BlockType::MangroveRoots | BlockType::MangroveLog
                    );
                    if solid {
                        replaced += 1;
                    }
                    assert_eq!(
                        chunk.liquid_levels[x][y][z],
                        if solid { 0 } else { WATER_SOURCE },
                        "({x},{y},{z})"
                    );
                }
            }
        }
        assert_eq!(replaced, 15);
    }
    #[test]
    fn tree_species_have_distinct_trunks_canopies_and_roots() {
        for tree in [Tree::Oak, Tree::Birch, Tree::Cherry, Tree::Mangrove] {
            let mut chunk = Chunk::new(0, 0, 42);
            chunk.blocks[8][100][8] = BlockType::Grass;
            assert!(grow(&mut chunk, 8, 100, 8, tree, 0));
            let (log, leaves) = tree.blocks();
            assert_eq!(chunk.blocks[8][103][8], log);
            assert!(
                chunk
                    .blocks
                    .iter()
                    .flatten()
                    .flatten()
                    .any(|&b| b == leaves)
            );
            assert_eq!(
                chunk.blocks[6][101][8] == BlockType::MangroveRoots,
                tree == Tree::Mangrove
            );
        }
    }
    #[test]
    fn meadow_nests_hang_under_leaves_with_a_clear_exit() {
        let mut chunk = Chunk::new(0, 0, 42);
        assert!(grow(&mut chunk, 8, 100, 8, Tree::Birch, 0));
        attach_nest(&mut chunk, 8, 100, 8, Tree::Birch, 17, Biome::Meadow);
        let y = (101..112)
            .find(|&y| chunk.blocks[9][y][8] == BlockType::BeeNest)
            .unwrap();
        assert_eq!(chunk.blocks[9][y + 1][8], BlockType::BirchLeaves);
        assert_eq!(chunk.blocks[9][y][9], BlockType::Air);
    }
}
