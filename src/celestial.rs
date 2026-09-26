//! Textured celestial bodies sharing the orbit used by deferred lighting.
use crate::renderer::{self, Mesh, Shader, Texture2D};
use glam::{Mat4, Vec3, Vec4};

pub struct Celestial {
    sun: Texture2D,
    moon: Texture2D,
    quad: Mesh,
}

pub fn phase_uv(day: u64) -> Vec4 {
    let phase = (day % 8) as u32;
    Vec4::new(
        (phase % 4) as f32 * 0.25,
        (phase / 4) as f32 * 0.5,
        0.25,
        0.5,
    )
}

fn body_transform(eye: Vec3, direction: Vec3, half_size: f32) -> Mat4 {
    // A fixed orbit axis avoids the cross-with-up singularity at noon.
    let axis = if direction.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let up = direction.cross(axis).normalize();
    let right = up.cross(direction).normalize();
    Mat4::from_cols(
        (right * half_size).extend(0.0),
        (up * half_size).extend(0.0),
        direction.extend(0.0),
        (eye + direction * 450.0).extend(1.0),
    )
}

fn sun_pixels() -> Vec<u8> {
    let mut pixels = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let r = ((x as f32 - 31.5).abs().max((y as f32 - 31.5).abs())) / 32.0;
            let glow = ((1.0 - r) / 0.78).clamp(0.0, 1.0).powi(5);
            let core = r < 0.22;
            pixels.extend_from_slice(&[
                255,
                if core { 251 } else { 199 },
                if core { 222 } else { 115 },
                (glow * 255.0) as u8,
            ]);
        }
    }
    pixels
}

fn moon_pixels() -> Vec<u8> {
    let mut pixels = vec![0; 128 * 64 * 4];
    for phase in 0..8 {
        let angle = phase as f32 * std::f32::consts::FRAC_PI_4;
        for y in 0..32 {
            for x in 0..32 {
                let nx = (x as f32 - 15.5) / 12.0;
                let ny = (y as f32 - 15.5) / 12.0;
                if nx.abs().max(ny.abs()) > 1.0 {
                    continue;
                }
                let depth = (1.0 - nx * nx).max(0.0).sqrt();
                if depth * angle.cos() - nx * angle.sin() <= 0.0 {
                    continue;
                }
                let crater = [(9i32, 10i32, 3i32), (21, 19, 4), (13, 24, 2)]
                    .iter()
                    .any(|&(cx, cy, r)| (x as i32 - cx).pow(2) + (y as i32 - cy).pow(2) <= r * r);
                let value = if crater {
                    151
                } else {
                    215 + ((x / 2 * 7 + y / 2 * 11) % 4) as u8 * 5
                };
                let at = (((phase / 4 * 32 + y) * 128) + phase % 4 * 32 + x) * 4;
                pixels[at..at + 4].copy_from_slice(&[value, value, value.saturating_add(8), 255]);
            }
        }
    }
    pixels
}

