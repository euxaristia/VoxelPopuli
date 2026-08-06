use crate::block::{BlockType, Particle};
use crate::world::World;
use glam::Vec3;
use rand::RngExt;

pub fn explode(world: &mut World, x: i32, y: i32, z: i32, radius: i32) {
    let total_radius = radius as f32;
    let r_int = total_radius as i32;
    let r2 = total_radius * total_radius;
    let explosion_center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);

    let mut newly_ignited_tnt = Vec::new();
    let mut last_chunk_coords: Option<(i32, i32)> = None;

    for dx in -r_int..=r_int {
        for dy in -r_int..=r_int {
            for dz in -r_int..=r_int {
                let dist_sq = (dx * dx + dy * dy + dz * dz) as f32;
                if dist_sq <= r2 + 0.5 {
                    let bx = x + dx;
                    let by = y + dy;
                    let bz = z + dz;

                    let b = world.get_block(bx, by, bz);
                    if b != BlockType::Air && b != BlockType::Bedrock {
                        if b == BlockType::TNT {
                            let mut rng = rand::rng();
                            let dir = Vec3::new(
                                dx as f32 + rng.random_range(-0.2..0.2),
                                dy as f32 + 0.3 + rng.random_range(0.1..0.3),
                                dz as f32 + rng.random_range(-0.2..0.2),
                            )
                            .normalize_or_zero();
                            let speed = rng.random_range(1.5..3.5);
                            newly_ignited_tnt.push(crate::block::ActiveExplosive {
                                position: Vec3::new(bx as f32, by as f32, bz as f32),
                                velocity: dir * speed,
                                fuse: rng.random_range(0.5..1.5),
                                initial_fuse: 2.0,
                            });
                        }
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

    world.explosives.extend(newly_ignited_tnt);

    // Apply blast shockwave impulse to all active explosives in range (TNT chain reactions!)
    for exp in &mut world.explosives {
        let exp_center = exp.position + Vec3::splat(0.5);
        let dist = exp_center.distance(explosion_center);
        let max_dist = total_radius * 1.5;
        if dist > 0.01 && dist <= max_dist {
            let dir = (exp_center - explosion_center).normalize();
            let force = (1.0 - (dist / max_dist).clamp(0.0, 1.0)) * 3.5;
            exp.velocity += dir * force;
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
