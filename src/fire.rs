// Minecraft-style fire: a non-solid block drawn as animated crossed
// sprites. Flint and steel places it; it spreads to flammable neighbors
// and burns them out. Twist: magenta-rimmed flames, and coal ore holds
// fire forever the way netherrack does in vanilla.
use crate::block::BlockType;
use crate::renderer::Mesh;

pub const FIRE_FRAME_COUNT: usize = 8;
pub const FIRE_ATLAS_TY: u8 = 11;

pub fn is_flammable(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::OakLog
            | BlockType::SpruceLog
            | BlockType::OakLeaves
            | BlockType::SpruceLeaves
            | BlockType::OakPlanks
            | BlockType::CraftingTable
            | BlockType::Chest
            | BlockType::Bookshelf
            | BlockType::Wool
            | BlockType::Wheat
            | BlockType::Grass
            | BlockType::SnowyGrass
            | BlockType::TNT
    )
}

/// Coal ore is this world's netherrack: fire on it never goes out.
pub fn is_eternal_fuel(block: BlockType) -> bool {
    block == BlockType::CoalOre
}

pub fn can_occupy(block: BlockType) -> bool {
    matches!(block, BlockType::Air | BlockType::Fire)
}

pub fn atlas_frame(time: f32) -> (u8, u8) {
    let frame = ((time * 12.0).floor() as usize) % FIRE_FRAME_COUNT;
    (frame as u8, FIRE_ATLAS_TY)
}

pub fn build_frame_mesh(frame: usize) -> Mesh {
    let (tx, ty) = (frame as u8, FIRE_ATLAS_TY);
    let ts = 1.0 / 16.0;
    let pad = 0.5 / 256.0;
    let u0 = tx as f32 * ts + pad;
    let v0 = ty as f32 * ts + pad;
    let u1 = (tx as f32 + 1.0) * ts - pad;
    let v1 = (ty as f32 + 1.0) * ts - pad;
    // Two crossed quads, Minecraft fire style. Vertex color is fullbright
    // so night exposure does not snuff the sprite.
    let color = [255u8, 210, 255, 230];
    let mut v = Vec::new();
    let mut t = Vec::new();
    let mut n = Vec::new();
    let mut c = Vec::new();
    let mut push_quad = |verts: [f32; 18], nx: f32, ny: f32, nz: f32| {
        v.extend_from_slice(&verts);
        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
        for _ in 0..6 {
            n.extend_from_slice(&[nx, ny, nz]);
            c.extend_from_slice(&color);
        }
    };
    // Z-facing plane through the cell center.
    push_quad(
        [
            0.1, 0.0, 0.5, 0.9, 0.0, 0.5, 0.9, 1.15, 0.5, 0.1, 0.0, 0.5, 0.9, 1.15, 0.5, 0.1, 1.15,
            0.5,
        ],
        0.0,
        0.0,
        1.0,
    );
    // X-facing plane.
    push_quad(
        [
            0.5, 0.0, 0.1, 0.5, 0.0, 0.9, 0.5, 1.15, 0.9, 0.5, 0.0, 0.1, 0.5, 1.15, 0.9, 0.5, 1.15,
            0.1,
        ],
        1.0,
        0.0,
        0.0,
    );
    Mesh::new(&v, Some(&t), Some(&n), Some(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grass_and_wood_burn_but_stone_does_not() {
        assert!(is_flammable(BlockType::Grass));
        assert!(is_flammable(BlockType::OakLog));
        assert!(is_flammable(BlockType::Wool));
        assert!(!is_flammable(BlockType::Stone));
        assert!(!is_flammable(BlockType::Dirt));
        assert!(is_eternal_fuel(BlockType::CoalOre));
        assert!(!is_eternal_fuel(BlockType::Grass));
    }

    #[test]
    fn fire_frames_walk_the_atlas_row() {
        let (tx0, ty) = atlas_frame(0.0);
        assert_eq!(ty, FIRE_ATLAS_TY);
        assert_eq!(tx0, 0);
        let (tx, _) = atlas_frame(100.0);
        assert!(tx < FIRE_FRAME_COUNT as u8);
    }
}
