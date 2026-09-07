//! Original pixel art and articulated cuboid models. One shared atlas, cached
//! meshes per joint; details on a limb travel with that limb in one draw.
use crate::mob::Mob;
use crate::mob_catalog::{MobKind, Shape};
use crate::renderer::{Mesh, Shader, Texture2D};
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;

pub const ATLAS_SIZE: usize = 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Surface {
    Body,
    Head,
    Limb,
    Accent,
    Dark,
    Muzzle,
    Wood,
    Wool,
    Pants,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Face {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
}
#[derive(Clone, Copy)]
enum Action {
    Fixed,
    Head,
    Stride(f32),
    Arm(f32),
    Wing(f32),
    Tail,
    Feelers(f32),
    DolphinTail,
    DolphinFluke,
    DolphinFin(f32),
}
#[derive(Clone, Copy)]
struct Joint {
    pivot: Vec3,
    action: Action,
    parent: Option<usize>,
}
struct Cube {
    joint: usize,
    min: Vec3,
    size: Vec3,
    surface: Surface,
    rotation: Vec3,
}
struct Design {
    joints: Vec<Joint>,
    cubes: Vec<Cube>,
}
impl Design {
    fn new() -> Self {
        Self {
            joints: vec![Joint {
                pivot: Vec3::ZERO,
                action: Action::Fixed,
                parent: None,
            }],
            cubes: Vec::new(),
        }
    }
    fn joint(&mut self, pivot: [f32; 3], action: Action) -> usize {
        self.joints.push(Joint {
            pivot: Vec3::from_array(pivot),
            action,
            parent: None,
        });
        self.joints.len() - 1
    }
    fn cube(&mut self, joint: usize, min: [f32; 3], size: [f32; 3], surface: Surface) {
        self.cubes.push(Cube {
            joint,
            min: Vec3::from_array(min),
            size: Vec3::from_array(size),
            surface,
            rotation: Vec3::ZERO,
        });
    }
    fn tilt(&mut self, rotation: [f32; 3]) {
        self.cubes.last_mut().unwrap().rotation = Vec3::from_array(rotation);
    }
    fn leg(&mut self, x: f32, z: f32, height: f32, width: f32, sign: f32, hoof: bool) {
        let j = self.joint([x, height, z], Action::Stride(sign));
        self.cube(
            j,
            [x - width / 2.0, 0.0, z - width / 2.0],
            [width, height, width],
            Surface::Limb,
        );
        if hoof {
            self.cube(
                j,
                [x - width / 2.0 - 0.08, 0.0, z - width / 2.0 - 0.08],
                [width + 0.16, 1.8, width + 0.16],
                Surface::Dark,
            );
        }
    }
}

fn design(kind: MobKind) -> Design {
    use MobKind::*;
    use Surface::*;
    let mut d = Design::new();
    match kind.species().shape {
        Shape::Person => {
            let bones = matches!(kind, Skeleton | Stray | Bogged | Parched);
            let tall = kind == Enderman;
            let hip = if tall { 19.0 } else { 12.0 };
            let neck = hip + 12.0;
            d.leg(
                -2.0,
                0.0,
                hip,
                if bones || tall { 2.0 } else { 4.0 },
                1.0,
                !bones && !tall,
            );
            d.leg(
                2.0,
                0.0,
                hip,
                if bones || tall { 2.0 } else { 4.0 },
                -1.0,
                !bones && !tall,
            );
            if bones {
                d.cube(0, [-1.0, hip, -1.0], [2.0, 12.0, 2.0], Body);
                for y in [hip + 4.0, hip + 7.0, hip + 10.0] {
                    d.cube(0, [-4.0, y, -2.0], [8.0, 1.5, 4.0], Body);
                }
                d.cube(0, [-3.0, hip, -2.0], [6.0, 2.0, 4.0], Body);
            } else {
                d.cube(0, [-4.0, hip, -2.0], [8.0, 12.0, 4.0], Body);
            }
            let head = d.joint([0.0, neck, 0.0], Action::Head);
            d.cube(head, [-4.0, neck, -4.0], [8.0, 8.0, 8.0], Head);
            if kind == Bogged {
                d.cube(head, [-4.0, neck + 7.0, -4.0], [8.0, 2.0, 8.0], Accent);
                d.cube(head, [1.0, neck + 9.0, -1.0], [1.0, 2.0, 1.0], Wood);
                d.cube(head, [-1.0, neck + 11.0, -3.0], [5.0, 1.0, 5.0], Muzzle);
            }
            for (x, sign) in [(-5.5, -1.0), (5.5, 1.0)] {
                let arm = d.joint([x, neck - 1.0, 0.0], Action::Arm(sign));
                let w = if bones || tall { 2.0 } else { 3.0 };
                let h = if tall { 24.0 } else { 12.0 };
                d.cube(
                    arm,
                    [x - w / 2.0, neck - 1.0 - h, -w / 2.0],
                    [w, h, w],
                    Limb,
                );
                if !bones && !tall {
                    d.cube(
                        arm,
                        [x - w / 2.0 - 0.04, neck - 5.0, -w / 2.0 - 0.04],
                        [w + 0.08, 4.0, w + 0.08],
                        Body,
                    );
                }
                if bones && sign > 0.0 {
                    for (dy, z) in [(0.0, 1.0), (3.0, 2.0), (6.0, 3.0), (9.0, 2.0), (12.0, 1.0)] {
                        d.cube(arm, [x - 0.7, neck - 16.0 + dy, z], [1.4, 3.0, 1.4], Wood);
                    }
                    d.cube(arm, [x - 0.25, neck - 15.0, 0.0], [0.5, 14.0, 0.5], Accent);
                }
            }
        }
        Shape::Villager => {
            d.leg(-2.0, 0.0, 8.0, 3.5, 1.0, true);
            d.leg(2.0, 0.0, 8.0, 3.5, -1.0, true);
            d.cube(0, [-4.5, 4.0, -3.0], [9.0, 16.0, 6.0], Body);
            d.cube(0, [-4.6, 6.0, -3.1], [9.2, 1.0, 6.2], Accent);
            d.cube(0, [-6.0, 14.0, 2.2], [12.0, 3.0, 3.5], Body);
            d.cube(0, [-2.0, 14.2, 4.0], [4.0, 2.4, 2.0], Limb);
            let h = d.joint([0.0, 20.0, 0.0], Action::Head);
            d.cube(h, [-4.0, 20.0, -4.0], [8.0, 10.0, 8.0], Head);
            d.cube(h, [-1.0, 21.0, 4.0], [2.0, 4.0, 2.5], Muzzle);
            if kind == Witch {
                for (min, size) in [
                    ([-6.0, 29.0, -6.0], [12.0, 1.5, 12.0]),
                    ([-4.0, 30.5, -4.0], [8.0, 4.0, 8.0]),
                    ([-2.5, 34.5, -2.5], [5.0, 3.0, 5.0]),
                    ([-1.0, 37.5, -1.0], [2.0, 2.0, 2.0]),
                ] {
                    d.cube(h, min, size, Dark);
                }
                d.cube(h, [-1.0, 31.0, 4.05], [2.0, 2.0, 0.2], Accent);
            }
            if matches!(kind, Pillager | Vindicator) {
                d.cube(0, [3.0, 10.0, 5.0], [1.5, 9.0, 1.5], Wood);
                d.cube(
                    0,
                    [-2.0, 16.0, 5.0],
                    [7.0, 2.5, 2.0],
                    if kind == Pillager { Wood } else { Accent },
                );
            }
            if kind == WanderingTrader {
                d.cube(h, [-4.4, 27.0, -4.4], [8.8, 3.4, 8.8], Body);
            }
        }
        Shape::Golem => {
            if kind == SnowGolem {
                d.cube(0, [-6.0, 0.0, -6.0], [12.0, 12.0, 12.0], Body);
                d.cube(0, [-5.0, 12.0, -5.0], [10.0, 8.0, 10.0], Body);
                let h = d.joint([0.0, 20.0, 0.0], Action::Head);
                d.cube(h, [-4.0, 20.0, -4.0], [8.0, 8.0, 8.0], Head);
                for sign in [-1.0, 1.0] {
                    d.cube(0, [sign * 9.0 - 4.0, 15.0, 0.0], [8.0, 1.0, 1.0], Wood);
                    d.tilt([0.0, 0.0, sign * 0.2]);
                }
            } else {
                let small = kind == CopperGolem;
                let hip = if small { 6.0 } else { 14.0 };
                let neck = if small { 16.0 } else { 34.0 };
                let width = if small { 10.0 } else { 18.0 };
                d.leg(
                    -width * 0.25,
                    0.0,
                    hip,
                    if small { 4.0 } else { 5.0 },
                    1.0,
                    false,
                );
                d.leg(
                    width * 0.25,
                    0.0,
                    hip,
                    if small { 4.0 } else { 5.0 },
                    -1.0,
                    false,
                );
                d.cube(0, [-width / 2.0, hip, -4.0], [width, neck - hip, 8.0], Body);
                let h = d.joint([0.0, neck, 0.0], Action::Head);
                let hw = if kind == Warden { 12.0 } else { 8.0 };
                d.cube(h, [-hw / 2.0, neck, -4.0], [hw, 8.0, 8.0], Head);
                if kind != Warden {
                    d.cube(h, [-1.0, neck + 1.0, 4.0], [2.0, 3.0, 2.0], Muzzle);
                }
                if small {
                    d.cube(h, [-1.0, neck + 8.0, -1.0], [2.0, 3.0, 2.0], Accent);
                }
                if kind == Warden {
                    for sign in [-1.0, 1.0] {
                        d.cube(
                            h,
                            [sign * 8.0 - 1.0, neck + 3.0, -1.0],
                            [2.0, 8.0, 2.0],
                            Accent,
                        );
                        d.cube(
                            h,
                            [sign * 9.0 - 2.0, neck + 9.0, -1.0],
                            [4.0, 2.0, 2.0],
                            Accent,
                        );
                    }
                    d.cube(0, [-5.0, hip + 6.0, 4.05], [10.0, 9.0, 0.3], Accent);
                }
                for sign in [-1.0, 1.0] {
                    let x = sign * (width / 2.0 + 2.0);
                    let arm = d.joint([x, neck - 2.0, 0.0], Action::Arm(sign));
                    let len = if small { 10.0 } else { 24.0 };
                    d.cube(
                        arm,
                        [x - 2.0, neck - 2.0 - len, -2.5],
                        [4.0, len, 5.0],
                        Body,
                    );
                    d.cube(
                        arm,
                        [x - 2.4, neck - 2.0 - len, -2.9],
                        [4.8, 4.0, 5.8],
                        Accent,
                    );
                }
            }
        }
        Shape::Creeper => {
            for (x, z, s) in [
                (-3.0, -3.0, 1.0),
                (3.0, -3.0, -1.0),
                (-3.0, 3.0, -1.0),
                (3.0, 3.0, 1.0),
            ] {
                d.leg(x, z, 6.0, 4.0, s, false);
            }
            d.cube(0, [-4.0, 6.0, -2.5], [8.0, 12.0, 5.0], Body);
            let h = d.joint([0.0, 18.0, 0.0], Action::Head);
            d.cube(h, [-4.0, 18.0, -4.0], [8.0, 8.0, 8.0], Head);
        }
        Shape::Grazer => {
            let pig = kind == Pig;
            let wool = kind == Sheep;
            let leg = if pig { 5.0 } else { 7.0 };
            for (x, z, s) in [
                (-3.0, -5.0, 1.0),
                (3.0, -5.0, -1.0),
                (-3.0, 5.0, -1.0),
                (3.0, 5.0, 1.0),
            ] {
                d.leg(x, z, leg, 3.0, s, true);
            }
            d.cube(
                0,
                [-5.0, leg, -8.0],
                [10.0, if pig { 8.0 } else { 10.0 }, 16.0],
                if wool { Wool } else { Body },
            );
            let hy = leg + if pig { 4.0 } else { 6.0 };
            let h = d.joint([0.0, hy, 7.0], Action::Head);
            d.cube(h, [-3.5, hy, 6.0], [7.0, 7.0, 7.0], Head);
            d.cube(h, [-2.5, hy + 0.5, 12.5], [5.0, 3.0, 2.0], Muzzle);
            for sign in [-1.0, 1.0] {
                d.cube(h, [sign * 4.0 - 1.0, hy + 4.0, 7.0], [2.0, 2.0, 3.0], Limb);
                if matches!(kind, Cow | Mooshroom | Goat) {
                    d.cube(
                        h,
                        [sign * 2.8 - 0.7, hy + 7.0, 7.0],
                        [1.4, if kind == Goat { 5.0 } else { 2.5 }, 1.4],
                        Accent,
                    );
                }
            }
            if wool {
                d.cube(h, [-3.8, hy + 4.0, 5.7], [7.6, 3.5, 4.5], Wool);
            }
            if kind == Goat {
                d.cube(h, [-1.0, hy - 2.0, 10.0], [2.0, 3.0, 2.0], Wool);
            }
            if kind == Mooshroom {
                for (x, z) in [(-2.0, -4.0), (2.0, 1.0)] {
                    d.cube(0, [x, 17.0, z], [1.0, 3.0, 1.0], Accent);
                    d.cube(0, [x - 2.0, 20.0, z - 2.0], [5.0, 2.0, 5.0], Head);
                }
            }
            let t = d.joint([0.0, leg + 4.0, -8.0], Action::Tail);
            d.cube(t, [-0.7, leg + 1.0, -9.0], [1.4, 4.0, 1.4], Limb);
        }
        Shape::Horse => {
            let camel = matches!(kind, Camel | CamelHusk);
            let llama = matches!(kind, Llama | TraderLlama);
            let hip = if camel { 15.0 } else { 12.0 };
            for (x, z, s) in [
                (-4.0, -7.0, 1.0),
                (4.0, -7.0, -1.0),
                (-4.0, 7.0, -1.0),
                (4.0, 7.0, 1.0),
            ] {
                d.leg(x, z, hip, 3.0, s, true);
            }
            d.cube(0, [-6.0, hip, -10.0], [12.0, 11.0, 20.0], Body);
            let h = d.joint([0.0, hip + 7.0, 7.0], Action::Head);
            let neck = if camel || llama { 15.0 } else { 10.0 };
            d.cube(h, [-2.5, hip + 7.0, 6.0], [5.0, neck, 6.0], Body);
            d.cube(h, [-3.0, hip + neck + 4.0, 7.0], [6.0, 6.0, 10.0], Head);
            d.cube(h, [-3.0, hip + neck + 4.0, 15.0], [6.0, 3.0, 3.0], Muzzle);
            for x in [-2.0, 2.0] {
                d.cube(
                    h,
                    [x - 0.75, hip + neck + 10.0, 8.0],
                    [
                        1.5,
                        if llama || matches!(kind, Donkey | Mule) {
                            5.0
                        } else {
                            3.0
                        },
                        2.0,
                    ],
                    Limb,
                );
            }
            if camel {
                d.cube(0, [-4.5, hip + 11.0, -4.0], [9.0, 5.0, 8.0], Body);
            }
            if !camel && !llama {
                d.cube(h, [-0.6, hip + 7.0, 5.0], [1.2, neck + 4.0, 2.0], Dark);
            }
            if kind == TraderLlama {
                d.cube(0, [-6.15, hip + 5.0, -7.0], [12.3, 5.0, 12.0], Accent);
            }
            let t = d.joint([0.0, hip + 8.0, -10.0], Action::Tail);
            d.cube(t, [-1.0, hip - 2.0, -12.0], [2.0, 11.0, 3.0], Dark);
        }
        Shape::Cat | Shape::Canine => {
            let cat = kind.species().shape == Shape::Cat;
            let axolotl = kind == Axolotl;
            let leg = if axolotl { 2.0 } else { 5.0 };
            for (x, z, s) in [
                (-2.5, -4.0, 1.0),
                (2.5, -4.0, -1.0),
                (-2.5, 4.0, -1.0),
                (2.5, 4.0, 1.0),
            ] {
                d.leg(x, z, leg, if cat { 1.5 } else { 2.0 }, s, false);
            }
            d.cube(
                0,
                [-3.0, leg, -7.0],
                [6.0, if axolotl { 4.0 } else { 6.0 }, 14.0],
                Body,
            );
            let h = d.joint([0.0, leg + 3.0, 6.0], Action::Head);
            d.cube(
                h,
                [-3.0, leg + 2.0, 5.0],
                [6.0, if axolotl { 4.0 } else { 6.0 }, 6.0],
                Head,
            );
            if !axolotl {
                d.cube(h, [-2.0, leg + 2.0, 10.0], [4.0, 2.5, 2.0], Muzzle);
                for x in [-2.0, 2.0] {
                    d.cube(h, [x - 0.75, leg + 8.0, 6.0], [1.5, 3.0, 2.0], Accent);
                }
            } else {
                for sign in [-1.0, 1.0] {
                    for z in [5.0, 7.0, 9.0] {
                        d.cube(h, [sign * 4.0 - 0.5, leg + 3.0, z], [1.0, 4.0, 1.0], Accent);
                        d.tilt([0.0, 0.0, -sign * 0.6]);
                    }
                }
            }
            let t = d.joint([0.0, leg + 3.0, -7.0], Action::Tail);
            d.cube(
                t,
                [-1.0, leg + 2.0, -15.0],
                [2.0, if kind == Fox { 4.0 } else { 2.0 }, 9.0],
                if axolotl { Accent } else { Body },
            );
            if kind == Fox {
                d.cube(t, [-1.05, leg + 2.0, -15.5], [2.1, 4.0, 3.0], Accent);
            }
        }
        Shape::Bear => {
            for (x, z, s) in [
                (-4.5, -6.0, 1.0),
                (4.5, -6.0, -1.0),
                (-4.5, 6.0, -1.0),
                (4.5, 6.0, 1.0),
            ] {
                d.leg(x, z, 8.0, 4.0, s, true);
            }
            d.cube(0, [-7.0, 7.0, -10.0], [14.0, 12.0, 20.0], Body);
            let h = d.joint([0.0, 15.0, 8.0], Action::Head);
            d.cube(h, [-5.0, 13.0, 7.0], [10.0, 8.0, 9.0], Head);
            d.cube(h, [-3.0, 13.0, 15.0], [6.0, 4.0, 3.0], Muzzle);
            for x in [-4.0, 4.0] {
                d.cube(
                    h,
                    [x - 1.0, 21.0, 8.0],
                    [2.0, kind_height(kind == Ravager, 6.0, 2.0), 2.0],
                    Accent,
                );
            }
        }
        Shape::Rabbit => {
            d.cube(0, [-3.0, 3.0, -4.0], [6.0, 5.0, 8.0], Body);
            for (x, z, s) in [
                (-2.0, -2.0, 1.0),
                (2.0, -2.0, -1.0),
                (-2.0, 3.0, -1.0),
                (2.0, 3.0, 1.0),
            ] {
                d.leg(x, z, 4.0, 2.0, s, false);
            }
            let h = d.joint([0.0, 7.0, 3.0], Action::Head);
            d.cube(h, [-2.5, 7.0, 2.0], [5.0, 5.0, 5.0], Head);
            for x in [-1.5, 1.5] {
                d.cube(h, [x - 0.6, 12.0, 2.0], [1.2, 6.0, 1.5], Accent);
            }
            d.cube(0, [-1.5, 5.0, -6.0], [3.0, 3.0, 3.0], Accent);
        }
        Shape::Bird => {
            d.leg(-1.5, 0.0, 4.0, 1.0, 1.0, false);
            d.leg(1.5, 0.0, 4.0, 1.0, -1.0, false);
            d.cube(0, [-3.0, 4.0, -4.0], [6.0, 7.0, 8.0], Body);
            let h = d.joint([0.0, 10.0, 2.0], Action::Head);
            d.cube(h, [-2.5, 10.0, 0.0], [5.0, 6.0, 5.0], Head);
            d.cube(h, [-1.5, 11.0, 5.0], [3.0, 2.0, 2.5], Muzzle);
            if kind == Chicken {
                d.cube(h, [-1.0, 9.0, 4.0], [2.0, 2.0, 1.0], Accent);
            }
            for sign in [-1.0, 1.0] {
                let w = d.joint([sign * 3.0, 10.0, 0.0], Action::Wing(sign));
                d.cube(
                    w,
                    [sign * 3.5 - 0.5, 5.0, -3.0],
                    [1.0, 5.0, 6.0],
                    if kind == Chicken { Body } else { Accent },
                );
            }
            d.cube(
                0,
                [-1.5, 6.0, -8.0],
                [3.0, 2.0, 5.0],
                if kind == Chicken { Body } else { Accent },
            );
            d.tilt([-0.5, 0.0, 0.0]);
        }
        Shape::Bat => {
            d.cube(0, [-2.0, 3.0, -2.0], [4.0, 7.0, 4.0], Body);
            let h = d.joint([0.0, 10.0, 0.0], Action::Head);
            d.cube(h, [-2.5, 10.0, -2.5], [5.0, 5.0, 5.0], Head);
            for sign in [-1.0, 1.0] {
                d.cube(h, [sign * 1.5 - 0.5, 15.0, -1.0], [1.0, 3.0, 2.0], Accent);
                let w = d.joint([sign * 2.0, 9.0, 0.0], Action::Wing(sign));
                d.cube(
                    w,
                    [if sign < 0.0 { -11.0 } else { 2.0 }, 6.0, -0.6],
                    [9.0, 5.0, 1.2],
                    Body,
                );
                d.cube(
                    w,
                    [if sign < 0.0 { -15.0 } else { 10.0 }, 4.0, -0.5],
                    [5.0, 5.0, 1.0],
                    Accent,
                );
                d.cube(
                    w,
                    [if sign < 0.0 { -11.0 } else { 2.0 }, 10.0, -0.7],
                    [9.0, 1.0, 1.4],
                    Dark,
                );
            }
        }
        Shape::Bee => {
            d.cube(0, [-4.0, 4.0, -5.0], [8.0, 6.0, 10.0], Head);
            for sign in [-1.0, 1.0] {
                let w = d.joint([sign * 2.0, 10.0, 0.0], Action::Wing(sign));
                d.cube(
                    w,
                    [if sign < 0.0 { -9.0 } else { 2.0 }, 10.0, -4.0],
                    [7.0, 0.5, 7.0],
                    Wool,
                );
                for z in [-3.0, 0.0, 3.0] {
                    d.cube(0, [sign * 3.0 - 0.4, 2.0, z], [0.8, 2.0, 0.8], Dark);
                }
                d.cube(0, [sign * 2.0 - 0.4, 9.0, 4.0], [0.8, 3.0, 0.8], Dark);
            }
        }
        Shape::Spider => {
            d.cube(0, [-4.0, 5.0, -6.0], [8.0, 6.0, 9.0], Body);
            let h = d.joint([0.0, 7.0, 4.0], Action::Head);
            d.cube(h, [-4.0, 5.0, 3.0], [8.0, 5.0, 7.0], Head);
            for sign in [-1.0, 1.0] {
                for i in 0..4 {
                    let z = i as f32 * 2.5 - 4.0;
                    let j = d.joint(
                        [sign * 4.0, 6.0, z],
                        Action::Stride(if i % 2 == 0 { sign } else { -sign }),
                    );
                    d.cube(
                        j,
                        [if sign < 0.0 { -12.0 } else { 4.0 }, 4.0, z - 0.75],
                        [8.0, 1.5, 1.5],
                        Limb,
                    );
                    d.tilt([0.0, (i as f32 - 1.5) * 0.3, sign * 0.35]);
                    d.cube(
                        j,
                        [sign * 12.0 - 0.75, 0.0, z - 0.75],
                        [1.5, 5.0, 1.5],
                        Limb,
                    );
                    d.tilt([0.0, 0.0, -sign * 0.3]);
                }
            }
        }
        Shape::Fish => {
            if kind == Dolphin {
                d.cube(0, [-4.0, 3.0, -7.0], [8.0, 7.0, 14.0], Head);
                d.cube(0, [-1.5, 4.0, 7.0], [3.0, 2.0, 5.0], Muzzle);
                d.cube(0, [-0.6, 9.0, -4.0], [1.2, 4.0, 5.0], Body);
                d.tilt([-0.35, 0.0, 0.0]);
                let tail = d.joint([0.0, 6.0, -6.0], Action::DolphinTail);
                d.cube(tail, [-2.5, 3.5, -14.0], [5.0, 5.0, 8.0], Body);
                let fluke = d.joint([0.0, 5.5, -13.0], Action::DolphinFluke);
                d.joints[fluke].parent = Some(tail);
                d.cube(fluke, [-6.0, 4.8, -17.0], [12.0, 1.4, 5.0], Body);
                for sign in [-1.0, 1.0] {
                    let fin = d.joint([sign * 3.5, 4.5, 3.0], Action::DolphinFin(sign));
                    d.cube(
                        fin,
                        [if sign < 0.0 { -9.0 } else { 3.0 }, 4.0, -1.0],
                        [6.0, 1.0, 5.0],
                        Body,
                    );
                    d.tilt([0.0, sign * 0.35, 0.0]);
                }
                return d;
            }
            let dolphin = kind == Dolphin;
            d.cube(0, [-2.0, 3.0, -6.0], [4.0, 5.0, 12.0], Head);
            d.cube(0, [-0.5, 8.0, -2.0], [1.0, 3.0, 5.0], Accent);
            let t = d.joint([0.0, 5.0, -6.0], Action::Tail);
            d.cube(t, [-1.0, 4.0, -10.0], [2.0, 3.0, 5.0], Body);
            d.cube(
                t,
                if dolphin {
                    [-5.0, 4.5, -12.0]
                } else {
                    [-0.5, 1.5, -13.0]
                },
                if dolphin {
                    [10.0, 1.0, 4.0]
                } else {
                    [1.0, 9.0, 4.0]
                },
                Accent,
            );
            for sign in [-1.0, 1.0] {
                d.cube(0, [sign * 3.0 - 1.0, 3.0, 0.0], [2.0, 1.0, 5.0], Accent);
                d.tilt([0.0, sign * 0.4, 0.0]);
            }
            if dolphin {
                d.cube(0, [-1.0, 4.0, 6.0], [2.0, 2.0, 5.0], Muzzle);
            }
        }
        Shape::Squid | Shape::Ghast => {
            let ghast = kind == HappyGhast;
            let w = if ghast { 16.0 } else { 8.0 };
            d.cube(0, [-w / 2.0, 10.0, -w / 2.0], [w, w, w], Head);
            let count = if ghast { 9 } else { 8 };
            for i in 0..count {
                let a = i as f32 * std::f32::consts::TAU / count as f32;
                let x = a.cos() * w * 0.35;
                let z = a.sin() * w * 0.35;
                let j = d.joint([x, 10.0, z], Action::Feelers(a));
                d.cube(j, [x - 0.8, 0.0, z - 0.8], [1.6, 10.0, 1.6], Body);
            }
        }
        Shape::Nautilus => {
            d.cube(0, [-4.0, 3.0, -6.0], [8.0, 10.0, 12.0], Body);
            d.cube(0, [-4.2, 5.0, -4.0], [8.4, 6.0, 8.0], Accent);
            d.cube(0, [-3.0, 2.0, 4.0], [6.0, 4.0, 4.0], Head);
            for i in 0..4 {
                let x = i as f32 * 1.4 - 2.1;
                let j = d.joint([x, 3.0, 7.0], Action::Feelers(i as f32));
                d.cube(j, [x - 0.4, 2.0, 7.0], [0.8, 0.8, 5.0], Muzzle);
            }
        }
        Shape::Turtle => {
            d.cube(0, [-6.0, 1.0, -7.0], [12.0, 4.0, 14.0], Body);
            d.cube(0, [-5.0, 5.0, -6.0], [10.0, 2.0, 12.0], Body);
            let h = d.joint([0.0, 3.0, 7.0], Action::Head);
            d.cube(h, [-2.0, 2.0, 7.0], [4.0, 3.0, 5.0], Head);
            for (x, z, s) in [
                (-6.0, -4.0, 1.0),
                (6.0, -4.0, -1.0),
                (-6.0, 4.0, -1.0),
                (6.0, 4.0, 1.0),
            ] {
                let j = d.joint([x, 2.0, z], Action::Stride(s));
                d.cube(j, [x - 2.0, 0.5, z - 2.0], [4.0, 1.0, 5.0], Accent);
            }
        }
        Shape::Frog => {
            d.cube(0, [-4.0, 2.0, -4.0], [8.0, 5.0, 8.0], Head);
            for x in [-3.0, 3.0] {
                d.cube(0, [x - 1.0, 7.0, 2.0], [2.0, 2.0, 2.0], Head);
            }
            for (x, z, s) in [
                (-4.0, -2.0, 1.0),
                (4.0, -2.0, -1.0),
                (-3.0, 3.0, -1.0),
                (3.0, 3.0, 1.0),
            ] {
                let j = d.joint([x, 3.0, z], Action::Stride(s));
                d.cube(j, [x - 1.0, 0.0, z - 1.0], [2.0, 3.0, 4.0], Limb);
            }
        }
        Shape::Slime => {
            d.cube(0, [-6.0, 0.0, -6.0], [12.0, 12.0, 12.0], Head);
        }
        Shape::Guardian => {
            d.cube(0, [-4.0, 4.0, -4.0], [8.0, 8.0, 8.0], Head);
            for (min, size) in [
                ([-1.0, 12.0, -1.0], [2.0, 4.0, 2.0]),
                ([-1.0, 0.0, -1.0], [2.0, 4.0, 2.0]),
                ([-8.0, 7.0, -1.0], [4.0, 2.0, 2.0]),
                ([4.0, 7.0, -1.0], [4.0, 2.0, 2.0]),
            ] {
                d.cube(0, min, size, Accent);
            }
            let t = d.joint([0.0, 8.0, -4.0], Action::Tail);
            d.cube(t, [-1.5, 6.5, -10.0], [3.0, 3.0, 6.0], Body);
            d.cube(t, [-0.5, 4.0, -13.0], [1.0, 8.0, 4.0], Accent);
        }
        Shape::Sniffer => {
            for sign in [-1.0, 1.0] {
                for (i, z) in [-8.0, 0.0, 8.0].into_iter().enumerate() {
                    d.leg(
                        sign * 7.0,
                        z,
                        8.0,
                        4.0,
                        if i % 2 == 0 { sign } else { -sign },
                        true,
                    );
                }
            }
            d.cube(0, [-10.0, 7.0, -13.0], [20.0, 13.0, 26.0], Limb);
            d.cube(0, [-10.5, 17.0, -13.5], [21.0, 5.0, 27.0], Body);
            let h = d.joint([0.0, 12.0, 12.0], Action::Head);
            d.cube(h, [-7.0, 9.0, 12.0], [14.0, 9.0, 10.0], Head);
            d.cube(h, [-6.0, 9.0, 21.0], [12.0, 4.0, 3.0], Muzzle);
        }
        Shape::Creaking => {
            d.leg(-3.0, 0.0, 15.0, 3.0, 1.0, false);
            d.leg(3.0, 0.0, 18.0, 3.0, -1.0, false);
            d.cube(0, [-4.0, 14.0, -2.5], [8.0, 14.0, 5.0], Body);
            d.tilt([0.0, 0.0, 0.08]);
            let h = d.joint([0.0, 28.0, 0.0], Action::Head);
            d.cube(h, [-4.0, 28.0, -3.0], [8.0, 7.0, 6.0], Head);
            for (x, len, sign) in [(-6.0, 21.0, -1.0), (6.0, 16.0, 1.0)] {
                let a = d.joint([x, 27.0, 0.0], Action::Arm(sign));
                d.cube(a, [x - 1.5, 27.0 - len, -1.5], [3.0, len, 3.0], Body);
                d.cube(0, [x - 1.0, 27.0, -1.0], [2.0, 8.0, 2.0], Body);
                d.tilt([0.0, 0.0, -sign * 0.35]);
            }
        }
        Shape::Breeze => {
            for (y, w) in [(0.0, 4.0), (3.0, 8.0), (6.0, 12.0), (9.0, 9.0)] {
                d.cube(0, [-w / 2.0, y, -w / 2.0], [w, 2.0, w], Accent);
                d.tilt([0.0, y * 0.15, 0.0]);
            }
            d.cube(0, [-3.0, 11.0, -3.0], [6.0, 5.0, 6.0], Body);
            let h = d.joint([0.0, 16.0, 0.0], Action::Head);
            d.cube(h, [-4.0, 16.0, -4.0], [8.0, 8.0, 8.0], Head);
            d.cube(h, [-5.0, 15.0, -5.0], [10.0, 2.0, 10.0], Accent);
        }
        Shape::Sprite => {
            d.cube(0, [-1.5, 3.0, -1.0], [3.0, 5.0, 2.0], Body);
            let h = d.joint([0.0, 8.0, 0.0], Action::Head);
            d.cube(h, [-3.0, 8.0, -3.0], [6.0, 6.0, 6.0], Head);
            for sign in [-1.0, 1.0] {
                let a = d.joint([sign * 2.0, 7.0, 0.0], Action::Arm(sign));
                d.cube(a, [sign * 2.0 - 0.5, 2.0, -0.5], [1.0, 5.0, 1.0], Limb);
                let w = d.joint([sign * 1.0, 7.0, -1.0], Action::Wing(sign));
                d.cube(
                    w,
                    [if sign < 0.0 { -5.0 } else { 1.0 }, 4.0, -2.0],
                    [4.0, 5.0, 0.5],
                    Accent,
                );
            }
        }
        Shape::Wither => {
            d.cube(0, [-1.5, 0.0, -1.5], [3.0, 24.0, 3.0], Body);
            for y in [9.0, 14.0, 19.0] {
                d.cube(0, [-8.0, y, -1.5], [16.0, 2.0, 3.0], Body);
            }
            for (x, y) in [(-9.0, 20.0), (0.0, 26.0), (9.0, 20.0)] {
                let h = d.joint([x, y, 0.0], Action::Head);
                d.cube(h, [x - 4.0, y, -4.0], [8.0, 8.0, 8.0], Head);
            }
        }
        Shape::Armadillo => {
            let bug = matches!(kind, Silverfish | Endermite);
            for i in 0..3 {
                let z = i as f32 * 4.0 - 6.0;
                d.cube(
                    0,
                    [-4.0, 3.0, z],
                    [8.0, if i == 1 { 6.0 } else { 5.0 }, 4.0],
                    Body,
                );
                for sign in [-1.0, 1.0] {
                    d.leg(
                        sign * 3.0,
                        z + 2.0,
                        3.0,
                        if bug { 1.0 } else { 2.0 },
                        if i % 2 == 0 { sign } else { -sign },
                        false,
                    );
                }
            }
            let h = d.joint([0.0, 4.0, 6.0], Action::Head);
            d.cube(h, [-2.5, 3.0, 6.0], [5.0, 4.0, 4.0], Head);
            if !bug {
                for x in [-1.5, 1.5] {
                    d.cube(h, [x - 0.5, 7.0, 7.0], [1.0, 3.0, 1.0], Accent);
                }
            }
            let t = d.joint([0.0, 4.0, -6.0], Action::Tail);
            d.cube(t, [-0.6, 3.0, -11.0], [1.2, 1.2, 5.0], Accent);
        }
    }
    d
}

fn kind_height(condition: bool, yes: f32, no: f32) -> f32 {
    if condition { yes } else { no }
}

#[derive(Default)]
struct Geometry {
    positions: Vec<f32>,
    normals: Vec<f32>,
    uv: Vec<f32>,
}
struct DrawGroup {
    joint: usize,
    tint: bool,
    mesh: Mesh,
}
struct Model {
    joints: Vec<Joint>,
    groups: Vec<DrawGroup>,
    height: f32,
}
pub struct MobVisuals {
    texture: Texture2D,
    models: Vec<Model>,
}

struct Atlas {
    pixels: Vec<u8>,
    x: usize,
    y: usize,
    row: usize,
    cache: HashMap<(MobKind, Surface, Face, usize, usize), [f32; 4]>,
}
impl Atlas {
    fn new() -> Self {
        Self {
            pixels: vec![0; ATLAS_SIZE * ATLAS_SIZE * 4],
            x: 0,
            y: 0,
            row: 0,
            cache: HashMap::new(),
        }
    }
    fn face(
        &mut self,
        kind: MobKind,
        surface: Surface,
        face: Face,
        w: usize,
        h: usize,
    ) -> [f32; 4] {
        let key = (kind, surface, face, w, h);
        if let Some(rect) = self.cache.get(&key) {
            return *rect;
        }
        if self.x + w + 4 > ATLAS_SIZE {
            self.x = 0;
            self.y += self.row;
            self.row = 0;
        }
        assert!(self.y + h + 4 <= ATLAS_SIZE, "mob texture atlas is full");
        let (ox, oy) = (self.x + 2, self.y + 2);
        for y in 0..h + 4 {
            for x in 0..w + 4 {
                let px = x.saturating_sub(2).min(w - 1);
                let py = y.saturating_sub(2).min(h - 1);
                let rgb = shade(
                    texel(kind, surface, face, px, py, w, h),
                    match face {
                        Face::Top => 5,
                        Face::Right => -15,
                        Face::Left | Face::Back => -8,
                        Face::Bottom => -25,
                        Face::Front => 0,
                    },
                );
                let index = ((self.y + y) * ATLAS_SIZE + self.x + x) * 4;
                self.pixels[index..index + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        let size = ATLAS_SIZE as f32;
        let rect = [
            (ox as f32 + 0.01) / size,
            (oy as f32 + 0.01) / size,
            ((ox + w) as f32 - 0.01) / size,
            ((oy + h) as f32 - 0.01) / size,
        ];
        self.cache.insert(key, rect);
        self.x += w + 4;
        self.row = self.row.max(h + 4);
        rect
    }
}

fn shade(rgb: [u8; 3], amount: i32) -> [u8; 3] {
    rgb.map(|c| (c as i32 + amount).clamp(0, 255) as u8)
}
fn texel(
    kind: MobKind,
    surface: Surface,
    face: Face,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> [u8; 3] {
    use MobKind::*;
    use Surface::*;
    let s = kind.species();
    let humanoid = matches!(s.shape, Shape::Person | Shape::Villager);
    let bone = matches!(
        kind,
        Skeleton | Stray | Bogged | Parched | SkeletonHorse | Wither
    );
    let skin = if matches!(kind, Villager | WanderingTrader) {
        [199, 155, 112]
    } else {
        s.accent
    };
    let base = match surface {
        Head if humanoid && !bone && kind != Enderman => skin,
        Head if kind == SnowGolem => s.accent,
        Limb if matches!(
            kind,
            Zombie | Husk | Drowned | ZombieVillager | ZombifiedPiglin | Sniffer
        ) =>
        {
            skin
        }
        Limb if kind == Sheep => s.accent,
        Accent => s.accent,
        Dark => [54, 46, 40],
        Muzzle if matches!(kind, Chicken | Parrot | Sniffer) => [219, 174, 72],
        Muzzle if kind == Pig => [207, 128, 140],
        Muzzle if kind == Dolphin => [139, 164, 170],
        Body | Head if kind == Dolphin && face == Face::Bottom => [189, 202, 198],
        Body | Head if kind == Dolphin && face == Face::Top => [103, 133, 145],
        Muzzle if matches!(kind, Cow | Mooshroom) => [162, 132, 115],
        Muzzle if humanoid => shade(skin, -12),
        Muzzle => s.accent,
        Wood => [122, 86, 50],
        Wool => [227, 225, 215],
        Pants if kind == Zombie => [62, 67, 117],
        Pants => shade(s.coat, -30),
        _ => s.coat,
    };
    // Sparse pixel clusters, with a quiet palette and seams instead of static noise.
    let hash = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ (kind as u32 * 97);
    let grain = match (hash ^ (hash >> 13)) % 13 {
        0 => -12,
        1 | 2 => -5,
        3 => 6,
        _ => 0,
    };
    let mut c = shade(base, grain);
    if matches!(surface, Body | Head | Limb | Wool) {
        if matches!(kind, Cow | Mooshroom)
            && ((x / 3 * 7 + y / 3 * 11 + (face as usize) * 3) % 13 < 4)
        {
            c = shade(s.accent, grain / 2);
        }
        if kind == Sheep
            && surface == Wool
            && ((x + 2 * (y / 3)).is_multiple_of(4) || y.is_multiple_of(4))
        {
            c = shade(base, -13);
        }
        if matches!(kind, Creeper | Bogged | Zombie | Husk | Drowned)
            && (x / 2 * 3 + y / 2 * 7) % 11 < 3
        {
            c = shade(base, -22);
        }
        if kind == Bee && !matches!(face, Face::Front | Face::Back) && y % 4 < 2 {
            c = s.accent;
        }
        if matches!(kind, Creaking | Armadillo | Silverfish | Endermite)
            && (y.is_multiple_of(4) || (x + y / 5).is_multiple_of(5))
        {
            c = shade(base, -25);
        }
        if matches!(kind, Golem | CopperGolem) {
            if y == 0 || x == 0 || y + 1 == h {
                c = shade(base, -28);
            }
            if (x + y / 3) % 9 == 2 && y > h / 3 {
                c = shade(s.accent, grain);
            }
        }
        if matches!(kind, Panda)
            && (surface == Limb || (surface == Body && y > h / 3 && y < h * 2 / 3))
        {
            c = s.accent;
        }
        if matches!(kind, Ocelot | Cat | TropicalFish) && (x / 2 + y / 3 * 3).is_multiple_of(7) {
            c = s.accent;
        }
        if kind == Turtle && (x.is_multiple_of(4) || y.is_multiple_of(4)) {
            c = shade(base, -25);
        }
        if matches!(kind, Nautilus | ZombieNautilus) && matches!(face, Face::Right | Face::Left) {
            let dx = x as f32 - w as f32 / 2.0;
            let dy = y as f32 - h as f32 / 2.0;
            if ((dx * dx + dy * dy).sqrt() + dy.atan2(dx) * 1.6).rem_euclid(3.5) < 1.0 {
                c = s.accent;
            }
        }
        if humanoid && surface == Body && !bone && (x == w / 2 || y == h - 2) {
            c = shade(base, -22);
        }
        if kind == Sniffer && surface == Body && (x / 3 + y / 2).is_multiple_of(5) {
            c = shade(base, 25);
        }
        if matches!(kind, Slime | SulfurCube) && (x == 1 || y == 1) {
            c = shade(base, 28);
        }
    }
    if surface == Muzzle && face == Face::Front && w >= 4 && y == h / 2 && (x == 1 || x == w - 2) {
        c = shade(base, -68);
    }
    if surface == Head
        && matches!(s.shape, Shape::Fish | Shape::Bird)
        && matches!(face, Face::Right | Face::Left)
    {
        let (px, py) = (x * 8 / w, y * 8 / h);
        let front = if face == Face::Right {
            px <= 1
        } else {
            px >= 6
        };
        if front && (2..4).contains(&py) {
            return [30, 29, 27];
        }
    }
    if s.shape == Shape::Fish {
        return c;
    }
    if surface != Head || face != Face::Front || w < 4 || h < 4 {
        return c;
    }
    let (px, py) = (x * 8 / w, y * 8 / h);
    let dark = [32, 29, 30];
    if kind == Creeper {
        if ((1..3).contains(&px) || (5..7).contains(&px)) && (2..4).contains(&py)
            || (3..5).contains(&px) && (3..6).contains(&py)
            || (2..6).contains(&px) && (5..7).contains(&py)
        {
            return dark;
        }
        return c;
    }
    if kind == Warden {
        if (2..6).contains(&px) && py >= 5 && (px + py) % 2 == 0 {
            return s.accent;
        }
        return c;
    }
    if kind == Creaking {
        if [(1, 3), (4, 2), (6, 4)].contains(&(px, py)) {
            return s.accent;
        }
        return c;
    }
    if matches!(kind, Guardian | ElderGuardian) {
        if (2..6).contains(&px) && (2..6).contains(&py) {
            return if (3..5).contains(&px) && (3..5).contains(&py) {
                [124, 50, 43]
            } else {
                [222, 218, 188]
            };
        }
        return c;
    }
    let eye = (px == 1 || px == 2 || px == 5 || px == 6) && (py == 2 || py == 3);
    if eye {
        if w < 8 || matches!(kind, Slime | SulfurCube) {
            return shade(base, -100);
        }
        if matches!(kind, Spider | CaveSpider | Golem) {
            return [177, 49, 41];
        }
        if kind == Enderman {
            return [207, 134, 238];
        }
        if kind == Phantom {
            return [167, 215, 90];
        }
        if matches!(kind, Allay | Vex | GlowSquid) {
            return [214, 249, 240];
        }
        if bone || kind == SnowGolem {
            return dark;
        }
        if px == 1 || px == 6 {
            return [226, 223, 197];
        }
        return dark;
    }
    if humanoid && py == 1 && (1..7).contains(&px) {
        return shade(base, -55);
    }
    if py == 6 && (2..6).contains(&px) {
        return shade(base, -65);
    }
    if kind == Panda && ((px <= 2 || px >= 5) && (1..5).contains(&py)) {
        return s.accent;
    }
    c
}

fn cube_faces(cube: &Cube) -> [(Face, [Vec3; 4], Vec3, usize, usize); 6] {
    let a = cube.min;
    let b = a + cube.size;
    let v = |x, y, z| Vec3::new(x, y, z);
    let [w, h, d] = cube.size.to_array().map(|x| x.round().max(1.0) as usize);
    [
        (
            Face::Front,
            [
                v(a.x, a.y, b.z),
                v(b.x, a.y, b.z),
                v(b.x, b.y, b.z),
                v(a.x, b.y, b.z),
            ],
            Vec3::Z,
            w,
            h,
        ),
        (
            Face::Back,
            [
                v(b.x, a.y, a.z),
                v(a.x, a.y, a.z),
                v(a.x, b.y, a.z),
                v(b.x, b.y, a.z),
            ],
            -Vec3::Z,
            w,
            h,
        ),
        (
            Face::Right,
            [
                v(b.x, a.y, b.z),
                v(b.x, a.y, a.z),
                v(b.x, b.y, a.z),
                v(b.x, b.y, b.z),
            ],
            Vec3::X,
            d,
            h,
        ),
        (
            Face::Left,
            [
                v(a.x, a.y, a.z),
                v(a.x, a.y, b.z),
                v(a.x, b.y, b.z),
                v(a.x, b.y, a.z),
            ],
            -Vec3::X,
            d,
            h,
        ),
        (
            Face::Top,
            [
                v(a.x, b.y, b.z),
                v(b.x, b.y, b.z),
                v(b.x, b.y, a.z),
                v(a.x, b.y, a.z),
            ],
            Vec3::Y,
            w,
            d,
        ),
        (
            Face::Bottom,
            [
                v(a.x, a.y, a.z),
                v(b.x, a.y, a.z),
                v(b.x, a.y, b.z),
                v(a.x, a.y, b.z),
            ],
            -Vec3::Y,
            w,
            d,
        ),
    ]
}

fn bake(kind: MobKind, design: &Design, atlas: &mut Atlas) -> Vec<(usize, bool, Geometry)> {
    let mut groups: Vec<(usize, bool, Geometry)> = Vec::new();
    for cube in &design.cubes {
        let tint = kind == MobKind::Sheep && cube.surface == Surface::Wool
            || kind == MobKind::Villager && cube.surface == Surface::Body;
        let index = groups
            .iter()
            .position(|(j, t, _)| *j == cube.joint && *t == tint)
            .unwrap_or_else(|| {
                groups.push((cube.joint, tint, Geometry::default()));
                groups.len() - 1
            });
        let geometry = &mut groups[index].2;
        let center = cube.min + cube.size * 0.5;
        let rot = Mat4::from_euler(
            glam::EulerRot::XYZ,
            cube.rotation.x,
            cube.rotation.y,
            cube.rotation.z,
        );
        for (face, vertices, normal, w, h) in cube_faces(cube) {
            let surface = if cube.surface == Surface::Limb
                && matches!(design.joints[cube.joint].action, Action::Stride(_))
                && matches!(
                    kind,
                    MobKind::Zombie | MobKind::Husk | MobKind::Drowned | MobKind::ZombifiedPiglin
                ) {
                Surface::Pants
            } else {
                cube.surface
            };
            let [u0, v0, u1, v1] = atlas.face(kind, surface, face, w, h);
            let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
            for i in [0, 1, 2, 0, 2, 3] {
                let pos = rot.transform_point3(vertices[i] - center) + center
                    - design.joints[cube.joint].pivot;
                geometry.positions.extend_from_slice(&pos.to_array());
                geometry
                    .normals
                    .extend_from_slice(&rot.transform_vector3(normal).to_array());
                geometry.uv.extend_from_slice(&uv[i]);
            }
        }
    }
    groups
}

pub fn facing(yaw: f32) -> Mat4 {
    Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2 - yaw)
}
impl MobVisuals {
    pub fn new() -> Self {
        let mut atlas = Atlas::new();
        let models = MobKind::ALL
            .iter()
            .map(|&kind| {
                let design = design(kind);
                let height = design
                    .cubes
                    .iter()
                    .map(|c| c.min.y + c.size.y)
                    .fold(1.0, f32::max);
                let groups = bake(kind, &design, &mut atlas)
                    .into_iter()
                    .map(|(joint, tint, g)| DrawGroup {
                        joint,
                        tint,
                        mesh: Mesh::new(
                            &g.positions,
                            Some(&g.uv),
                            Some(&g.normals),
                            Some(&vec![255; g.positions.len() / 3 * 4]),
                        ),
                    })
                    .collect();
                Model {
                    joints: design.joints,
                    groups,
                    height,
                }
            })
            .collect();
        Self {
            texture: Texture2D::from_data(&atlas.pixels, ATLAS_SIZE as i32, ATLAS_SIZE as i32),
            models,
        }
    }
    pub fn draw(&self, mob: &Mob, shader: &Shader, time: f32, viewer: Vec3) {
        self.texture.bind(0);
        let model = &self.models[mob.kind as usize];
        let scale = mob.height() / model.height;
        let base = Mat4::from_translation(mob.position)
            * facing(mob.yaw)
            * Mat4::from_rotation_x(if mob.kind == MobKind::Dolphin {
                mob.swim_pitch - 0.05 - 0.05 * (time * 6.0).cos()
            } else if mob.kind.species().motion == crate::mob_catalog::Motion::Swim {
                mob.swim_pitch
            } else {
                0.0
            })
            * Mat4::from_scale(Vec3::splat(scale));
        let transforms = joint_transforms(&model.joints, mob, time, viewer);
        for group in &model.groups {
            let transform = base * transforms[group.joint];
            let tint = if group.tint {
                variant_tint(mob)
            } else {
                Vec4::ONE
            };
            shader.set_vec4(shader.get_uniform_location("colDiffuse"), tint);
            shader.set_mat4(shader.get_uniform_location("uModel"), &transform);
            group.mesh.draw();
        }
    }
}

fn joint_transforms(joints: &[Joint], mob: &Mob, time: f32, viewer: Vec3) -> Vec<Mat4> {
    let mut transforms: Vec<Mat4> = Vec::with_capacity(joints.len());
    for joint in joints {
        let swing = mob.walk_phase.sin() * mob.walk_blend;
        let mut rotation = Vec3::ZERO;
        match joint.action {
            Action::Fixed => {}
            Action::Head => {
                if viewer.distance_squared(mob.position) < 64.0 {
                    let target = viewer - mob.position;
                    let delta = (target.z.atan2(target.x) - mob.yaw + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    rotation.y = -delta.clamp(-0.55, 0.55);
                }
                if mob.kind == MobKind::Sheep && mob.animal.eat_time > 0.0 {
                    rotation.x = 0.55;
                }
            }
            Action::Stride(sign) => rotation.x = swing * sign * 0.9,
            Action::Arm(sign) => {
                rotation.x = -swing * sign * 0.45;
                if matches!(
                    mob.kind,
                    MobKind::Zombie | MobKind::Husk | MobKind::Drowned | MobKind::ZombifiedPiglin
                ) {
                    rotation.x -= 1.35;
                }
                if matches!(
                    mob.kind,
                    MobKind::Skeleton | MobKind::Stray | MobKind::Bogged | MobKind::Parched
                ) {
                    rotation.x -= 0.8;
                }
            }
            Action::Wing(sign) => {
                rotation.z = (time * if mob.kind == MobKind::Bee { 34.0 } else { 12.0 }).sin()
                    * sign
                    * if mob.kind == MobKind::Chicken {
                        mob.walk_blend * 0.12
                    } else {
                        0.65
                    }
            }
            Action::DolphinTail => rotation.x = -0.10 * (time * 6.0).cos(),
            Action::DolphinFluke => rotation.x = -0.20 * (time * 6.0).cos(),
            Action::DolphinFin(sign) => rotation.z = -sign * (0.40 + 0.30 * (time * 4.0).cos()),
            Action::Tail => {
                rotation.y = (time * 4.0 + mob.variant as f32).sin() * 0.18 + swing * 0.12
            }
            Action::Feelers(phase) => rotation.x = (time * 2.5 + phase).sin() * 0.23,
        }
        let rotation = Mat4::from_euler(glam::EulerRot::XYZ, rotation.x, rotation.y, rotation.z);
        let transform = if let Some(parent) = joint.parent {
            transforms[parent]
                * Mat4::from_translation(joint.pivot - joints[parent].pivot)
                * rotation
        } else {
            Mat4::from_translation(joint.pivot) * rotation
        };
        transforms.push(transform);
    }
    transforms
}

fn variant_tint(mob: &Mob) -> Vec4 {
    let rgb = if mob.kind == MobKind::Sheep {
        match mob.variant % 5 {
            0..=2 => [255, 255, 255],
            3 => [240, 169, 188],
            _ => [141, 130, 119],
        }
    } else {
        match mob.variant % 3 {
            0 => [255, 255, 255],
            1 => [160, 213, 148],
            _ => [187, 182, 212],
        }
    };
    Vec4::new(
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dolphin_stroke_moves_vertical_tail_and_inherits_fluke_pivot() {
        let d = design(MobKind::Dolphin);
        let mob = Mob::new(MobKind::Dolphin, Vec3::ZERO, Vec3::ZERO, 0);
        let a = joint_transforms(&d.joints, &mob, 0.0, Vec3::ZERO);
        let b = joint_transforms(&d.joints, &mob, std::f32::consts::PI / 6.0, Vec3::ZERO);
        let tail = d
            .joints
            .iter()
            .position(|j| matches!(j.action, Action::DolphinTail))
            .unwrap();
        let fluke = d
            .joints
            .iter()
            .position(|j| matches!(j.action, Action::DolphinFluke))
            .unwrap();
        assert_eq!(d.joints[fluke].parent, Some(tail));
        let tip = Vec3::new(0.0, 0.0, -4.0);
        let delta = a[fluke].transform_point3(tip) - b[fluke].transform_point3(tip);
        assert!(delta.y.abs() > 2.0, "tail must visibly beat up and down");
        assert!(delta.x.abs() < 1e-5, "dolphins must not use fish tail yaw");
        for (i, j) in d.joints.iter().enumerate() {
            if matches!(j.action, Action::DolphinFin(_)) {
                assert_ne!(a[i], b[i], "pectoral fins must articulate");
            }
        }
        let attachment = d.joints[fluke].pivot - d.joints[tail].pivot;
        assert!(
            a[fluke]
                .transform_point3(Vec3::ZERO)
                .distance(a[tail].transform_point3(attachment))
                < 1e-5
        );
    }
    #[test]
    fn every_species_has_textured_outward_faces_and_articulated_geometry() {
        let mut atlas = Atlas::new();
        for &kind in MobKind::ALL {
            let d = design(kind);
            let groups = bake(kind, &d, &mut atlas);
            assert!(!groups.is_empty());
            assert!(groups.len() <= 16, "too many draw groups for {kind:?}");
            for (_, _, g) in groups {
                assert_eq!(g.uv.len(), g.positions.len() / 3 * 2);
                assert!(g.uv.iter().all(|v| *v > 0.0 && *v < 1.0));
                for (p, n) in g
                    .positions
                    .as_chunks::<9>()
                    .0
                    .iter()
                    .zip(g.normals.as_chunks::<9>().0.iter())
                {
                    let a = Vec3::from_slice(p);
                    let b = Vec3::from_slice(&p[3..]);
                    let c = Vec3::from_slice(&p[6..]);
                    assert!(
                        (b - a).cross(c - a).dot(Vec3::from_slice(n)) > 0.0,
                        "inward face on {kind:?}"
                    );
                }
            }
        }
        assert!(atlas.y < ATLAS_SIZE);
    }
    #[test]
    fn faces_follow_the_same_heading_as_movement() {
        for yaw in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
            let look = facing(yaw).transform_vector3(Vec3::Z);
            assert!(look.distance(Vec3::new(yaw.cos(), 0.0, yaw.sin())) < 1e-5);
        }
    }
    #[test]
    fn textures_have_faces_not_repeated_wool_tiles() {
        for kind in [
            MobKind::Pig,
            MobKind::Cow,
            MobKind::Zombie,
            MobKind::Skeleton,
            MobKind::Villager,
            MobKind::Creeper,
        ] {
            let front: Vec<_> = (0..8)
                .flat_map(|y| {
                    (0..8).map(move |x| texel(kind, Surface::Head, Face::Front, x, y, 8, 8))
                })
                .collect();
            let back: Vec<_> = (0..8)
                .flat_map(|y| {
                    (0..8).map(move |x| texel(kind, Surface::Head, Face::Back, x, y, 8, 8))
                })
                .collect();
            assert_ne!(front, back, "missing face on {kind:?}");
        }
    }
}
