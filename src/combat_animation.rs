//! Combat poses evaluated independently of damage and rendering.
//! Reference formulas and limits of parity are recorded in docs/combat.md.
use glam::Vec3;

pub const SWING_SECONDS: f32 = 0.3;
pub const HURT_SECONDS: f32 = 0.5;
pub const DEATH_SECONDS: f32 = 1.0;

#[derive(Clone, Debug)]
pub struct Swing {
    elapsed: f32,
}

impl Default for Swing {
    fn default() -> Self {
        Self {
            elapsed: SWING_SECONDS,
        }
    }
}

impl Swing {
    pub fn start(&mut self) {
        if !self.active() {
            self.elapsed = 0.0;
        }
    }

    pub fn active(&self) -> bool {
        self.elapsed < SWING_SECONDS
    }

    pub fn update(&mut self, dt: f32, held: bool) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if held {
            self.start();
        }
        if self.active() {
            self.elapsed += dt;
            if held && self.elapsed >= SWING_SECONDS {
                let remainder = self.elapsed.rem_euclid(SWING_SECONDS);
                self.elapsed = if remainder < 1e-6 {
                    SWING_SECONDS
                } else {
                    remainder
                };
            } else {
                self.elapsed = self.elapsed.min(SWING_SECONDS);
            }
        }
    }

    pub fn progress(&self) -> f32 {
        if self.active() {
            self.elapsed / SWING_SECONDS
        } else {
            0.0
        }
    }

    pub fn amount(&self) -> f32 {
        (self.progress() * std::f32::consts::PI).sin()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn sin_deg(degrees: f32) -> f32 {
    degrees.to_radians().sin()
}

/// Bedrock's first_person.attack_rotation, in model pixels and degrees.
/// The engine supplies rotation_factor; it is not defined by the sample pack.
pub fn first_person_pose(progress: f32, rotation_factor: f32) -> (Vec3, Vec3) {
    let p = progress.clamp(0.0, 1.0);
    if p == 0.0 || p == 1.0 {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let a = rotation_factor * p;
    let b = rotation_factor * (1.0 - p).powi(2);
    let side = sin_deg(a * 112.0);
    let rotation = sin_deg(b * 280.0);
    (
        Vec3::new(
            (-15.5 * side).clamp(-7.0, 999.0) * side,
            sin_deg(b * 200.0) * 7.5 - a * 15.0,
            sin_deg(a * 120.0) * 1.75,
        ),
        Vec3::new(-60.0, 40.0, 20.0) * rotation,
    )
}

/// Bedrock zombie.attack_bare_hand, adult without a spear. Right is +1.
pub fn zombie_arm(progress: f32, life_seconds: f32, side: f32) -> Vec3 {
    let a = sin_deg(progress * 180.0) * 57.3;
    let b = sin_deg((1.0 - (1.0 - progress).powi(2)) * 180.0) * 57.3;
    Vec3::new(
        -90.0 - (a * 1.2 - b * 0.4) + side * sin_deg(life_seconds * 76.776_375) * 2.865,
        side * (a * 0.6 - 5.73),
        side * ((life_seconds * 103.132_44).to_radians().cos() * 2.865 + 2.865),
    )
    .map(f32::to_radians)
}

pub fn golem_arm(remaining_seconds: f32) -> f32 {
    let ticks = remaining_seconds * 20.0;
    (-114.0 + ((1.5 * (ticks.rem_euclid(10.0) - 5.0).abs() - 2.5) / 5.0) * 57.3).to_radians()
}

#[derive(Clone, Debug, Default)]
pub struct MobAnimation {
    pub swing: Swing,
    pub hurt_remaining: f32,
    pub golem_attack_remaining: f32,
    pub death_elapsed: f32,
}

impl MobAnimation {
    pub fn update(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.swing.update(dt, false);
        self.hurt_remaining = (self.hurt_remaining - dt).max(0.0);
        self.golem_attack_remaining = (self.golem_attack_remaining - dt).max(0.0);
    }

    pub fn attack(&mut self) {
        self.swing.start();
        self.golem_attack_remaining = 0.5;
    }

    pub fn death_roll(&self) -> f32 {
        (self.death_elapsed * 1.6).sqrt().min(1.0) * std::f32::consts::FRAC_PI_2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_swing_is_independent_of_frame_partition() {
        let mut coarse = Swing::default();
        let mut fine = Swing::default();
        coarse.update(0.47, true);
        for _ in 0..47 {
            fine.update(0.01, true);
        }
        assert!((coarse.progress() - fine.progress()).abs() < 1e-5);
        coarse.update(0.2, false);
        assert!(!coarse.active());
    }

    #[test]
    fn zero_time_does_not_start_or_advance_an_animation() {
        let mut swing = Swing::default();
        swing.update(0.0, true);
        assert!(!swing.active());
        swing.start();
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            swing.update(dt, true);
        }
        assert_eq!(swing.progress(), 0.0);
    }

    #[test]
    fn published_first_person_pose_sample() {
        // p=.5, factor=1: the JSON evaluates sin(56), sin(50), sin(60), sin(70).
        let (position, rotation) = first_person_pose(0.5, 1.0);
        assert!(position.abs_diff_eq(Vec3::new(-5.803_263, -1.754_667, 1.515_544), 1e-5));
        assert!(rotation.abs_diff_eq(Vec3::new(-56.381_557, 37.587_704, 18.793_852), 1e-5));
    }

    #[test]
    fn published_zombie_pose_at_half_swing() {
        let right = zombie_arm(0.5, 0.0, 1.0).map(f32::to_degrees);
        let left = zombie_arm(0.5, 0.0, -1.0).map(f32::to_degrees);
        assert!(right.abs_diff_eq(Vec3::new(-142.553_12, 28.65, 5.73), 1e-4));
        assert!(left.abs_diff_eq(Vec3::new(right.x, -right.y, -right.z), 1e-4));
    }

    #[test]
    fn golem_attack_uses_ten_tick_triangle() {
        assert!((golem_arm(0.5).to_degrees() + 56.7).abs() < 1e-4);
        assert!((golem_arm(0.25).to_degrees() + 142.65).abs() < 1e-4);
    }
}
