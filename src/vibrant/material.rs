//! Linear material maps aligned with the procedural 16-by-16 terrain tiles.
use super::VibrantPack;
use super::lighting::Mers;
use super::texture_set::Layer;

pub const ATLAS_SIZE: usize = 256;
pub const TILE_SIZE: usize = 16;
const ATLAS_BYTES: usize = ATLAS_SIZE * ATLAS_SIZE * 4;
const FLAT_NORMAL: [u8; 4] = [128, 128, 255, 255];

#[derive(Debug)]
pub struct MaterialAtlas {
    /// Linear RGBA: metalness, emission, roughness, subsurface, each in 0..=255.
    pub mers: Vec<u8>,
    /// Linear tangent-space XYZ, mapped from -1..=1; +Y follows increasing V.
    pub normals: Vec<u8>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Plain,
    Stone,
    Wood,
    Soil,
    Wool,
    Leaf,
    Glass,
    Metal,
    Gem,
    Snow,
    Lava,
    Torch,
    Flame,
    Campfire,
    Brick,
    Chest,
    Tool,
}

struct Tile {
    names: &'static [&'static str],
    surface: Surface,
    item: bool,
}

fn tile(tx: usize, ty: usize) -> Tile {
    use Surface::*;
    let (names, surface): (&[&str], Surface) = match (tx, ty) {
        (0, 0) => (&["grass_top"], Soil),
        (1, 0) => (&["stone"], Stone),
        (2, 0) => (&["dirt"], Soil),
        (3, 0) => (&["grass_side"], Soil),
        (4, 0) => (&["oak_log", "log_oak"], Wood),
        (5, 0) => (&["oak_leaves", "leaves_oak"], Leaf),
        (6, 0) => (&["sand"], Soil),
        (7, 0) => (&["gravel"], Stone),
        (8, 0) => (&["snow", "powder_snow"], Snow),
        (9, 0) => (&["snow_top"], Snow),
        (10, 0) => (&["grass_side_snowed"], Soil),
        (11, 0) => (&["spruce_log", "log_spruce"], Wood),
        (12, 0) => (&["spruce_leaves", "leaves_spruce"], Leaf),
        (1, 1) => (&["bedrock"], Stone),
        (2, 1) => (&["coal_ore"], Stone),
        (3, 1) => (&["iron_ore"], Stone),
        (4, 1) => (&["tnt_side"], Wood),
        (5, 1) => (&["oak_log_top", "log_oak_top"], Wood),
        (6, 1) => (&["iron_block"], Metal),
        (7, 1) => (&["raw_iron"], Stone),
        (8, 1) => (&["iron_ingot"], Metal),
        (9, 1) => (&["flint_and_steel"], Metal),
        (10, 1) => (&["tnt_top"], Wood),
        (11, 1) => (&["cactus_top"], Leaf),
        (12, 1) => (&["bell"], Metal),
        (0, 2) => (&["cobblestone"], Stone),
        (1, 2) => (&["oak_planks", "planks_oak"], Wood),
        (2, 2) => (&["crafting_table_top"], Wood),
        (3, 2) => (&["crafting_table_side"], Wood),
        (4, 2) => (&["furnace_front"], Stone),
        (5, 2) => (&["furnace_side"], Stone),
        (6, 2) => (&["glass"], Glass),
        (7, 2) => (&["gold_ore"], Stone),
        (8, 2) => (&["diamond_ore"], Stone),
        (9, 2) => (&["obsidian"], Gem),
        (10, 2) => (&["bricks", "brick"], Brick),
        (11, 2) => (&["stone_bricks", "stonebrick"], Brick),
        (12, 2) => (&["sandstone", "sandstone_normal"], Stone),
        (13, 2) => (&["white_wool", "wool_colored_white"], Wool),
        (14, 2) => (&["bookshelf"], Wood),
        (15, 2) => (&["sponge"], Soil),
        (0, 4) => (&["stick"], Wood),
        (1, 4) => (&["coal"], Stone),
        (2, 4) => (&["diamond"], Gem),
        (3, 4) => (&["gold_ingot"], Metal),
        (4, 4) => (&["lapis_lazuli"], Gem),
        (5, 4) => (&["string"], Wool),
        (6, 4) => (&["gunpowder"], Soil),
        (7, 4) => (&["leather"], Wood),
        (8, 4) => (&["redstone_dust"], Plain),
        (9, 4) => (&["wheat_seeds"], Leaf),
        (10, 4) => (&["bucket"], Metal),
        (11, 4) => (&["water_bucket"], Metal),
        (12, 4) => (&["lava_bucket"], Metal),
        (13, 4) => (&["bed"], Wool),
        (14, 4) => (&["oak_door", "door_wood"], Wood),
        (15, 4) => (&["iron_door", "door_iron"], Metal),
        (0..=9, 5) | (0..=14, 6) => (&[], Tool),
        (0, 7) => (&["chest_side", "chest"], Chest),
        (1, 7) => (&["chest_top"], Wood),
        (2, 7) => (&["mossy_cobblestone"], Stone),
        (3, 7) => (&["lapis_block"], Gem),
        (4, 7) => (&["lapis_ore"], Stone),
        (5, 7) => (&["torch_on", "torch"], Torch),
        (7, 7) => (&["lava_still", "lava"], Lava),
        (8, 7) => (&["cactus_side"], Leaf),
        (9, 7) => (&["clay"], Soil),
        (10, 7) => (&["farmland", "farmland_dry"], Soil),
        (11, 7) => (&["wheat", "wheat_stage_7"], Leaf),
        (12, 7) => (&["redstone_ore"], Stone),
        (13, 7) => (&["mob_spawner"], Metal),
        (14, 7) => (&["cactus_bottom"], Leaf),
        (7, 8) => (&["bow"], Wood),
        (8, 8) => (&["arrow"], Wood),
        (0..=3, 9) => (&[], Wood),
        (4..=11, 9) => (&[], Metal),
        (12..=15, 9) => (&[], Gem),
        (0..=7, 11) => (&["fire_0", "fire"], Flame),
        (0, 12) => (&["orange_wool", "wool_colored_orange"], Wool),
        (1, 12) => (&["magenta_wool", "wool_colored_magenta"], Wool),
        (2, 12) => (&["light_blue_wool", "wool_colored_light_blue"], Wool),
        (3, 12) => (&["yellow_wool", "wool_colored_yellow"], Wool),
        (4, 12) => (&["lime_wool", "wool_colored_lime"], Wool),
        (5, 12) => (&["pink_wool", "wool_colored_pink"], Wool),
        (6, 12) => (&["gray_wool", "wool_colored_gray"], Wool),
        (7, 12) => (&["light_gray_wool", "wool_colored_silver"], Wool),
        (8, 12) => (&["cyan_wool", "wool_colored_cyan"], Wool),
        (9, 12) => (&["purple_wool", "wool_colored_purple"], Wool),
        (10, 12) => (&["blue_wool", "wool_colored_blue"], Wool),
        (11, 12) => (&["brown_wool", "wool_colored_brown"], Wool),
        (12, 12) => (&["green_wool", "wool_colored_green"], Wool),
        (13, 12) => (&["water_still", "water"], Glass),
        (14, 12) => (&["red_wool", "wool_colored_red"], Wool),
        (15, 12) => (&["black_wool", "wool_colored_black"], Wool),
        (0, 14) => (&["poppy"], Leaf),
        (1, 14) => (&["dandelion"], Leaf),
        (2, 14) => (&["bee_nest_side", "bee_nest"], Wood),
        (3, 14) => (&["beehive_side", "beehive"], Wood),
        (4, 14) => (&["honeycomb"], Wood),
        (5, 14) => (&["glass_bottle"], Glass),
        (6, 14) => (&["honey_bottle"], Glass),
        (7, 14) => (&["shears"], Metal),
        (8, 14) => (&["campfire"], Campfire),
        (9, 14) => (&["birch_log", "log_birch"], Wood),
        (10, 14) => (&["birch_leaves", "leaves_birch"], Leaf),
        (11, 14) => (&["birch_planks", "planks_birch"], Wood),
        (12, 14) => (&["mangrove_log"], Wood),
        (13, 14) => (&["mangrove_leaves"], Leaf),
        (14, 14) => (&["mangrove_planks"], Wood),
        (15, 14) => (&["cherry_log"], Wood),
        (0, 15) => (&["cherry_leaves"], Leaf),
        (1, 15) => (&["cherry_planks"], Wood),
        (2, 15) => (&["mangrove_roots_side", "mangrove_roots"], Wood),
        (3, 15) => (&["mud"], Soil),
        (4, 15) => (&["sunflower_bottom"], Leaf),
        (5, 15) => (&["sunflower_top"], Leaf),
        (6, 15) => (&["pink_petals"], Leaf),
        (7, 15) => (&["cornflower"], Leaf),
        (8, 15) => (&["allium"], Leaf),
        (9, 15) => (&["oxeye_daisy"], Leaf),
        _ => (&[], Plain),
    };
    // Several logical blocks share sprite slots. The atlas artwork, not enum
    // iteration order, determines their material and category.
    let item = matches!(
        (tx, ty),
        (7..=9, 1) | (0..=12, 4) | (_, 5..=6) | (_, 8..=9) | (5..=6, 10) | (_, 13) | (4..=7, 14)
    );
    Tile {
        names,
        surface,
        item,
    }
}

