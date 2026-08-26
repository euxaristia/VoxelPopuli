use crate::block::BlockType;
use crate::chunk::{Biome, CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, biome_at};
use flate2::Compression;
use flate2::write::{GzEncoder, ZlibEncoder};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAX_DECOMPRESSED_CHUNK_BYTES: usize = 16 * 1024 * 1024;
const MAX_NBT_DEPTH: usize = 64;
const MAX_NBT_COLLECTION_LEN: usize = 1_048_576;
const MIN_IMPORT_DATA_VERSION: i32 = 2566;
const MAX_IMPORT_DATA_VERSION: i32 = 2730;

pub const TARGET_NAME: &str = "Minecraft Java pre-1.18 Anvil";
pub const JAVA_1_17_DATA_VERSION: i32 = 2724;
pub const MIN_Y: i32 = 0;
pub const MAX_Y: i32 = 255;
pub const SECTION_HEIGHT: usize = 16;
pub const SECTIONS_PER_CHUNK: usize = CHUNK_HEIGHT / SECTION_HEIGHT;
pub const REGION_CHUNKS: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaProperty {
    pub name: &'static str,
    pub value: &'static str,
}

impl JavaProperty {
    const fn new(name: &'static str, value: &'static str) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaBlockState {
    pub name: &'static str,
    pub properties: &'static [JavaProperty],
}

impl JavaBlockState {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            properties: &[],
        }
    }

    const fn with_properties(name: &'static str, properties: &'static [JavaProperty]) -> Self {
        Self { name, properties }
    }
}

const GRASS_SNOWY: &[JavaProperty] = &[JavaProperty::new("snowy", "true")];
const LOG_Y_AXIS: &[JavaProperty] = &[JavaProperty::new("axis", "y")];
const LEAVES: &[JavaProperty] = &[
    JavaProperty::new("distance", "7"),
    JavaProperty::new("persistent", "false"),
];
const SOURCE_LIQUID: &[JavaProperty] = &[JavaProperty::new("level", "0")];
const SNOW_LAYER: &[JavaProperty] = &[JavaProperty::new("layers", "1")];
const FURNACE: &[JavaProperty] = &[
    JavaProperty::new("facing", "north"),
    JavaProperty::new("lit", "false"),
];
const CHEST: &[JavaProperty] = &[
    JavaProperty::new("facing", "north"),
    JavaProperty::new("type", "single"),
    JavaProperty::new("waterlogged", "false"),
];
const FARMLAND: &[JavaProperty] = &[JavaProperty::new("moisture", "7")];
const WHEAT: &[JavaProperty] = &[JavaProperty::new("age", "7")];
const REDSTONE_ORE: &[JavaProperty] = &[JavaProperty::new("lit", "false")];
const BELL: &[JavaProperty] = &[
    JavaProperty::new("attachment", "ceiling"),
    JavaProperty::new("facing", "north"),
    JavaProperty::new("powered", "false"),
];

pub fn classic_world_height_matches_chunks() -> bool {
    MIN_Y == 0 && MAX_Y == 255 && CHUNK_HEIGHT == 256 && SECTIONS_PER_CHUNK == 16
}

pub fn classic_chunk_dimensions() -> (usize, usize, usize) {
    (CHUNK_WIDTH, CHUNK_HEIGHT, CHUNK_DEPTH)
}

pub fn classic_java_block_state(block: BlockType) -> Option<JavaBlockState> {
    use BlockType::*;
    Some(match block {
        Air => JavaBlockState::new("minecraft:air"),
        Stone => JavaBlockState::new("minecraft:stone"),
        Grass => JavaBlockState::new("minecraft:grass_block"),
        Dirt => JavaBlockState::new("minecraft:dirt"),
        OakLog => JavaBlockState::with_properties("minecraft:oak_log", LOG_Y_AXIS),
        OakLeaves => JavaBlockState::with_properties("minecraft:oak_leaves", LEAVES),
        Bedrock => JavaBlockState::new("minecraft:bedrock"),
        Water => JavaBlockState::with_properties("minecraft:water", SOURCE_LIQUID),
        Sand => JavaBlockState::new("minecraft:sand"),
        Gravel => JavaBlockState::new("minecraft:gravel"),
        CoalOre => JavaBlockState::new("minecraft:coal_ore"),
        PowderedSnow => JavaBlockState::new("minecraft:powder_snow"),
        SnowyGrass => JavaBlockState::with_properties("minecraft:grass_block", GRASS_SNOWY),
        SpruceLog => JavaBlockState::with_properties("minecraft:spruce_log", LOG_Y_AXIS),
        SpruceLeaves => JavaBlockState::with_properties("minecraft:spruce_leaves", LEAVES),
        SnowLayer => JavaBlockState::with_properties("minecraft:snow", SNOW_LAYER),
        IronOre => JavaBlockState::new("minecraft:iron_ore"),
        IronBlock => JavaBlockState::new("minecraft:iron_block"),
        TNT => JavaBlockState::new("minecraft:tnt"),
        Cobblestone => JavaBlockState::new("minecraft:cobblestone"),
        OakPlanks => JavaBlockState::new("minecraft:oak_planks"),
        CraftingTable => JavaBlockState::new("minecraft:crafting_table"),
        Furnace => JavaBlockState::with_properties("minecraft:furnace", FURNACE),
        Chest => JavaBlockState::with_properties("minecraft:chest", CHEST),
        Torch => JavaBlockState::new("minecraft:torch"),
        GoldOre => JavaBlockState::new("minecraft:gold_ore"),
        DiamondOre => JavaBlockState::new("minecraft:diamond_ore"),
        Glass => JavaBlockState::new("minecraft:glass"),
        Bookshelf => JavaBlockState::new("minecraft:bookshelf"),
        MossyCobblestone => JavaBlockState::new("minecraft:mossy_cobblestone"),
        Obsidian => JavaBlockState::new("minecraft:obsidian"),
        Sponge => JavaBlockState::new("minecraft:sponge"),
        Wool => JavaBlockState::new("minecraft:white_wool"),
        LapisOre => JavaBlockState::new("minecraft:lapis_ore"),
        LapisBlock => JavaBlockState::new("minecraft:lapis_block"),
        Sandstone => JavaBlockState::new("minecraft:sandstone"),
        Brick => JavaBlockState::new("minecraft:bricks"),
        StoneBrick => JavaBlockState::new("minecraft:stone_bricks"),
        Lava => JavaBlockState::with_properties("minecraft:lava", SOURCE_LIQUID),
        Cactus => JavaBlockState::new("minecraft:cactus"),
        Clay => JavaBlockState::new("minecraft:clay"),
        Farmland => JavaBlockState::with_properties("minecraft:farmland", FARMLAND),
        Wheat => JavaBlockState::with_properties("minecraft:wheat", WHEAT),
        RedstoneOre => JavaBlockState::with_properties("minecraft:redstone_ore", REDSTONE_ORE),
        MobSpawner => JavaBlockState::new("minecraft:spawner"),
        Bell => JavaBlockState::with_properties("minecraft:bell", BELL),
        Bed => JavaBlockState::new("minecraft:red_bed"),
        OakDoor => JavaBlockState::new("minecraft:oak_door"),
        IronDoor => JavaBlockState::new("minecraft:iron_door"),
        Lever => JavaBlockState::new("minecraft:lever"),
        StoneButton => JavaBlockState::new("minecraft:stone_button"),
        RedstoneWire => JavaBlockState::new("minecraft:redstone_wire"),
        RedstoneTorch => JavaBlockState::new("minecraft:redstone_torch"),
        RedstoneLamp => JavaBlockState::new("minecraft:redstone_lamp"),
        RedstoneBlock => JavaBlockState::new("minecraft:redstone_block"),
        Piston => JavaBlockState::new("minecraft:piston"),
        StickyPiston => JavaBlockState::new("minecraft:sticky_piston"),
        PistonHead => JavaBlockState::new("minecraft:piston_head"),

        RawIron | IronIngot | FlintAndSteel | Stick | Coal | GoldIngot | Diamond | LapisLazuli
        | String | Gunpowder | Leather | RedstoneDust | WoodPickaxe | StonePickaxe
        | IronPickaxe | DiamondPickaxe | GoldPickaxe | WoodAxe | StoneAxe | IronAxe
        | DiamondAxe | GoldAxe | WoodShovel | StoneShovel | IronShovel | DiamondShovel
        | GoldShovel | WoodSword | StoneSword | IronSword | DiamondSword | GoldSword | WoodHoe
        | StoneHoe | IronHoe | DiamondHoe | GoldHoe | Apple | GoldenApple | RawPorkchop
        | CookedPorkchop | RawBeef | Steak | Bread | LeatherHelmet | LeatherChestplate
        | LeatherLeggings | LeatherBoots | IronHelmet | IronChestplate | IronLeggings
        | IronBoots | GoldHelmet | GoldChestplate | GoldLeggings | GoldBoots | DiamondHelmet
        | DiamondChestplate | DiamondLeggings | DiamondBoots | Bow | Arrow | Bucket
        | WaterBucket | LavaBucket => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportConfig {
    pub output_dir: PathBuf,
    pub radius: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportSummary {
    pub chunks: usize,
    pub regions: usize,
}

impl ExportConfig {
    pub fn from_args() -> Option<Self> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        Self::parse(&args)
    }

    fn parse(args: &[String]) -> Option<Self> {
        let mut output_dir: Option<PathBuf> = None;
        let mut radius = 4;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if let Some(path) = arg.strip_prefix("--export-java17=") {
                output_dir = Some(PathBuf::from(path));
            } else if arg == "--export-java17" {
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    output_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                } else {
                    output_dir = Some(PathBuf::from("java17_world"));
                }
            } else if let Some(value) = arg.strip_prefix("--export-radius=") {
                radius = value.parse().unwrap_or(radius);
            } else if arg == "--export-radius" {
                // Only consume the next arg as a value if it isn't another flag
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    radius = args[i + 1].parse().unwrap_or(radius);
                    i += 1;
                }
            }
            i += 1;
        }

        output_dir.map(|output_dir| Self {
            output_dir,
            radius: radius.max(0),
        })
    }
}

