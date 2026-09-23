use super::{
    blocks,
    palette::{Storage, Subchunk, chunk_key},
    store::Database,
};
use crate::{
    block::BlockType,
    chunk::{CHUNK_HEIGHT, Chunk},
    java_compat::NbtTag,
};
use std::{collections::HashMap, io};

fn cell(index: usize, section: usize) -> (usize, usize, usize) {
    (index / 256, section * 16 + index % 16, (index / 16) % 16)
}

fn depth(state: &NbtTag) -> u8 {
    match state
        .get("states")
        .and_then(|states| states.get("liquid_depth"))
    {
        Some(NbtTag::Int(n @ 0..=15)) => *n as u8,
        _ => 0,
    }
}

/// The current simulation uses Y 0..255. Other sections remain untouched in storage.
#[cfg(test)]
pub fn read_chunk(
    db: &Database,
    x: i32,
    z: i32,
    dimension: i32,
    seed: u64,
) -> io::Result<Option<Chunk>> {
    read_records(db, x, z, dimension)?.decode(x, z, seed)
}

pub struct ChunkRecords {
    records: Vec<Option<Vec<u8>>>,
}

pub fn read_records(db: &Database, x: i32, z: i32, dimension: i32) -> io::Result<ChunkRecords> {
    let keys = std::iter::once(chunk_key(x, z, dimension, 0x2c, None)).chain(
        (0..CHUNK_HEIGHT / 16).map(|section| chunk_key(x, z, dimension, 0x2f, Some(section as i8))),
    );
    Ok(ChunkRecords {
        records: db.get_many(keys)?,
    })
}

impl ChunkRecords {
    pub fn exists(&self) -> bool {
        self.records.iter().any(Option::is_some)
    }

    pub fn decode(self, x: i32, z: i32, seed: u64) -> io::Result<Option<Chunk>> {
        if !self.exists() {
            return Ok(None);
        }
        let mut chunk = Chunk::new(x, z, seed);
        for (section, bytes) in self.records.into_iter().skip(1).enumerate() {
            let Some(bytes) = bytes else {
                continue;
            };
            let subchunk = Subchunk::decode(&bytes, section as i8)?;
            let Some(layer) = subchunk.layers.first() else {
                continue;
            };
            let palette: Vec<_> = layer
                .palette
                .iter()
                .map(|state| {
                    (
                        blocks::block(state).unwrap_or(BlockType::Bedrock),
                        depth(state),
                    )
                })
                .collect();
            for (index, &entry) in layer.indices.iter().enumerate() {
                let (x, y, z) = cell(index, section);
                let (block, depth) = palette[entry as usize];
                chunk.blocks[x][y][z] = block;
                if matches!(block, BlockType::Water | BlockType::Lava) {
                    chunk.liquid_levels[x][y][z] = depth + 1;
                }
            }
        }
        chunk.calculate_lighting();
        Ok(Some(chunk))
    }
}