fn builtin_mers(surface: Surface, tx: usize, ty: usize, x: usize, y: usize, p: [u8; 4]) -> Mers {
    use Surface::*;
    let (metalness, emissive, roughness, subsurface) = match surface {
        Plain => (0.0, 0.0, 0.9, 0.0),
        Stone | Brick => (0.0, 0.0, 0.84, 0.0),
        Wood | Chest => (0.0, 0.0, 0.72, 0.0),
        Soil => (0.0, 0.0, 0.94, 0.0),
        Wool => (0.0, 0.0, 0.98, 0.08),
        Leaf => (0.0, 0.0, 0.81, 0.55),
        Glass => (0.0, 0.0, 0.08, 0.0),
        Metal => (0.95, 0.0, 0.28, 0.0),
        Gem => (0.0, 0.0, 0.34, 0.0),
        Snow => (0.0, 0.0, 0.88, 0.18),
        Lava => (0.0, 0.72 + p[1] as f32 / 255.0 * 0.28, 0.62, 0.0),
        Torch if (2..6).contains(&y) && (6..10).contains(&x) => (0.0, 1.0, 0.8, 0.0),
        Torch | Campfire => (0.0, 0.0, 0.8, 0.0),
        Flame => (0.0, 1.0, 1.0, 0.0),
        Tool => {
            let wooden_pixel = p[0] < 190
                && p[1] < 150
                && p[2] < 100
                && p[0] > p[1].saturating_add(15)
                && p[1] > p[2].saturating_add(15);
            if wooden_pixel || ty == 5 && tx < 5 {
                (0.0, 0.0, 0.72, 0.0)
            } else if ty == 5 {
                (0.0, 0.0, 0.84, 0.0)
            } else if !(5..10).contains(&tx) {
                (0.95, 0.0, 0.25, 0.0)
            } else {
                (0.0, 0.0, 0.22, 0.0)
            }
        }
    };
    let mut result = Mers {
        metalness,
        emissive,
        roughness,
        subsurface,
    };
    if surface == Chest && (6..=9).contains(&x) && (5..=8).contains(&y) {
        result.metalness = 0.85;
        result.roughness = 0.35;
    }
    if surface == Campfire && y <= 10 && (5..12).contains(&x) {
        result.emissive = 0.95;
    }
    // Native ore is not refined metal. Only the actual gold flecks have a
    // metallic response; the surrounding stone remains a dielectric.
    if (tx, ty) == (7, 2) && p[0] > 220 && p[1] > 180 && p[2] < 90 {
        result.metalness = 0.8;
        result.roughness = 0.38;
    }
    if (tx, ty) == (8, 2) && p[1] > 180 && p[2] > 180 && p[0] < 110 {
        result.roughness = 0.32;
    }
    if (tx, ty) == (10, 0) && y < 3 {
        result.roughness = 0.88;
        result.subsurface = 0.18;
    }
    result
}

