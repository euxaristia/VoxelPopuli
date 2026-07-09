// Seed-deterministic villages spanning multiple chunks.
//
// The world is divided into square regions of REGION_CHUNKS x REGION_CHUNKS
// chunks; each region rolls at most one village. Placement, layout, and every
// block a village stamps are pure functions of (seed, region), so each chunk
// can independently generate its own slice of a village and the pieces line
// up across chunk borders.

use crate::block::BlockType;
use crate::chunk::{
    Biome, CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, ChunkRng, chunk_hash, hash_unit,
    terrain_height_and_biome,
};
use glam::Vec3;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

// Region lookups are pure but expensive (each candidate site samples the
// terrain noise dozens of times), and every generated chunk consults up to
// four regions. Cache results process-wide, keyed by seed so tests and
// reseeded worlds can't cross-contaminate. A long session that explores far
// enough visits unboundedly many distinct regions, so the map is capped: once
// it would grow past REGION_CACHE_CAP entries, it's dropped and rebuilt from
// scratch rather than growing forever.
type RegionCache = RwLock<HashMap<(u64, i32, i32), Option<Arc<Village>>>>;
static REGION_CACHE: OnceLock<RegionCache> = OnceLock::new();
const REGION_CACHE_CAP: usize = 20_000;

pub(crate) fn region_village(seed: u64, rx: i32, rz: i32) -> Option<Arc<Village>> {
    let cache = REGION_CACHE.get_or_init(Default::default);
    if let Ok(map) = cache.read()
        && let Some(hit) = map.get(&(seed, rx, rz))
    {
        return hit.clone();
    }
    // Compute outside the lock; a racing thread may duplicate the work,
    // which is harmless since the result is deterministic.
    let computed = Village::for_region(seed, rx, rz).map(Arc::new);
    if let Ok(mut map) = cache.write() {
        if map.len() >= REGION_CACHE_CAP {
            map.clear();
        }
        map.insert((seed, rx, rz), computed.clone());
    }
    computed
}

pub const REGION_CHUNKS: i32 = 24;
// How far roads run from the well, in blocks. Everything a village stamps
// stays within this reach plus one plot depth.
const ROAD_REACH: i32 = 34;
// Total half-extent of a village's influence, in blocks.
const VILLAGE_RADIUS: i32 = ROAD_REACH + 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StructureKind {
    Well,
    House,
    Farm,
    BellFrame,
    Lamp,
}

#[derive(Clone, Copy, Debug)]
pub struct Structure {
    pub kind: StructureKind,
    // Inclusive world-coordinate footprint
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
    pub floor_y: i32,
    // Door side: 0 = +x, 1 = -x, 2 = +z, 3 = -z
    pub facing: u8,
    pub variant: u32,
}

impl Structure {
    fn intersects_chunk(&self, cx: i32, cz: i32) -> bool {
        let bx0 = cx * CHUNK_WIDTH as i32;
        let bz0 = cz * CHUNK_DEPTH as i32;
        let bx1 = bx0 + CHUNK_WIDTH as i32 - 1;
        let bz1 = bz0 + CHUNK_DEPTH as i32 - 1;
        self.x0 <= bx1 && self.x1 >= bx0 && self.z0 <= bz1 && self.z1 >= bz0
    }
}

#[derive(Clone, Debug)]
pub struct Village {
    pub center_x: i32,
    pub center_z: i32,
    pub base_y: i32,
    pub desert: bool,
    pub structures: Vec<Structure>,
}