/// Prepare replacement records without writing. Preserve untouched states, extra
/// storage layers, and every section outside the simulation's vertical range.
pub fn chunk_records(
    db: &Database,
    chunk: &Chunk,
    dimension: i32,
) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut records = Vec::new();
    let keys: Vec<_> = (0..CHUNK_HEIGHT / 16)
        .map(|section| chunk_key(chunk.x, chunk.z, dimension, 0x2f, Some(section as i8)))
        .collect();
    let originals = db.get_many(keys.iter().cloned())?;
    for (section, (key, original)) in keys.into_iter().zip(originals).enumerate() {
        let mut subchunk = if let Some(bytes) = original.as_ref() {
            Subchunk::decode(bytes, section as i8)?
        } else {
            Subchunk {
                y: section as i8,
                layers: vec![],
            }
        };
        if subchunk.layers.is_empty() {
            subchunk.layers.push(Storage {
                palette: vec![blocks::state(BlockType::Air, 0).unwrap()],
                indices: vec![0; 4096],
            });
        }
        let layer = &mut subchunk.layers[0];
        let old: Vec<_> = layer
            .palette
            .iter()
            .map(|state| {
                (
                    blocks::block(state).unwrap_or(BlockType::Bedrock),
                    depth(state),
                )
            })
            .collect();
        let mut replacements = HashMap::new();
        let mut changed = original.is_none();
        for index in 0..4096 {
            let (x, y, z) = cell(index, section);
            let block = chunk.blocks[x][y][z];
            let liquid = if matches!(block, BlockType::Water | BlockType::Lava) {
                chunk.liquid_levels[x][y][z].saturating_sub(1).min(15)
            } else {
                0
            };
            if old[layer.indices[index] as usize] == (block, liquid) {
                continue;
            }
            changed = true;
            let id = if let Some(&id) = replacements.get(&(block, liquid)) {
                id
            } else {
                let state = blocks::state(block, liquid).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Item cannot be stored as a terrain block",
                    )
                })?;
                let id = layer
                    .palette
                    .iter()
                    .position(|entry| *entry == state)
                    .unwrap_or_else(|| {
                        layer.palette.push(state);
                        layer.palette.len() - 1
                    }) as u16;
                replacements.insert((block, liquid), id);
                id
            };
            layer.indices[index] = id;
        }
        if changed {
            // Remove unused palette slots after replacement to keep the wire palette <=4096.
            let mut compact = Vec::new();
            let mut ids = HashMap::new();
            for id in &mut layer.indices {
                *id = *ids.entry(*id).or_insert_with(|| {
                    compact.push(layer.palette[*id as usize].clone());
                    (compact.len() - 1) as u16
                });
            }
            layer.palette = compact;
            records.push((key, subchunk.encode()?));
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terrain_preserves_unknown_states_layers_and_external_heights() {
        let path = std::env::temp_dir().join(format!(
            "voxel-terrain-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let db = Database::create(&path, &NbtTag::Compound(vec![])).unwrap();
        let unknown = NbtTag::Compound(vec![
            ("name".into(), NbtTag::String("addon:unimplemented".into())),
            (
                "states".into(),
                NbtTag::Compound(vec![("custom".into(), NbtTag::Int(12))]),
            ),
        ]);
        let layer = Storage {
            palette: vec![unknown.clone()],
            indices: vec![0; 4096],
        };
        let original = Subchunk {
            y: 0,
            layers: vec![layer.clone(), layer.clone()],
        };
        let outside = Subchunk {
            y: -4,
            layers: vec![layer],
        }
        .encode()
        .unwrap();
        db.commit([
            (
                chunk_key(-2, -3, 0, 0x2f, Some(0)),
                original.encode().unwrap(),
            ),
            (chunk_key(-2, -3, 0, 0x2f, Some(-4)), outside.clone()),
        ])
        .unwrap();
        db.flush().unwrap();
        let mut chunk = read_chunk(&db, -2, -3, 0, 1).unwrap().unwrap();
        assert_eq!(crate::chunk::unpack_light(chunk.light[0][255][0]).0, 15);
        assert_eq!(chunk.blocks[0][0][0], BlockType::Bedrock);
        chunk.blocks[1][2][3] = BlockType::Stone;
        chunk.blocks[3][17][4] = BlockType::Water;
        chunk.liquid_levels[3][17][4] = 12;
        db.commit(chunk_records(&db, &chunk, 0).unwrap()).unwrap();
        let decoded = Subchunk::decode(
            &db.get(&chunk_key(-2, -3, 0, 0x2f, Some(0)))
                .unwrap()
                .unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(decoded.layers[1], original.layers[1]);
        assert_eq!(
            decoded.layers[0].palette[decoded.layers[0].indices[0] as usize],
            unknown
        );
        assert_eq!(
            db.get(&chunk_key(-2, -3, 0, 0x2f, Some(-4)))
                .unwrap()
                .unwrap()
                .as_slice(),
            outside
        );
        let restored = read_chunk(&db, -2, -3, 0, 1).unwrap().unwrap();
        assert_eq!(restored.blocks[1][2][3], BlockType::Stone);
        assert_eq!(restored.liquid_levels[3][17][4], 12);
        assert!(read_chunk(&db, -2, -3, 1, 1).unwrap().is_none());
        assert!(chunk_records(&db, &restored, 0).unwrap().is_empty());
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
}
