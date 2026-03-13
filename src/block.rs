#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BlockType {
    Air = 0,
    Stone,
    Grass,
    Dirt,
    OakLog,
    OakLeaves,
    Bedrock,
    Water,
    Sand,
    Gravel,
    CoalOre,
    #[allow(dead_code)]
    PowderedSnow,
    SnowyGrass,
    SpruceLog,
    SpruceLeaves,
    SnowLayer,
}

#[allow(dead_code)]
impl BlockType {
    pub const COUNT: usize = 16;
    
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => BlockType::Air,
            1 => BlockType::Stone,
            2 => BlockType::Grass,
            3 => BlockType::Dirt,
            4 => BlockType::OakLog,
            5 => BlockType::OakLeaves,
            6 => BlockType::Bedrock,
            7 => BlockType::Water,
            8 => BlockType::Sand,
            9 => BlockType::Gravel,
            10 => BlockType::CoalOre,
            11 => BlockType::PowderedSnow,
            12 => BlockType::SnowyGrass,
            13 => BlockType::SpruceLog,
            14 => BlockType::SpruceLeaves,
            15 => BlockType::SnowLayer,
            _ => BlockType::Air,
        }
    }
}
