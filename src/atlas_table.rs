// Per-BlockType lookup table for the GPU compute mesher (M2+, see the
// hashed-splashing-haven plan). Built once from the existing atlas_uv*
// functions and is_solid/is_transparent so the GPU never hand-duplicates
// that logic; the shader just indexes this table by block id.
use crate::block::BlockType;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AtlasEntry {
    pub default_tx: u32,
    pub default_ty: u32,
    pub top_tx: u32,
    pub top_ty: u32,
    pub top_height: f32,
    pub is_solid: u32,
    pub is_transparent: u32,
    pub _pad0: f32,
}

pub fn build_atlas_table() -> Vec<AtlasEntry> {
    (0..BlockType::COUNT as u8)
        .map(|id| {
            let block = BlockType::from_u8(id);
            let (dtx, dty) = crate::item::atlas_uv(block);
            let (ttx, tty) = crate::item::atlas_uv_top(block);
            let top_height = match block {
                BlockType::SnowLayer => 0.125,
                BlockType::Torch => 0.625,
                BlockType::Bell => 0.6,
                _ => 1.0,
            };
            AtlasEntry {
                default_tx: dtx as u32,
                default_ty: dty as u32,
                top_tx: ttx as u32,
                top_ty: tty as u32,
                top_height,
                is_solid: block.is_solid() as u32,
                is_transparent: block.is_transparent() as u32,
                _pad0: 0.0,
            }
        })
        .collect()
}
