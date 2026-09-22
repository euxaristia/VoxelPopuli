//! Engine block identities mapped to persistent Bedrock states.
//! Imported states are retained separately; this constructs states for new blocks.
use crate::{block::BlockType, java_compat::NbtTag};

pub const BLOCK_VERSION: i32 = 18_168_865;

pub fn state(block: BlockType, liquid_depth: u8) -> Option<NbtTag> {
    use BlockType::*;
    use NbtTag::{Byte, Int, String as Text};
    let identity = crate::java_compat::classic_java_block_state(block)?;
    let name = match block {
        SnowLayer => "minecraft:snow_layer",
        MobSpawner => "minecraft:mob_spawner",
        Bed => "minecraft:bed",
        OakDoor => "minecraft:wooden_door",
        PistonHead => "minecraft:piston_arm_collision",
        _ => identity.name,
    };
    let fields: Vec<(&str, NbtTag)> = match block {
        OakLog | SpruceLog => vec![("pillar_axis", Text("y".into()))],
        OakLeaves | SpruceLeaves => vec![("persistent_bit", Byte(0)), ("update_bit", Byte(0))],
        Bedrock => vec![("infiniburn_bit", Byte(0))],
        Water | Lava => vec![("liquid_depth", Int(i32::from(liquid_depth.min(15))))],
        SnowLayer => vec![("covered_bit", Byte(0)), ("height", Int(0))],
        TNT => vec![("explode_bit", Byte(0))],
        Furnace | Chest => vec![("minecraft:cardinal_direction", Text("north".into()))],
        Torch | RedstoneTorch => vec![("torch_facing_direction", Text("top".into()))],
        Cactus | Fire => vec![("age", Int(0))],
        Farmland => vec![("moisturized_amount", Int(7))],
        Wheat | WheatStage0 | WheatStage1 | WheatStage2 => vec![(
            "growth",
            Int(match block {
                WheatStage0 => 0,
                WheatStage1 => 2,
                WheatStage2 => 4,
                _ => 7,
            }),
        )],
        Bell => vec![
            ("attachment", Text("standing".into())),
            ("direction", Int(0)),
            ("toggle_bit", Byte(0)),
        ],
        Bed => vec![
            ("direction", Int(0)),
            ("head_piece_bit", Byte(0)),
            ("occupied_bit", Byte(0)),
        ],
        OakDoor | IronDoor => vec![
            ("door_hinge_bit", Byte(0)),
            ("minecraft:cardinal_direction", Text("north".into())),
            ("open_bit", Byte(0)),
            ("upper_block_bit", Byte(0)),
        ],
        Lever => vec![
            ("lever_direction", Text("up_north_south".into())),
            ("open_bit", Byte(0)),
        ],
        StoneButton => vec![
            ("button_pressed_bit", Byte(0)),
            ("facing_direction", Int(1)),
        ],
        RedstoneWire => vec![("redstone_signal", Int(0))],
        Piston | StickyPiston | PistonHead => vec![("facing_direction", Int(2))],
        _ => vec![],
    };
    Some(NbtTag::Compound(vec![
        ("name".into(), Text(name.into())),
        (
            "states".into(),
            NbtTag::Compound(
                fields
                    .into_iter()
                    .map(|(key, value)| (key.into(), value))
                    .collect(),
            ),
        ),
        ("version".into(), Int(BLOCK_VERSION)),
    ]))
}

/// None means unsupported, never air. Callers must retain the original state.
pub fn block(state: &NbtTag) -> Option<BlockType> {
    let NbtTag::String(name) = state.get("name")? else {
        return None;
    };
    if name == "minecraft:wheat" {
        let growth = state.get("states")?.get("growth")?;
        return Some(match growth {
            NbtTag::Int(0..=1) => BlockType::WheatStage0,
            NbtTag::Int(2..=3) => BlockType::WheatStage1,
            NbtTag::Int(4..=6) => BlockType::WheatStage2,
            NbtTag::Int(7) => BlockType::Wheat,
            _ => return None,
        });
    }
    // Palette-level conversion, not a per-voxel search.
    (0..BlockType::COUNT)
        .map(|id| BlockType::from_u16(id as u16))
        .find(|candidate| {
            self::state(*candidate, 0).and_then(|tag| tag.get("name").cloned())
                == Some(NbtTag::String(name.clone()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_block_identities_round_trip_and_unknown_is_not_air() {
        for id in 0..BlockType::COUNT {
            let original = BlockType::from_u16(id as u16);
            if let Some(encoded) = state(original, 0) {
                let expected = if original == BlockType::SnowyGrass {
                    BlockType::Grass
                } else {
                    original
                };
                assert_eq!(block(&encoded), Some(expected), "{original:?}");
            }
        }
        let unknown = NbtTag::Compound(vec![(
            "name".into(),
            NbtTag::String("addon:custom_block".into()),
        )]);
        assert_eq!(block(&unknown), None);
        assert!(state(BlockType::IronPickaxe, 0).is_none());
    }
    #[test]
    fn bedrock_uses_typed_properties_and_distinct_identifiers() {
        let log = state(BlockType::OakLog, 0).unwrap();
        assert_eq!(
            log.get("states").unwrap().get("pillar_axis"),
            Some(&NbtTag::String("y".into()))
        );
        let leaves = state(BlockType::OakLeaves, 0).unwrap();
        assert_eq!(
            leaves.get("states").unwrap().get("persistent_bit"),
            Some(&NbtTag::Byte(0))
        );
        let snow = state(BlockType::SnowLayer, 0).unwrap();
        assert_eq!(
            snow.get("name"),
            Some(&NbtTag::String("minecraft:snow_layer".into()))
        );
        let water = state(BlockType::Water, 11).unwrap();
        assert_eq!(
            water.get("states").unwrap().get("liquid_depth"),
            Some(&NbtTag::Int(11))
        );
    }
}
