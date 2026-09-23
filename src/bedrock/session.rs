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
    // Data3D: 256 little-endian height values followed by 24 uniform biome storages.
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
    let biome: i32 = if dimension == 1 { 8 } else { 1 };
    for _ in 0..24 {
        biomes.push(1); // zero bits per entry; biome IDs use the runtime flag
        biomes.extend_from_slice(&biome.to_le_bytes());
    }
    vec![
        (key(0x2c), vec![42]),
        (key(0x36), 2i32.to_le_bytes().to_vec()),
        (key(0x2b), biomes),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
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
