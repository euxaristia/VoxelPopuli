use super::World;
use crate::block::BlockType;
use crate::chunk::{CHUNK_HEIGHT, unpack_light, water_render_height};
use glam::Vec3;
use std::collections::HashMap;

pub const EXTENT: [u32; 3] = [96, 64, 96];
const BRICK: i32 = 16;
const BRICK_BYTES: usize = 16 * 16 * 16 * 4;

struct Brick {
    revision: Option<u64>,
    pixels: Vec<u8>,
}

#[derive(Default)]
pub struct RenderVolume {
    origin: Option<[i32; 3]>,
    bricks: HashMap<[i32; 3], Brick>,
}

impl RenderVolume {
    pub fn update(
        &mut self,
        world: &World,
        camera: Vec3,
        upload: impl FnMut([i32; 3], [u32; 3], [u32; 3], &[u8]),
    ) {
        let origin = volume_origin(camera);
        self.update_with(
            origin,
            |position| {
                if position[1] < 0 || position[1] >= CHUNK_HEIGHT as i32 {
                    Some(0)
                } else {
                    world
                        .get_chunk(position[0].div_euclid(16), position[2].div_euclid(16))
                        .map(|chunk| chunk.render_revision)
                }
            },
            |position| sample_world(world, position),
            upload,
        );
    }

    fn update_with(
        &mut self,
        origin: [i32; 3],
        revision: impl Fn([i32; 3]) -> Option<u64>,
        sample: impl Fn([i32; 3]) -> [u8; 4],
        mut upload: impl FnMut([i32; 3], [u32; 3], [u32; 3], &[u8]),
    ) {
        let moved = self.origin != Some(origin);
        self.bricks.retain(|key, _| {
            (0..3).all(|axis| {
                key[axis] >= origin[axis] && key[axis] < origin[axis] + EXTENT[axis] as i32
            })
        });
        for z in (0..EXTENT[2]).step_by(BRICK as usize) {
            for y in (0..EXTENT[1]).step_by(BRICK as usize) {
                for x in (0..EXTENT[0]).step_by(BRICK as usize) {
                    let position = [
                        origin[0] + x as i32,
                        origin[1] + y as i32,
                        origin[2] + z as i32,
                    ];
                    let current_revision = revision(position);
                    let changed = self
                        .bricks
                        .get(&position)
                        .is_none_or(|brick| brick.revision != current_revision);
                    if changed {
                        let mut pixels = self
                            .bricks
                            .remove(&position)
                            .map_or_else(|| vec![0; BRICK_BYTES], |brick| brick.pixels);
                        for bz in 0..BRICK {
                            for by in 0..BRICK {
                                for bx in 0..BRICK {
                                    let index =
                                        ((bz * BRICK * BRICK + by * BRICK + bx) * 4) as usize;
                                    let voxel = sample([
                                        position[0] + bx,
                                        position[1] + by,
                                        position[2] + bz,
                                    ]);
                                    pixels[index..index + 4].copy_from_slice(&voxel);
                                }
                            }
                        }
                        self.bricks.insert(
                            position,
                            Brick {
                                revision: current_revision,
                                pixels,
                            },
                        );
                    }
                    if changed || moved {
                        let brick = &self.bricks[&position];
                        upload(origin, [x, y, z], [16, 16, 16], &brick.pixels);
                    }
                }
            }
        }
        self.origin = Some(origin);
    }
}

fn volume_origin(camera: Vec3) -> [i32; 3] {
    let coordinates = camera.to_array();
    std::array::from_fn(|axis| {
        let cell = if coordinates[axis].is_finite() {
            coordinates[axis]
                .clamp(-1_000_000_000.0, 1_000_000_000.0)
                .floor() as i32
        } else {
            0
        };
        cell.div_euclid(BRICK) * BRICK - EXTENT[axis] as i32 / 2
    })
}

fn encode_voxel(block: BlockType, light: u8, level: u8) -> [u8; 4] {
    let opaque = block.is_solid() && !block.is_transparent();
    let sky = unpack_light(light).0;
    let water = block == BlockType::Water;
    [
        if opaque { 255 } else { 128 },
        sky * 17,
        if water { 255 } else { 0 },
        if water {
            (water_render_height(level) * 255.0).round() as u8
        } else {
            0
        },
    ]
}

fn sample_world(world: &World, [x, y, z]: [i32; 3]) -> [u8; 4] {
    if y < 0 {
        return [255, 0, 0, 0];
    }
    if y >= CHUNK_HEIGHT as i32 {
        return [128, 255, 0, 0];
    }
    let Some(chunk) = world.get_chunk(x.div_euclid(16), z.div_euclid(16)) else {
        return [0; 4];
    };
    let (bx, by, bz) = (
        x.rem_euclid(16) as usize,
        y as usize,
        z.rem_euclid(16) as usize,
    );
    encode_voxel(
        chunk.blocks[bx][by][bz],
        chunk.light[bx][by][bz],
        chunk.liquid_levels[bx][by][bz],
    )
}

