//! Original per-face pixel patterns, authored independently of texture files.
use super::{Face, MobKind, Surface, shade};

const INK: [u8; 3] = [30, 27, 25];
const WHITE: [u8; 3] = [239, 235, 223];

#[allow(clippy::too_many_arguments)]
pub(super) fn texel(
    kind: MobKind,
    surface: Surface,
    cube: usize,
    face: Face,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> [u8; 3] {
    use MobKind::*;
    let front = face == Face::Front;
    let side = matches!(face, Face::Left | Face::Right);
    let u = x * 16 / w;
    let v = y * 16 / h;
    let base = match kind {
        Goat => [224, 220, 207],
        Llama | TraderLlama => [221, 209, 176],
        Camel => [191, 145, 85],
        CamelHusk => [135, 117, 73],
        Pig => [230, 157, 157],
        Wolf => [190, 188, 180],
        Fox => [199, 104, 40],
        Panda => [229, 228, 216],
        PolarBear => [235, 235, 219],
        _ => unreachable!("procedural animal palette"),
    };
    let mut color = base;
    match kind {
        Goat => {
            if matches!(cube, 9 | 10) {
                color = [90, 86, 77];
            }
            if (2..=5).contains(&cube) && v >= 12 {
                color = [70, 65, 58];
            }
            if cube == 6
                && side
                && (5..=8).contains(&v)
                && (if face == Face::Left { u >= 10 } else { u < 6 })
            {
                return if v == 6 || v == 7 {
                    INK
                } else {
                    [173, 153, 103]
                };
            }
            if cube == 6 && front && v >= 12 {
                color = [166, 157, 140];
            }
        }
        Llama | TraderLlama => {
            if cube < 4 && v >= 14 {
                color = [112, 90, 60];
            }
            if cube == 6 {
                color = [143, 112, 70];
            }
            if cube == 9 && kind == TraderLlama {
                return if v >= 13 || u.is_multiple_of(8) {
                    [219, 174, 50]
                } else if (u / 3 + v / 3).is_multiple_of(3) {
                    [159, 50, 48]
                } else {
                    [40, 70, 130]
                };
            }
        }
        Camel | CamelHusk => {
            if (2..=5).contains(&cube) && v >= 14 {
                color = shade(base, -40);
            }
            if cube == 8 && front && v <= 5 && (u <= 3 || u >= 12) {
                return INK;
            }
            if cube == 11 && v >= 11 {
                color = shade(base, -40);
            }
        }
        Pig => {
            if (1..=4).contains(&cube) && v >= 13 {
                color = [145, 90, 80];
            }
            if cube == 6 {
                color = [223, 139, 145];
                if front && (5..=10).contains(&v) && (u <= 4 || u >= 11) {
                    return [112, 60, 70];
                }
            }
        }
        Wolf => {
            if cube == 1 {
                color = shade(base, -22);
            }
            if cube == 7 || (cube == 6 && v >= 10) {
                color = WHITE;
            }
            if cube == 7 && front && v < 6 {
                return INK;
            }
            if matches!(cube, 8 | 9) && front {
                color = [133, 121, 111];
            }
        }
        Fox => {
            if (1..=4).contains(&cube) && v >= 8 || matches!(cube, 7 | 8) {
                color = INK;
            }
            if cube == 6
                || cube == 5 && v >= 10
                || cube == 0 && face == Face::Bottom
                || cube == 9
                    && (face == Face::Back
                        || matches!(face, Face::Top | Face::Bottom) && v < 5
                        || side && if face == Face::Left { u < 5 } else { u >= 11 })
            {
                color = WHITE;
            }
            if cube == 6 && front && v < 8 && (5..=10).contains(&u) {
                return INK;
            }
        }
        Panda => {
            if (1..=4).contains(&cube)
                || matches!(cube, 7 | 8)
                || cube == 0 && (front || side && if face == Face::Left { u >= 11 } else { u < 5 })
            {
                color = INK;
            }
            if cube == 5 && front && (4..=10).contains(&v) && (u <= 5 || u >= 10) {
                color = INK;
            }
            if cube == 6 && front && v < 6 && (5..=10).contains(&u) {
                return INK;
            }
        }
        PolarBear => {
            if cube == 6 && front && v < 6 && (4..=11).contains(&u) {
                return INK;
            }
            if matches!(cube, 7 | 8) && front {
                color = [179, 176, 162];
            }
        }
        _ => {}
    }
    if surface == Surface::Head && kind != Goat {
        let eye_y = if matches!(kind, Llama | TraderLlama | Camel | CamelHusk) {
            2
        } else {
            6
        };
        if front && (eye_y..eye_y + 3).contains(&v) && !(4..12).contains(&u) {
            return if matches!(kind, Pig | Llama | TraderLlama | Wolf) && (u == 2 || u == 12) {
                WHITE
            } else {
                INK
            };
        }
        if side
            && matches!(kind, Llama | TraderLlama | Camel | CamelHusk)
            && (eye_y..eye_y + 2).contains(&v)
            && (if face == Face::Left { u >= 12 } else { u < 4 })
        {
            return INK;
        }
    }
    let grain = ((x * 17 + y * 31 + cube * 11) ^ (x * y * 7)) % 13;
    shade(
        color,
        match grain {
            0 => -7,
            1 => 4,
            _ => 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_cubes_keep_their_markings_in_the_atlas() {
        let mut atlas = super::super::Atlas::new();
        let body = atlas.face_for_cube(
            MobKind::TraderLlama,
            Surface::Body,
            Face::Front,
            12,
            10,
            Some(4),
        );
        let cloth = atlas.face_for_cube(
            MobKind::TraderLlama,
            Surface::Body,
            Face::Front,
            12,
            10,
            Some(9),
        );
        assert_ne!(body, cloth);
        for kind in MobKind::ALL
            .iter()
            .copied()
            .filter(|k| super::super::animals::height(*k).is_some())
        {
            let design = super::super::design(kind);
            assert!(!super::super::bake(kind, &design, &mut atlas).is_empty());
        }
    }
    #[test]
    fn long_neck_eyes_stay_at_the_top_and_pig_has_sclera() {
        assert_eq!(
            texel(MobKind::Llama, Surface::Head, 5, Face::Front, 0, 2, 16, 16),
            INK
        );
        assert_ne!(
            texel(MobKind::Llama, Surface::Head, 5, Face::Front, 0, 10, 16, 16),
            INK
        );
        assert_eq!(
            texel(MobKind::Pig, Surface::Head, 5, Face::Front, 2, 6, 16, 16),
            WHITE
        );
        assert_eq!(
            texel(MobKind::Pig, Surface::Head, 5, Face::Front, 0, 6, 16, 16),
            INK
        );
    }
}
