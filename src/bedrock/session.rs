//! Serialize world read/modify/write operations above the database's batch boundary.
use super::{
    palette::chunk_key,
    store::{Database, read_level},
    terrain,
};
use crate::{chunk::Chunk, java_compat::NbtTag};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct WorldStore {
    database: Mutex<Database>,
    pub seed: u64,
    pub metadata: NbtTag,
    path: PathBuf,
}

impl WorldStore {
    #[cfg(test)]
    pub(crate) fn with_locked_database(&self, f: impl FnOnce()) {
        let _guard = self.database.lock().unwrap();
        f();
    }

    pub fn containers(
        &self,
        x: i32,
        z: i32,
        dimension: i32,
    ) -> io::Result<Vec<((i32, i32, i32), crate::container::Container)>> {
        let Some(bytes) = self.record(&chunk_key(x, z, dimension, 0x31, None))? else {
            return Ok(vec![]);
        };
        let mut containers = Vec::new();
        for record in super::records::decode_stream(&bytes)? {
            if let Some(container) = super::records::decode_container(&record)? {
                let position = super::records::position(&record)?;
                if position.0.div_euclid(16) != x || position.2.div_euclid(16) != z {
                    return Err(super::records::invalid(
                        "Block entity stored in the wrong chunk",
                    ));
                }
                if (0..256).contains(&position.1) {
                    containers.push((position, container));
                }
            }
        }
        Ok(containers)
    }
    pub fn open(path: &Path) -> io::Result<Self> {
        let database = Database::open_writable(path)?;
        let metadata = read_level(path)?;
        let Some(NbtTag::Long(seed)) = metadata.get("RandomSeed") else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Bedrock metadata has no valid RandomSeed",
            ));
        };
        Ok(Self {
            database: Mutex::new(database),
            seed: *seed as u64,
            metadata,
            path: path.to_path_buf(),
        })
    }

    pub fn create(path: &Path, seed: u64) -> io::Result<Self> {
        let metadata = new_metadata(seed)?;
        let database = Database::create(path, &metadata)?;
        Ok(Self {
            database: Mutex::new(database),
            seed,
            metadata,
            path: path.to_path_buf(),
        })
    }

    pub fn read_chunk(&self, x: i32, z: i32, dimension: i32) -> io::Result<Option<Chunk>> {
        let database = self
            .database
            .lock()
            .map_err(|_| io::Error::other("World storage lock poisoned"))?;
        let records = terrain::read_records(&database, x, z, dimension)?;
        drop(database);
        records.decode(x, z, self.seed)
    }

    /// The caller supplies generated terrain, including its dimension-specific generator.
    /// Recheck under the transaction lock in case another worker already persisted it.
    pub fn insert_generated(&self, chunk: &Chunk, dimension: i32) -> io::Result<()> {
        let database = self
            .database
            .lock()
            .map_err(|_| io::Error::other("World storage lock poisoned"))?;
        if terrain::read_records(&database, chunk.x, chunk.z, dimension)?.exists() {
            return Ok(());
        }
        let mut records = terrain::chunk_records(&database, chunk, dimension)?;
        records.extend(new_chunk_metadata(chunk, dimension));
        database.commit(records)
    }

    pub fn save_chunks<'a>(
        &self,
        chunks: impl IntoIterator<Item = &'a Chunk>,
        dimension: i32,
        extra_records: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> io::Result<()> {
        let database = self
            .database
            .lock()
            .map_err(|_| io::Error::other("World storage lock poisoned"))?;
        let mut records = extra_records;
        for chunk in chunks {
            records.extend(terrain::chunk_records(&database, chunk, dimension)?);
        }
        database.commit(records)
    }

    pub fn update_metadata(
        &self,
        fields: impl IntoIterator<Item = (&'static str, NbtTag)>,
    ) -> io::Result<()> {
        let _database = self
            .database
            .lock()
            .map_err(|_| io::Error::other("World storage lock poisoned"))?;
        let mut metadata = super::store::read_level(&self.path)?;
        for (key, value) in fields {
            super::records::set(&mut metadata, key, value)?;
        }
        super::store::write_level(&self.path, &metadata)
    }

    pub fn record(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        let database = self
            .database
            .lock()
            .map_err(|_| io::Error::other("World storage lock poisoned"))?;
        database.get(key)
    }
}

