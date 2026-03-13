use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct ActiveExplosive {
    pub position: Vec3,
    pub fuse: f32, // remaining time in seconds
    pub block_type: BlockType,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub scale: f32,
}

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
    IronOre,
    RawIron,
    IronIngot,
    IronBlock,
    TNT,
    Nuke,
    FlintAndSteel,
}

#[allow(dead_code)]
impl BlockType {
    pub const COUNT: usize = 23;
    
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
            16 => BlockType::IronOre,
            17 => BlockType::RawIron,
            18 => BlockType::IronIngot,
            19 => BlockType::IronBlock,
            20 => BlockType::TNT,
            21 => BlockType::Nuke,
            22 => BlockType::FlintAndSteel,
            _ => BlockType::Air,
        }
    }

    pub fn is_item(&self) -> bool {
        match self {
            BlockType::RawIron | BlockType::IronIngot | BlockType::FlintAndSteel => true,
            _ => false,
        }
    }

    pub fn is_solid(&self) -> bool {
        match self {
            BlockType::Air | BlockType::Water | BlockType::SnowLayer => false,
            _ => !self.is_item(),
        }
    }
}
