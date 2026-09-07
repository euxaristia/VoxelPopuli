//! Overworld encounter roster, checked against Bedrock 26.40 (September 2026).
//! Stats are balanced for this game's combat; identifiers follow Bedrock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Person,
    Villager,
    Golem,
    Creeper,
    Grazer,
    Horse,
    Cat,
    Canine,
    Bear,
    Rabbit,
    Bird,
    Bat,
    Bee,
    Spider,
    Fish,
    Squid,
    Nautilus,
    Turtle,
    Frog,
    Slime,
    Guardian,
    Sniffer,
    Creaking,
    Breeze,
    Sprite,
    Ghast,
    Wither,
    Armadillo,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Walk,
    Swim,
    Fly,
    Hop,
    Amphibious,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Temper {
    Passive,
    Neutral,
    Hostile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Habitat {
    Grassland,
    Desert,
    Snow,
    Taiga,
    Mountains,
    Ocean,
    River,
    Cave,
    Night,
    Village,
    Special,
}

/// The terrain generator has six biomes. Structure-only species stay in the
/// sandbox catalogue until their structures are implemented.
pub fn can_spawn(
    kind: MobKind,
    biome: crate::chunk::Biome,
    water: bool,
    cave: bool,
    night: bool,
) -> bool {
    use crate::chunk::Biome;
    let species = kind.species();
    if species.habitat == Habitat::Special {
        return false;
    }
    if matches!(species.motion, Motion::Swim) && !water {
        return false;
    }
    if water && !matches!(species.motion, Motion::Swim | Motion::Amphibious) {
        return false;
    }
    if species.temper == Temper::Hostile && !cave && !night && !water {
        return false;
    }
    if cave {
        return species.habitat == Habitat::Cave
            || (!water && species.habitat == Habitat::Night && species.temper == Temper::Hostile);
    }
    match species.habitat {
        Habitat::Grassland => !water && matches!(biome, Biome::Plains | Biome::SnowyTaiga),
        Habitat::Desert => !water && biome == Biome::Desert,
        Habitat::Snow => !water && matches!(biome, Biome::SnowyTundra | Biome::SnowyTaiga),
        Habitat::Taiga => !water && biome == Biome::SnowyTaiga,
        Habitat::Mountains => !water && matches!(biome, Biome::Mountains | Biome::HighHills),
        Habitat::Ocean | Habitat::River => water,
        Habitat::Night => night && !water,
        Habitat::Cave | Habitat::Village | Habitat::Special => false,
    }
}

pub struct Species {
    pub name: &'static str,
    pub id: &'static str,
    pub shape: Shape,
    pub motion: Motion,
    pub temper: Temper,
    pub habitat: Habitat,
    pub height: f32,
    pub width: f32,
    pub health: f32,
    pub speed: f32,
    pub damage: i32,
    pub coat: [u8; 3],
    pub accent: [u8; 3],
}

macro_rules! species {
    ($($kind:ident, $name:literal, $id:literal, $shape:ident, $motion:ident, $temper:ident, $habitat:ident, $h:literal, $w:literal, $hp:literal, $speed:literal, $damage:literal, $coat:expr, $accent:expr;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum MobKind { $($kind,)+ }
        impl MobKind {
            pub const ALL: &'static [Self] = &[$(Self::$kind,)+];
            pub fn species(self) -> &'static Species {
                &SPECIES[self as usize]
            }
            pub fn from_id(id: &str) -> Option<Self> {
                let id = id.strip_prefix("minecraft:").unwrap_or(id);
                Self::ALL.iter().copied().find(|kind| kind.species().id == id)
            }
        }
        pub const SPECIES: &[Species] = &[$(Species {
            name: $name, id: $id, shape: Shape::$shape, motion: Motion::$motion,
            temper: Temper::$temper, habitat: Habitat::$habitat,
            height: $h, width: $w, health: $hp, speed: $speed, damage: $damage,
            coat: $coat, accent: $accent,
        },)+];
    }
}

species! {
    Villager, "Villager", "villager", Villager, Walk, Passive, Village, 1.8, 0.56, 20.0, 1.7, 0, [153,107,70], [203,157,116];
    Golem, "Iron Golem", "iron_golem", Golem, Walk, Neutral, Village, 2.5, 1.1, 100.0, 1.15, 12, [203,202,181], [74,115,65];
    Zombie, "Zombie", "zombie", Person, Walk, Hostile, Night, 1.95, 0.6, 20.0, 1.5, 3, [55,139,144], [100,139,74];
    Skeleton, "Skeleton", "skeleton", Person, Walk, Hostile, Night, 1.99, 0.6, 20.0, 1.6, 4, [210,207,185], [105,101,88];
    Creeper, "Creeper", "creeper", Creeper, Walk, Hostile, Night, 1.7, 0.6, 20.0, 1.4, 0, [84,159,66], [37,69,35];
    Pig, "Pig", "pig", Grazer, Walk, Passive, Grassland, 0.9, 0.9, 10.0, 2.6875, 0, [224,153,157], [177,104,118];
    Cow, "Cow", "cow", Grazer, Walk, Passive, Grassland, 1.4, 0.9, 10.0, 2.15, 0, [87,64,49], [231,226,209];
    Sheep, "Sheep", "sheep", Grazer, Walk, Passive, Grassland, 1.3, 0.9, 8.0, 2.4725, 0, [227,225,209], [157,129,105];
    Allay, "Allay", "allay", Sprite, Fly, Passive, Special, 0.6, 0.35, 20.0, 2.1, 0, [83,193,225], [170,244,244];
    Armadillo, "Armadillo", "armadillo", Armadillo, Walk, Passive, Special, 0.65, 0.7, 12.0, 1.2, 0, [171,124,103], [222,165,145];
    Axolotl, "Axolotl", "axolotl", Canine, Amphibious, Passive, Special, 0.42, 0.6, 14.0, 1.5, 2, [238,175,190], [183,60,110];
    Bat, "Bat", "bat", Bat, Fly, Passive, Cave, 0.8, 0.5, 6.0, 1.6, 0, [87,61,48], [155,116,84];
    Bee, "Bee", "bee", Bee, Fly, Neutral, Grassland, 0.6, 0.7, 10.0, 1.6, 2, [232,181,58], [79,56,38];
    Bogged, "Bogged", "bogged", Person, Walk, Hostile, Special, 1.99, 0.6, 16.0, 1.5, 4, [140,146,102], [76,116,66];
    Breeze, "Breeze", "breeze", Breeze, Hop, Hostile, Special, 1.77, 0.6, 30.0, 2.0, 3, [143,170,169], [89,114,111];
    Camel, "Camel", "camel", Horse, Walk, Passive, Desert, 2.375, 1.7, 32.0, 2.0, 0, [198,160,101], [115,85,55];
    CamelHusk, "Camel Husk", "camel_husk", Horse, Walk, Passive, Desert, 2.375, 1.7, 32.0, 2.0, 0, [159,136,79], [84,80,45];
    Cat, "Cat", "cat", Cat, Walk, Passive, Village, 0.7, 0.6, 10.0, 2.4, 0, [166,128,89], [227,204,161];
    CaveSpider, "Cave Spider", "cave_spider", Spider, Walk, Hostile, Special, 0.5, 0.7, 12.0, 1.9, 2, [39,70,73], [159,36,44];
    Chicken, "Chicken", "chicken", Bird, Walk, Passive, Grassland, 0.7, 0.4, 4.0, 1.6, 0, [235,230,211], [210,62,49];
    Cod, "Cod", "cod", Fish, Swim, Passive, Ocean, 0.3, 0.5, 3.0, 1.5, 0, [149,123,78], [210,194,148];
    CopperGolem, "Copper Golem", "copper_golem", Golem, Walk, Passive, Special, 1.0, 0.65, 12.0, 1.3, 0, [181,105,65], [89,160,126];
    Creaking, "Creaking", "creaking", Creaking, Walk, Hostile, Special, 2.7, 0.9, 30.0, 1.6, 4, [88,79,65], [240,143,32];
    Dolphin, "Dolphin", "dolphin", Fish, Swim, Neutral, Ocean, 0.6, 0.9, 10.0, 3.0, 3, [120,157,172], [199,215,216];
    Donkey, "Donkey", "donkey", Horse, Walk, Passive, Grassland, 1.5, 1.2, 20.0, 2.0, 0, [127,118,100], [203,191,163];
    Drowned, "Drowned", "drowned", Person, Amphibious, Hostile, Ocean, 1.95, 0.6, 20.0, 1.4, 3, [59,123,124], [115,158,121];
    ElderGuardian, "Elder Guardian", "elder_guardian", Guardian, Swim, Hostile, Special, 1.99, 1.99, 80.0, 1.0, 6, [188,182,149], [132,93,66];
    Enderman, "Enderman", "enderman", Person, Walk, Neutral, Night, 2.9, 0.6, 40.0, 2.6, 7, [38,35,43], [199,116,237];
    Endermite, "Endermite", "endermite", Armadillo, Walk, Hostile, Special, 0.3, 0.4, 8.0, 1.5, 2, [93,62,105], [197,124,180];
    Evoker, "Evoker", "evocation_illager", Villager, Walk, Hostile, Special, 1.95, 0.6, 24.0, 1.6, 6, [49,48,54], [133,154,143];
    Fox, "Fox", "fox", Canine, Walk, Passive, Taiga, 0.7, 0.6, 10.0, 2.4, 0, [207,120,49], [235,219,181];
    Frog, "Frog", "frog", Frog, Amphibious, Passive, Special, 0.5, 0.5, 10.0, 1.0, 0, [168,119,72], [211,185,115];
    GlowSquid, "Glow Squid", "glow_squid", Squid, Swim, Passive, Cave, 0.8, 0.8, 10.0, 1.0, 0, [39,134,127], [152,234,188];
    Goat, "Goat", "goat", Grazer, Walk, Neutral, Mountains, 1.3, 0.9, 10.0, 2.1, 2, [218,214,193], [134,114,85];
    Guardian, "Guardian", "guardian", Guardian, Swim, Hostile, Special, 0.85, 0.85, 30.0, 1.8, 5, [77,129,127], [199,138,83];
    HappyGhast, "Happy Ghast", "happy_ghast", Ghast, Fly, Passive, Special, 4.0, 4.0, 20.0, 1.0, 0, [230,229,216], [150,157,159];
    Horse, "Horse", "horse", Horse, Walk, Passive, Grassland, 1.6, 1.2, 24.0, 2.7, 0, [140,88,52], [223,205,172];
    Husk, "Husk", "husk", Person, Walk, Hostile, Desert, 1.95, 0.6, 20.0, 1.5, 3, [135,112,73], [164,144,98];
    Llama, "Llama", "llama", Horse, Walk, Neutral, Mountains, 1.87, 0.9, 24.0, 1.8, 2, [207,187,142], [116,86,55];
    Mooshroom, "Mooshroom", "mooshroom", Grazer, Walk, Passive, Special, 1.4, 0.9, 10.0, 2.15, 0, [171,47,41], [232,220,196];
    Mule, "Mule", "mule", Horse, Walk, Passive, Special, 1.6, 1.2, 24.0, 2.2, 0, [99,76,53], [189,166,125];
    Nautilus, "Nautilus", "nautilus", Nautilus, Swim, Neutral, Ocean, 0.9, 0.9, 20.0, 2.0, 2, [219,172,113], [157,79,50];
    Ocelot, "Ocelot", "ocelot", Cat, Walk, Passive, Special, 0.7, 0.6, 10.0, 2.5, 0, [210,171,71], [81,61,41];
    Panda, "Panda", "panda", Bear, Walk, Neutral, Special, 1.25, 1.3, 20.0, 1.0, 6, [227,222,200], [51,48,45];
    Parrot, "Parrot", "parrot", Bird, Fly, Passive, Special, 0.9, 0.5, 6.0, 1.8, 0, [202,60,52], [63,138,167];
    Parched, "Parched", "parched", Person, Walk, Hostile, Desert, 1.99, 0.6, 16.0, 1.5, 4, [183,158,105], [104,94,61];
    Phantom, "Phantom", "phantom", Bat, Fly, Hostile, Special, 0.5, 0.9, 20.0, 3.0, 4, [66,91,125], [162,199,105];
    Pillager, "Pillager", "pillager", Villager, Walk, Hostile, Special, 1.95, 0.6, 24.0, 1.8, 4, [99,77,74], [139,155,147];
    PolarBear, "Polar Bear", "polar_bear", Bear, Walk, Neutral, Snow, 1.4, 1.3, 30.0, 1.5, 6, [229,228,207], [91,83,71];
    Pufferfish, "Pufferfish", "pufferfish", Guardian, Swim, Neutral, Ocean, 0.5, 0.5, 3.0, 1.0, 2, [210,184,65], [139,103,47];
    Rabbit, "Rabbit", "rabbit", Rabbit, Hop, Passive, Grassland, 0.5, 0.4, 3.0, 1.7, 0, [166,139,101], [223,206,175];
    Ravager, "Ravager", "ravager", Bear, Walk, Hostile, Special, 2.2, 1.95, 100.0, 1.6, 8, [107,110,104], [185,180,155];
    Salmon, "Salmon", "salmon", Fish, Swim, Passive, River, 0.4, 0.7, 3.0, 1.8, 0, [153,77,67], [82,116,86];
    Silverfish, "Silverfish", "silverfish", Armadillo, Walk, Hostile, Special, 0.3, 0.4, 8.0, 1.5, 1, [126,134,136], [74,83,89];
    SkeletonHorse, "Skeleton Horse", "skeleton_horse", Horse, Walk, Passive, Special, 1.6, 1.2, 15.0, 2.7, 0, [201,199,177], [87,87,78];
    Slime, "Slime", "slime", Slime, Hop, Hostile, Cave, 1.0, 1.0, 16.0, 1.3, 3, [103,171,72], [57,100,48];
    Sniffer, "Sniffer", "sniffer", Sniffer, Walk, Passive, Special, 1.75, 1.9, 14.0, 1.1, 0, [89,132,67], [150,65,48];
    SnowGolem, "Snow Golem", "snow_golem", Golem, Walk, Passive, Special, 1.9, 0.7, 4.0, 1.2, 0, [226,234,227], [210,123,42];
    Spider, "Spider", "spider", Spider, Walk, Hostile, Night, 0.9, 1.4, 16.0, 1.9, 2, [75,61,52], [184,39,42];
    Squid, "Squid", "squid", Squid, Swim, Passive, Ocean, 0.8, 0.8, 10.0, 1.0, 0, [58,81,119], [191,153,139];
    Stray, "Stray", "stray", Person, Walk, Hostile, Snow, 1.99, 0.6, 20.0, 1.6, 4, [169,190,192], [77,115,124];
    SulfurCube, "Sulfur Cube", "sulfur_cube", Slime, Hop, Passive, Special, 1.0, 1.0, 16.0, 1.3, 0, [199,210,119], [93,137,114];
    Tadpole, "Tadpole", "tadpole", Fish, Swim, Passive, Special, 0.25, 0.4, 6.0, 1.1, 0, [98,78,47], [157,123,65];
    TraderLlama, "Trader Llama", "trader_llama", Horse, Walk, Neutral, Special, 1.87, 0.9, 24.0, 1.8, 2, [215,209,186], [68,109,173];
    TropicalFish, "Tropical Fish", "tropicalfish", Fish, Swim, Passive, Ocean, 0.4, 0.5, 3.0, 1.5, 0, [230,142,63], [235,220,183];
    Turtle, "Turtle", "turtle", Turtle, Amphibious, Passive, Ocean, 0.4, 1.2, 30.0, 0.7, 0, [74,127,69], [192,187,108];
    Vex, "Vex", "vex", Sprite, Fly, Hostile, Special, 0.8, 0.4, 14.0, 2.8, 4, [154,178,180], [70,111,128];
    Vindicator, "Vindicator", "vindicator", Villager, Walk, Hostile, Special, 1.95, 0.6, 24.0, 2.0, 7, [66,83,89], [139,154,146];
    WanderingTrader, "Wandering Trader", "wandering_trader", Villager, Walk, Passive, Special, 1.8, 0.56, 20.0, 1.7, 0, [61,109,165], [207,165,83];
    Warden, "Warden", "warden", Golem, Walk, Hostile, Special, 2.9, 0.9, 500.0, 1.4, 15, [34,74,73], [95,194,175];
    Witch, "Witch", "witch", Villager, Walk, Hostile, Special, 1.95, 0.6, 26.0, 1.6, 4, [99,64,120], [143,154,108];
    Wither, "Wither", "wither", Wither, Fly, Hostile, Special, 3.5, 0.9, 300.0, 1.4, 10, [57,61,64], [145,151,151];
    Wolf, "Wolf", "wolf", Canine, Walk, Neutral, Taiga, 0.85, 0.6, 20.0, 2.4, 4, [175,173,157], [230,222,199];
    ZombieHorse, "Zombie Horse", "zombie_horse", Horse, Walk, Passive, Night, 1.6, 1.2, 15.0, 2.5, 0, [102,135,79], [61,79,52];
    ZombieNautilus, "Zombie Nautilus", "zombie_nautilus", Nautilus, Swim, Neutral, Ocean, 0.9, 0.9, 20.0, 2.0, 2, [117,150,108], [67,94,74];
    ZombieVillager, "Zombie Villager", "zombie_villager_v2", Villager, Walk, Hostile, Night, 1.95, 0.6, 20.0, 1.5, 3, [130,100,67], [110,148,77];
    ZombifiedPiglin, "Zombified Piglin", "zombie_pigman", Person, Walk, Neutral, Special, 1.95, 0.6, 20.0, 1.8, 5, [159,115,97], [106,139,76];
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural_spawns_respect_biomes_water_darkness_and_missing_structures() {
        use crate::chunk::Biome;
        assert!(can_spawn(
            MobKind::Camel,
            Biome::Desert,
            false,
            false,
            false
        ));
        assert!(!can_spawn(
            MobKind::Camel,
            Biome::SnowyTaiga,
            false,
            false,
            false
        ));
        assert!(!can_spawn(MobKind::Cod, Biome::Plains, false, false, false));
        assert!(can_spawn(MobKind::Cod, Biome::Plains, true, false, false));
        assert!(!can_spawn(
            MobKind::Husk,
            Biome::Desert,
            false,
            false,
            false
        ));
        assert!(can_spawn(MobKind::Husk, Biome::Desert, false, false, true));
        for &kind in MobKind::ALL {
            if kind.species().habitat == Habitat::Special {
                for water in [false, true] {
                    assert!(!can_spawn(kind, Biome::Plains, water, false, true));
                    assert!(!can_spawn(kind, Biome::Plains, water, true, true));
                }
            }
        }
    }
    #[test]
    fn every_species_has_unique_resolvable_identity_and_valid_dimensions() {
        let mut ids = std::collections::HashSet::new();
        for &kind in MobKind::ALL {
            let s = kind.species();
            assert!(ids.insert(s.id));
            assert_eq!(MobKind::from_id(s.id), Some(kind));
            assert_eq!(MobKind::from_id(&format!("minecraft:{}", s.id)), Some(kind));
            assert!(s.height > 0.0 && s.width > 0.0 && s.health > 0.0 && s.speed > 0.0);
        }
        assert!(MobKind::ALL.len() > 70);
        for excluded in [
            "blaze",
            "ghast",
            "ender_dragon",
            "shulker",
            "piglin",
            "strider",
        ] {
            assert_eq!(MobKind::from_id(excluded), None);
        }
        for current in [
            "copper_golem",
            "nautilus",
            "camel_husk",
            "parched",
            "sulfur_cube",
        ] {
            assert!(MobKind::from_id(current).is_some());
        }
    }
}
