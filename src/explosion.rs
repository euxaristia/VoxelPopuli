use crate::block::{BlockType, Particle};
use crate::world::World;
use glam::Vec3;
use rand::RngExt;

pub fn explode(world: &mut World, x: i32, y: i32, z: i32, radius: i32) {
    let total_radius = radius as f32;
    let r_int = total_radius as i32;
    let r2 = total_radius * total_radius;

    // Perform destruction with chunk caching
    let mut last_chunk_coords: Option<(i32, i32)> = None;

    for dx in -r_int..=r_int {
        for dy in -r_int..=r_int {
            for dz in -r_int..=r_int {
                // Use a slightly more inclusive check to avoid floating grass/trees
                let dist_sq = (dx * dx + dy * dy + dz * dz) as f32;
                if dist_sq <= r2 + 0.5 {
                    let bx = x + dx;
                    let by = y + dy;
                    let bz = z + dz;

                    let b = world.get_block(bx, by, bz);
                    if b != BlockType::Air && b != BlockType::Bedrock {
                        world.set_block(bx, by, bz, BlockType::Air);

                        let cx = bx.div_euclid(16);
                        let cz = bz.div_euclid(16);
                        if last_chunk_coords != Some((cx, cz)) {
                            last_chunk_coords = Some((cx, cz));
                        }
                    }
                }
            }
        }
    }

    // Spawn Particles (Shockwave/Flash)
    // 1. Core Shockwave
    world.particles.push(Particle {
        position: Vec3::new(x as f32 + 0.1, y as f32 + 0.1, z as f32 + 0.1),
        velocity: Vec3::ZERO,
        color: glam::Vec4::new(1.0, 0.92, 0.68, 0.75),
        life: 0.5,
        max_life: 0.5,
        scale: total_radius * 2.0,
    });

    // 2. Flying debris/smoke
    for _ in 0..10 {
        let mut rng = rand::rng();
        let vx = rng.random_range(-30.0..30.0);
        let vy = rng.random_range(5.0..40.0);
        let vz = rng.random_range(-30.0..30.0);
        world.particles.push(Particle {
            position: Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
            velocity: Vec3::new(vx, vy, vz),
            color: glam::Vec4::new(0.55, 0.52, 0.48, 0.85),
            life: rng.random_range(1.0..3.0),
            max_life: 3.0,
            scale: rng.random_range(0.2..1.5),
        });
    }
}