fn new_metadata(seed: u64) -> io::Result<NbtTag> {
    use crate::block::BlockType;
    use NbtTag::*;
    let mut chunk = Chunk::new(0, 0, seed);
    chunk.generate();
    let mut columns: Vec<_> = (0..16).flat_map(|x| (0..16).map(move |z| (x, z))).collect();
    columns.sort_by_key(|&(x, z)| (x as i32 - 8).pow(2) + (z as i32 - 8).pow(2));
    let spawn = [false, true]
        .into_iter()
        .find_map(|allow_water| {
            columns.iter().find_map(|&(x, z)| {
                (1..crate::chunk::CHUNK_HEIGHT - 1).rev().find_map(|y| {
                    let ground = chunk.blocks[x][y - 1][z];
                    let safe = (ground.is_solid()
                        && !matches!(
                            ground,
                            BlockType::OakLeaves | BlockType::SpruceLeaves | BlockType::Cactus
                        ))
                        || (allow_water && ground == BlockType::Water);
                    (safe
                        && chunk.blocks[x][y][z] == BlockType::Air
                        && chunk.blocks[x][y + 1][z] == BlockType::Air)
                        .then_some((x as i32, y as i32, z as i32))
                })
            })
        })
        .ok_or_else(|| io::Error::other("Could not find a safe generated spawn"))?;
    let version = || List {
        element_type: 3,
        tags: vec![Int(1), Int(26), Int(51), Int(1), Int(0)],
    };
    Ok(Compound(vec![
        ("LevelName".into(), String("VoxelPopuli".into())),
        ("RandomSeed".into(), Long(seed as i64)),
        ("StorageVersion".into(), Int(10)),
        ("NetworkVersion".into(), Int(2193)),
        ("lastOpenedWithVersion".into(), version()),
        ("MinimumCompatibleClientVersion".into(), version()),
        ("baseGameVersion".into(), String("1.26.51".into())),
        ("GameType".into(), Int(0)),
        ("Generator".into(), Int(1)),
        (
            "voxelpopuli:generator_version".into(),
            Int(crate::chunk::GeneratorVersion::Habitats as i32),
        ),
        ("Difficulty".into(), Int(2)),
        ("SpawnX".into(), Int(spawn.0)),
        ("SpawnY".into(), Int(spawn.1)),
        ("SpawnZ".into(), Int(spawn.2)),
        ("Time".into(), Long(0)),
        ("currentTick".into(), Long(0)),
        ("commandsEnabled".into(), Byte(0)),
        ("spawnMobs".into(), Byte(1)),
        ("hasBeenLoadedInCreative".into(), Byte(0)),
        ("worldStartCount".into(), Long(1)),
    ]))
}

fn new_chunk_metadata(chunk: &Chunk, dimension: i32) -> Vec<(Vec<u8>, Vec<u8>)> {
    let key = |tag| chunk_key(chunk.x, chunk.z, dimension, tag, None);
    // Data3D: 256 little-endian heights followed by 24 runtime-ID biome palettes.
    let mut biomes = Vec::with_capacity(632);
    for z in 0..16 {
        for x in 0..16 {
            let top = (0..crate::chunk::CHUNK_HEIGHT)
                .rev()
                .find(|&y| chunk.blocks[x][y][z] != crate::block::BlockType::Air)
                .map_or(0, |y| y + 1);
            biomes.extend_from_slice(&((top + 64) as u16).to_le_bytes());
        }
    }
    let storage = biome_storage(chunk, dimension);
    for _ in 0..24 {
        biomes.extend_from_slice(&storage);
    }
    vec![
        (key(0x2c), vec![42]),
        (key(0x36), 2i32.to_le_bytes().to_vec()),
        (key(0x2b), biomes),
    ]
}

