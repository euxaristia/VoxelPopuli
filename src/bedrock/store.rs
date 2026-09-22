use super::nbt;
use crate::java_compat::NbtTag;
use bedrock_leveldb::{Db, OpenOptions, WriteBatch, WriteOptions};
use std::{
    io::{self, Write},
    path::Path,
};

pub struct Database {
    // Fields drop in declaration order: close the database before releasing LOCK.
    db: Db,
    _lock: super::lock::DatabaseLock,
}

impl std::ops::Deref for Database {
    type Target = Db;
    fn deref(&self) -> &Db {
        &self.db
    }
}

impl Database {
    pub fn open_writable(path: &Path) -> io::Result<Self> {
        read_level(path)?;
        open_database(path, false, false)
    }

    /// Create only in a new directory so a failed migration cannot replace a save.
    pub fn create(path: &Path, level: &NbtTag) -> io::Result<Self> {
        let bytes = encode_level(level)?;
        std::fs::create_dir(path)?;
        let result = (|| {
            std::fs::create_dir(path.join("db"))?;
            let database = open_database(path, false, true)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.join("level.dat"))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            Ok(database)
        })();
        if result.is_err() {
            // Exclusively created above, never an existing world directory.
            let _ = std::fs::remove_dir_all(path);
        }
        result
    }

    /// Commit complete records in one synced WAL transaction. Unknown keys remain untouched.
    pub fn commit(&self, records: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>) -> io::Result<()> {
        let mut batch = WriteBatch::new();
        for (key, value) in records {
            batch.put(key, value);
        }
        self.db
            .write(batch, WriteOptions { sync: true })
            .map_err(io::Error::other)
    }
}

fn encode_level(root: &NbtTag) -> io::Result<Vec<u8>> {
    if !matches!(root, NbtTag::Compound(_)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected compound level metadata",
        ));
    }
    let payload = nbt::encode("", root)?;
    let mut data = 10u32.to_le_bytes().to_vec();
    data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(&payload);
    Ok(data)
}

pub fn write_level(path: &Path, root: &NbtTag) -> io::Result<()> {
    let data = encode_level(root)?;
    let temporary = path.join(format!("level.dat.{}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path.join("level.dat"))?;
        #[cfg(unix)]
        std::fs::File::open(path)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

pub fn read_level(path: &Path) -> io::Result<NbtTag> {
    let data = std::fs::read(path.join("level.dat"))?;
    if data.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Truncated level.dat",
        ));
    }
    let version = u32::from_le_bytes(data[..4].try_into().unwrap());
    let len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    if version != 10 || len != data.len() - 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported level.dat header",
        ));
    }
    let (_, root, used) = nbt::decode(&data[8..])?;
    if used != len || !matches!(root, NbtTag::Compound(_)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid level.dat root",
        ));
    }
    Ok(root)
}

pub fn read_database(path: &Path) -> io::Result<Database> {
    open_database(path, true, false)
}