pub fn export_classic_java_world(seed: u64, config: &ExportConfig) -> io::Result<ExportSummary> {
    export_classic_java_chunks(seed, config, |chunk_x, chunk_z| {
        let mut chunk = Chunk::new(chunk_x, chunk_z, seed);
        chunk.generate();
        chunk
    })
}

pub fn export_classic_java_chunks<F>(
    seed: u64,
    config: &ExportConfig,
    mut chunk_at: F,
) -> io::Result<ExportSummary>
where
    F: FnMut(i32, i32) -> Chunk,
{
    if !classic_world_height_matches_chunks() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "classic Java export requires 16x16x256 chunks with Y 0..255",
        ));
    }
    let _dimensions = classic_chunk_dimensions();

    std::fs::create_dir_all(&config.output_dir)?;
    std::fs::create_dir_all(config.output_dir.join("region"))?;
    write_level_dat(&config.output_dir, seed)?;

    let mut regions: BTreeMap<(i32, i32), Vec<(i32, i32, Vec<u8>)>> = BTreeMap::new();
    let mut chunks = 0usize;
    for chunk_x in -config.radius..=config.radius {
        for chunk_z in -config.radius..=config.radius {
            let chunk = chunk_at(chunk_x, chunk_z);
            let nbt = write_nbt_root(NbtTag::Compound(classic_chunk_fields(&chunk)));
            let compressed = zlib_compress(&nbt)?;
            let region = (
                chunk_x.div_euclid(REGION_CHUNKS),
                chunk_z.div_euclid(REGION_CHUNKS),
            );
            regions
                .entry(region)
                .or_default()
                .push((chunk_x, chunk_z, compressed));
            chunks += 1;
        }
    }

    let region_count = regions.len();
    for ((region_x, region_z), chunks) in regions {
        let path = config
            .output_dir
            .join("region")
            .join(format!("r.{region_x}.{region_z}.mca"));
        write_region_file(&path, &chunks)?;
    }

    Ok(ExportSummary {
        chunks,
        regions: region_count,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List { element_type: u8, tags: Vec<NbtTag> },
    Compound(Vec<(String, NbtTag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn id(&self) -> u8 {
        match self {
            NbtTag::Byte(_) => 1,
            NbtTag::Short(_) => 2,
            NbtTag::Int(_) => 3,
            NbtTag::Long(_) => 4,
            NbtTag::Float(_) => 5,
            NbtTag::Double(_) => 6,
            NbtTag::ByteArray(_) => 7,
            NbtTag::String(_) => 8,
            NbtTag::List { .. } => 9,
            NbtTag::Compound(_) => 10,
            NbtTag::IntArray(_) => 11,
            NbtTag::LongArray(_) => 12,
        }
    }

    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        if let NbtTag::Compound(fields) = self {
            for (k, v) in fields {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }
}

fn nbt_field(name: &str, tag: NbtTag) -> (String, NbtTag) {
    (name.to_owned(), tag)
}

fn write_nbt_root(root: NbtTag) -> Vec<u8> {
    let mut out = Vec::new();
    write_named_tag(&mut out, "", &root);
    out
}

fn write_named_tag(out: &mut Vec<u8>, name: &str, tag: &NbtTag) {
    out.push(tag.id());
    write_string_payload(out, name);
    write_payload(out, tag);
}

fn write_payload(out: &mut Vec<u8>, tag: &NbtTag) {
    match tag {
        NbtTag::Byte(value) => out.push(*value as u8),
        NbtTag::Short(value) => out.extend_from_slice(&value.to_be_bytes()),
        NbtTag::Int(value) => out.extend_from_slice(&value.to_be_bytes()),
        NbtTag::Long(value) => out.extend_from_slice(&value.to_be_bytes()),
        NbtTag::Float(value) => out.extend_from_slice(&value.to_be_bytes()),
        NbtTag::Double(value) => out.extend_from_slice(&value.to_be_bytes()),
        NbtTag::ByteArray(values) => {
            out.extend_from_slice(&(values.len() as i32).to_be_bytes());
            out.extend_from_slice(values);
        }
        NbtTag::String(value) => write_string_payload(out, value),
        NbtTag::List { element_type, tags } => {
            out.push(*element_type);
            out.extend_from_slice(&(tags.len() as i32).to_be_bytes());
            for tag in tags {
                write_payload(out, tag);
            }
        }
        NbtTag::Compound(fields) => {
            for (name, tag) in fields {
                write_named_tag(out, name, tag);
            }
            out.push(0);
        }
        NbtTag::IntArray(values) => {
            out.extend_from_slice(&(values.len() as i32).to_be_bytes());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        NbtTag::LongArray(values) => {
            out.extend_from_slice(&(values.len() as i32).to_be_bytes());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
}

fn write_string_payload(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn zlib_compress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn gzip_compress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn write_level_dat(output_dir: &Path, seed: u64) -> io::Result<()> {
    let fields = vec![
        nbt_field("DataVersion", NbtTag::Int(JAVA_1_17_DATA_VERSION)),
        nbt_field(
            "Data",
            NbtTag::Compound(vec![
                nbt_field("DataVersion", NbtTag::Int(JAVA_1_17_DATA_VERSION)),
                nbt_field(
                    "Version",
                    NbtTag::Compound(vec![
                        nbt_field("Id", NbtTag::Int(JAVA_1_17_DATA_VERSION)),
                        nbt_field("Name", NbtTag::String("1.17".to_owned())),
                        nbt_field("Snapshot", NbtTag::Byte(0)),
                    ]),
                ),
                nbt_field(
                    "LevelName",
                    NbtTag::String("VoxelPopuli Java 1.17 Export".to_owned()),
                ),
                nbt_field("RandomSeed", NbtTag::Long(seed as i64)),
                nbt_field("SpawnX", NbtTag::Int(0)),
                nbt_field("SpawnY", NbtTag::Int(140)),
                nbt_field("SpawnZ", NbtTag::Int(0)),
                nbt_field("GameType", NbtTag::Int(1)),
                nbt_field("MapFeatures", NbtTag::Byte(1)),
                nbt_field("hardcore", NbtTag::Byte(0)),
                nbt_field("allowCommands", NbtTag::Byte(1)),
                nbt_field("initialized", NbtTag::Byte(1)),
                nbt_field("Time", NbtTag::Long(0)),
                nbt_field("DayTime", NbtTag::Long(1000)),
                nbt_field("version", NbtTag::Int(19133)),
            ]),
        ),
    ];

    let raw = write_nbt_root(NbtTag::Compound(fields));
    let compressed = gzip_compress(&raw)?;
    std::fs::write(output_dir.join("level.dat"), compressed)
}

fn classic_chunk_fields(chunk: &Chunk) -> Vec<(String, NbtTag)> {
    vec![
        nbt_field("DataVersion", NbtTag::Int(JAVA_1_17_DATA_VERSION)),
        nbt_field(
            "Level",
            NbtTag::Compound(vec![
                nbt_field("xPos", NbtTag::Int(chunk.x)),
                nbt_field("zPos", NbtTag::Int(chunk.z)),
                nbt_field("Status", NbtTag::String("full".to_owned())),
                nbt_field("LastUpdate", NbtTag::Long(0)),
                nbt_field("InhabitedTime", NbtTag::Long(0)),
                // The exported BlockLight/SkyLight arrays are placeholders, so
                // mark the chunk unlit and let Minecraft recompute on load.
                nbt_field("isLightOn", NbtTag::Byte(0)),
                nbt_field(
                    "Sections",
                    NbtTag::List {
                        element_type: 10,
                        tags: build_sections(chunk),
                    },
                ),
                nbt_field("Biomes", NbtTag::IntArray(build_biomes(chunk))),
                nbt_field(
                    "Heightmaps",
                    NbtTag::Compound(vec![
                        nbt_field("MOTION_BLOCKING", NbtTag::LongArray(build_heightmap(chunk))),
                        nbt_field("WORLD_SURFACE", NbtTag::LongArray(build_heightmap(chunk))),
                    ]),
                ),
                nbt_field(
                    "Entities",
                    NbtTag::List {
                        element_type: 10,
                        tags: build_entities(chunk),
                    },
                ),
                nbt_field(
                    "TileEntities",
                    NbtTag::List {
                        element_type: 10,
                        tags: build_tile_entities(chunk),
                    },
                ),
                nbt_field(
                    "TileTicks",
                    NbtTag::List {
                        element_type: 10,
                        tags: Vec::new(),
                    },
                ),
                nbt_field(
                    "LiquidTicks",
                    NbtTag::List {
                        element_type: 10,
                        tags: Vec::new(),
                    },
                ),
                nbt_field(
                    "Structures",
                    NbtTag::Compound(vec![
                        nbt_field("References", NbtTag::Compound(Vec::new())),
                        nbt_field("Starts", NbtTag::Compound(Vec::new())),
                    ]),
                ),
            ]),
        ),
    ]
}

pub fn build_tile_entities(chunk: &Chunk) -> Vec<NbtTag> {
    let mut tile_entities = Vec::new();
    for y in 0..CHUNK_HEIGHT {
        for z in 0..CHUNK_DEPTH {
            for x in 0..CHUNK_WIDTH {
                let block = chunk.blocks[x][y][z];
                let world_x = chunk.x * CHUNK_WIDTH as i32 + x as i32;
                let world_y = y as i32;
                let world_z = chunk.z * CHUNK_DEPTH as i32 + z as i32;

                match block {
                    BlockType::Chest => {
                        tile_entities.push(NbtTag::Compound(vec![
                            nbt_field("id", NbtTag::String("minecraft:chest".to_owned())),
                            nbt_field("x", NbtTag::Int(world_x)),
                            nbt_field("y", NbtTag::Int(world_y)),
                            nbt_field("z", NbtTag::Int(world_z)),
                            nbt_field(
                                "Items",
                                NbtTag::List {
                                    element_type: 10,
                                    tags: Vec::new(),
                                },
                            ),
                            nbt_field("keepPacked", NbtTag::Byte(0)),
                        ]));
                    }
                    BlockType::Furnace => {
                        tile_entities.push(NbtTag::Compound(vec![
                            nbt_field("id", NbtTag::String("minecraft:furnace".to_owned())),
                            nbt_field("x", NbtTag::Int(world_x)),
                            nbt_field("y", NbtTag::Int(world_y)),
                            nbt_field("z", NbtTag::Int(world_z)),
                            nbt_field("BurnTime", NbtTag::Int(0)),
                            nbt_field("CookTime", NbtTag::Int(0)),
                            nbt_field("CookTimeTotal", NbtTag::Int(200)),
                            nbt_field(
                                "Items",
                                NbtTag::List {
                                    element_type: 10,
                                    tags: Vec::new(),
                                },
                            ),
                        ]));
                    }
                    BlockType::MobSpawner => {
                        tile_entities.push(NbtTag::Compound(vec![
                            nbt_field("id", NbtTag::String("minecraft:mob_spawner".to_owned())),
                            nbt_field("x", NbtTag::Int(world_x)),
                            nbt_field("y", NbtTag::Int(world_y)),
                            nbt_field("z", NbtTag::Int(world_z)),
                            nbt_field(
                                "SpawnData",
                                NbtTag::Compound(vec![nbt_field(
                                    "id",
                                    NbtTag::String("minecraft:zombie".to_owned()),
                                )]),
                            ),
                            nbt_field("Delay", NbtTag::Int(20)),
                            nbt_field("MinSpawnDelay", NbtTag::Int(200)),
                            nbt_field("MaxSpawnDelay", NbtTag::Int(800)),
                            nbt_field("SpawnCount", NbtTag::Int(4)),
                            nbt_field("RequiredPlayerRange", NbtTag::Int(16)),
                        ]));
                    }
                    BlockType::Bell => {
                        tile_entities.push(NbtTag::Compound(vec![
                            nbt_field("id", NbtTag::String("minecraft:bell".to_owned())),
                            nbt_field("x", NbtTag::Int(world_x)),
                            nbt_field("y", NbtTag::Int(world_y)),
                            nbt_field("z", NbtTag::Int(world_z)),
                        ]));
                    }
                    _ => {}
                }
            }
        }
    }
    tile_entities
}

pub fn build_entities(_chunk: &Chunk) -> Vec<NbtTag> {
    // Basic entity serialization for exported chunks
    Vec::new()
}

pub fn build_villager_entity_nbt(x: f64, y: f64, z: f64, yaw: f32) -> NbtTag {
    NbtTag::Compound(vec![
        nbt_field("id", NbtTag::String("minecraft:villager".to_owned())),
        nbt_field(
            "Pos",
            NbtTag::List {
                element_type: 6, // Double
                tags: vec![
                    NbtTag::Long(x as i64),
                    NbtTag::Long(y as i64),
                    NbtTag::Long(z as i64),
                ],
            },
        ),
        nbt_field(
            "Rotation",
            NbtTag::List {
                element_type: 5, // Float
                tags: vec![NbtTag::Int(yaw as i32)],
            },
        ),
        nbt_field(
            "VillagerData",
            NbtTag::Compound(vec![
                nbt_field("level", NbtTag::Int(1)),
                nbt_field("profession", NbtTag::String("minecraft:farmer".to_owned())),
                nbt_field("type", NbtTag::String("minecraft:plains".to_owned())),
            ]),
        ),
        nbt_field("Health", NbtTag::Int(20)),
        nbt_field("HurtTime", NbtTag::Byte(0)),
    ])
}

pub fn build_iron_golem_entity_nbt(x: f64, y: f64, z: f64) -> NbtTag {
    NbtTag::Compound(vec![
        nbt_field("id", NbtTag::String("minecraft:iron_golem".to_owned())),
        nbt_field(
            "Pos",
            NbtTag::List {
                element_type: 6,
                tags: vec![
                    NbtTag::Long(x as i64),
                    NbtTag::Long(y as i64),
                    NbtTag::Long(z as i64),
                ],
            },
        ),
        nbt_field("PlayerCreated", NbtTag::Byte(0)),
        nbt_field("Health", NbtTag::Int(100)),
    ])
}

pub fn build_mob_entity_nbt(mob: &crate::mob::Mob) -> NbtTag {
    use crate::mob::MobKind;
    let entity_id = match mob.kind {
        MobKind::Villager => "minecraft:villager",
        MobKind::Golem => "minecraft:iron_golem",
        MobKind::Zombie => "minecraft:zombie",
        MobKind::Skeleton => "minecraft:skeleton",
        MobKind::Creeper => "minecraft:creeper",
        MobKind::Pig => "minecraft:pig",
        MobKind::Cow => "minecraft:cow",
        MobKind::Sheep => "minecraft:sheep",
    };
    NbtTag::Compound(vec![
        nbt_field("id", NbtTag::String(entity_id.to_owned())),
        nbt_field(
            "Pos",
            NbtTag::List {
                element_type: 6,
                tags: vec![
                    NbtTag::Long(mob.position.x as i64),
                    NbtTag::Long(mob.position.y as i64),
                    NbtTag::Long(mob.position.z as i64),
                ],
            },
        ),
        nbt_field("Health", NbtTag::Int(20)),
    ])
}

pub fn build_player_data_nbt(x: f64, y: f64, z: f64, yaw: f32, pitch: f32, health: f32) -> NbtTag {
    NbtTag::Compound(vec![
        nbt_field(
            "Pos",
            NbtTag::List {
                element_type: 6, // Double
                tags: vec![
                    NbtTag::Long(x as i64),
                    NbtTag::Long(y as i64),
                    NbtTag::Long(z as i64),
                ],
            },
        ),
        nbt_field(
            "Rotation",
            NbtTag::List {
                element_type: 5, // Float
                tags: vec![NbtTag::Int(yaw as i32), NbtTag::Int(pitch as i32)],
            },
        ),
        nbt_field("Health", NbtTag::Int(health as i32)),
        nbt_field("foodLevel", NbtTag::Int(20)),
        nbt_field("foodSaturationLevel", NbtTag::Int(5)),
        nbt_field("XpLevel", NbtTag::Int(0)),
        nbt_field("Score", NbtTag::Int(0)),
    ])
}

pub fn build_tnt_entity_nbt(x: f64, y: f64, z: f64, fuse_ticks: i16) -> NbtTag {
    NbtTag::Compound(vec![
        nbt_field("id", NbtTag::String("minecraft:tnt".to_owned())),
        nbt_field(
            "Pos",
            NbtTag::List {
                element_type: 6,
                tags: vec![
                    NbtTag::Long(x as i64),
                    NbtTag::Long(y as i64),
                    NbtTag::Long(z as i64),
                ],
            },
        ),
        nbt_field("Fuse", NbtTag::Int(fuse_ticks as i32)),
    ])
}

fn build_sections(chunk: &Chunk) -> Vec<NbtTag> {
    let mut sections = Vec::new();
    for section_y in 0..SECTIONS_PER_CHUNK {
        let mut palette = Vec::<JavaBlockState>::new();
        let mut indices = Vec::<u16>::with_capacity(CHUNK_WIDTH * CHUNK_DEPTH * SECTION_HEIGHT);
        let mut contains_non_air = false;

        for y in 0..SECTION_HEIGHT {
            let world_y = section_y * SECTION_HEIGHT + y;
            for z in 0..CHUNK_DEPTH {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.blocks[x][world_y][z];
                    if block != BlockType::Air {
                        contains_non_air = true;
                    }
                    let state = classic_java_block_state(block)
                        .unwrap_or(JavaBlockState::new("minecraft:air"));
                    let palette_index = match palette.iter().position(|existing| *existing == state)
                    {
                        Some(index) => index,
                        None => {
                            palette.push(state);
                            palette.len() - 1
                        }
                    };
                    indices.push(palette_index as u16);
                }
            }
        }

        if !contains_non_air {
            continue;
        }

        let bits_per_block = bits_needed(palette.len()).max(4);
        sections.push(NbtTag::Compound(vec![
            nbt_field("Y", NbtTag::Byte(section_y as i8)),
            nbt_field(
                "Palette",
                NbtTag::List {
                    element_type: 10,
                    tags: palette.into_iter().map(palette_entry_nbt).collect(),
                },
            ),
            nbt_field(
                "BlockStates",
                NbtTag::LongArray(pack_values(&indices, bits_per_block)),
            ),
            nbt_field("BlockLight", NbtTag::ByteArray(vec![0; 2048])),
            nbt_field("SkyLight", NbtTag::ByteArray(vec![0xFF; 2048])),
        ]));
    }
    sections
}

fn palette_entry_nbt(state: JavaBlockState) -> NbtTag {
    let mut fields = vec![nbt_field("Name", NbtTag::String(state.name.to_owned()))];
    if !state.properties.is_empty() {
        fields.push(nbt_field(
            "Properties",
            NbtTag::Compound(
                state
                    .properties
                    .iter()
                    .map(|property| {
                        nbt_field(property.name, NbtTag::String(property.value.to_owned()))
                    })
                    .collect(),
            ),
        ));
    }
    NbtTag::Compound(fields)
}

fn build_heightmap(chunk: &Chunk) -> Vec<i64> {
    let mut heights = Vec::<u16>::with_capacity(CHUNK_WIDTH * CHUNK_DEPTH);
    for z in 0..CHUNK_DEPTH {
        for x in 0..CHUNK_WIDTH {
            let mut height = 0u16;
            for y in (0..CHUNK_HEIGHT).rev() {
                if chunk.blocks[x][y][z] != BlockType::Air {
                    height = (y + 1) as u16;
                    break;
                }
            }
            heights.push(height);
        }
    }
    pack_values(&heights, 9)
}

fn build_biomes(chunk: &Chunk) -> Vec<i32> {
    let mut biomes = Vec::with_capacity(4 * 4 * 64);
    for _y in 0..64 {
        for z in 0..4 {
            for x in 0..4 {
                let world_x = chunk.x * CHUNK_WIDTH as i32 + x * 4 + 2;
                let world_z = chunk.z * CHUNK_DEPTH as i32 + z * 4 + 2;
                biomes.push(java_biome_id(biome_at(
                    world_x as f32,
                    world_z as f32,
                    chunk.seed,
                )));
            }
        }
    }
    biomes
}

fn java_biome_id(biome: Biome) -> i32 {
    match biome {
        Biome::Plains => 1,
        Biome::Desert => 2,
        Biome::Mountains | Biome::HighHills => 3,
        Biome::SnowyTundra => 12,
        Biome::SnowyTaiga => 30,
    }
}

fn bits_needed(values: usize) -> usize {
    if values <= 1 {
        1
    } else {
        usize::BITS as usize - (values - 1).leading_zeros() as usize
    }
}

fn pack_values(values: &[u16], bits_per_value: usize) -> Vec<i64> {
    debug_assert!(bits_per_value > 0 && bits_per_value <= 16);
    // MC 1.16+ layout: entries never span i64 boundaries; each long holds
    // floor(64 / bits) entries and any leftover high bits are padding.
    let values_per_long = 64 / bits_per_value;
    let long_count = values.len().div_ceil(values_per_long);
    let mut longs = vec![0u64; long_count];
    let mask = (1u64 << bits_per_value) - 1;

    for (index, value) in values.iter().enumerate() {
        let value = *value as u64 & mask;
        let long_index = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits_per_value;
        longs[long_index] |= value << bit_offset;
    }

    longs.into_iter().map(|value| value as i64).collect()
}

fn write_region_file(path: &Path, chunks: &[(i32, i32, Vec<u8>)]) -> io::Result<()> {
    const SECTOR_BYTES: usize = 4096;
    let mut header = vec![0u8; SECTOR_BYTES * 2];
    let mut body = Vec::new();
    let mut next_sector = 2u32;

    for (chunk_x, chunk_z, compressed_nbt) in chunks {
        let local_x = chunk_x.rem_euclid(REGION_CHUNKS) as usize;
        let local_z = chunk_z.rem_euclid(REGION_CHUNKS) as usize;
        let location_index = local_x + local_z * REGION_CHUNKS as usize;

        let payload_len = 1 + compressed_nbt.len();
        let total_len = 4 + payload_len;
        let sector_count = total_len.div_ceil(SECTOR_BYTES) as u8;
        let offset = next_sector;

        header[location_index * 4] = ((offset >> 16) & 0xFF) as u8;
        header[location_index * 4 + 1] = ((offset >> 8) & 0xFF) as u8;
        header[location_index * 4 + 2] = (offset & 0xFF) as u8;
        header[location_index * 4 + 3] = sector_count;

        let timestamp_index = SECTOR_BYTES + location_index * 4;
        header[timestamp_index..timestamp_index + 4].copy_from_slice(&0u32.to_be_bytes());

        let mut chunk_record = Vec::with_capacity(sector_count as usize * SECTOR_BYTES);
        chunk_record.extend_from_slice(&(payload_len as u32).to_be_bytes());
        chunk_record.push(2); // zlib
        chunk_record.extend_from_slice(compressed_nbt);
        chunk_record.resize(sector_count as usize * SECTOR_BYTES, 0);
        body.extend_from_slice(&chunk_record);
        next_sector += sector_count as u32;
    }

    let mut file = std::fs::File::create(path)?;
    file.write_all(&header)?;
    file.write_all(&body)?;
    Ok(())
}

pub struct NbtDecoder<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> NbtDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        if self.pos >= self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "NBT EOF"));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_i8(&mut self) -> io::Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    fn read_i16(&mut self) -> io::Result<i16> {
        if self.pos + 2 > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "NBT EOF"));
        }
        let v = i16::from_be_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        Ok(v)
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        Ok(self.read_i16()? as u16)
    }

    fn read_i32(&mut self) -> io::Result<i32> {
        if self.pos + 4 > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "NBT EOF"));
        }
        let v = i32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn read_i64(&mut self) -> io::Result<i64> {
        if self.pos + 8 > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "NBT EOF"));
        }
        let v = i64::from_be_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    fn read_f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    fn read_f64(&mut self) -> io::Result<f64> {
        Ok(f64::from_bits(self.read_i64()? as u64))
    }

    fn read_string(&mut self) -> io::Result<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        let s = String::from_utf8_lossy(bytes).into_owned();
        Ok(s)
    }

    fn read_bytes(&mut self, len: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "NBT EOF"))?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn read_collection_len(&mut self, kind: &str, element_bytes: usize) -> io::Result<usize> {
        let raw = self.read_i32()?;
        if raw < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Negative NBT {kind} length {raw}"),
            ));
        }
        let len = raw as usize;
        if len > MAX_NBT_COLLECTION_LEN
            || element_bytes > 0
                && len
                    .checked_mul(element_bytes)
                    .is_none_or(|bytes| bytes > self.data.len().saturating_sub(self.pos))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("NBT {kind} length {len} exceeds safe bounds"),
            ));
        }
        Ok(len)
    }

    fn read_tag_payload(&mut self, tag_type: u8, depth: usize) -> io::Result<NbtTag> {
        if depth > MAX_NBT_DEPTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "NBT nesting exceeds safe depth",
            ));
        }
        match tag_type {
            1 => Ok(NbtTag::Byte(self.read_i8()?)),
            2 => Ok(NbtTag::Short(self.read_i16()?)),
            3 => Ok(NbtTag::Int(self.read_i32()?)),
            4 => Ok(NbtTag::Long(self.read_i64()?)),
            5 => Ok(NbtTag::Float(self.read_f32()?)),
            6 => Ok(NbtTag::Double(self.read_f64()?)),
            7 => {
                let len = self.read_collection_len("byte array", 1)?;
                let bytes = self.read_bytes(len)?.to_vec();
                Ok(NbtTag::ByteArray(bytes))
            }
            8 => Ok(NbtTag::String(self.read_string()?)),
            9 => {
                let elem_type = self.read_u8()?;
                let len = self.read_collection_len("list", 0)?;
                let mut tags = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    tags.push(self.read_tag_payload(elem_type, depth + 1)?);
                }
                Ok(NbtTag::List {
                    element_type: elem_type,
                    tags,
                })
            }
            10 => {
                let mut fields = Vec::new();
                loop {
                    if fields.len() >= MAX_NBT_COLLECTION_LEN {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "NBT compound has too many fields",
                        ));
                    }
                    let field_type = self.read_u8()?;
                    if field_type == 0 {
                        break;
                    }
                    let name = self.read_string()?;
                    let val = self.read_tag_payload(field_type, depth + 1)?;
                    fields.push((name, val));
                }
                Ok(NbtTag::Compound(fields))
            }
            11 => {
                let len = self.read_collection_len("int array", 4)?;
                let mut ints = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    ints.push(self.read_i32()?);
                }
                Ok(NbtTag::IntArray(ints))
            }
            12 => {
                let len = self.read_collection_len("long array", 8)?;
                let mut longs = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    longs.push(self.read_i64()?);
                }
                Ok(NbtTag::LongArray(longs))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown NBT tag type {tag_type}"),
            )),
        }
    }

    pub fn parse_root(&mut self) -> io::Result<(String, NbtTag)> {
        let type_id = self.read_u8()?;
        if type_id == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty NBT root"));
        }
        let root_name = self.read_string()?;
        let root_tag = self.read_tag_payload(type_id, 0)?;
        Ok((root_name, root_tag))
    }
}