impl Village {
    /// The village (if any) rolled by the given region cell.
    pub fn for_region(seed: u64, rx: i32, rz: i32) -> Option<Village> {
        if hash_unit(seed, rx, rz, 9001) > 0.45 {
            return None;
        }

        // Try several candidate spots in the cell and settle the first
        // viable one. Candidates sit well inside the cell so a village
        // never leaks into a neighboring region.
        let margin = 5;
        let span = (REGION_CHUNKS - margin * 2) as u32;
        for attempt in 0..8i32 {
            let ccx = rx * REGION_CHUNKS
                + margin
                + (chunk_hash(seed, rx, rz, 9002 + attempt * 2) % span) as i32;
            let ccz = rz * REGION_CHUNKS
                + margin
                + (chunk_hash(seed, rx, rz, 9003 + attempt * 2) % span) as i32;
            let center_x = ccx * CHUNK_WIDTH as i32 + 8;
            let center_z = ccz * CHUNK_DEPTH as i32 + 8;

            let (center_h, biome) =
                terrain_height_and_biome(center_x as f32, center_z as f32, seed);
            if biome != Biome::Plains && biome != Biome::Desert {
                continue;
            }

            // Reject steep or beach-level plazas: sample a coarse grid
            // around the center. Individual plots re-check their own ground.
            let mut min_h = center_h;
            let mut max_h = center_h;
            for dx in [-16i32, -8, 0, 8, 16] {
                for dz in [-16i32, -8, 0, 8, 16] {
                    let (h, _) = terrain_height_and_biome(
                        (center_x + dx) as f32,
                        (center_z + dz) as f32,
                        seed,
                    );
                    min_h = min_h.min(h);
                    max_h = max_h.max(h);
                }
            }
            if min_h < 126 || max_h - min_h > 9 || max_h + 12 >= CHUNK_HEIGHT {
                continue;
            }
            let base_y = center_h as i32;

            let mut village = Village {
                center_x,
                center_z,
                base_y,
                desert: biome == Biome::Desert,
                structures: Vec::new(),
            };
            village.layout(seed, ccx, ccz);
            return Some(village);
        }
        None
    }

    fn layout(&mut self, seed: u64, ccx: i32, ccz: i32) {
        let mut rng = ChunkRng::new(seed, ccx, ccz, 9010);
        let cx = self.center_x;
        let cz = self.center_z;

        // Well plaza at the center
        self.structures.push(Structure {
            kind: StructureKind::Well,
            x0: cx - 2,
            z0: cz - 2,
            x1: cx + 2,
            z1: cz + 2,
            floor_y: self.base_y,
            facing: 0,
            variant: rng.next_u32(),
        });

        // Bell frame just off the plaza
        self.structures.push(Structure {
            kind: StructureKind::BellFrame,
            x0: cx + 4,
            z0: cz + 4,
            x1: cx + 6,
            z1: cz + 4,
            floor_y: self.base_y,
            facing: 0,
            variant: rng.next_u32(),
        });

        // Lamp posts at the four road ends
        for (dx, dz) in [(ROAD_REACH, 0), (-ROAD_REACH, 0), (0, ROAD_REACH), (0, -ROAD_REACH)] {
            let (h, _) = terrain_height_and_biome((cx + dx) as f32, (cz + dz) as f32, seed);
            let floor_y = h as i32;
            if (floor_y - self.base_y).abs() <= 6 {
                self.structures.push(Structure {
                    kind: StructureKind::Lamp,
                    x0: cx + dx,
                    z0: cz + dz,
                    x1: cx + dx,
                    z1: cz + dz,
                    floor_y,
                    facing: 0,
                    variant: rng.next_u32(),
                });
            }
        }

        // Building plots line both sides of both roads. Roll each plot:
        // house, farm, or left empty.
        let mut houses = 0u32;
        let offsets = [9i32, 21, 32];
        for &along in &offsets {
            for dir in [-1i32, 1] {
                for side in [-1i32, 1] {
                    // Plot on the X road, then the mirrored plot on the Z road
                    for axis in 0..2u8 {
                        let roll = rng.next_u32() % 100;
                        let (kind, w, d) = if roll < 55 {
                            let w = 5 + (rng.next_u32() % 3) as i32;
                            let d = 5 + (rng.next_u32() % 3) as i32;
                            (StructureKind::House, w, d)
                        } else if roll < 80 {
                            (StructureKind::Farm, 7, 9)
                        } else {
                            continue;
                        };
                        if kind == StructureKind::House {
                            if houses >= 6 {
                                continue;
                            }
                            houses += 1;
                        }

                        // Anchor: distance `along` down the road, plot pushed
                        // 3 blocks off the road edge.
                        let (ax, az, facing) = if axis == 0 {
                            (
                                cx + dir * along,
                                cz + side * (3 + d / 2 + 1),
                                if side > 0 { 3 } else { 2 },
                            )
                        } else {
                            (
                                cx + side * (3 + d / 2 + 1),
                                cz + dir * along,
                                if side > 0 { 1 } else { 0 },
                            )
                        };
                        // For plots along the Z road the footprint is rotated
                        let (w, d) = if axis == 0 { (w, d) } else { (d, w) };

                        let (h, _) =
                            terrain_height_and_biome(ax as f32, az as f32, seed);
                        let floor_y = h as i32;
                        if (floor_y - self.base_y).abs() > 5 {
                            continue;
                        }

                        let s = Structure {
                            kind,
                            x0: ax - w / 2,
                            z0: az - d / 2,
                            x1: ax - w / 2 + w - 1,
                            z1: az - d / 2 + d - 1,
                            floor_y,
                            facing,
                            variant: rng.next_u32(),
                        };
                        // Skip plots that would collide with the plaza or an
                        // earlier structure.
                        let collides = self.structures.iter().any(|o| {
                            s.x0 <= o.x1 + 1 && s.x1 >= o.x0 - 1 && s.z0 <= o.z1 + 1 && s.z1 >= o.z0 - 1
                        });
                        if !collides {
                            self.structures.push(s);
                        }
                    }
                }
            }
        }
    }