fn byte(value: f32) -> u8 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        0
    }
}

fn mers_bytes(mers: Mers) -> [u8; 4] {
    mers.to_array().map(byte)
}

fn index(tx: usize, ty: usize, x: usize, y: usize) -> usize {
    ((ty * TILE_SIZE + y) * ATLAS_SIZE + tx * TILE_SIZE + x) * 4
}

fn height(surface: Surface, x: usize, y: usize, p: [u8; 4]) -> f32 {
    let luma = (p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722) / 255.0;
    let amplitude = match surface {
        Surface::Stone | Surface::Brick => 0.35,
        Surface::Soil => 0.20,
        Surface::Wood | Surface::Chest => 0.30,
        Surface::Wool => 0.10,
        Surface::Leaf | Surface::Snow => 0.12,
        Surface::Metal | Surface::Gem | Surface::Tool => 0.04,
        _ => 0.0,
    };
    let weave = if surface == Surface::Wool {
        ((x + y) % 3) as f32 * 0.012
    } else {
        0.0
    };
    let mortar = if surface == Surface::Brick && y.is_multiple_of(4) {
        -0.04
    } else {
        0.0
    };
    luma * amplitude + weave + mortar
}

fn normal_from_height(
    heights: &[f32; TILE_SIZE * TILE_SIZE],
    mask: &[bool; TILE_SIZE * TILE_SIZE],
    x: usize,
    y: usize,
) -> [u8; 4] {
    let center = y * TILE_SIZE + x;
    if !mask[center] {
        return FLAT_NORMAL;
    }
    let sample = |sx: usize, sy: usize| {
        let i = sy * TILE_SIZE + sx;
        if mask[i] { heights[i] } else { heights[center] }
    };
    let left = x.saturating_sub(1);
    let right = (x + 1).min(TILE_SIZE - 1);
    let top = y.saturating_sub(1);
    let bottom = (y + 1).min(TILE_SIZE - 1);
    let dx = (sample(right, y) - sample(left, y)) / (right - left) as f32;
    let dy = (sample(x, bottom) - sample(x, top)) / (bottom - top) as f32;
    encode_normal([-dx, -dy, 1.0])
}

fn encode_normal(v: [f32; 3]) -> [u8; 4] {
    let length = v.iter().map(|c| c * c).sum::<f32>().sqrt();
    if !length.is_finite() || length < 0.0001 || v[2] <= 0.0 {
        return FLAT_NORMAL;
    }
    [
        byte(v[0] / length * 0.5 + 0.5),
        byte(v[1] / length * 0.5 + 0.5),
        byte(v[2] / length * 0.5 + 0.5),
        255,
    ]
}

fn normalize_pixel(p: [u8; 4]) -> [u8; 4] {
    // 128 is the authored zero for an 8-bit normal, rather than 127.5.
    let component = |c: u8| {
        if c >= 128 {
            (c as f32 - 128.0) / 127.0
        } else {
            (c as f32 - 128.0) / 128.0
        }
    };
    encode_normal([component(p[0]), component(p[1]), component(p[2])])
}

/// Produces two 256x256 RGBA8 maps without changing albedo. Invalid external
/// layers report warnings and keep the category fallback or built-in detail.
/// Browser builds generate the same built-ins without any filesystem access.
pub fn generate_material_atlas(pack: &VibrantPack, albedo: &[u8]) -> MaterialAtlas {
    let mut atlas = MaterialAtlas {
        mers: vec![0; ATLAS_BYTES],
        normals: FLAT_NORMAL.repeat(ATLAS_SIZE * ATLAS_SIZE),
        warnings: Vec::new(),
    };
    for pixel in atlas.mers.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&mers_bytes(pack.pbr_fallback.blocks));
    }
    if albedo.len() != ATLAS_BYTES {
        atlas
            .warnings
            .push("material atlas: expected 256x256 RGBA albedo".into());
        return atlas;
    }
    for ty in 0..ATLAS_SIZE / TILE_SIZE {
        for tx in 0..ATLAS_SIZE / TILE_SIZE {
            let tile = tile(tx, ty);
            let fallback = if tile.item {
                pack.pbr_fallback.items
            } else {
                pack.pbr_fallback.blocks
            };
            let authored = pack.material_fallback_authored[usize::from(tile.item)]
                || fallback != Mers::DEFAULT;
            let set_entry = tile
                .names
                .iter()
                .find_map(|name| pack.texture_set(name).map(|set| (*name, set)));
            let mut heights = [0.0; TILE_SIZE * TILE_SIZE];
            let mut mask = [false; TILE_SIZE * TILE_SIZE];
            let mers_layer =
                set_entry.and_then(|(name, set)| set.mers_layer().map(|layer| (name, set, layer)));
            let mers_map = mers_layer
                .and_then(|(name, _, layer)| layer_data(pack, name, layer, &mut atlas.warnings));
            let normal_layer =
                set_entry.and_then(|(name, set)| set.normal.as_ref().map(|layer| (name, layer)));
            let normal_map = normal_layer
                .and_then(|(name, layer)| layer_data(pack, name, layer, &mut atlas.warnings));
            let height_layer =
                set_entry.and_then(|(name, set)| set.heightmap.as_ref().map(|layer| (name, layer)));
            let height_map = if normal_map.is_none() {
                height_layer
                    .and_then(|(name, layer)| layer_data(pack, name, layer, &mut atlas.warnings))
            } else {
                None
            };
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let i = index(tx, ty, x, y);
                    let p = [albedo[i], albedo[i + 1], albedo[i + 2], albedo[i + 3]];
                    let local = y * TILE_SIZE + x;
                    mask[local] = p[3] != 0;
                    heights[local] = height_map.as_ref().map_or_else(
                        || height(tile.surface, x, y, p),
                        |map| map.pixel(x, y)[0] as f32 / 255.0,
                    );
                    let mut mers =
                        if set_entry.is_some() || authored || tile.surface == Surface::Plain {
                            mers_bytes(fallback)
                        } else {
                            mers_bytes(builtin_mers(tile.surface, tx, ty, x, y, p))
                        };
                    if let Some(map) = &mers_map {
                        let pixel = map.pixel(x, y);
                        mers[..3].copy_from_slice(&pixel[..3]);
                        if mers_layer.is_some_and(|(_, set, _)| {
                            set.metalness_emissive_roughness_subsurface.is_some()
                        }) {
                            mers[3] = pixel[3];
                        }
                    }
                    if !mask[local] {
                        mers = [0, 0, 255, 0];
                    }
                    atlas.mers[i..i + 4].copy_from_slice(&mers);
                }
            }
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let p = if !mask[y * TILE_SIZE + x] {
                        FLAT_NORMAL
                    } else {
                        normal_map.as_ref().map_or_else(
                            || normal_from_height(&heights, &mask, x, y),
                            |map| normalize_pixel(map.pixel(x, y)),
                        )
                    };
                    let i = index(tx, ty, x, y);
                    atlas.normals[i..i + 4].copy_from_slice(&p);
                }
            }
        }
    }
    atlas
}

