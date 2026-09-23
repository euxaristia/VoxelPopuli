use super::*;
use crate::{
    bedrock::{records as n, session::WorldStore},
    java_compat::NbtTag,
};

const SESSION_KEY: &[u8] = b"voxelpopuli:session";
const PLAYER_KEY: &[u8] = b"~local_player";

fn floats(values: impl IntoIterator<Item = f32>) -> NbtTag {
    NbtTag::List {
        element_type: 5,
        tags: values.into_iter().map(NbtTag::Float).collect(),
    }
}

fn vector<const N: usize>(tag: Option<&NbtTag>) -> io::Result<[f32; N]> {
    let Some(NbtTag::List { tags, .. }) = tag else {
        return Err(n::invalid("Missing player vector"));
    };
    if tags.len() != N {
        return Err(n::invalid("Invalid player vector length"));
    }
    let mut result = [0.0; N];
    for (out, value) in result.iter_mut().zip(tags) {
        *out =
            n::number(Some(value)).ok_or_else(|| n::invalid("Invalid player coordinate"))? as f32;
        if !out.is_finite() {
            return Err(n::invalid("Player coordinate outside supported range"));
        }
    }
    Ok(result)
}

impl GameSave {
    /// Materialize every edited chunk into a new native world, retaining the legacy
    /// file. The destination becomes visible only after all records are durable.
    pub fn migrate_to_bedrock(&self, destination: &Path) -> io::Result<()> {
        if destination.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Native world destination already exists",
            ));
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let temporary = parent.join(format!(
            ".voxel-migration-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let result = (|| {
            let store = WorldStore::create(&temporary, self.seed)?;
            let mut chunks =
                std::collections::BTreeMap::<(i32, i32), Vec<((i32, i32, i32), BlockType)>>::new();
            let position = self.player.position;
            chunks
                .entry((
                    (position.x.floor() as i32).div_euclid(16),
                    (position.z.floor() as i32).div_euclid(16),
                ))
                .or_default();
            for &(position, block) in &self.edits {
                if !(0..256).contains(&position.1) {
                    return Err(n::invalid("Legacy edit outside supported terrain height"));
                }
                chunks
                    .entry((position.0.div_euclid(16), position.2.div_euclid(16)))
                    .or_default()
                    .push((position, block));
            }
            for &((x, _, z), _) in &self.containers {
                chunks
                    .entry((x.div_euclid(16), z.div_euclid(16)))
                    .or_default();
            }
            for ((x, z), edits) in chunks {
                let imported = self
                    .import_world
                    .as_ref()
                    .map(|path| crate::java_compat::import_classic_java_chunk(path, x, z))
                    .transpose()?
                    .flatten();
                let mut chunk = imported.unwrap_or_else(|| {
                    let mut chunk = crate::chunk::Chunk::new(x, z, self.seed);
                    chunk.generator_version = self.generator_version;
                    chunk.generate();
                    chunk
                });
                for ((x, y, z), block) in edits {
                    chunk.set_block(
                        x.rem_euclid(16) as usize,
                        y as usize,
                        z.rem_euclid(16) as usize,
                        block,
                    );
                }
                store.insert_generated(&chunk, 0)?;
            }
            store.save_chunks(std::iter::empty(), 0, self.bedrock_records(&store)?)?;
            self.update_bedrock_metadata(&store)?;
            drop(store);
            if destination.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Native world destination was created during migration",
                ));
            }
            std::fs::rename(&temporary, destination)?;
            #[cfg(unix)]
            std::fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() && temporary.exists() {
            let _ = std::fs::remove_dir_all(&temporary);
        }
        result
    }

    pub fn write_bedrock(&self, world: &World) -> io::Result<()> {
        let store = world
            .bedrock
            .as_ref()
            .ok_or_else(|| io::Error::other("No native world storage attached"))?;
        if let Some(error) = &world.storage_error {
            return Err(io::Error::other(error.clone()));
        }
        store.save_chunks(
            world.chunks.iter().filter_map(|chunk| chunk.as_deref()),
            world.dimension,
            self.bedrock_records(store)?,
        )?;
        self.update_bedrock_metadata(store)
    }
    fn update_bedrock_metadata(&self, store: &WorldStore) -> io::Result<()> {
        store.update_metadata([
            (
                "voxelpopuli:generator_version",
                NbtTag::Int(self.generator_version as i32),
            ),
            (
                "Time",
                NbtTag::Long(
                    self.day_count
                        .saturating_mul(24000)
                        .saturating_add((self.day_time * 20.0) as u64)
                        .min(i64::MAX as u64) as i64,
                ),
            ),
            ("GameType", NbtTag::Int(i32::from(self.player.sandbox))),
            ("Difficulty", NbtTag::Int(self.difficulty as i32)),
        ])
    }

    pub fn read_bedrock(store: &WorldStore) -> io::Result<Self> {
        let mut save = if let Some(bytes) = store.record(SESSION_KEY)? {
            Self::decode(&bytes)?
        } else {
            let mut player =
                Player::new(n::number(store.metadata.get("SpawnY")).unwrap_or(160.0) as f32);
            player.position.x = n::number(store.metadata.get("SpawnX")).unwrap_or(0.0) as f32 + 0.5;
            player.position.z = n::number(store.metadata.get("SpawnZ")).unwrap_or(0.0) as f32 + 0.5;
            let mut world = World::simulation(store.seed);
            if let Some(NbtTag::Long(ticks)) = store.metadata.get("Time") {
                let ticks = (*ticks).max(0) as u64;
                world.day_count = ticks / 24000;
                world.day_time = (ticks % 24000) as f32 / 20.0;
            }
            world.generator_version = match store.metadata.get("voxelpopuli:generator_version") {
                None => crate::chunk::GeneratorVersion::Legacy,
                Some(NbtTag::Int(value)) => crate::chunk::GeneratorVersion::from_u32(*value as u32)
                    .ok_or_else(|| n::invalid("Unsupported terrain generator"))?,
                _ => return Err(n::invalid("Invalid terrain generator metadata")),
            };
            Self::capture(
                &world,
                &player,
                &[None; INVENTORY_SLOT_COUNT],
                Vec2::ZERO,
                GameSettings::default(),
                None,
                None,
                &[None; crate::inventory::CRAFT_TABLE_SLOT_COUNT],
            )?
        };
        if save.seed != store.seed {
            return Err(n::invalid("World metadata and session seed disagree"));
        }
        // Containers are read from authoritative native block entities as chunks stream.
        save.containers.clear();
        if let Some(bytes) = store.record(PLAYER_KEY)? {
            let root = n::decode(&bytes)?;
            if n::number(root.get("DimensionId")).unwrap_or(0.0) != 0.0 {
                return Err(n::invalid(
                    "This player's dimension is not yet supported by the game loader",
                ));
            }
            save.player.position = Vec3::from_array(vector(root.get("Pos"))?) - Vec3::Y * 1.62;
            if !(0.0..256.0).contains(&save.player.position.y) {
                return Err(n::invalid(
                    "Player is outside the current simulation height (Y 0..255); world left intact",
                ));
            }
            if let Some(motion) = root.get("Motion") {
                save.player.velocity = Vec3::from_array(vector(Some(motion))?) * 20.0;
            }
            if let Some(rotation) = root.get("Rotation") {
                let [yaw, pitch] = vector(Some(rotation))?;
                save.camera_angle = Vec2::new(-yaw.to_radians(), -pitch.to_radians());
            }
            if let Some(inventory) = root.get("Inventory") {
                n::decode_inventory(inventory, &mut save.inventory[..36])?;
            }
            if let Some(NbtTag::List { tags, .. }) = root.get("Armor") {
                save.player.equipped_armor = [None; 4];
                for (slot, tag) in tags.iter().enumerate() {
                    let mut tag = tag.clone();
                    n::set(&mut tag, "Slot", NbtTag::Byte(slot as i8))?;
                    if let Some((_, item)) = n::decode_item(&tag)? {
                        if slot >= 4 {
                            return Err(n::invalid("Unsupported additional armor slot"));
                        }
                        save.inventory[36 + slot] = Some(item);
                        save.player.equipped_armor[slot] =
                            Some((item.block, item.durability.unwrap_or(0)));
                    } else if slot < 4 {
                        save.inventory[36 + slot] = None;
                    }
                }
            }
            if let Some(NbtTag::List { tags, .. }) = root.get("Offhand") {
                if tags
                    .iter()
                    .any(|tag| n::number(tag.get("Count")).unwrap_or(0.0) != 0.0)
                {
                    return Err(n::invalid(
                        "Offhand items are not yet supported; move the item to the main inventory in Bedrock first",
                    ));
                }
            }
            if let Some(NbtTag::List { tags, .. }) = root.get("Attributes") {
                for attribute in tags {
                    let Some(NbtTag::String(name)) = attribute.get("Name") else {
                        continue;
                    };
                    let Some(value) = n::number(attribute.get("Current")) else {
                        continue;
                    };
                    match name.as_str() {
                        "minecraft:health" => save.player.health = (value as i32).clamp(0, 20),
                        "minecraft:player.hunger" => {
                            save.player.hunger = (value as i32).clamp(0, 20)
                        }
                        "minecraft:player.saturation" => {
                            save.player.saturation = (value as f32).clamp(0.0, 20.0)
                        }
                        "minecraft:player.exhaustion" => {
                            save.player.exhaustion = (value as f32).clamp(0.0, 3.999)
                        }
                        _ => {}
                    }
                }
            }
            save.player.selected_slot =
                (n::number(root.get("SelectedInventorySlot")).unwrap_or(0.0) as usize).min(8);
            save.player.xp_level =
                n::number(root.get("PlayerLevel")).unwrap_or(0.0).max(0.0) as u32;
            save.player.xp_progress =
                (n::number(root.get("PlayerLevelProgress")).unwrap_or(0.0) as f32).clamp(0.0, 1.0);
            save.player.sandbox = n::number(root.get("PlayerGameMode")).unwrap_or(0.0) == 1.0;
        }
        Ok(save)
    }

    pub fn bedrock_records(&self, store: &WorldStore) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut player = store
            .record(PLAYER_KEY)?
            .map(|bytes| n::decode(&bytes))
            .transpose()?
            .unwrap_or_else(|| NbtTag::Compound(vec![]));
        n::set(
            &mut player,
            "Pos",
            floats((self.player.position + Vec3::Y * 1.62).to_array()),
        )?;
        n::set(
            &mut player,
            "Motion",
            floats((self.player.velocity / 20.0).to_array()),
        )?;
        n::set(
            &mut player,
            "Rotation",
            floats([
                -self.camera_angle.x.to_degrees(),
                -self.camera_angle.y.to_degrees(),
            ]),
        )?;
        n::set(
            &mut player,
            "Inventory",
            n::inventory(&self.inventory[..36])?,
        )?;
        let armor = &self.inventory[36..40];
        let mut tags = Vec::new();
        for (i, stack) in armor.iter().enumerate() {
            tags.push(if let Some(stack) = stack {
                n::item(*stack, i as u8)?
            } else {
                NbtTag::Compound(vec![
                    ("Count".into(), NbtTag::Byte(0)),
                    ("Name".into(), NbtTag::String(String::new())),
                    ("Damage".into(), NbtTag::Short(0)),
                ])
            });
        }
        n::set(
            &mut player,
            "Armor",
            NbtTag::List {
                element_type: 10,
                tags,
            },
        )?;
        let mut attributes = match player.get("Attributes") {
            Some(NbtTag::List { tags, .. }) => tags.clone(),
            _ => vec![],
        };
        for (name, current, max) in [
            ("minecraft:health", self.player.health as f32, 20.0),
            ("minecraft:player.hunger", self.player.hunger as f32, 20.0),
            ("minecraft:player.saturation", self.player.saturation, 20.0),
            ("minecraft:player.exhaustion", self.player.exhaustion, 4.0),
        ] {
            if let Some(attribute) = attributes
                .iter_mut()
                .find(|a| a.get("Name") == Some(&NbtTag::String(name.into())))
            {
                n::set(attribute, "Current", NbtTag::Float(current))?;
            } else {
                attributes.push(NbtTag::Compound(vec![
                    ("Name".into(), NbtTag::String(name.into())),
                    ("Current".into(), NbtTag::Float(current)),
                    ("Base".into(), NbtTag::Float(max)),
                    ("Min".into(), NbtTag::Float(0.0)),
                    ("Max".into(), NbtTag::Float(max)),
                    ("DefaultMin".into(), NbtTag::Float(0.0)),
                    ("DefaultMax".into(), NbtTag::Float(max)),
                ]));
            }
        }
        n::set(
            &mut player,
            "Attributes",
            NbtTag::List {
                element_type: 10,
                tags: attributes,
            },
        )?;
        n::set(&mut player, "DimensionId", NbtTag::Int(0))?;
        n::set(
            &mut player,
            "SelectedInventorySlot",
            NbtTag::Int(self.player.selected_slot as i32),
        )?;
        n::set(
            &mut player,
            "PlayerGameMode",
            NbtTag::Int(i32::from(self.player.sandbox)),
        )?;
        n::set(
            &mut player,
            "PlayerLevel",
            NbtTag::Int(self.player.xp_level as i32),
        )?;
        n::set(
            &mut player,
            "PlayerLevelProgress",
            NbtTag::Float(self.player.xp_progress),
        )?;
        if player.get("UniqueID").is_none() {
            n::set(&mut player, "UniqueID", NbtTag::Long(-1))?;
        }
        // Non-Bedrock UI state (crafting cursor/settings) is an additional namespaced
        // record. The standard player and terrain records remain authoritative.
        let mut session = self.clone();
        session.edits.clear();
        let mut records = vec![
            (SESSION_KEY.to_vec(), session.encode()?),
            (PLAYER_KEY.to_vec(), n::encode(&player)?),
        ];
        let mut affected = std::collections::BTreeSet::new();
        let containers: std::collections::HashMap<_, _> = self
            .containers
            .iter()
            .map(|(position, container)| (*position, container))
            .collect();
        let edits: std::collections::HashMap<_, _> = self.edits.iter().copied().collect();
        for &(x, _, z) in containers.keys().chain(edits.keys()) {
            affected.insert((x.div_euclid(16), z.div_euclid(16)));
        }
        for (x, z) in affected {
            let key = crate::bedrock::palette::chunk_key(x, z, 0, 0x31, None);
            let existing = store
                .record(&key)?
                .map(|bytes| n::decode_stream(&bytes))
                .transpose()?
                .unwrap_or_default();
            let mut merged = Vec::new();
            let mut written = std::collections::HashSet::new();
            for mut record in existing {
                let position = n::position(&record)?;
                if let Some(container) = containers.get(&position) {
                    n::container(&mut record, position, container)?;
                    written.insert(position);
                } else if let Some(replacement) = edits.get(&position) {
                    let expected = match record.get("id") {
                        Some(NbtTag::String(id)) if id == "Chest" => Some(BlockType::Chest),
                        Some(NbtTag::String(id)) if id == "Furnace" => Some(BlockType::Furnace),
                        _ => None,
                    };
                    if expected.is_some_and(|expected| expected != *replacement) {
                        continue;
                    }
                }
                merged.extend_from_slice(&n::encode(&record)?);
            }
            for (&position, container) in &containers {
                if position.0.div_euclid(16) == x
                    && position.2.div_euclid(16) == z
                    && !written.contains(&position)
                {
                    let mut record = NbtTag::Compound(vec![]);
                    n::container(&mut record, position, container)?;
                    merged.extend_from_slice(&n::encode(&record)?);
                }
            }
            if !merged.is_empty() || store.record(&key)?.is_some() {
                records.push((key, merged));
            }
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_legacy_bytes_and_materializes_negative_coordinate_edits() {
        let root = std::env::temp_dir().join(format!(
            "voxel-migrate-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut world = World::simulation(42);
        world.install_edits(&[((-17, 80, -1), BlockType::DiamondOre)]);
        let save = GameSave::capture(
            &world,
            &Player::new(130.0),
            &[None; INVENTORY_SLOT_COUNT],
            Vec2::ZERO,
            GameSettings::default(),
            None,
            None,
            &[None; 10],
        )
        .unwrap();
        let legacy = root.join("world.vps");
        save.write_to(&legacy).unwrap();
        let before = std::fs::read(&legacy).unwrap();
        let destination = root.join("native");
        save.migrate_to_bedrock(&destination).unwrap();
        assert_eq!(std::fs::read(&legacy).unwrap(), before);
        assert!(save.migrate_to_bedrock(&destination).is_err());
        let store = WorldStore::open(&destination).unwrap();
        assert_eq!(
            store.read_chunk(-2, -1, 0).unwrap().unwrap().blocks[15][80][15],
            BlockType::DiamondOre
        );
        assert!(GameSave::read_bedrock(&store).unwrap().edits.is_empty());
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_player_records_round_trip_and_external_inventory_is_authoritative() {
        let path = std::env::temp_dir().join(format!(
            "voxel-player-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = WorldStore::create(&path, 42).unwrap();
        let mut player = Player::new(70.0);
        player.health = 12;
        let mut inventory = [None; INVENTORY_SLOT_COUNT];
        inventory[3] = Some(ItemStack::new(BlockType::Stone, 17));
        let save = GameSave::capture(
            &World::simulation(42),
            &player,
            &inventory,
            Vec2::new(0.2, -0.3),
            GameSettings::default(),
            None,
            None,
            &[None; 10],
        )
        .unwrap();
        store
            .save_chunks(std::iter::empty(), 0, save.bedrock_records(&store).unwrap())
            .unwrap();
        let loaded = GameSave::read_bedrock(&store).unwrap();
        assert_eq!(loaded.inventory, inventory);
        assert_eq!(loaded.player.health, 12);
        assert_eq!(loaded.player.position, player.position);
        assert!(loaded.camera_angle.abs_diff_eq(save.camera_angle, 0.00001));
        let mut native = n::decode(&store.record(PLAYER_KEY).unwrap().unwrap()).unwrap();
        assert!((vector::<3>(native.get("Pos")).unwrap()[1] - 71.62).abs() < 0.00001);
        inventory[3] = Some(ItemStack::new(BlockType::Dirt, 8));
        n::set(
            &mut native,
            "Inventory",
            n::inventory(&inventory[..36]).unwrap(),
        )
        .unwrap();
        store
            .save_chunks(
                std::iter::empty(),
                0,
                vec![(PLAYER_KEY.to_vec(), n::encode(&native).unwrap())],
            )
            .unwrap();
        assert_eq!(
            GameSave::read_bedrock(&store).unwrap().inventory[3],
            inventory[3]
        );
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