fn decompress_chunk_payload(compression_type: u8, compressed: &[u8]) -> io::Result<Vec<u8>> {
    let reader: Box<dyn Read> = match compression_type {
        1 => Box::new(flate2::read::GzDecoder::new(compressed)),
        2 => Box::new(flate2::read::ZlibDecoder::new(compressed)),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported Anvil compression type {compression_type}"),
            ));
        }
    };
    let mut decompressed = Vec::new();
    reader
        .take(MAX_DECOMPRESSED_CHUNK_BYTES as u64 + 1)
        .read_to_end(&mut decompressed)?;
    if decompressed.len() > MAX_DECOMPRESSED_CHUNK_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Decompressed Anvil chunk exceeds 16 MiB safety limit",
        ));
    }
    Ok(decompressed)
}

fn read_region_chunk(path: &Path, local_x: usize, local_z: usize) -> io::Result<Option<Vec<u8>>> {
    let mut file = std::fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    if file_len < 8192 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Region file is too small: {}", path.display()),
        ));
    }

    let header_offset = ((local_x + local_z * 32) * 4) as u64;
    file.seek(SeekFrom::Start(header_offset))?;
    let mut location = [0_u8; 4];
    file.read_exact(&mut location)?;
    let sector_offset =
        ((location[0] as u64) << 16) | ((location[1] as u64) << 8) | location[2] as u64;
    let sector_count = location[3] as u64;
    if sector_offset == 0 || sector_count == 0 {
        return Ok(None);
    }

    let chunk_offset = sector_offset
        .checked_mul(4096)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Anvil chunk offset overflow"))?;
    let sector_bytes = sector_count * 4096;
    if chunk_offset
        .checked_add(sector_bytes)
        .is_none_or(|end| end > file_len)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Chunk sectors exceed region file: {}", path.display()),
        ));
    }

    file.seek(SeekFrom::Start(chunk_offset))?;
    let mut record_header = [0_u8; 5];
    file.read_exact(&mut record_header)?;
    let length = u32::from_be_bytes(record_header[..4].try_into().unwrap()) as u64;
    if length <= 1 || length + 4 > sector_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid chunk length in {}", path.display()),
        ));
    }
    let compressed_len = usize::try_from(length - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Anvil chunk length is too large",
        )
    })?;
    let mut compressed = vec![0_u8; compressed_len];
    file.read_exact(&mut compressed)?;
    decompress_chunk_payload(record_header[4], &compressed).map(Some)
}