pub fn camera_in_water(world: &World, camera: Vec3) -> bool {
    if !camera.is_finite() {
        return false;
    }
    let [x, y, z] = camera.floor().as_ivec3().to_array();
    if world.get_block(x, y, z) != BlockType::Water {
        return false;
    }
    if world.get_block(x, y + 1, z) == BlockType::Water {
        return true;
    }
    let corner = |cx, cz| {
        let mut height = 0.0;
        let mut count = 0;
        for dx in [-1, 0] {
            for dz in [-1, 0] {
                let (bx, bz) = (cx + dx, cz + dz);
                if world.get_block(bx, y + 1, bz) == BlockType::Water {
                    return 1.0;
                }
                let block = world.get_block(bx, y, bz);
                if block == BlockType::Water {
                    height += water_render_height(world.get_liquid_level(bx, y, bz));
                    count += 1;
                } else if !block.is_solid() {
                    count += 1;
                }
            }
        }
        if count == 0 {
            1.0
        } else {
            height / count as f32
        }
    };
    let (u, v) = (camera.x - x as f32, camera.z - z as f32);
    let h00 = corner(x, z);
    let h10 = corner(x + 1, z);
    let h11 = corner(x + 1, z + 1);
    let h01 = corner(x, z + 1);
    // Match the two triangles emitted by the liquid mesher, not a bilinear patch.
    let surface = if v >= u {
        h00 + (h11 - h01) * u + (h01 - h00) * v
    } else {
        h00 + (h10 - h00) * u + (h11 - h10) * v
    };
    camera.y - (y as f32) < surface
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, WATER_SOURCE, pack_light};
    use std::cell::Cell;

    #[test]
    fn volume_distinguishes_unknown_air_solid_water_and_lava() {
        assert_eq!(
            encode_voxel(BlockType::Air, pack_light(15, 0), 0),
            [128, 255, 0, 0]
        );
        assert_eq!(encode_voxel(BlockType::Stone, 0, 0), [255, 0, 0, 0]);
        assert_eq!(
            encode_voxel(BlockType::Water, pack_light(9, 0), WATER_SOURCE),
            [128, 153, 255, 255]
        );
        assert_eq!(encode_voxel(BlockType::Lava, 0, WATER_SOURCE)[2..], [0, 0]);
        assert!(encode_voxel(BlockType::Water, 0, 7)[3] < 100);
    }

    #[test]
    fn negative_camera_positions_use_euclidean_brick_alignment() {
        assert_eq!(volume_origin(Vec3::new(-0.1, 100.0, -16.1)), [-64, 64, -80]);
        let origin = volume_origin(Vec3::splat(f32::NAN));
        assert_eq!(origin, [-48, -32, -48]);
    }

    #[test]
    fn stable_frames_do_not_resample_or_upload_the_world() {
        let mut volume = RenderVolume::default();
        let samples = Cell::new(0);
        let uploads = Cell::new(0);
        let sample = |_| {
            samples.set(samples.get() + 1);
            [128, 255, 0, 0]
        };
        let upload = |_, _, _, _: &[u8]| uploads.set(uploads.get() + 1);
        volume.update_with([0; 3], |_| Some(1), sample, upload);
        assert_eq!(samples.get(), 96 * 64 * 96);
        assert_eq!(uploads.get(), 6 * 4 * 6);
        samples.set(0);
        uploads.set(0);
        volume.update_with([0; 3], |_| Some(1), sample, upload);
        assert_eq!(samples.get(), 0);
        assert_eq!(uploads.get(), 0);
    }

    #[test]
    fn camera_scroll_reuses_overlapping_bricks_and_bounds_storage() {
        let mut volume = RenderVolume::default();
        volume.update_with([0; 3], |_| Some(1), |_| [128, 255, 0, 0], |_, _, _, _| {});
        let samples = Cell::new(0);
        volume.update_with(
            [16, 0, 0],
            |_| Some(1),
            |_| {
                samples.set(samples.get() + 1);
                [128, 255, 0, 0]
            },
            |_, _, _, _| {},
        );
        assert_eq!(samples.get(), 16 * 64 * 96);
        assert_eq!(volume.bricks.len(), 6 * 4 * 6);
    }

    #[test]
    fn edits_and_unloads_replace_cached_voxels_without_stale_water() {
        let mut volume = RenderVolume::default();
        volume.update_with(
            [0; 3],
            |_| Some(1),
            |_| [128, 255, 255, 255],
            |_, _, _, _| {},
        );
        let mut changed = 0;
        volume.update_with(
            [0; 3],
            |p| {
                if p[0] == 0 && p[2] == 0 {
                    None
                } else {
                    Some(1)
                }
            },
            |_| [0; 4],
            |_, offset, _, pixels| {
                changed += 1;
                assert_eq!(offset[0], 0);
                assert_eq!(offset[2], 0);
                assert!(pixels.iter().all(|value| *value == 0));
            },
        );
        assert_eq!(changed, 4);
        assert_eq!(volume.bricks.len(), 144);
    }

    #[test]
    fn camera_submersion_uses_the_rendered_surface_not_sea_level() {
        let mut world = super::World::simulation(7);
        let mut chunk = Chunk::new(0, 0, 7);
        chunk.set_block(1, 40, 1, BlockType::Water);
        world.insert_chunk(chunk);
        assert!(camera_in_water(&world, Vec3::new(1.2, 40.2, 1.2)));
        assert!(!camera_in_water(&world, Vec3::new(1.2, 41.2, 1.2)));
        world.set_block(1, 40, 1, BlockType::Stone);
        assert!(!camera_in_water(&world, Vec3::new(1.2, 40.2, 1.2)));
    }

    #[test]
    fn chunk_revisions_track_edits_lighting_and_slot_reuse() {
        let mut chunk = Chunk::new(0, 0, 42);
        let initial = chunk.render_revision;
        chunk.set_block(1, 2, 3, BlockType::Stone);
        assert_ne!(chunk.render_revision, initial);
        let edited = chunk.render_revision;
        chunk.calculate_lighting();
        assert_ne!(chunk.render_revision, edited);
        let replacement = Chunk::new(0, 0, 42);
        assert_ne!(replacement.render_revision, chunk.render_revision);
        chunk.copy_terrain_from(&replacement);
        assert_ne!(chunk.render_revision, replacement.render_revision);
    }
}