fn biome_storage(chunk: &Chunk, dimension: i32) -> Vec<u8> {
    use crate::chunk::{Biome, biome_at_version};
    let mut palette = Vec::<i32>::new();
    let mut indices = Vec::with_capacity(4096);
    for x in 0..16 {
        for z in 0..16 {
            let id = if dimension == 1 {
                8
            } else {
                match biome_at_version(
                    (chunk.x * 16 + x) as f32,
                    (chunk.z * 16 + z) as f32,
                    chunk.seed,
                    chunk.generator_version,
                ) {
                    Biome::Plains => 1,
                    Biome::Desert => 2,
                    Biome::Mountains | Biome::HighHills => 3,
                    Biome::Forest => 4,
                    Biome::SnowyTundra => 12,
                    Biome::BirchForest => 27,
                    Biome::SnowyTaiga => 30,
                    Biome::SunflowerPlains => 129,
                    Biome::FlowerForest => 132,
                    Biome::Meadow => 186,
                    Biome::MangroveSwamp => 191,
                    Biome::CherryGrove => 192,
                }
            };
            let index = palette.iter().position(|&v| v == id).unwrap_or_else(|| {
                palette.push(id);
                palette.len() - 1
            });
            indices.extend(std::iter::repeat_n(index as u32, 16));
        }
    }
    let width = if palette.len() == 1 {
        0
    } else if palette.len() <= 2 {
        1
    } else if palette.len() <= 4 {
        2
    } else if palette.len() <= 8 {
        3
    } else {
        4
    };
    let mut bytes = vec![(width << 1) | 1];
    if width > 0 {
        for cells in indices.chunks(32 / width as usize) {
            let word = cells
                .iter()
                .enumerate()
                .fold(0u32, |v, (i, &index)| v | (index << (i * width as usize)));
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&(palette.len() as u32).to_le_bytes());
    }
    for id in palette {
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_biome_palette_contains_cherry_grove_and_valid_indices() {
        let chunk = Chunk::new(-384, -174, 42);
        let bytes = biome_storage(&chunk, 0);
        assert_eq!(bytes[0] & 1, 1);
        let width = (bytes[0] >> 1) as usize;
        let words = if width == 0 {
            0
        } else {
            4096usize.div_ceil(32 / width)
        };
        let mut offset = 1 + words * 4;
        let count = if width == 0 {
            1
        } else {
            let n = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            n
        };
        let ids: Vec<_> = bytes[offset..]
            .chunks_exact(4)
            .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(ids.len(), count);
        assert!(ids.contains(&192));
        if width > 0 {
            for cell in 0..4096 {
                let word = 1 + (cell / (32 / width)) * 4;
                let value = u32::from_le_bytes(bytes[word..word + 4].try_into().unwrap());
                let index = (value >> ((cell % (32 / width)) * width)) & ((1 << width) - 1);
                assert!((index as usize) < count);
            }
        }
        assert_eq!(biome_storage(&chunk, 1), vec![1, 8, 0, 0, 0]);
    }
    #[test]
    #[ignore = "release-mode storage profile"]
    fn profile_world_loading() {
        use std::time::Instant;
        let path = std::env::temp_dir().join(format!(
            "voxel-profile-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = WorldStore::create(&path, 1074691402050369410).unwrap();
        let positions: Vec<_> = (-16..=16)
            .flat_map(|x| (-16..=16).map(move |z| (x, z)))
            .collect();
        let start = Instant::now();
        let mut chunks: Vec<_> = positions
            .iter()
            .map(|&(x, z)| Chunk::new(x, z, store.seed))
            .collect();
        rayon::scope(|scope| {
            for chunk in &mut chunks {
                scope.spawn(move |_| chunk.generate());
            }
        });
        println!(
            "PROFILE generation {} chunks: {:?}",
            chunks.len(),
            start.elapsed()
        );
        let start = Instant::now();
        rayon::scope(|scope| {
            for chunk in &chunks {
                let store = &store;
                scope.spawn(move |_| store.insert_generated(chunk, 0).unwrap());
            }
        });
        println!("PROFILE persist: {:?}", start.elapsed());
        assert!(
            start.elapsed().as_secs() < 30,
            "chunk persistence exceeded the release profiling budget"
        );
        let start = Instant::now();
        rayon::scope(|scope| {
            for &(x, z) in &positions {
                let store = &store;
                scope.spawn(move |_| assert!(store.read_chunk(x, z, 0).unwrap().is_some()));
            }
        });
        println!("PROFILE reload: {:?}", start.elapsed());
        let start = Instant::now();
        store.save_chunks(chunks.iter(), 0, vec![]).unwrap();
        println!("PROFILE unchanged save: {:?}", start.elapsed());
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn new_world_spawn_stands_on_terrain_with_headroom() {
        for seed in [42, 0, u64::MAX] {
            let metadata = new_metadata(seed).unwrap();
            let coordinate = |name| match metadata.get(name) {
                Some(NbtTag::Int(value)) => *value as usize,
                _ => panic!("missing spawn coordinate"),
            };
            let (x, y, z) = (
                coordinate("SpawnX"),
                coordinate("SpawnY"),
                coordinate("SpawnZ"),
            );
            let mut chunk = Chunk::new(0, 0, seed);
            chunk.generate();
            assert_ne!(
                chunk.blocks[x][y - 1][z],
                crate::block::BlockType::Air,
                "spawn floats above terrain for seed {seed}"
            );
            assert_eq!(chunk.blocks[x][y][z], crate::block::BlockType::Air);
            assert_eq!(chunk.blocks[x][y + 1][z], crate::block::BlockType::Air);
        }
    }
    #[test]
    fn generated_chunks_persist_and_dimensions_do_not_overlap() {
        let path = std::env::temp_dir().join(format!(
            "voxel-session-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = WorldStore::create(&path, u64::MAX).unwrap();
        let mut chunk = Chunk::new(-1, 2, u64::MAX);
        chunk.blocks[3][42][7] = crate::block::BlockType::Stone;
        store.insert_generated(&chunk, 0).unwrap();
        chunk.blocks[3][42][7] = crate::block::BlockType::Lava;
        store.insert_generated(&chunk, 0).unwrap(); // must not replace existing terrain
        store.insert_generated(&chunk, 1).unwrap();
        drop(store);
        let store = WorldStore::open(&path).unwrap();
        assert_eq!(store.seed, u64::MAX);
        assert_eq!(
            store.read_chunk(-1, 2, 0).unwrap().unwrap().blocks[3][42][7],
            crate::block::BlockType::Stone
        );
        assert_eq!(
            store.read_chunk(-1, 2, 1).unwrap().unwrap().blocks[3][42][7],
            crate::block::BlockType::Lava
        );
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