impl Celestial {
    pub fn new() -> Self {
        Self {
            sun: Texture2D::from_data(&sun_pixels(), 64, 64),
            moon: Texture2D::from_data(&moon_pixels(), 128, 64),
            quad: Mesh::new(
                &[
                    -1., -1., 0., 1., -1., 0., 1., 1., 0., -1., -1., 0., 1., 1., 0., -1., 1., 0.,
                ],
                Some(&[0., 1., 1., 1., 1., 0., 0., 1., 1., 0., 0., 0.]),
                None,
                Some(&[255; 24]),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        shader: &Shader,
        eye: Vec3,
        direction: Vec3,
        day: u64,
        mvp: Mat4,
        overlay: bool,
        hdr_scale: f32,
        hdr: bool,
    ) {
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_float(shader.get_uniform_location("uHdrScale"), hdr_scale);
        shader.set_int(shader.get_uniform_location("uHdrOutput"), i32::from(hdr));
        renderer::set_depth_test(!overlay);
        renderer::set_depth_write(false);
        renderer::set_blend(true);
        renderer::set_additive(true);
        renderer::set_cull(false);
        for (texture, dir, size, uv) in [
            (&self.sun, direction, 135.0, Vec4::new(0., 0., 1., 1.)),
            (&self.moon, -direction, 90.0, phase_uv(day)),
        ] {
            texture.bind(0);
            shader.set_mat4(
                shader.get_uniform_location("uModel"),
                &body_transform(eye, dir, size),
            );
            shader.set_vec4(shader.get_uniform_location("uColor"), uv);
            self.quad.draw();
        }
        renderer::set_cull(true);
        renderer::set_additive(false);
        renderer::set_depth_write(true);
        renderer::set_depth_test(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bodies_follow_orbit_instead_of_camera() {
        for (width, height) in [(800.0, 600.0), (1920.0, 1080.0), (600.0, 800.0)] {
            for fov in [60.0_f32, 90.0, 110.0] {
                for pitch in [0.0_f32, 30.0, 75.0] {
                    for yaw in [-30.0_f32, 0.0, 30.0] {
                        let eye = Vec3::new(71.0, 120.0, -35.0);
                        let pitch = pitch.to_radians();
                        let yaw = yaw.to_radians();
                        let forward = Vec3::new(
                            yaw.sin() * pitch.cos(),
                            pitch.sin(),
                            yaw.cos() * pitch.cos(),
                        );
                        let view =
                            glam::camera::rh::view::look_at_mat4(eye, eye + forward, Vec3::Y);
                        let mvp = glam::camera::rh::proj::directx::perspective(
                            fov.to_radians(),
                            width / height,
                            0.1,
                            1000.0,
                        ) * view;
                        for elevation in [35.0_f32, 45.0, 80.0, 90.0, 100.0, 145.0] {
                            let elevation = elevation.to_radians();
                            let dir = Vec3::new(0.0, elevation.sin(), elevation.cos());
                            for size in [90.0, 135.0] {
                                let model = body_transform(eye, dir, size);
                                for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                                    let expected = eye
                                        + dir * 450.0
                                        + Vec3::X * (x * size)
                                        + dir.cross(Vec3::X) * (y * size);
                                    let actual = model.transform_point3(Vec3::new(x, y, 0.0));
                                    assert!(
                                        actual.abs_diff_eq(expected, 0.0001),
                                        "camera changed orbit geometry: fov={fov}, pitch={pitch}, elevation={elevation}"
                                    );
                                    assert!(
                                        (mvp * actual.extend(1.0))
                                            .abs_diff_eq(mvp * expected.extend(1.0), 0.001)
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn original_sprites_have_transparent_edges_and_eight_phases() {
        let sun = sun_pixels();
        assert_eq!(sun[3], 0);
        assert_eq!(sun[(32 * 64 + 32) * 4 + 3], 255);
        let moon = moon_pixels();
        let counts: Vec<_> = (0..8)
            .map(|phase| {
                (0..32)
                    .flat_map(|y| (0..32).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        moon[((phase / 4 * 32 + y) * 128 + phase % 4 * 32 + x) * 4 + 3] != 0
                    })
                    .count()
            })
            .collect();
        assert_eq!(counts[4], 0);
        assert!(counts[0] > counts[1] && counts[1] > counts[2] && counts[2] > counts[3]);
        assert_eq!(counts[1], counts[7]);
        assert_eq!(counts[2], counts[6]);
        assert_eq!(counts[3], counts[5]);
    }
    #[test]
    fn eight_distinct_phases_wrap_in_sheet_order() {
        for day in 0..8 {
            let uv = phase_uv(day);
            assert_eq!(uv, phase_uv(day + 8));
            assert_eq!(uv.x, (day % 4) as f32 / 4.0);
            assert_eq!(uv.y, (day / 4) as f32 / 2.0);
        }
    }
    #[test]
    fn orbit_is_finite_at_zenith_and_camera_anchored() {
        for dir in [
            Vec3::Y,
            -Vec3::Y,
            Vec3::Z,
            Vec3::X,
            Vec3::new(-0.5, 0.75, 0.25).normalize(),
        ] {
            let a = body_transform(Vec3::ZERO, dir, 135.0);
            let b = body_transform(Vec3::splat(70.0), dir, 135.0);
            assert!(a.is_finite());
            assert!(a.x_axis.truncate().dot(dir).abs() < 0.0001);
            assert!(a.y_axis.truncate().dot(dir).abs() < 0.0001);
            assert!((a.x_axis.length() - 135.0).abs() < 0.0001);
            assert!((a.y_axis.length() - 135.0).abs() < 0.0001);
            assert!((b.w_axis - a.w_axis).abs_diff_eq(Vec3::splat(70.0).extend(0.0), 0.0001));
        }
    }
}