struct LayerData {
    size: usize,
    rgba: Vec<u8>,
}

impl LayerData {
    fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let sx = ((2 * x + 1) * self.size / (2 * TILE_SIZE)).min(self.size - 1);
        let sy = ((2 * y + 1) * self.size / (2 * TILE_SIZE)).min(self.size - 1);
        let i = (sy * self.size + sx) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

fn layer_data(
    pack: &VibrantPack,
    texture: &str,
    layer: &Layer,
    warnings: &mut Vec<String>,
) -> Option<LayerData> {
    match layer {
        Layer::Value(color) => Some(LayerData {
            size: 1,
            rgba: color.raw().map(byte).to_vec(),
        }),
        Layer::Texture(name) => {
            #[cfg(not(target_arch = "wasm32"))]
            let result = load_map(pack, texture, name);
            #[cfg(target_arch = "wasm32")]
            let result: Result<LayerData, String> = {
                let _ = (pack, name);
                Err("external material maps are unavailable in browser builds".into())
            };
            match result {
                Ok(data) => Some(data),
                Err(reason) => {
                    warnings.push(format!("{texture}: material map rejected ({reason})"));
                    None
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn map_path(pack: &VibrantPack, texture: &str, name: &str) -> Result<std::path::PathBuf, String> {
    use std::path::Path;
    let root = pack.material_root.as_deref().ok_or("no loaded pack root")?;
    if name.is_empty()
        || name.len() > 240
        || name.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with('.')
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        })
    {
        return Err("invalid pack-relative PNG path".into());
    }
    let mut relative = std::path::PathBuf::from(name);
    match relative.extension().and_then(|s| s.to_str()) {
        None => {
            relative.set_extension("png");
        }
        Some("png") => {}
        _ => return Err("only PNG maps are supported".into()),
    }
    let base = if relative.starts_with(Path::new("textures")) {
        root
    } else {
        pack.texture_set_dirs
            .iter()
            .rev()
            .find(|(n, _)| n == texture)
            .map(|(_, dir)| dir.as_path())
            .ok_or("texture set has no pack-relative source")?
    };
    let path = base.join(relative);
    let canonical = std::fs::canonicalize(&path).map_err(|_| "map is missing".to_string())?;
    if !canonical.starts_with(root) {
        return Err("map escapes pack root".into());
    }
    if !canonical.is_file() {
        return Err("map is not a regular file".into());
    }
    Ok(canonical)
}

#[cfg(all(not(target_arch = "wasm32"), unix))]
fn open_map(root: &std::path::Path, path: &std::path::Path) -> Result<std::fs::File, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let root_name = CString::new(root.as_os_str().as_bytes()).map_err(|_| "invalid pack root")?;
    // Directory handles pin each component while O_NOFOLLOW prevents a pack
    // editor from substituting an escaping symlink between check and open.
    let fd = unsafe {
        libc::open(
            root_name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("cannot open pack root safely".into());
    }
    let mut current = unsafe { std::fs::File::from_raw_fd(fd) };
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "map escapes pack root")?;
    let mut components = relative.components().peekable();
    if components.peek().is_none() {
        return Err("map is not a regular file".into());
    }
    while let Some(component) = components.next() {
        let std::path::Component::Normal(name) = component else {
            return Err("invalid map component".into());
        };
        let name = CString::new(name.as_bytes()).map_err(|_| "invalid map component")?;
        let directory_flag = if components.peek().is_some() {
            libc::O_DIRECTORY
        } else {
            0
        };
        let fd = unsafe {
            libc::openat(
                current.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | directory_flag,
            )
        };
        if fd < 0 {
            return Err("cannot open map without following symlinks".into());
        }
        current = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(current)
}

#[cfg(all(not(target_arch = "wasm32"), windows))]
fn open_map(root: &std::path::Path, path: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(
            handle: *mut std::ffi::c_void,
            name: *mut u16,
            length: u32,
            flags: u32,
        ) -> u32;
    }
    let file = std::fs::File::open(path).map_err(|_| "cannot open map")?;
    let mut name = vec![0_u16; 32768];
    // Validate the opened handle, not the mutable filename, before reading
    // bytes. This also resolves junctions and other Windows reparse points.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            name.as_mut_ptr(),
            name.len() as u32,
            0,
        )
    } as usize;
    if length == 0 || length >= name.len() {
        return Err("cannot resolve opened map path".into());
    }
    let actual = std::path::PathBuf::from(std::ffi::OsString::from_wide(&name[..length]));
    if !actual.starts_with(root) {
        return Err("opened map escapes pack root".into());
    }
    Ok(file)
}

#[cfg(all(not(target_arch = "wasm32"), not(any(unix, windows))))]
fn open_map(_: &std::path::Path, _: &std::path::Path) -> Result<std::fs::File, String> {
    Err("external maps are unsupported on this platform".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_map(pack: &VibrantPack, texture: &str, name: &str) -> Result<LayerData, String> {
    use std::io::Read;
    const MAX_BYTES: usize = 2 * 1024 * 1024;
    let path = map_path(pack, texture, name)?;
    let root = pack.material_root.as_deref().ok_or("no loaded pack root")?;
    let file = open_map(root, &path)?;
    if !file.metadata().map_err(|_| "cannot inspect map")?.is_file() {
        return Err("map is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read map")?;
    if bytes.len() > MAX_BYTES {
        return Err("PNG exceeds 2 MiB encoded limit".into());
    }
    decode_map(&bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_map(bytes: &[u8]) -> Result<LayerData, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 2 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|_| "invalid or oversized PNG")?;
    let info = reader.info();
    if info.width == 0 || info.width != info.height || info.width > ATLAS_SIZE as u32 {
        return Err("PNG must be square and at most 256x256".into());
    }
    if info.animation_control.is_some() {
        return Err("animated PNG maps are unsupported".into());
    }
    let size = reader
        .output_buffer_size()
        .filter(|n| *n <= ATLAS_BYTES)
        .ok_or("PNG decoded buffer exceeds limit")?;
    let mut raw = vec![0; size];
    let output = reader
        .next_frame(&mut raw)
        .map_err(|_| "invalid PNG pixels")?;
    let mut rgba = Vec::with_capacity(output.width as usize * output.height as usize * 4);
    for p in raw[..output.buffer_size()].chunks_exact(output.color_type.samples()) {
        match output.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(p),
            png::ColorType::Rgb => rgba.extend_from_slice(&[p[0], p[1], p[2], 255]),
            png::ColorType::Grayscale => rgba.extend_from_slice(&[p[0], p[0], p[0], 255]),
            png::ColorType::GrayscaleAlpha => rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]),
            png::ColorType::Indexed => return Err("PNG palette was not expanded".into()),
        }
    }
    Ok(LayerData {
        size: output.width as usize,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockType;
    use crate::item::{atlas_uv, atlas_uv_side, atlas_uv_top};
    use crate::vibrant::json;
    use crate::vibrant::texture_set::TextureSet;

    fn maps(pack: &VibrantPack) -> MaterialAtlas {
        generate_material_atlas(pack, &crate::atlas::generate_atlas_data())
    }

    fn pixel(data: &[u8], tile: (u8, u8), x: usize, y: usize) -> [u8; 4] {
        let i = index(tile.0 as usize, tile.1 as usize, x, y);
        data[i..i + 4].try_into().unwrap()
    }

    fn set(pack: &mut VibrantPack, name: &str, source: &str) {
        pack.insert_texture_set(
            name.into(),
            TextureSet::parse(&json::parse(source).unwrap()).unwrap(),
        );
    }

    #[test]
    fn aligned_material_maps_are_deterministic_and_normals_are_unit_length() {
        let albedo = crate::atlas::generate_atlas_data();
        let a = generate_material_atlas(&VibrantPack::default(), &albedo);
        let b = generate_material_atlas(&VibrantPack::default(), &albedo);
        assert_eq!(a.mers.len(), albedo.len());
        assert_eq!(a.normals.len(), albedo.len());
        assert_eq!(a.mers, b.mers);
        assert_eq!(a.normals, b.normals);
        assert!(a.warnings.is_empty());
        for (normal, color) in a
            .normals
            .as_chunks::<4>()
            .0
            .iter()
            .zip(albedo.as_chunks::<4>().0)
        {
            let components = normal[..3].iter().map(|c| *c as f32 / 255.0 * 2.0 - 1.0);
            let length = components.map(|c| c * c).sum::<f32>().sqrt();
            assert!((length - 1.0).abs() < 0.015, "normal length {length}");
            assert_eq!(normal[3], 255);
            assert!(normal[2] >= 248, "builtin detail stays conservative");
            if color[3] == 0 {
                assert_eq!(*normal, FLAT_NORMAL);
            }
        }
        assert!(
            a.normals
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| *p != FLAT_NORMAL)
        );
    }

    #[test]
    fn actual_block_faces_have_distinct_roughness_and_dielectric_materials() {
        let a = maps(&VibrantPack::default());
        let rough = |b| pixel(&a.mers, atlas_uv(b), 8, 8)[2];
        assert!(rough(BlockType::Glass) < rough(BlockType::IronBlock));
        assert!(rough(BlockType::IronBlock) < rough(BlockType::OakLog));
        assert!(rough(BlockType::OakLog) < rough(BlockType::Stone));
        assert!(rough(BlockType::Stone) < rough(BlockType::Dirt));
        assert!(rough(BlockType::Dirt) < rough(BlockType::Wool));
        for b in [
            BlockType::Stone,
            BlockType::Dirt,
            BlockType::Wool,
            BlockType::Glass,
        ] {
            assert_eq!(pixel(&a.mers, atlas_uv(b), 8, 8)[0], 0);
        }
        assert!(pixel(&a.mers, atlas_uv(BlockType::IronBlock), 8, 8)[0] > 230);
        for face in [
            atlas_uv_top(BlockType::OakLog),
            atlas_uv_side(BlockType::CraftingTable),
            atlas_uv_top(BlockType::Grass),
        ] {
            assert_eq!(pixel(&a.mers, face, 8, 8)[0], 0);
            assert!(pixel(&a.mers, face, 8, 8)[2] > 160);
        }
        assert_eq!(
            pixel(&a.normals, atlas_uv(BlockType::Glass), 8, 8),
            FLAT_NORMAL
        );
    }

    #[test]
    fn every_wool_color_stays_rough_and_water_is_not_the_old_wool_slot() {
        let a = maps(&VibrantPack::default());
        for b in [
            BlockType::Wool,
            BlockType::OrangeWool,
            BlockType::MagentaWool,
            BlockType::LightBlueWool,
            BlockType::YellowWool,
            BlockType::LimeWool,
            BlockType::PinkWool,
            BlockType::GrayWool,
            BlockType::LightGrayWool,
            BlockType::CyanWool,
            BlockType::PurpleWool,
            BlockType::BlueWool,
            BlockType::BrownWool,
            BlockType::GreenWool,
            BlockType::RedWool,
            BlockType::BlackWool,
        ] {
            let p = pixel(&a.mers, atlas_uv(b), 8, 8);
            assert_eq!(p[0], 0, "{b:?}");
            assert!(p[2] >= 245, "{b:?}");
        }
        assert!(pixel(&a.mers, atlas_uv(BlockType::Water), 8, 8)[2] < 30);
    }

    #[test]
    fn flames_emit_but_torch_handles_and_empty_pixels_do_not() {
        let albedo = crate::atlas::generate_atlas_data();
        let a = generate_material_atlas(&VibrantPack::default(), &albedo);
        let torch = atlas_uv(BlockType::Torch);
        assert_eq!(pixel(&a.mers, torch, 7, 3)[1], 255);
        assert_eq!(pixel(&a.mers, torch, 7, 9)[1], 0);
        assert_eq!(pixel(&a.mers, torch, 0, 0), [0, 0, 255, 0]);
        assert!(pixel(&a.mers, atlas_uv(BlockType::Lava), 8, 8)[1] >= 180);
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::Campfire), 7, 5)[1],
            byte(0.95)
        );
        assert_eq!(pixel(&a.mers, atlas_uv(BlockType::Campfire), 7, 14)[1], 0);
        for frame in 0..8 {
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let i = index(frame, 11, x, y);
                    assert_eq!(a.mers[i + 1], if albedo[i + 3] != 0 { 255 } else { 0 });
                }
            }
        }
    }

    #[test]
    fn leaf_subsurface_matches_all_real_leaf_tiles_and_not_their_holes() {
        let albedo = crate::atlas::generate_atlas_data();
        let a = generate_material_atlas(&VibrantPack::default(), &albedo);
        for b in [
            BlockType::OakLeaves,
            BlockType::SpruceLeaves,
            BlockType::BirchLeaves,
            BlockType::MangroveLeaves,
            BlockType::CherryLeaves,
        ] {
            let (tx, ty) = atlas_uv(b);
            let mut lit = 0;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let i = index(tx as usize, ty as usize, x, y);
                    if albedo[i + 3] == 0 {
                        assert_eq!(a.mers[i + 3], 0);
                    } else {
                        assert!(a.mers[i + 3] > 120);
                        lit += 1;
                    }
                }
            }
            assert!(lit > 100, "{b:?}");
        }
        assert_eq!(pixel(&a.mers, atlas_uv(BlockType::Stone), 8, 8)[3], 0);
    }

    #[test]
    fn shared_chest_tile_preserves_metal_latch_and_tools_preserve_wood_handles() {
        let a = maps(&VibrantPack::default());
        assert_eq!(pixel(&a.mers, atlas_uv(BlockType::Chest), 3, 7)[0], 0);
        assert!(pixel(&a.mers, atlas_uv(BlockType::Chest), 7, 7)[0] > 200);
        for b in [BlockType::IronPickaxe, BlockType::GoldPickaxe] {
            assert!(pixel(&a.mers, atlas_uv(b), 7, 2)[0] > 230, "{b:?} head");
            assert_eq!(pixel(&a.mers, atlas_uv(b), 3, 15)[0], 0, "{b:?} handle");
        }
    }

    #[test]
    fn material_and_normal_tiles_do_not_read_their_atlas_neighbors() {
        let mut albedo = crate::atlas::generate_atlas_data();
        let before = generate_material_atlas(&VibrantPack::default(), &albedo);
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let i = index(2, 0, x, y);
                albedo[i..i + 4].copy_from_slice(&[255, 0, 255, 255]);
            }
        }
        let after = generate_material_atlas(&VibrantPack::default(), &albedo);
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                assert_eq!(
                    pixel(&before.normals, (1, 0), x, y),
                    pixel(&after.normals, (1, 0), x, y)
                );
                assert_eq!(
                    pixel(&before.mers, (1, 0), x, y),
                    pixel(&after.mers, (1, 0), x, y)
                );
            }
        }
    }

    #[test]
    fn height_derivatives_have_correct_sign_and_no_transparent_rim() {
        let heights = std::array::from_fn(|i| (i % TILE_SIZE + i / TILE_SIZE) as f32 / 32.0);
        let mut mask = [true; TILE_SIZE * TILE_SIZE];
        for (x, y) in [(0, 0), (8, 8), (15, 15)] {
            let normal = normal_from_height(&heights, &mask, x, y);
            assert!(normal[0] < 128 && normal[1] < 128);
            assert!(normal[2] > 250);
        }
        mask[8 * TILE_SIZE + 7] = false;
        let flat = [0.5; TILE_SIZE * TILE_SIZE];
        assert_eq!(normal_from_height(&flat, &mask, 8, 8), FLAT_NORMAL);
        assert_eq!(normal_from_height(&flat, &mask, 7, 8), FLAT_NORMAL);
    }

    #[test]
    fn texture_set_constants_override_builtins_and_four_channel_mers_wins() {
        let mut pack = VibrantPack::default();
        set(
            &mut pack,
            "iron_block",
            r#"{"minecraft:texture_set":{"metalness_emissive_roughness":[255,0,255],"metalness_emissive_roughness_subsurface":[17,35,51,68],"normal":[128,128,255]}}"#,
        );
        let a = maps(&pack);
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::IronBlock), 8, 8),
            [17, 35, 51, 68]
        );
        assert_eq!(
            pixel(&a.normals, atlas_uv(BlockType::IronBlock), 8, 8),
            FLAT_NORMAL
        );
        set(
            &mut pack,
            "oak_leaves",
            r#"{"minecraft:texture_set":{"metalness_emissive_roughness":[9,18,27]}}"#,
        );
        pack.pbr_fallback.blocks.subsurface = 0.4;
        let a = maps(&pack);
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::OakLeaves), 8, 8),
            [9, 18, 27, 102]
        );
    }

    #[test]
    fn category_defaults_apply_to_missing_layers_and_unknown_tiles() {
        let mut pack = VibrantPack::default();
        pack.pbr_fallback.blocks = Mers {
            metalness: 0.2,
            emissive: 0.1,
            roughness: 0.3,
            subsurface: 0.4,
        };
        pack.pbr_fallback.items = Mers {
            metalness: 0.6,
            emissive: 0.5,
            roughness: 0.7,
            subsurface: 0.8,
        };
        set(
            &mut pack,
            "stone",
            r#"{"minecraft:texture_set":{"color":"stone"}}"#,
        );
        let a = maps(&pack);
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::Stone), 8, 8),
            mers_bytes(pack.pbr_fallback.blocks)
        );
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::IronBlock), 8, 8),
            mers_bytes(pack.pbr_fallback.blocks)
        );
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::IronIngot), 8, 8),
            mers_bytes(pack.pbr_fallback.items)
        );
        assert_eq!(
            pixel(&a.mers, atlas_uv(BlockType::RawBeef), 8, 8),
            mers_bytes(pack.pbr_fallback.items)
        );
    }

    #[test]
    fn invalid_albedo_is_bounded_and_reports_a_warning() {
        let a = generate_material_atlas(&VibrantPack::default(), &[0; 4]);
        assert_eq!(a.mers.len(), ATLAS_BYTES);
        assert_eq!(a.normals.len(), ATLAS_BYTES);
        assert_eq!(a.warnings.len(), 1);
        assert!(
            a.normals
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == FLAT_NORMAL)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct TempPack(PathBuf);

        impl TempPack {
            fn new() -> Self {
                static COUNT: AtomicU32 = AtomicU32::new(0);
                let id = COUNT.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("voxelpopuli_material_{}_{id}", std::process::id()));
                std::fs::create_dir_all(path.join("textures/blocks")).unwrap();
                Self(path)
            }

            fn write(&self, path: &str, bytes: &[u8]) {
                let path = self.0.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, bytes).unwrap();
            }

            fn texture(&self, name: &str, source: &str) {
                self.write(
                    &format!("textures/blocks/{name}.texture_set.json"),
                    source.as_bytes(),
                );
            }
        }

        impl Drop for TempPack {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn png_bytes(size: u32, rgba: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, size, size);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(rgba).unwrap();
            writer.finish().unwrap();
            bytes
        }

        #[test]
        fn png_mers_override_resamples_only_the_named_tile_and_preserves_linear_bytes() {
            let dir = TempPack::new();
            dir.texture("stone", r#"{"minecraft:texture_set":{"metalness_emissive_roughness_subsurface":"maps/stone_mers"}}"#);
            let rgba = [
                [10, 20, 30, 40],
                [50, 60, 70, 80],
                [90, 100, 110, 120],
                [130, 140, 150, 160],
            ]
            .concat();
            dir.write("textures/blocks/maps/stone_mers.png", &png_bytes(2, &rgba));
            let a = maps(&VibrantPack::load(&dir.0));
            assert!(a.warnings.is_empty(), "{:?}", a.warnings);
            assert_eq!(
                pixel(&a.mers, atlas_uv(BlockType::Stone), 0, 0),
                [10, 20, 30, 40]
            );
            assert_eq!(
                pixel(&a.mers, atlas_uv(BlockType::Stone), 15, 15),
                [130, 140, 150, 160]
            );
            assert_eq!(pixel(&a.mers, atlas_uv(BlockType::Dirt), 0, 0)[0], 0);
        }

        #[test]
        fn png_height_detail_and_normal_precedence_are_real_pixel_operations() {
            let dir = TempPack::new();
            let rgba: Vec<u8> = (0..TILE_SIZE * TILE_SIZE)
                .flat_map(|i| {
                    let h = ((i % TILE_SIZE) * 16) as u8;
                    [h, h, h, 255]
                })
                .collect();
            dir.write("textures/blocks/height.png", &png_bytes(16, &rgba));
            dir.texture(
                "stone",
                r#"{"minecraft:texture_set":{"heightmap":"height"}}"#,
            );
            let height = maps(&VibrantPack::load(&dir.0));
            let p = pixel(&height.normals, (1, 0), 8, 8);
            assert!(p[0] < 128);
            assert_eq!(p[1], 128);
            dir.write(
                "textures/blocks/normal.png",
                &png_bytes(1, &[128, 128, 255, 255]),
            );
            dir.texture("stone", r#"{"minecraft:texture_set":{"normal":"textures/blocks/normal.png","heightmap":"height"}}"#);
            let normal = maps(&VibrantPack::load(&dir.0));
            assert!(normal.warnings.is_empty());
            assert_eq!(pixel(&normal.normals, (1, 0), 8, 8), FLAT_NORMAL);
        }

        #[test]
        fn explicitly_authored_matte_category_is_not_replaced_by_builtin_metal() {
            let dir = TempPack::new();
            dir.write("pbr/global.json", br#"{"minecraft:pbr_fallback_settings":{"blocks":{"global_metalness_emissive_roughness_subsurface":[0,0,255,0]}}}"#);
            let a = maps(&VibrantPack::load(&dir.0));
            assert_eq!(
                pixel(&a.mers, atlas_uv(BlockType::IronBlock), 8, 8),
                [0, 0, 255, 0]
            );
            assert!(pixel(&a.mers, atlas_uv(BlockType::IronIngot), 8, 8)[0] > 230);
        }

        #[test]
        fn png_decoder_rejects_huge_dimensions_and_invalid_or_truncated_content() {
            let bytes = png_bytes(257, &vec![128; 257 * 257 * 4]);
            assert!(decode_map(&bytes).is_err());
            assert!(decode_map(b"not png").is_err());
            let small = png_bytes(1, &[0, 0, 255, 0]);
            assert!(decode_map(&small[..small.len() / 2]).is_err());
            assert_eq!(decode_map(&small).unwrap().rgba, [0, 0, 255, 0]);
            let mut huge = small;
            huge[16..20].copy_from_slice(&65536_u32.to_be_bytes());
            huge[20..24].copy_from_slice(&65536_u32.to_be_bytes());
            let mut crc = u32::MAX;
            for b in &huge[12..29] {
                crc ^= *b as u32;
                for _ in 0..8 {
                    crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
                }
            }
            huge[29..33].copy_from_slice(&(!crc).to_be_bytes());
            assert!(decode_map(&huge).is_err());
            let allowed = png_bytes(256, &vec![128; ATLAS_BYTES]);
            assert_eq!(decode_map(&allowed).unwrap().rgba.len(), ATLAS_BYTES);
        }

        #[test]
        fn declaration_directory_outside_root_cannot_resolve_an_image() {
            let dir = TempPack::new();
            dir.texture("stone", r#"{"minecraft:texture_set":{"color":"stone"}}"#);
            let outside = TempPack::new();
            outside.write("map.png", &png_bytes(1, &[255, 255, 255, 255]));
            let mut pack = VibrantPack::load(&dir.0);
            pack.texture_set_dirs[0].1 = outside.0.clone();
            assert!(
                map_path(&pack, "stone", "map")
                    .err()
                    .unwrap()
                    .contains("escapes pack root")
            );
        }

        #[test]
        fn encoded_image_read_is_bounded_before_decoding() {
            let dir = TempPack::new();
            dir.texture(
                "stone",
                r#"{"minecraft:texture_set":{"metalness_emissive_roughness":"large"}}"#,
            );
            dir.write("textures/blocks/large.png", &vec![0; 2 * 1024 * 1024 + 1]);
            let pack = VibrantPack::load(&dir.0);
            assert!(
                load_map(&pack, "stone", "large")
                    .err()
                    .unwrap()
                    .contains("encoded limit")
            );
            let a = maps(&pack);
            assert_eq!(a.warnings.len(), 1);
            assert_eq!(pixel(&a.mers, (1, 0), 8, 8), [0, 0, 255, 0]);
        }

        #[test]
        fn override_paths_reject_traversal_absolute_paths_and_non_png_inputs() {
            let dir = TempPack::new();
            dir.texture("stone", r#"{"minecraft:texture_set":{"color":"stone"}}"#);
            let pack = VibrantPack::load(&dir.0);
            for name in [
                "../outside",
                "maps/../../outside",
                "/tmp/map",
                "C:/map",
                "maps\\outside",
                "//server/share",
                "map.png:secret",
                "map.jpg",
                "./map",
                "map/",
                "maps//map",
                "trailing./map",
            ] {
                assert!(map_path(&pack, "stone", name).is_err(), "{name}");
            }
            assert!(load_map(&VibrantPack::default(), "stone", "map").is_err());
        }

        #[test]
        fn failed_image_overrides_report_warning_and_use_pack_category_not_ambient_files() {
            let dir = TempPack::new();
            dir.texture("stone", r#"{"minecraft:texture_set":{"metalness_emissive_roughness":"../outside","normal":"missing"}}"#);
            let mut pack = VibrantPack::load(&dir.0);
            pack.pbr_fallback.blocks.roughness = 0.42;
            let a = maps(&pack);
            assert_eq!(a.warnings.len(), 2);
            assert_eq!(pixel(&a.mers, (1, 0), 8, 8)[2], byte(0.42));
        }

        #[cfg(unix)]
        #[test]
        fn map_and_texture_set_symlinks_cannot_escape_the_pack_root() {
            use std::os::unix::fs::symlink;
            let outside = TempPack::new();
            outside.write("map.png", &png_bytes(1, &[255, 255, 255, 255]));
            outside.texture(
                "iron_block",
                r#"{"minecraft:texture_set":{"metalness_emissive_roughness":[255,255,255]}}"#,
            );
            let dir = TempPack::new();
            dir.texture(
                "stone",
                r#"{"minecraft:texture_set":{"metalness_emissive_roughness":"escape"}}"#,
            );
            symlink(
                outside.0.join("map.png"),
                dir.0.join("textures/blocks/escape.png"),
            )
            .unwrap();
            symlink(
                outside.0.join("textures/blocks"),
                dir.0.join("textures/outside"),
            )
            .unwrap();
            let pack = VibrantPack::load(&dir.0);
            assert!(pack.texture_set("iron_block").is_none());
            assert!(!pack.warnings.is_empty());
            assert!(
                map_path(&pack, "stone", "escape")
                    .err()
                    .unwrap()
                    .contains("escapes pack root")
            );
            let a = maps(&pack);
            assert_eq!(pixel(&a.mers, (1, 0), 8, 8), [0, 0, 255, 0]);
        }
    }
}