    pub fn house_count(&self) -> usize {
        self.structures
            .iter()
            .filter(|s| s.kind == StructureKind::House)
            .count()
    }

    /// Spawn points for the village's inhabitants, on the plaza edge.
    pub fn villager_spawns(&self) -> Vec<Vec3> {
        let n = 2 + self.house_count().min(6);
        let y = (self.base_y + 1) as f32;
        (0..n)
            .map(|i| {
                let angle = i as f32 / n as f32 * std::f32::consts::TAU;
                Vec3::new(
                    self.center_x as f32 + 0.5 + angle.cos() * 4.5,
                    y,
                    self.center_z as f32 + 0.5 + angle.sin() * 4.5,
                )
            })
            .collect()
    }

    pub fn golem_spawn(&self) -> Vec3 {
        Vec3::new(
            self.center_x as f32 - 3.5,
            (self.base_y + 1) as f32,
            self.center_z as f32 + 3.5,
        )
    }
}

/// Villages whose influence could touch the given chunk.
fn villages_near_chunk(seed: u64, cx: i32, cz: i32) -> Vec<Arc<Village>> {
    let reach_chunks = VILLAGE_RADIUS / CHUNK_WIDTH as i32 + 1;
    let r0x = (cx - reach_chunks).div_euclid(REGION_CHUNKS);
    let r1x = (cx + reach_chunks).div_euclid(REGION_CHUNKS);
    let r0z = (cz - reach_chunks).div_euclid(REGION_CHUNKS);
    let r1z = (cz + reach_chunks).div_euclid(REGION_CHUNKS);
    let mut villages = Vec::new();
    for rx in r0x..=r1x {
        for rz in r0z..=r1z {
            if let Some(v) = region_village(seed, rx, rz) {
                let bx = cx * CHUNK_WIDTH as i32 + 8;
                let bz = cz * CHUNK_DEPTH as i32 + 8;
                if (v.center_x - bx).abs() <= VILLAGE_RADIUS + 8
                    && (v.center_z - bz).abs() <= VILLAGE_RADIUS + 8
                {
                    villages.push(v);
                }
            }
        }
    }
    villages
}

/// Nearest village to a world position, searching outward by region rings.
/// `max_rings` bounds the search (each ring is REGION_CHUNKS * 16 blocks).
pub fn nearest_village(seed: u64, wx: i32, wz: i32, max_rings: i32) -> Option<Arc<Village>> {
    let rx0 = wx.div_euclid(REGION_CHUNKS * CHUNK_WIDTH as i32);
    let rz0 = wz.div_euclid(REGION_CHUNKS * CHUNK_DEPTH as i32);
    let mut best: Option<(i64, Arc<Village>)> = None;
    for ring in 0..=max_rings {
        for rx in (rx0 - ring)..=(rx0 + ring) {
            for rz in (rz0 - ring)..=(rz0 + ring) {
                if (rx - rx0).abs().max((rz - rz0).abs()) != ring {
                    continue;
                }
                if let Some(v) = region_village(seed, rx, rz) {
                    let dx = (v.center_x - wx) as i64;
                    let dz = (v.center_z - wz) as i64;
                    let d2 = dx * dx + dz * dz;
                    if best.as_ref().is_none_or(|(bd, _)| d2 < *bd) {
                        best = Some((d2, v));
                    }
                }
            }
        }
        // A hit this ring can't be beaten by a ring more than one further out
        if let Some((d2, _)) = &best {
            let safe = ((ring + 1) * REGION_CHUNKS * CHUNK_WIDTH as i32) as i64;
            if *d2 <= safe * safe {
                break;
            }
        }
    }
    best.map(|(_, v)| v)
}

