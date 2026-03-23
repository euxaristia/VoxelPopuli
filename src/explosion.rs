use crate::block::{BlockType, Particle};
use crate::world::World;
use glam::Vec3;
use rand::RngExt;

pub fn explode(world: &mut World, x: i32, y: i32, z: i32, initial_radius: i32, is_nuke: bool) {
    let mut total_radius = initial_radius as f32;
    let mut nuke_count = 1;

    if is_nuke {
        // Exponential Multiplication: Check adjacent blocks for more Nukes
        let mut queue = vec![(x, y, z)];
        let mut checked = std::collections::HashSet::new();
        checked.insert((x, y, z));

        while let Some((qx, qy, qz)) = queue.pop() {
            let neighbors = [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ];
            for (dx, dy, dz) in neighbors {
                let nx = qx + dx;
                let ny = qy + dy;
                let nz = qz + dz;
                if !checked.contains(&(nx, ny, nz))
                    && world.get_block(nx, ny, nz) == BlockType::Nuke
                {
                    world.set_block(nx, ny, nz, BlockType::Air);
                    nuke_count += 1;
                    queue.push((nx, ny, nz));
                    checked.insert((nx, ny, nz));
                }
            }
        }

        // Radius = BASE * 1.1^(nuke_count - 1) up to a tighter cap
        total_radius = (initial_radius as f32) * 1.1f32.powi(nuke_count - 1);
        if total_radius > 45.0 {
            total_radius = 45.0;
        } // Significantly lower cap for stability
    }

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
        life: 0.5,
        max_life: 0.5,
        scale: total_radius * 2.0,
    });

    // 2. Flying debris/smoke
    let particle_count = if is_nuke { 30 } else { 10 };
    for _ in 0..particle_count {
        let mut rng = rand::rng();
        let vx = rng.random_range(-30.0..30.0);
        let vy = rng.random_range(5.0..40.0);
        let vz = rng.random_range(-30.0..30.0);
        world.particles.push(Particle {
            position: Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
            velocity: Vec3::new(vx, vy, vz),
            life: rng.random_range(1.0..3.0),
            max_life: 3.0,
            scale: rng.random_range(0.2..1.5),
        });
    }
}