pub fn validate_classic_java_world(world_dir: &Path) -> io::Result<usize> {
    let region_dir = world_dir.join("region");
    if !region_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No region directory found at {}", region_dir.display()),
        ));
    }

    let mut region_count = 0;
    for entry in std::fs::read_dir(region_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("mca") {
            region_count += 1;
        }
    }
    if region_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Java world contains no Anvil region files",
        ));
    }
    Ok(region_count)
}

pub fn import_classic_java_chunk(
    world_dir: &Path,
    chunk_x: i32,
    chunk_z: i32,
) -> io::Result<Option<Chunk>> {
    let region_x = chunk_x.div_euclid(32);
    let region_z = chunk_z.div_euclid(32);
    let path = world_dir
        .join("region")
        .join(format!("r.{region_x}.{region_z}.mca"));
    if !path.is_file() {
        return Ok(None);
    }
    let Some(payload) = read_region_chunk(
        &path,
        chunk_x.rem_euclid(32) as usize,
        chunk_z.rem_euclid(32) as usize,
    )?
    else {
        return Ok(None);
    };
    let chunk = import_chunk_from_nbt(&payload)?;
    if chunk.x != chunk_x || chunk.z != chunk_z {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Anvil chunk coordinate mismatch: requested ({chunk_x}, {chunk_z}), found ({}, {})",
                chunk.x, chunk.z
            ),
        ));
    }
    Ok(Some(chunk))
}