/// The village whose center chunk is exactly (cx, cz), for mob spawning.
pub fn village_for_center_chunk(seed: u64, cx: i32, cz: i32) -> Option<Arc<Village>> {
    let rx = cx.div_euclid(REGION_CHUNKS);
    let rz = cz.div_euclid(REGION_CHUNKS);
    let v = region_village(seed, rx, rz)?;
    if v.center_x.div_euclid(CHUNK_WIDTH as i32) == cx
        && v.center_z.div_euclid(CHUNK_DEPTH as i32) == cz
    {
        Some(v)
    } else {
        None
    }
}

/// Stamp every village slice that intersects this chunk. Runs inside
/// Chunk::generate, before lighting.
pub fn stamp_chunk(chunk: &mut Chunk) {
    let villages = villages_near_chunk(chunk.seed, chunk.x, chunk.z);
    for village in villages {
        let mut stamper = Stamper { chunk };
        stamper.stamp_roads(&village);
        for s in &village.structures {
            if !s.intersects_chunk(stamper.chunk.x, stamper.chunk.z) {
                continue;
            }
            match s.kind {
                StructureKind::Well => stamper.build_well(&village, s),
                StructureKind::House => stamper.build_house(&village, s),
                StructureKind::Farm => stamper.build_farm(&village, s),
                StructureKind::BellFrame => stamper.build_bell_frame(&village, s),
                StructureKind::Lamp => stamper.build_lamp(&village, s),
            }
        }
    }
}

// Writes world-coordinate blocks into whatever slice of them falls inside
// one chunk.
struct Stamper<'a> {
    chunk: &'a mut Chunk,
}