fn open_database(path: &Path, read_only: bool, create: bool) -> io::Result<Database> {
    let lock = super::lock::DatabaseLock::acquire(&path.join("db"))?;
    let db = Db::open(
        path.join("db"),
        OpenOptions {
            read_only,
            create_if_missing: create,
            ..Default::default()
        },
    )
    .map_err(io::Error::other)?;
    Ok(Database { db, _lock: lock })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock_leveldb::VisitorControl;
    #[test]
    fn native_transactions_survive_reopen_without_replacing_unknown_data() {
        let path = std::env::temp_dir().join(format!(
            "voxel-native-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let metadata = NbtTag::Compound(vec![
            ("RandomSeed".into(), NbtTag::Long(-42)),
            (
                "unrecognized_metadata".into(),
                NbtTag::String("preserve me".into()),
            ),
        ]);
        let db = Database::create(&path, &metadata).unwrap();
        assert!(Database::create(&path, &metadata).is_err());
        db.commit([
            (b"future:record".to_vec(), vec![255, 0, 128]),
            (b"player".to_vec(), vec![1]),
        ])
        .unwrap();
        // A rejected batch must not apply its earlier valid operations.
        assert!(
            db.commit([(b"player".to_vec(), vec![2]), (vec![], vec![3])])
                .is_err()
        );
        assert_eq!(db.get(b"player").unwrap().unwrap().as_ref(), &[1]);
        drop(db);
        let db = Database::open_writable(&path).unwrap();
        db.commit([(b"player".to_vec(), vec![4])]).unwrap();
        drop(db);
        let db = read_database(&path).unwrap();
        assert_eq!(read_level(&path).unwrap(), metadata);
        assert_eq!(
            db.get(b"future:record").unwrap().unwrap().as_ref(),
            &[255, 0, 128]
        );
        assert_eq!(db.get(b"player").unwrap().unwrap().as_ref(), &[4]);
        assert!(db.commit([(b"player".to_vec(), vec![5])]).is_err());
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    #[ignore = "requires isolated local Bedrock world copies"]
    fn inspect_real_bedrock_worlds_without_writes() {
        for name in ["world-1", "world-2"] {
            let path = Path::new("target/bedrock-fixtures").join(name);
            let root = read_level(&path).unwrap();
            assert!(root.get("RandomSeed").is_some());
            for key in [
                "StorageVersion",
                "lastOpenedWithVersion",
                "MinimumCompatibleClientVersion",
                "NetworkVersion",
            ] {
                println!("{name}: {key} {:?}", root.get(key));
            }
            let db = read_database(&path).unwrap();
            if let Some(player) = db.get(b"~local_player").unwrap() {
                let root = super::super::records::decode(&player).unwrap();
                if let Some(NbtTag::List { tags, .. }) = root.get("Pos") {
                    println!(
                        "{name}: player Y fraction {:?}, OnGround {:?}",
                        super::super::records::number(tags.get(1)).map(f64::fract),
                        root.get("OnGround")
                    );
                }
                for key in [
                    "Inventory",
                    "Armor",
                    "Attributes",
                    "Pos",
                    "Rotation",
                    "DimensionId",
                ] {
                    if let Some(NbtTag::List { tags, .. }) = root.get(key) {
                        let fields = tags.first().and_then(|tag| {
                            if let NbtTag::Compound(fields) = tag {
                                Some(
                                    fields
                                        .iter()
                                        .map(|(name, value)| (name.as_str(), value.id()))
                                        .collect::<Vec<_>>(),
                                )
                            } else {
                                None
                            }
                        });
                        println!(
                            "{name}: player {key} entries {}, first entry fields {fields:?}",
                            tags.len()
                        );
                    }
                }
            }
            let mut count = 0;
            let mut subchunks = 0;
            let mut inspected_version = false;
            let mut inspected_biomes = false;
            let mut sample_chunk = None;
            db.for_each_entry(Default::default(), |key, value| {
                count += 1;
                if matches!(key.len(), 9 | 13) && key[key.len() - 1] == 0x2c && !inspected_version {
                    println!("{name}: chunk version {value:?}");
                    inspected_version = true;
                }
                if matches!(key.len(), 9 | 13) && key[key.len() - 1] == 0x2b && !inspected_biomes {
                    println!(
                        "{name}: biome prefix {:?}",
                        &value[512..value.len().min(536)]
                    );
                    inspected_biomes = true;
                }
                if matches!(key.len(), 10 | 14) && key[key.len() - 2] == 0x2f {
                    if key.len() == 10 {
                        sample_chunk = Some((
                            i32::from_le_bytes(key[..4].try_into().unwrap()),
                            i32::from_le_bytes(key[4..8].try_into().unwrap()),
                        ));
                    }
                    let y = key[key.len() - 1] as i8;
                    let decoded = super::super::palette::Subchunk::decode(value, y).unwrap();
                    if subchunks == 0 {
                        if let Some(state) = decoded
                            .layers
                            .first()
                            .and_then(|layer| layer.palette.first())
                        {
                            println!("{name}: block state version {:?}", state.get("version"));
                        }
                    }
                    assert_eq!(
                        super::super::palette::Subchunk::decode(&decoded.encode().unwrap(), y)
                            .unwrap(),
                        decoded
                    );
                    subchunks += 1;
                }
                Ok(VisitorControl::Continue)
            })
            .unwrap();
            assert!(count > 0);
            assert!(subchunks > 0);
            if let Some((x, z)) = sample_chunk {
                if let (Some(chunk), Some(data)) = (
                    super::super::terrain::read_chunk(&db, x, z, 0, 0).unwrap(),
                    db.get(&super::super::palette::chunk_key(x, z, 0, 0x2b, None))
                        .unwrap(),
                ) {
                    println!(
                        "{name}: column top {:?}, stored height {}",
                        (0..256)
                            .rev()
                            .find(|&y| chunk.blocks[0][y][0] != crate::block::BlockType::Air),
                        i16::from_le_bytes(data[..2].try_into().unwrap())
                    );
                }
            }
            println!("{name}: {count} database records, {subchunks} round-tripped subchunks");
        }
    }
}
