use glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Perspective {
    #[default]
    FirstPerson,
    ThirdPersonRear,
    ThirdPersonFront,
}

impl Perspective {
    pub fn next(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPersonRear,
            Self::ThirdPersonRear => Self::ThirdPersonFront,
            Self::ThirdPersonFront => Self::FirstPerson,
        }
    }

    pub fn first_person(self) -> bool {
        self == Self::FirstPerson
    }

    /// Sweep a camera-sized box from the player's eye. Checking the near-plane
    /// corners prevents seeing through walls when the center ray misses an edge.
    pub fn view(self, eye: Vec3, look: Vec3, mut solid: impl FnMut(Vec3) -> bool) -> (Vec3, Vec3) {
        let look = look.normalize_or(Vec3::Z);
        if self.first_person() {
            return (eye, look);
        }
        let outward = if self == Self::ThirdPersonRear {
            -look
        } else {
            look
        };
        // Fit inside the player's 0.22 half-width and the 0.23 clearance
        // above a crouched eye, so parallel surfaces do not trap the camera.
        const HALF: f32 = 0.2;
        let mut distance = 0.0;
        'sweep: for step in 1..=80 {
            let candidate = eye + outward * (step as f32 * 0.05);
            for x in [-HALF, HALF] {
                for y in [-HALF, HALF] {
                    for z in [-HALF, HALF] {
                        if solid(candidate + Vec3::new(x, y, z)) {
                            break 'sweep;
                        }
                    }
                }
            }
            distance = step as f32 * 0.05;
        }
        (eye + outward * distance, -outward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cycles_first_rear_front_first() {
        let first = Perspective::FirstPerson;
        assert_eq!(first.next(), Perspective::ThirdPersonRear);
        assert_eq!(first.next().next(), Perspective::ThirdPersonFront);
        assert_eq!(first.next().next().next(), first);
    }
    #[test]
    fn perspective_changes_view_without_changing_player_aim() {
        let eye = Vec3::new(2.0, 4.0, 6.0);
        assert_eq!(
            Perspective::FirstPerson.view(eye, Vec3::Z, |_| panic!("no collision sweep")),
            (eye, Vec3::Z)
        );
        assert_eq!(
            Perspective::ThirdPersonRear.view(eye, Vec3::Z, |_| false),
            (eye - Vec3::Z * 4.0, Vec3::Z)
        );
        assert_eq!(
            Perspective::ThirdPersonFront.view(eye, Vec3::Z, |_| false),
            (eye + Vec3::Z * 4.0, -Vec3::Z)
        );
    }
    #[test]
    fn third_person_clears_parallel_walls_and_crouching_ceilings() {
        for mode in [Perspective::ThirdPersonRear, Perspective::ThirdPersonFront] {
            let eye = Vec3::new(0.22, 1.27, 0.0);
            let (camera, _) = mode.view(eye, Vec3::Z, |p| p.x < 0.0 || p.y >= 1.5);
            assert_eq!(
                camera.distance(eye),
                4.0,
                "{mode:?} collapsed into the head"
            );
        }
    }
    #[test]
    fn camera_corners_stop_at_walls_and_release_afterward() {
        let mode = Perspective::ThirdPersonRear;
        let (pos, _) = mode.view(Vec3::ZERO, Vec3::Z, |p| p.z < -2.0 && p.x > 0.15);
        assert!(pos.z > -1.86 && pos.z < -1.74);
        assert_eq!(mode.view(Vec3::ZERO, Vec3::Z, |_| false).0.z, -4.0);
        assert_eq!(mode.view(Vec3::ZERO, Vec3::Z, |_| true).0, Vec3::ZERO);
    }
}