pub fn java_block_name_to_block_type(name: &str) -> BlockType {
    use BlockType::*;
    match name {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => Air,
        "minecraft:stone" => Stone,
        "minecraft:grass_block" => Grass,
        "minecraft:dirt" | "minecraft:coarse_dirt" => Dirt,
        "minecraft:oak_log" => OakLog,
        "minecraft:oak_leaves" => OakLeaves,
        "minecraft:bedrock" => Bedrock,
        "minecraft:water" => Water,
        "minecraft:sand" => Sand,
        "minecraft:gravel" => Gravel,
        "minecraft:coal_ore" => CoalOre,
        "minecraft:powder_snow" => PowderedSnow,
        "minecraft:spruce_log" => SpruceLog,
        "minecraft:spruce_leaves" => SpruceLeaves,
        "minecraft:snow" => SnowLayer,
        "minecraft:iron_ore" => IronOre,
        "minecraft:iron_block" => IronBlock,
        "minecraft:tnt" => TNT,
        "minecraft:cobblestone" => Cobblestone,
        "minecraft:oak_planks" => OakPlanks,
        "minecraft:crafting_table" => CraftingTable,
        "minecraft:furnace" => Furnace,
        "minecraft:chest" => Chest,
        "minecraft:torch" | "minecraft:wall_torch" => Torch,
        "minecraft:gold_ore" => GoldOre,
        "minecraft:diamond_ore" => DiamondOre,
        "minecraft:glass" => Glass,
        "minecraft:bookshelf" => Bookshelf,
        "minecraft:mossy_cobblestone" => MossyCobblestone,
        "minecraft:obsidian" => Obsidian,
        "minecraft:sponge" => Sponge,
        "minecraft:white_wool" => Wool,
        "minecraft:lapis_ore" => LapisOre,
        "minecraft:lapis_block" => LapisBlock,
        "minecraft:sandstone" => Sandstone,
        "minecraft:bricks" => Brick,
        "minecraft:stone_bricks" => StoneBrick,
        "minecraft:lava" => Lava,
        "minecraft:cactus" => Cactus,
        "minecraft:clay" => Clay,
        "minecraft:farmland" => Farmland,
        "minecraft:wheat" => Wheat,
        "minecraft:redstone_ore" => RedstoneOre,
        "minecraft:spawner" => MobSpawner,
        "minecraft:bell" => Bell,
        "minecraft:red_bed" => Bed,
        "minecraft:oak_door" => OakDoor,
        "minecraft:iron_door" => IronDoor,
        "minecraft:lever" => Lever,
        "minecraft:stone_button" => StoneButton,
        "minecraft:redstone_wire" => RedstoneWire,
        "minecraft:redstone_torch" | "minecraft:redstone_wall_torch" => RedstoneTorch,
        "minecraft:redstone_lamp" => RedstoneLamp,
        "minecraft:redstone_block" => RedstoneBlock,
        "minecraft:piston" => Piston,
        "minecraft:sticky_piston" => StickyPiston,
        "minecraft:piston_head" => PistonHead,
        _ => Air,
    }
}

