use crate::block::{ActiveExplosive, BlockType, Particle};
use crate::world::World;
use glam::Vec3;
use rand::RngExt;
use std::collections::HashSet;

pub fn explode(world: &mut World, x: i32, y: i32, z: i32, blast_size: i32) {
    let mut rng = rand::rng();
    let explosion_center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
    let size = blast_size as f32; // Standard TNT blast size = 4.0

    // 1. Raytracing Sphere Pass (Minecraft Vanilla 16x16x16 face sampling)
    let mut destroyed_blocks = HashSet::new();

    for xi in 0..16 {
        for yi in 0..16 {
            for zi in 0..16 {
                // Only sample rays ending on the outer surfaces of the 16x16x16 cube
                if xi == 0 || xi == 15 || yi == 0 || yi == 15 || zi == 0 || zi == 15 {
                    let rx = xi as f32 / 15.0 * 2.0 - 1.0;
                    let ry = yi as f32 / 15.0 * 2.0 - 1.0;
                    let rz = zi as f32 / 15.0 * 2.0 - 1.0;
                    let dir = Vec3::new(rx, ry, rz).normalize_or_zero();

                    let mut ray_power = size * (0.7 + rng.random_range(0.0..0.6));
                    let step = 0.3f32;
                    let mut current_pos = explosion_center;

                    while ray_power > 0.0 {
                        let bx = current_pos.x.floor() as i32;
                        let by = current_pos.y.floor() as i32;
                        let bz = current_pos.z.floor() as i32;

                        let block = world.get_block(bx, by, bz);
                        if block != BlockType::Air {
                            let resistance = block.blast_resistance();
                            let attenuation = (resistance / 5.0 + 0.3) * step;
                            ray_power -= attenuation;

                            if ray_power > 0.0 {
                                destroyed_blocks.insert((bx, by, bz));
                            }
                        }

                        ray_power -= 0.75 * step;
                        current_pos += dir * step;
                    }
                }
            }
        }
    }

    // 2. Destroy Blocks & Process Ignitions
    let mut newly_ignited_tnt = Vec::new();
    for (bx, by, bz) in destroyed_blocks {
        let b = world.get_block(bx, by, bz);
        if b == BlockType::Bedrock || b == BlockType::Air {
            continue;
        }

        if b == BlockType::TNT {
            let block_center = Vec3::new(bx as f32 + 0.5, by as f32 + 0.5, bz as f32 + 0.5);
            let dir = (block_center - explosion_center).normalize_or_zero();
            let speed = rng.random_range(2.5..6.0);
            let fuse = rng.random_range(0.5..1.5); // 10..30 ticks in MC
            newly_ignited_tnt.push(ActiveExplosive {
                position: Vec3::new(bx as f32, by as f32, bz as f32),
                velocity: dir * speed + Vec3::new(0.0, 1.5, 0.0),
                fuse,
                initial_fuse: fuse,
            });
        }
        world.set_block(bx, by, bz, BlockType::Air);
    }
    world.explosives.extend(newly_ignited_tnt);

    // 3. Shockwave Impulse on Entities (Primed TNT & Mobs)
    let max_impact_dist = size * 2.0; // 8.0 blocks

    // Shockwave on active TNT entities (chain reaction physics)
    for exp in &mut world.explosives {
        let exp_center = exp.position + Vec3::splat(0.5);
        let dist = exp_center.distance(explosion_center);
        if dist > 0.01 && dist <= max_impact_dist {
            let impact = (1.0 - dist / max_impact_dist).clamp(0.0, 1.0);
            let dir = (exp_center - explosion_center).normalize_or_zero();
            let force = impact * 14.0;
            exp.velocity += dir * force + Vec3::new(0.0, 2.0 * impact, 0.0);
        }
    }

    // Shockwave on Mobs
    for mob in &mut world.mobs {
        let mob_center = mob.position + Vec3::new(0.0, mob.height() * 0.5, 0.0);
        let dist = mob_center.distance(explosion_center);
        if dist <= max_impact_dist {
            let impact = (1.0 - dist / max_impact_dist).clamp(0.0, 1.0);
            let dir = (mob_center - explosion_center).normalize_or_zero();
            let force = impact * 16.0;
            mob.velocity += dir * force + Vec3::new(0.0, 3.0 * impact, 0.0);
            mob.grounded = false;
        }
    }

    // 4. Spawn Particles (Shockwave, Flash & Smoke)
    world.particles.push(Particle {
        position: explosion_center,
        velocity: Vec3::ZERO,
        color: glam::Vec4::new(1.0, 0.92, 0.68, 0.85),
        life: 0.4,
        max_life: 0.4,
        scale: size * 2.2,
    });

    for _ in 0..16 {
        let vx = rng.random_range(-25.0..25.0);
        let vy = rng.random_range(6.0..35.0);
        let vz = rng.random_range(-25.0..25.0);
        world.particles.push(Particle {
            position: explosion_center,
            velocity: Vec3::new(vx, vy, vz),
            color: glam::Vec4::new(0.45, 0.42, 0.38, 0.85),
            life: rng.random_range(0.8..2.5),
            max_life: 2.5,
            scale: rng.random_range(0.3..1.6),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blast_resistance_values() {
        assert_eq!(BlockType::Bedrock.blast_resistance(), 3_600_000.0);
        assert_eq!(BlockType::Obsidian.blast_resistance(), 1200.0);
        assert_eq!(BlockType::Stone.blast_resistance(), 30.0);
        assert_eq!(BlockType::Dirt.blast_resistance(), 2.5);
        assert_eq!(BlockType::TNT.blast_resistance(), 0.0);
        assert_eq!(BlockType::Air.blast_resistance(), 0.0);
    }

    #[test]
    fn test_tnt_chain_ignition_logic() {
        let mut rng = rand::rng();
        let center = Vec3::new(10.5, 10.5, 10.5);
        let tnt_pos = Vec3::new(12.5, 10.5, 10.5);
        let dir = (tnt_pos - center).normalize_or_zero();
        let speed = rng.random_range(2.5..6.0);
        let active = ActiveExplosive {
            position: tnt_pos,
            velocity: dir * speed + Vec3::new(0.0, 1.5, 0.0),
            fuse: 1.0,
            initial_fuse: 1.0,
        };
        assert!(active.velocity.y > 1.0);
        assert!(active.fuse >= 0.5 && active.fuse <= 1.5);
    }
}