impl Stamper<'_> {
    fn local(&self, wx: i32, wz: i32) -> Option<(usize, usize)> {
        let lx = wx - self.chunk.x * CHUNK_WIDTH as i32;
        let lz = wz - self.chunk.z * CHUNK_DEPTH as i32;
        if (0..CHUNK_WIDTH as i32).contains(&lx) && (0..CHUNK_DEPTH as i32).contains(&lz) {
            Some((lx as usize, lz as usize))
        } else {
            None
        }
    }

    fn set(&mut self, wx: i32, wy: i32, wz: i32, b: BlockType) {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return;
        }
        if let Some((lx, lz)) = self.local(wx, wz) {
            let y = wy as usize;
            self.chunk.blocks[lx][y][lz] = b;
            self.chunk.liquid_levels[lx][y][lz] =
                if b == BlockType::Water || b == BlockType::Lava {
                    crate::chunk::WATER_SOURCE
                } else {
                    0
                };
        }
    }

    /// Fill downward from just below floor_y until natural ground, so
    /// structures on uneven terrain get foundations instead of floating.
    fn foundation(&mut self, wx: i32, wz: i32, floor_y: i32, material: BlockType) {
        let Some((lx, lz)) = self.local(wx, wz) else {
            return;
        };
        let mut y = floor_y - 1;
        let floor_stop = (floor_y - 24).max(1);
        while y >= floor_stop {
            let existing = self.chunk.blocks[lx][y as usize][lz];
            let is_ground = matches!(
                existing,
                BlockType::Stone
                    | BlockType::Dirt
                    | BlockType::Grass
                    | BlockType::SnowyGrass
                    | BlockType::Sand
                    | BlockType::Sandstone
                    | BlockType::Gravel
                    | BlockType::Clay
                    | BlockType::Bedrock
            );
            if is_ground {
                break;
            }
            self.set(wx, y, wz, material);
            y -= 1;
        }
    }

    /// Clear space above a column so trees and terrain bumps don't poke
    /// through buildings and roads.
    fn clear_above(&mut self, wx: i32, wz: i32, from_y: i32, height: i32) {
        for y in from_y..from_y + height {
            self.set(wx, y, wz, BlockType::Air);
        }
    }

    fn stamp_roads(&mut self, v: &Village) {
        let road = if v.desert {
            BlockType::Sandstone
        } else {
            BlockType::Gravel
        };
        for lx in 0..CHUNK_WIDTH {
            for lz in 0..CHUNK_DEPTH {
                let wx = self.chunk.x * CHUNK_WIDTH as i32 + lx as i32;
                let wz = self.chunk.z * CHUNK_DEPTH as i32 + lz as i32;
                let on_x_road =
                    (wz - v.center_z).abs() <= 1 && (wx - v.center_x).abs() <= ROAD_REACH;
                let on_z_road =
                    (wx - v.center_x).abs() <= 1 && (wz - v.center_z).abs() <= ROAD_REACH;
                if !(on_x_road || on_z_road) {
                    continue;
                }
                // Leave the plaza to the well builder
                if (wx - v.center_x).abs() <= 2 && (wz - v.center_z).abs() <= 2 {
                    continue;
                }
                let Some(surface) = self.chunk.surface_y(lx, lz) else {
                    continue;
                };
                let surface = surface as i32;
                if (surface - v.base_y).abs() > 8 {
                    continue;
                }
                let top = self.chunk.blocks[lx][surface as usize][lz];
                if matches!(
                    top,
                    BlockType::Grass
                        | BlockType::SnowyGrass
                        | BlockType::Dirt
                        | BlockType::Sand
                        | BlockType::Stone
                ) {
                    self.set(wx, surface, wz, road);
                    self.clear_above(wx, wz, surface + 1, 5);
                }
            }
        }
    }

    fn build_house(&mut self, v: &Village, s: &Structure) {
        let (wall, corner, roof, floor) = if v.desert {
            (
                BlockType::Sandstone,
                BlockType::Cobblestone,
                BlockType::Sandstone,
                BlockType::Sandstone,
            )
        } else {
            (
                BlockType::OakPlanks,
                BlockType::OakLog,
                BlockType::OakLog,
                BlockType::OakPlanks,
            )
        };
        let fy = s.floor_y;

        for wx in s.x0..=s.x1 {
            for wz in s.z0..=s.z1 {
                self.foundation(wx, wz, fy, corner);
                self.set(wx, fy, wz, floor);
                let is_corner = (wx == s.x0 || wx == s.x1) && (wz == s.z0 || wz == s.z1);
                let is_edge = wx == s.x0 || wx == s.x1 || wz == s.z0 || wz == s.z1;
                for dy in 1..=3 {
                    let b = if is_corner {
                        corner
                    } else if is_edge {
                        wall
                    } else {
                        BlockType::Air
                    };
                    self.set(wx, fy + dy, wz, b);
                }
                self.set(wx, fy + 4, wz, roof);
                self.clear_above(wx, wz, fy + 5, 4);
            }
        }

        // Door: a 2-high gap centered on the facing side
        let (dx, dz) = match s.facing {
            0 => (s.x1, (s.z0 + s.z1) / 2),
            1 => (s.x0, (s.z0 + s.z1) / 2),
            2 => ((s.x0 + s.x1) / 2, s.z1),
            _ => ((s.x0 + s.x1) / 2, s.z0),
        };
        self.set(dx, fy + 1, dz, BlockType::Air);
        self.set(dx, fy + 2, dz, BlockType::Air);
        // Doorstep so the entrance is walkable from the road
        self.foundation(dx, dz, fy, corner);

        // Windows on the two sides adjacent to the door wall
        let mx = (s.x0 + s.x1) / 2;
        let mz = (s.z0 + s.z1) / 2;
        if s.facing < 2 {
            self.set(mx, fy + 2, s.z0, BlockType::Glass);
            self.set(mx, fy + 2, s.z1, BlockType::Glass);
        } else {
            self.set(s.x0, fy + 2, mz, BlockType::Glass);
            self.set(s.x1, fy + 2, mz, BlockType::Glass);
        }

        // Interior: torch in a back corner plus variant furniture
        let (bx, bz) = match s.facing {
            0 => (s.x0 + 1, s.z0 + 1),
            1 => (s.x1 - 1, s.z1 - 1),
            2 => (s.x0 + 1, s.z0 + 1),
            _ => (s.x1 - 1, s.z1 - 1),
        };
        self.set(bx, fy + 1, bz, BlockType::Torch);
        let furniture = match s.variant % 3 {
            0 => BlockType::CraftingTable,
            1 => BlockType::Furnace,
            _ => BlockType::Bookshelf,
        };
        // Opposite corner from the torch, still inside the walls
        let (ux, uz) = match s.facing {
            0 => (s.x0 + 1, s.z1 - 1),
            1 => (s.x1 - 1, s.z0 + 1),
            2 => (s.x1 - 1, s.z0 + 1),
            _ => (s.x0 + 1, s.z1 - 1),
        };
        self.set(ux, fy + 1, uz, furniture);
        if s.variant.is_multiple_of(4) {
            self.set(bx, fy + 1, uz, BlockType::Chest);
        }
    }

    fn build_farm(&mut self, v: &Village, s: &Structure) {
        let rim = if v.desert {
            BlockType::Sandstone
        } else {
            BlockType::OakLog
        };
        let fy = s.floor_y;
        for wx in s.x0..=s.x1 {
            for wz in s.z0..=s.z1 {
                self.foundation(wx, wz, fy, BlockType::Dirt);
                self.clear_above(wx, wz, fy + 1, 5);
                let is_edge = wx == s.x0 || wx == s.x1 || wz == s.z0 || wz == s.z1;
                if is_edge {
                    self.set(wx, fy, wz, rim);
                    continue;
                }
                // Irrigation channel down the long axis center
                let channel = if s.x1 - s.x0 >= s.z1 - s.z0 {
                    wz == (s.z0 + s.z1) / 2
                } else {
                    wx == (s.x0 + s.x1) / 2
                };
                if channel {
                    self.set(wx, fy, wz, BlockType::Water);
                } else {
                    self.set(wx, fy, wz, BlockType::Farmland);
                    if (wx + wz).rem_euclid(2) == 0 {
                        self.set(wx, fy + 1, wz, BlockType::Wheat);
                    }
                }
            }
        }
        // A torch on one rim corner keeps crops visible at night
        self.set(s.x0, fy + 1, s.z0, BlockType::Torch);
    }

    fn build_well(&mut self, v: &Village, s: &Structure) {
        let stone = if s.variant.is_multiple_of(3) {
            BlockType::MossyCobblestone
        } else {
            BlockType::Cobblestone
        };
        let fy = s.floor_y;
        for wx in s.x0..=s.x1 {
            for wz in s.z0..=s.z1 {
                self.foundation(wx, wz, fy, BlockType::Cobblestone);
                self.clear_above(wx, wz, fy + 1, 6);
                let is_edge = wx == s.x0 || wx == s.x1 || wz == s.z0 || wz == s.z1;
                if is_edge {
                    self.set(wx, fy, wz, stone);
                } else {
                    // Water basin two deep with a stone bottom
                    self.set(wx, fy - 2, wz, BlockType::Cobblestone);
                    self.set(wx, fy - 1, wz, BlockType::Water);
                    self.set(wx, fy, wz, BlockType::Water);
                }
                if v.desert && is_edge {
                    // Desert wells trade cobble for sandstone rims
                    self.set(wx, fy, wz, BlockType::Sandstone);
                }
            }
        }
        // Corner posts and a flat canopy
        for (wx, wz) in [(s.x0, s.z0), (s.x0, s.z1), (s.x1, s.z0), (s.x1, s.z1)] {
            self.set(wx, fy + 1, wz, stone);
            self.set(wx, fy + 2, wz, stone);
        }
        for wx in s.x0..=s.x1 {
            for wz in s.z0..=s.z1 {
                self.set(wx, fy + 3, wz, stone);
            }
        }
    }

    fn build_bell_frame(&mut self, v: &Village, s: &Structure) {
        let post = if v.desert {
            BlockType::Sandstone
        } else {
            BlockType::OakLog
        };
        let fy = s.floor_y;
        for wx in s.x0..=s.x1 {
            self.foundation(wx, s.z0, fy, post);
            self.clear_above(wx, s.z0, fy + 1, 5);
        }
        // Two posts with a crossbar, bell hanging from the middle
        for dy in 1..=3 {
            self.set(s.x0, fy + dy, s.z0, post);
            self.set(s.x1, fy + dy, s.z0, post);
        }
        let mid = (s.x0 + s.x1) / 2;
        self.set(mid, fy + 3, s.z0, post);
        self.set(mid, fy + 2, s.z0, BlockType::Bell);
    }

    fn build_lamp(&mut self, v: &Village, s: &Structure) {
        let post = if v.desert {
            BlockType::Sandstone
        } else {
            BlockType::OakLog
        };
        let fy = s.floor_y;
        self.foundation(s.x0, s.z0, fy, post);
        self.set(s.x0, fy, s.z0, post);
        self.clear_above(s.x0, s.z0, fy + 1, 4);
        self.set(s.x0, fy + 1, s.z0, post);
        self.set(s.x0, fy + 2, s.z0, post);
        self.set(s.x0, fy + 3, s.z0, BlockType::Torch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn find_test_village(seed: u64) -> (i32, i32, Village) {
        for rx in -20..20 {
            for rz in -20..20 {
                if let Some(v) = Village::for_region(seed, rx, rz) {
                    return (rx, rz, v);
                }
            }
        }
        panic!("no village found in 40x40 regions — placement is likely broken");
    }

    #[test]
    fn village_layout_is_deterministic() {
        let seed = 12_345u64;
        let (rx, rz, first) = find_test_village(seed);
        let second = Village::for_region(seed, rx, rz).unwrap();
        assert_eq!(first.center_x, second.center_x);
        assert_eq!(first.structures.len(), second.structures.len());
        for (a, b) in first.structures.iter().zip(second.structures.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!((a.x0, a.z0, a.x1, a.z1, a.floor_y), (b.x0, b.z0, b.x1, b.z1, b.floor_y));
        }
    }

    #[test]
    fn village_has_core_structures() {
        let (_, _, v) = find_test_village(12_345);
        assert!(v.structures.iter().any(|s| s.kind == StructureKind::Well));
        assert!(
            v.structures
                .iter()
                .any(|s| s.kind == StructureKind::BellFrame)
        );
    }

    #[test]
    fn village_stays_inside_its_region() {
        let seed = 999u64;
        let (rx, rz, v) = find_test_village(seed);
        let cell_x0 = rx * REGION_CHUNKS * CHUNK_WIDTH as i32;
        let cell_z0 = rz * REGION_CHUNKS * CHUNK_DEPTH as i32;
        let cell_x1 = cell_x0 + REGION_CHUNKS * CHUNK_WIDTH as i32;
        let cell_z1 = cell_z0 + REGION_CHUNKS * CHUNK_DEPTH as i32;
        for s in &v.structures {
            assert!(s.x0 >= cell_x0 && s.x1 < cell_x1, "structure leaks region in x");
            assert!(s.z0 >= cell_z0 && s.z1 < cell_z1, "structure leaks region in z");
        }
    }

    #[test]
    fn structures_do_not_overlap() {
        let (_, _, v) = find_test_village(4242);
        for (i, a) in v.structures.iter().enumerate() {
            for b in v.structures.iter().skip(i + 1) {
                let overlap =
                    a.x0 <= b.x1 && a.x1 >= b.x0 && a.z0 <= b.z1 && a.z1 >= b.z0;
                assert!(!overlap, "{:?} overlaps {:?}", a, b);
            }
        }
    }

    #[test]
    fn cached_region_lookup_matches_fresh_compute() {
        let seed = 12_345u64;
        let (rx, rz, fresh) = find_test_village(seed);
        // First call populates the cache, second call hits it
        let a = region_village(seed, rx, rz).expect("cached lookup");
        let b = region_village(seed, rx, rz).expect("cache hit");
        assert_eq!(a.center_x, fresh.center_x);
        assert_eq!(a.structures.len(), fresh.structures.len());
        assert!(Arc::ptr_eq(&a, &b), "second lookup should share the cached Arc");
    }

    #[test]
    fn center_chunk_lookup_matches_placement() {
        let seed = 12_345u64;
        let (_, _, v) = find_test_village(seed);
        let cx = v.center_x.div_euclid(CHUNK_WIDTH as i32);
        let cz = v.center_z.div_euclid(CHUNK_DEPTH as i32);
        let found = village_for_center_chunk(seed, cx, cz).expect("center chunk lookup");
        assert_eq!(found.center_x, v.center_x);
        assert_eq!(found.center_z, v.center_z);
        assert!(village_for_center_chunk(seed, cx + 1, cz).is_none());
    }

    #[test]
    fn village_chunks_stamp_deterministically() {
        let seed = 12_345u64;
        let (_, _, v) = find_test_village(seed);
        let cx = v.center_x.div_euclid(CHUNK_WIDTH as i32);
        let cz = v.center_z.div_euclid(CHUNK_DEPTH as i32);
        let mut a = Chunk::new(cx, cz, seed);
        let mut b = Chunk::new(cx, cz, seed);
        a.generate();
        b.generate();
        assert_eq!(a.blocks, b.blocks);
        // The well pumps water into the center chunk, so the village
        // definitely left a mark.
        let has_bell_or_cobble = a.blocks.iter().flatten().flatten().any(|&blk| {
            blk == BlockType::Cobblestone || blk == BlockType::MossyCobblestone
        });
        assert!(has_bell_or_cobble, "village center chunk shows no stamping");
    }
}


#[cfg(test)]
mod layout_probe {
    use super::*;

    #[test]
    #[ignore]
    fn print_village_map() {
        let seed = 12_345u64;
        let (_, _, v) = super::tests::find_test_village(seed);
        let r = 40i32;
        let x0 = v.center_x - r;
        let z0 = v.center_z - r;
        // Generate all chunks covering the area, then sample surfaces
        let c0x = (x0).div_euclid(16);
        let c1x = (v.center_x + r).div_euclid(16);
        let c0z = (z0).div_euclid(16);
        let c1z = (v.center_z + r).div_euclid(16);
        let mut chunks = std::collections::HashMap::new();
        for cx in c0x..=c1x {
            for cz in c0z..=c1z {
                let mut c = Chunk::new(cx, cz, seed);
                c.generate();
                chunks.insert((cx, cz), c);
            }
        }
        for wz in z0..=(v.center_z + r) {
            let mut row = String::new();
            for wx in x0..=(v.center_x + r) {
                let c = &chunks[&(wx.div_euclid(16), wz.div_euclid(16))];
                let (lx, lz) = (wx.rem_euclid(16) as usize, wz.rem_euclid(16) as usize);
                let mut ch = '.';
                for y in (1..255usize).rev() {
                    let b = c.blocks[lx][y][lz];
                    ch = match b {
                        BlockType::Air => continue,
                        BlockType::Gravel => '#',
                        BlockType::OakPlanks | BlockType::OakLog => 'H',
                        BlockType::Sandstone => 'S',
                        BlockType::Cobblestone | BlockType::MossyCobblestone => 'W',
                        BlockType::Farmland | BlockType::Wheat => 'f',
                        BlockType::Water => '~',
                        BlockType::Bell => 'B',
                        BlockType::Torch => 't',
                        BlockType::Glass => 'g',
                        BlockType::OakLeaves | BlockType::SpruceLeaves => '%',
                        BlockType::Grass | BlockType::SnowyGrass => '.',
                        BlockType::Sand => ',',
                        _ => '?',
                    };
                    break;
                }
                row.push(ch);
            }
            println!("{row}");
        }
        println!("center X={} Z={} base_y={} desert={} structures={}",
            v.center_x, v.center_z, v.base_y, v.desert, v.structures.len());
    }
}

#[cfg(test)]
mod seed_probe {
    use super::*;

    #[test]
    #[ignore]
    fn find_nearest_village_for_seed() {
        let seed: u64 = std::env::var("VP_SEED")
            .expect("set VP_SEED")
            .parse::<i64>()
            .expect("VP_SEED must be an integer") as u64;
        match nearest_village(seed, 32, 32, 12) {
            Some(v) => println!(
                "nearest village: X={} Z={} desert={} base_y={} structures={}",
                v.center_x, v.center_z, v.desert, v.base_y, v.structures.len()
            ),
            None => println!("no village within 12 regions"),
        }
    }
}