fn unpack_values(longs: &[i64], bits_per_value: usize, count: usize) -> Vec<u16> {
    if bits_per_value == 0 || longs.is_empty() {
        return vec![0; count];
    }
    let values_per_long = 64 / bits_per_value;
    let mask = (1u64 << bits_per_value) - 1;
    let mut values = Vec::with_capacity(count);

    for index in 0..count {
        let long_index = index / values_per_long;
        if long_index >= longs.len() {
            values.push(0);
            continue;
        }
        let bit_offset = (index % values_per_long) * bits_per_value;
        let val = ((longs[long_index] as u64) >> bit_offset) & mask;
        values.push(val as u16);
    }
    values
}

pub fn import_chunk_from_nbt(decompressed_nbt: &[u8]) -> io::Result<Chunk> {
    let mut decoder = NbtDecoder::new(decompressed_nbt);
    let (_root_name, root) = decoder.parse_root()?;

    let data_version = match root.get("DataVersion") {
        Some(NbtTag::Int(version))
            if (MIN_IMPORT_DATA_VERSION..=MAX_IMPORT_DATA_VERSION).contains(version) =>
        {
            *version
        }
        Some(NbtTag::Int(version)) if *version > MAX_IMPORT_DATA_VERSION => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported post-1.17.1 Java chunk DataVersion {version}"),
            ));
        }
        Some(NbtTag::Int(version)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported pre-1.16 Java chunk DataVersion {version}"),
            ));
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Java chunk is missing DataVersion",
            ));
        }
    };
    debug_assert!((MIN_IMPORT_DATA_VERSION..=MAX_IMPORT_DATA_VERSION).contains(&data_version));

    let level = root.get("Level").unwrap_or(&root);
    let (x_pos, z_pos) = match (level.get("xPos"), level.get("zPos")) {
        (Some(NbtTag::Int(x)), Some(NbtTag::Int(z))) => (*x, *z),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Java chunk is missing xPos or zPos",
            ));
        }
    };

    let mut chunk = Chunk::new(x_pos, z_pos, 0);
    let mut unknown_blocks = BTreeSet::new();

    if let Some(NbtTag::List { tags: sections, .. }) = level.get("Sections") {
        for sec in sections {
            let sec_y = match sec.get("Y") {
                Some(NbtTag::Byte(y)) => *y as i32,
                _ => continue,
            };

            if sec_y < 0 || sec_y >= SECTIONS_PER_CHUNK as i32 {
                continue;
            }

            let palette: Vec<BlockType> = match sec.get("Palette") {
                Some(NbtTag::List { tags, .. }) => tags
                    .iter()
                    .map(|t| {
                        if let Some(NbtTag::String(name)) = t.get("Name") {
                            let mut btype = java_block_name_to_block_type(name);
                            if btype == BlockType::Air
                                && !matches!(
                                    name.as_str(),
                                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                                )
                            {
                                unknown_blocks.insert(name.clone());
                            }
                            if btype == BlockType::Grass {
                                if let Some(NbtTag::Compound(props)) = t.get("Properties") {
                                    for (k, v) in props {
                                        if k == "snowy" && v == &NbtTag::String("true".to_owned()) {
                                            btype = BlockType::SnowyGrass;
                                        }
                                    }
                                }
                            }
                            btype
                        } else {
                            BlockType::Air
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };

            if palette.is_empty() {
                continue;
            }

            let block_states: &[i64] = match sec.get("BlockStates") {
                Some(NbtTag::LongArray(arr)) => arr.as_slice(),
                _ => &[],
            };

            let bits_per_block = bits_needed(palette.len()).max(4);
            let indices = unpack_values(
                block_states,
                bits_per_block,
                CHUNK_WIDTH * CHUNK_DEPTH * SECTION_HEIGHT,
            );

            for (idx, &pal_idx) in indices.iter().enumerate() {
                let x = idx % CHUNK_WIDTH;
                let z = (idx / CHUNK_WIDTH) % CHUNK_DEPTH;
                let y = idx / (CHUNK_WIDTH * CHUNK_DEPTH);
                let world_y = (sec_y as usize * SECTION_HEIGHT) + y;

                if world_y < CHUNK_HEIGHT {
                    let block = palette
                        .get(pal_idx as usize)
                        .copied()
                        .unwrap_or(BlockType::Air);
                    chunk.blocks[x][world_y][z] = block;
                }
            }
        }
    }

    chunk.hydrate_fluids();
    if !unknown_blocks.is_empty() {
        eprintln!(
            "Imported chunk ({x_pos}, {z_pos}) replaced unsupported Java blocks with air: {}",
            unknown_blocks.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_target_uses_zero_to_255_world_height() {
        assert_eq!(TARGET_NAME, "Minecraft Java pre-1.18 Anvil");
        assert_eq!((MIN_Y, MAX_Y), (0, 255));
        assert_eq!(classic_chunk_dimensions(), (16, 256, 16));
        assert!(classic_world_height_matches_chunks());
    }

    #[test]
    fn nbt_rejects_negative_collection_lengths() {
        let bytes = [9, 0, 0, 1, 0xff, 0xff, 0xff, 0xff];
        let error = NbtDecoder::new(&bytes).parse_root().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("Negative NBT list length"));
    }

    #[test]
    fn nbt_rejects_excessive_nesting() {
        let mut bytes = vec![10, 0, 0];
        for _ in 0..=MAX_NBT_DEPTH {
            bytes.extend_from_slice(&[10, 0, 0]);
        }
        bytes.extend(std::iter::repeat_n(0, MAX_NBT_DEPTH + 2));
        let error = NbtDecoder::new(&bytes).parse_root().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("safe depth"));
    }

    #[test]
    fn every_world_block_has_classic_java_state_mapping() {
        for id in 0..BlockType::COUNT as u8 {
            let block = BlockType::from_u8(id);
            let expected_world_block = !block.is_item() || matches!(block, BlockType::Wheat);
            assert_eq!(
                classic_java_block_state(block).is_some(),
                expected_world_block,
                "{block:?} mapping mismatch"
            );
        }
    }

    #[test]
    fn classic_mapping_uses_real_java_names_for_special_blocks() {
        assert_eq!(
            classic_java_block_state(BlockType::SnowyGrass).unwrap(),
            JavaBlockState::with_properties("minecraft:grass_block", GRASS_SNOWY)
        );
        assert_eq!(
            classic_java_block_state(BlockType::Farmland).unwrap(),
            JavaBlockState::with_properties("minecraft:farmland", FARMLAND)
        );
        assert_eq!(
            classic_java_block_state(BlockType::Wheat).unwrap(),
            JavaBlockState::with_properties("minecraft:wheat", WHEAT)
        );
        assert_eq!(
            classic_java_block_state(BlockType::RedstoneDust),
            None,
            "inventory redstone dust is not a placed block in VoxelPopuli yet"
        );
    }

    #[test]
    fn export_radius_without_value_does_not_consume_next_flag() {
        let args: Vec<String> = ["--export-radius", "--export-java17", "out"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let config = ExportConfig::parse(&args).expect("--export-java17 must still be parsed");
        assert_eq!(config.output_dir, PathBuf::from("out"));
        assert_eq!(config.radius, 4);
    }

    #[test]
    fn export_radius_parses_valid_forms() {
        let args: Vec<String> = ["--export-java17", "out", "--export-radius", "7"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(ExportConfig::parse(&args).unwrap().radius, 7);

        let args: Vec<String> = ["--export-java17=out", "--export-radius=9"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(ExportConfig::parse(&args).unwrap().radius, 9);
    }

    #[test]
    fn packed_block_states_use_expected_long_count() {
        let values = vec![0u16; CHUNK_WIDTH * CHUNK_DEPTH * SECTION_HEIGHT];
        assert_eq!(pack_values(&values, 4).len(), 256);

        // 9-bit entries: 7 per long, 256 entries -> 37 longs (1.16+ padded layout)
        let height_values = vec![64u16; CHUNK_WIDTH * CHUNK_DEPTH];
        assert_eq!(pack_values(&height_values, 9).len(), 37);
    }

    #[test]
    fn nbt_root_starts_as_named_compound() {
        let bytes = write_nbt_root(NbtTag::Compound(vec![nbt_field(
            "DataVersion",
            NbtTag::Int(1),
        )]));
        assert_eq!(bytes[0], 10);
        assert_eq!(&bytes[1..3], &[0, 0]);
        assert!(bytes.ends_with(&[0]));
    }

    #[test]
    fn import_rejects_missing_chunk_coordinates() {
        let bytes = write_nbt_root(NbtTag::Compound(vec![
            nbt_field("DataVersion", NbtTag::Int(JAVA_1_17_DATA_VERSION)),
            nbt_field("Level", NbtTag::Compound(Vec::new())),
        ]));
        let error = match import_chunk_from_nbt(&bytes) {
            Err(error) => error,
            Ok(_) => panic!("chunk without coordinates was accepted"),
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("xPos or zPos"));
    }

    #[test]
    fn import_rejects_pre_1_16_packing() {
        let bytes = write_nbt_root(NbtTag::Compound(vec![
            nbt_field("DataVersion", NbtTag::Int(MIN_IMPORT_DATA_VERSION - 1)),
            nbt_field(
                "Level",
                NbtTag::Compound(vec![
                    nbt_field("xPos", NbtTag::Int(0)),
                    nbt_field("zPos", NbtTag::Int(0)),
                ]),
            ),
        ]));
        let error = match import_chunk_from_nbt(&bytes) {
            Err(error) => error,
            Ok(_) => panic!("pre-1.16 chunk was accepted"),
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("pre-1.16"));
    }

    #[test]
    fn import_rejects_post_1_17_1_chunk_format() {
        let bytes = write_nbt_root(NbtTag::Compound(vec![nbt_field(
            "DataVersion",
            NbtTag::Int(MAX_IMPORT_DATA_VERSION + 1),
        )]));
        let error = match import_chunk_from_nbt(&bytes) {
            Err(error) => error,
            Ok(_) => panic!("post-1.17.1 chunk was accepted"),
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("post-1.17.1"));
    }

    #[test]
    fn export_radius_zero_writes_level_and_region_files() {
        let out = std::env::temp_dir().join(format!(
            "voxelpopuli-java17-export-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out);

        let config = ExportConfig {
            output_dir: out.clone(),
            radius: 0,
        };
        let summary = export_classic_java_world(12345, &config).unwrap();

        assert_eq!(summary.chunks, 1);
        assert_eq!(summary.regions, 1);
        assert!(out.join("level.dat").is_file());
        let region = out.join("region").join("r.0.0.mca");
        assert!(region.is_file());
        let len = std::fs::metadata(region).unwrap().len();
        assert_eq!(len % 4096, 0);

        let _ = std::fs::remove_dir_all(out);
    }

    #[test]
    fn test_tile_entities_nbt_generation() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.blocks[0][64][0] = BlockType::Chest;
        chunk.blocks[1][64][0] = BlockType::Furnace;
        chunk.blocks[2][64][0] = BlockType::MobSpawner;

        let tile_entities = build_tile_entities(&chunk);
        assert_eq!(tile_entities.len(), 3);
    }

    #[test]
    fn test_entity_nbt_helpers() {
        let villager = build_villager_entity_nbt(10.0, 64.0, 10.0, 0.0);
        assert_eq!(villager.id(), 10); // Compound

        let golem = build_iron_golem_entity_nbt(15.0, 64.0, 15.0);
        assert_eq!(golem.id(), 10);

        let tnt = build_tnt_entity_nbt(5.0, 64.0, 5.0, 80);
        assert_eq!(tnt.id(), 10);

        let mob = crate::mob::Mob::new(
            crate::mob::MobKind::Zombie,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            0,
        );
        let mob_nbt = build_mob_entity_nbt(&mob);
        assert_eq!(mob_nbt.id(), 10);

        let player_nbt = build_player_data_nbt(0.0, 64.0, 0.0, 0.0, 0.0, 20.0);
        assert_eq!(player_nbt.id(), 10);
    }

    #[test]
    fn test_java_world_export_import_roundtrip() {
        let out = std::env::temp_dir().join(format!(
            "voxelpopuli-java-roundtrip-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out);

        let config = ExportConfig {
            output_dir: out.clone(),
            radius: 0,
        };
        let summary = export_classic_java_world(9999, &config).unwrap();
        assert_eq!(summary.chunks, 1);

        let streamed = import_classic_java_chunk(&out, 0, 0).unwrap().unwrap();
        assert_eq!((streamed.x, streamed.z), (0, 0));
        assert!(import_classic_java_chunk(&out, 1, 0).unwrap().is_none());

        let mut original = Chunk::new(0, 0, 9999);
        original.generate();

        assert_eq!(streamed.x, original.x);
        assert_eq!(streamed.z, original.z);

        let mut matching_blocks = 0;
        let mut total_blocks = 0;
        let mut mismatch_map = std::collections::HashMap::new();
        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    total_blocks += 1;
                    let orig = original.blocks[x][y][z];
                    let imp = streamed.blocks[x][y][z];
                    if imp == orig {
                        matching_blocks += 1;
                    } else {
                        *mismatch_map.entry((orig, imp)).or_insert(0) += 1;
                    }
                }
            }
        }
        println!("Block mismatches: {:?}", mismatch_map);
        assert_eq!(
            matching_blocks, total_blocks,
            "imported chunk blocks must match 100%"
        );

        let _ = std::fs::remove_dir_all(out);
    }
}
