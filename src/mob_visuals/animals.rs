//! Adult proportions in Bedrock model pixels (16 per block). References and
//! deliberate adaptations are listed in docs/mob-proportions.md.
use super::{Action, Design, MobKind, Surface};

pub(super) fn scale(kind: MobKind) -> f32 {
    // polar_bear.entity.json applies an additional adult visual multiplier.
    if kind == MobKind::PolarBear { 1.2 } else { 1.0 }
}

pub(super) fn height(kind: MobKind) -> Option<f32> {
    use MobKind::*;
    Some(
        match kind {
            Goat => 26.0,
            Llama | TraderLlama => 36.0,
            Camel | CamelHusk => 44.0,
            Pig => 16.0,
            Wolf => 15.5,
            Fox => 12.0,
            Panda => 20.5,
            PolarBear => 22.0,
            _ => return None,
        } * scale(kind)
            / 16.0,
    )
}

pub(super) fn design(kind: MobKind) -> Option<Design> {
    use MobKind::*;
    use Surface::*;
    height(kind)?;
    let mut d = Design::new();
    match kind {
        Goat => {
            // Narrow sloping face, lateral ears, beard and substantial horns;
            // neither a cow muzzle nor the llama's upright elongated neck.
            d.cube(0, [-4.0, 6.0, -9.0], [9.0, 11.0, 16.0], Body);
            d.cube(0, [-5.0, 4.0, -3.0], [11.0, 14.0, 11.0], Wool);
            for (x, z, h, sign) in [
                (2.5, -5.5, 6.0, 1.0),
                (-1.5, -5.5, 6.0, -1.0),
                (2.5, 4.5, 10.0, -1.0),
                (-1.5, 4.5, 10.0, 1.0),
            ] {
                d.leg(x, z, h, 3.0, sign, false);
            }
            let head = d.joint([0.5, 17.0, 8.0], Action::Head);
            d.cube(head, [-2.0, 15.0, 6.0], [5.0, 7.0, 10.0], Head);
            d.tilt_about([1.0, 18.0, 8.0], [55f32.to_radians(), 0.0, 0.0]);
            for x in [-5.0, 3.0] {
                d.cube(head, [x, 19.0, 9.0], [3.0, 2.0, 1.0], Limb);
            }
            for x in [-1.99, 0.99] {
                d.cube(head, [x, 19.0, 8.0], [2.0, 7.0, 2.0], Dark);
            }
            // Thin double-sided beard replaces the reference's zero-width card.
            d.cube(head, [0.35, 6.0, 9.0], [0.3, 7.0, 5.0], Wool);
        }
        Llama | TraderLlama => {
            for (x, z, s) in [
                (-3.5, -6.0, 1.0),
                (3.5, -6.0, -1.0),
                (-3.5, 5.0, -1.0),
                (3.5, 5.0, 1.0),
            ] {
                d.leg(x, z, 14.0, 4.0, s, false);
            }
            d.cube(0, [-6.0, 14.0, -9.0], [12.0, 10.0, 18.0], Body);
            let h = d.joint([0.0, 17.0, 6.0], Action::Head);
            d.cube(h, [-4.0, 15.0, 6.0], [8.0, 18.0, 6.0], Head);
            d.cube(h, [-2.0, 27.0, 7.0], [4.0, 4.0, 9.0], Muzzle);
            for x in [-4.0, 1.0] {
                d.cube(h, [x, 33.0, 8.0], [3.0, 3.0, 2.0], Body);
            }
            if kind == TraderLlama {
                d.cube(0, [-6.15, 14.0, -9.1], [12.3, 6.0, 18.2], Accent);
            }
        }
        Camel | CamelHusk => {
            d.cube(0, [-7.5, 20.0, -13.0], [15.0, 12.0, 27.0], Body);
            d.cube(0, [-4.5, 32.0, -5.0], [9.0, 5.0, 11.0], Body);
            for (x, z, s) in [
                (-4.9, -9.5, 1.0),
                (4.9, -9.5, -1.0),
                (-4.9, 10.5, -1.0),
                (4.9, 10.5, 1.0),
            ] {
                d.leg(x, z, 21.0, 5.0, s, false);
            }
            let h = d.joint([0.5, 25.0, 10.0], Action::Head);
            d.cube(h, [-3.5, 22.0, 6.0], [7.0, 8.0, 19.0], Body);
            d.cube(h, [-3.5, 30.0, 18.0], [7.0, 14.0, 7.0], Head);
            d.cube(h, [-2.5, 39.0, 25.0], [5.0, 5.0, 6.0], Muzzle);
            for x in [-6.0, 3.0] {
                d.cube(h, [x, 42.5, 18.5], [3.0, 1.0, 2.0], Body);
            }
            let t = d.joint([0.0, 29.0, -13.0], Action::Tail);
            d.cube(t, [-1.5, 15.0, -13.15], [3.0, 14.0, 0.3], Body);
        }
        Pig => {
            d.cube(0, [-5.0, 6.0, -8.0], [10.0, 8.0, 16.0], Body);
            for (x, z, s) in [
                (-3.0, -7.0, 1.0),
                (3.0, -7.0, -1.0),
                (-3.0, 5.0, -1.0),
                (3.0, 5.0, 1.0),
            ] {
                d.leg(x, z, 6.0, 4.0, s, false);
            }
            let h = d.joint([0.0, 12.0, 6.0], Action::Head);
            d.cube(h, [-4.0, 8.0, 6.0], [8.0, 8.0, 8.0], Head);
            d.cube(h, [-2.0, 9.0, 14.0], [4.0, 3.0, 1.0], Muzzle);
        }
        Wolf | Fox => {
            let wolf = kind == Wolf;
            let leg = if wolf { 8.0 } else { 6.0 };
            d.cube(
                0,
                [-3.0, leg - 1.0, -8.0],
                [6.0, 6.0, if wolf { 9.0 } else { 11.0 }],
                Body,
            );
            if wolf {
                d.cube(0, [-4.0, 7.0, -1.0], [8.0, 7.0, 6.0], Body);
            }
            for (x, z, s) in [
                (-2.0, -7.0, 1.0),
                (2.0, -7.0, -1.0),
                (-2.0, 4.0, -1.0),
                (2.0, 4.0, 1.0),
            ] {
                d.leg(x, z, leg, 2.0, s, false);
            }
            let h = d.joint(
                [
                    0.0,
                    if wolf { 10.5 } else { 8.0 },
                    if wolf { 7.0 } else { 3.0 },
                ],
                Action::Head,
            );
            if wolf {
                d.cube(h, [-3.0, 7.5, 5.0], [6.0, 6.0, 4.0], Head);
                d.cube(h, [-1.5, 7.51563, 8.0], [3.0, 3.0, 4.0], Muzzle);
                for x in [-3.0, 1.0] {
                    d.cube(h, [x, 13.5, 6.0], [2.0, 2.0, 1.0], Body);
                }
            } else {
                d.cube(h, [-4.0, 4.0, 3.0], [8.0, 6.0, 6.0], Head);
                d.cube(h, [-2.0, 4.0, 9.0], [4.0, 2.0, 3.0], Muzzle);
                for x in [-4.0, 2.0] {
                    d.cube(h, [x, 10.0, 7.0], [2.0, 2.0, 1.0], Dark);
                }
            }
            let t = d.joint([0.0, leg + 1.0, -7.0], Action::Tail);
            let w = if wolf { 2.0 } else { 4.0 };
            d.cube(t, [-w / 2.0, leg - 2.0, -16.0], [w, w, 9.0], Body);
        }
        Panda | PolarBear => {
            let panda = kind == Panda;
            let (width, leg, length) = if panda {
                (19.0, 9.0, 26.0)
            } else {
                (14.0, 10.0, 26.0)
            };
            d.cube(
                0,
                [-width / 2.0, leg - 1.0, -12.0],
                [width, if panda { 12.5 } else { 13.0 }, length],
                Body,
            );
            for (x, z, s) in [
                (-5.5, -9.0, 1.0),
                (5.5, -9.0, -1.0),
                (-5.5, 9.0, -1.0),
                (5.5, 9.0, 1.0),
            ] {
                d.leg(x, z, leg, if panda { 6.0 } else { 4.0 }, s, false);
            }
            let h = d.joint([0.0, if panda { 12.5 } else { 14.0 }, 16.0], Action::Head);
            if panda {
                d.cube(h, [-6.5, 7.5, 12.0], [13.0, 10.0, 9.0], Head);
                d.cube(h, [-3.5, 7.5, 21.0], [7.0, 5.0, 2.0], Muzzle);
                for x in [-8.5, 3.5] {
                    d.cube(h, [x, 16.5, 17.0], [5.0, 4.0, 1.0], Dark);
                }
            } else {
                d.cube(h, [-3.5, 10.0, 12.0], [7.0, 7.0, 7.0], Head);
                d.cube(h, [-2.5, 10.0, 19.0], [5.0, 3.0, 3.0], Muzzle);
                for x in [-4.5, 2.5] {
                    d.cube(h, [x, 16.0, 16.0], [2.0, 2.0, 1.0], Body);
                }
            }
        }
        _ => unreachable!(),
    }
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::Mob;
    use glam::Vec3;

    #[test]
    fn goat_and_llama_have_distinct_reference_heads_at_block_scale() {
        for (kind, head_size, leg_height) in [
            (MobKind::Goat, [5., 7., 10.], 6.),
            (MobKind::Llama, [8., 18., 6.], 14.),
            (MobKind::Camel, [7., 14., 7.], 21.),
            (MobKind::Pig, [8., 8., 8.], 6.),
            (MobKind::Wolf, [6., 6., 4.], 8.),
            (MobKind::Fox, [8., 6., 6.], 6.),
            (MobKind::Panda, [13., 10., 9.], 9.),
            (MobKind::PolarBear, [7., 7., 7.], 10.),
        ] {
            let d = super::super::design(kind);
            let head = d.cubes.iter().find(|c| c.surface == Surface::Head).unwrap();
            assert_eq!(head.size, Vec3::from_array(head_size), "{kind:?}");
            let leg = d
                .cubes
                .iter()
                .find(|c| matches!(d.joints[c.joint].action, Action::Stride(_)))
                .unwrap();
            assert_eq!(leg.size.y, leg_height, "{kind:?}");
            let mut mob = Mob::new(kind, Vec3::ZERO, Vec3::ZERO, 0);
            assert_eq!(
                super::super::model_scale(&mob, 100.),
                scale(kind) / 16.,
                "horns must not shrink the animal"
            );
            mob.animal.growth = 1200.;
            assert_eq!(super::super::model_scale(&mob, 100.), scale(kind) / 32.);
        }
        let goat = design(MobKind::Goat).unwrap();
        let face = goat
            .cubes
            .iter()
            .find(|c| c.surface == Surface::Head)
            .unwrap();
        assert_eq!(face.rotation_pivot, Some(Vec3::new(1., 18., 8.)));
        assert_eq!(face.rotation.x, 55f32.to_radians());
    }
}
