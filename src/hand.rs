// First-person arm: one plain skin-coloured box, the way Minecraft draws it.
// Idle bob and a swing on use. Deliberately not a modelled hand -- an earlier
// version had a palm, four finger nubs and a thumb, and at first-person size
// that reads as a claw rather than an arm.
//
// Three conventions this file must not break:
//   * Every quad is wound counter-clockwise as seen from outside, so that
//     back-face culling and the normals `gbuffer.wgsl` feeds to the deferred
//     lighting pass agree. A reversed quad is lit as if it faced inward.
//   * The arm is drawn in view space (projection only, no view matrix), so
//     every transformed vertex has to stay in front of the near plane or the
//     box is sliced open and you see straight into its hollow inside.
//   * The arm's projection shares the world's near/far but pins its own FOV.
//     See `projection`.
use crate::block::BlockType;
use crate::hud;
use crate::item;
use crate::renderer::{self, Mesh, Shader};
use glam::{Mat4, Vec3};

/// Vertical FOV the arm is drawn at, independent of the world FOV setting
/// (`src/hud.rs` offers 60..100). Sharing the world FOV made the arm scale
/// with the slider, roughly twice as large at 60 as at 100.
const HAND_FOV_Y: f32 = 70.0;
/// Near and far planes. These MUST match the world projection in `main`.
///
/// The arm is rasterised into the shared G-buffer, so its fragments compete
/// in the same depth buffer as the terrain. Depth is non-linear in near/far,
/// so giving the arm its own tighter planes made identical true distances
/// encode to larger depths than the world used -- every arm fragment read as
/// further away than it was, and nearby terrain punched a hole through the
/// arm. Only the FOV may differ; it does not affect depth.
const NEAR_PLANE: f32 = 0.1;
const FAR_PLANE: f32 = 1000.0;
/// How far in front of the near plane the arm must stay.
const NEAR_MARGIN: f32 = 0.03;

/// Arm box in local space: square cross-section, hand end toward -Z, elbow
/// running back toward the camera. Roughly Minecraft's 4x4x12 proportions.
const ARM_THICKNESS: f32 = 0.155;
const ARM_FORWARD: f32 = 0.38;
const ARM_BACK: f32 = 0.145;
/// Half-extent of the held block cube.
const ITEM_SCALE: f32 = 0.17;

/// Camera height above `player.position`, matching `main`.
const EYE_HEIGHT: f32 = 1.6;
/// Hip joint the legs swing from.
const HIP_HEIGHT: f32 = 0.75;
/// Top of the torso.
///
/// Not the anatomical 1.5: the body is real world geometry, so a full-height
/// torso pokes above the bottom of the frustum and your own chest hangs in
/// the view while you walk. This keeps every torso corner below the bottom
/// plane even at the widest FOV the settings allow.
const TORSO_TOP: f32 = 1.08;
const TORSO_HALF_W: f32 = 0.16;
const TORSO_HALF_D: f32 = 0.08;
const LEG_HALF_W: f32 = 0.08;
const LEG_HALF_D: f32 = 0.08;

/// Body-local +Z is look-forward. The camera sits at XZ origin, `EYE_HEIGHT`.
///
/// A solid torso under the eye is a lid when you look down. Shoving the
/// torso behind the camera and the legs in front disconnects the body: you
/// see the lid AND the legs, which is what looking down was showing.
/// Legs stay under the torso. The torso mesh has no top face, so the
/// look-down ray goes through the collar onto the thighs and feet.
const TORSO_CENTER_Z: f32 = -0.08;
const LEG_CENTER_Z: f32 = TORSO_CENTER_Z;

/// Tints are applied through `colDiffuse`, not vertex colour.
///
/// `gbuffer.wgsl` builds albedo as `texel.rgb * col_diffuse.rgb`, and routes
/// the vertex colour to the lighting attachment instead -- where entity draws
/// overwrite it wholesale, because `entity_lighting` returns alpha 1. So a
/// colour baked into the mesh never reaches the screen; these meshes sample a
/// near-white tile and take their colour entirely from this tint, the same
/// way `World::render_mobs` tints body parts.
fn tint(color: [u8; 4]) -> glam::Vec4 {
    glam::Vec4::new(
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        1.0,
    )
}

/// The arm's own projection. Takes only the aspect ratio: the world FOV
/// deliberately does not reach it, so the arm is identical on every setting.
pub fn projection(aspect: f32) -> Mat4 {
    Mat4::perspective_rh(HAND_FOV_Y.to_radians(), aspect, NEAR_PLANE, FAR_PLANE)
}

/// Emit one quad as two triangles. `quad` is bottom-left, bottom-right,
/// top-right, top-left as seen from outside, which is counter-clockwise.
fn push_quad(
    v: &mut Vec<f32>,
    n: &mut Vec<f32>,
    c: &mut Vec<u8>,
    quad: [[f32; 3]; 4],
    nrm: [f32; 3],
    color: [u8; 4],
) {
    let [a, b, cc, d] = quad;
    for p in [a, b, cc, a, cc, d] {
        v.extend_from_slice(&p);
    }
    for _ in 0..6 {
        n.extend_from_slice(&nrm);
        c.extend_from_slice(&color);
    }
}

fn push_box(
    v: &mut Vec<f32>,
    n: &mut Vec<f32>,
    c: &mut Vec<u8>,
    origin: [f32; 3],
    size: [f32; 3],
    color: [u8; 4],
) {
    let [x0, y0, z0] = origin;
    let x1 = x0 + size[0];
    let y1 = y0 + size[1];
    let z1 = z0 + size[2];
    push_quad(
        v,
        n,
        c,
        [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
        [0.0, 0.0, -1.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [0.0, 0.0, 1.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        [-1.0, 0.0, 0.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
        [1.0, 0.0, 0.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
        [0.0, -1.0, 0.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
        [0.0, 1.0, 0.0],
        color,
    );
}

fn solid_uvs(count: usize) -> Vec<f32> {
    // Sample the near-white iron ingot tile so albedo comes from `colDiffuse`.
    let ts = 1.0 / 16.0;
    let u0 = 8.0 * ts + 0.4 / 256.0;
    let v0 = 1.0 * ts + 0.4 / 256.0;
    let u1 = 9.0 * ts - 0.4 / 256.0;
    let v1 = 2.0 * ts - 0.4 / 256.0;
    let mut t = Vec::with_capacity(count * 2);
    for _ in 0..(count / 6) {
        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
    }
    t
}

/// Origin and size of the single arm box, in local space.
fn arm_box() -> ([f32; 3], [f32; 3]) {
    let h = ARM_THICKNESS * 0.5;
    (
        [-h, -h, -ARM_FORWARD],
        [ARM_THICKNESS, ARM_THICKNESS, ARM_FORWARD + ARM_BACK],
    )
}

/// One box, hand end toward -Z (camera forward).
pub fn build_arm_mesh() -> Mesh {
    let (origin, size) = arm_box();
    let mut v = Vec::new();
    let mut n = Vec::new();
    let mut c = Vec::new();
    push_box(&mut v, &mut n, &mut c, origin, size, WHITE);
    let t = solid_uvs(v.len() / 3);
    Mesh::new(&v, Some(&t), Some(&n), Some(&c))
}

/// Meshes are built white; every colour arrives through `colDiffuse`.
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn body_colors(selected_skin: u8) -> ([u8; 4], [u8; 4]) {
    let shirt = match selected_skin {
        1 => [70u8, 90, 160, 255],
        2 => [90, 50, 40, 255],
        3 => [40, 70, 110, 255],
        _ => [55, 140, 150, 255],
    };
    ([64, 64, 128, 255], shirt)
}

/// Front and two sides. No top face: that is the lid you see looking down.
fn push_torso(v: &mut Vec<f32>, n: &mut Vec<f32>, c: &mut Vec<u8>, color: [u8; 4]) {
    let x0 = -TORSO_HALF_W;
    let x1 = TORSO_HALF_W;
    let y0 = HIP_HEIGHT;
    let y1 = TORSO_TOP;
    let z0 = TORSO_CENTER_Z - TORSO_HALF_D;
    let z1 = TORSO_CENTER_Z + TORSO_HALF_D;
    push_quad(
        v,
        n,
        c,
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [0.0, 0.0, 1.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        [-1.0, 0.0, 0.0],
        color,
    );
    push_quad(
        v,
        n,
        c,
        [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
        [1.0, 0.0, 0.0],
        color,
    );
}

/// Torso, in body-local space: the player stands at the origin facing +Z.
pub fn build_torso_mesh() -> Mesh {
    let mut v = Vec::new();
    let mut n = Vec::new();
    let mut c = Vec::new();
    push_torso(&mut v, &mut n, &mut c, WHITE);
    let t = solid_uvs(v.len() / 3);
    Mesh::new(&v, Some(&t), Some(&n), Some(&c))
}

/// One leg, hanging from a hip pivot at the local origin so it can swing.
pub fn build_leg_mesh() -> Mesh {
    let mut v = Vec::new();
    let mut n = Vec::new();
    let mut c = Vec::new();
    push_box(
        &mut v,
        &mut n,
        &mut c,
        [-LEG_HALF_W, -HIP_HEIGHT, -LEG_HALF_D],
        [LEG_HALF_W * 2.0, HIP_HEIGHT, LEG_HALF_D * 2.0],
        WHITE,
    );
    let t = solid_uvs(v.len() / 3);
    Mesh::new(&v, Some(&t), Some(&n), Some(&c))
}

/// Hip angle for the walk cycle. Legs are still when the player is.
pub fn leg_swing(speed: f32, time: f32) -> f32 {
    (time * 8.0).sin() * 0.45 * (speed / 4.0).clamp(0.0, 1.0)
}

/// Draw the player's own torso and legs in world space, so looking down
/// shows them. Unlike the arm these are ordinary world geometry: they use
/// the world view-projection and are revealed by pitch like anything else.
#[allow(clippy::too_many_arguments)]
pub fn draw_body(
    shader: &Shader,
    atlas: &renderer::Texture2D,
    torso: &Mesh,
    leg: &Mesh,
    world_mvp: &Mat4,
    selected_skin: u8,
    player_pos: Vec3,
    yaw: f32,
    speed: f32,
    time: f32,
    light: glam::Vec4,
) {
    atlas.bind(0);
    shader.bind();
    let loc_diff = shader.get_uniform_location("colDiffuse");
    let (pants, shirt) = body_colors(selected_skin);
    shader.set_vec4(shader.get_uniform_location("uColor"), light);
    shader.set_mat4(shader.get_uniform_location("uMVP"), world_mvp);
    let loc_model = shader.get_uniform_location("uModel");
    shader.set_vec4(loc_diff, tint(shirt));
    // Body-local +Z is forward, and `look_dir` is (sin yaw, _, cos yaw), so
    // the yaw rotation applies directly rather than negated as for mobs.
    let base = Mat4::from_translation(player_pos) * Mat4::from_rotation_y(yaw);
    shader.set_mat4(loc_model, &base);
    torso.draw();
    shader.set_vec4(loc_diff, tint(pants));
    let swing = leg_swing(speed, time);
    for (side, phase) in [(-1.0f32, swing), (1.0, -swing)] {
        let model = base
            * Mat4::from_translation(Vec3::new(side * LEG_HALF_W, HIP_HEIGHT, LEG_CENTER_Z))
            * Mat4::from_rotation_x(phase);
        shader.set_mat4(loc_model, &model);
        leg.draw();
    }
    shader.set_mat4(loc_model, &Mat4::IDENTITY);
    shader.set_vec4(loc_diff, glam::Vec4::ONE);
    shader.set_vec4(shader.get_uniform_location("uColor"), glam::Vec4::ZERO);
}

pub fn build_item_mesh(block: BlockType) -> Mesh {
    let (tx, ty) = item::atlas_uv(block);
    let ts = 1.0 / 16.0;
    let pad = 0.5 / 256.0;
    let u0 = tx as f32 * ts + pad;
    let v0 = ty as f32 * ts + pad;
    let u1 = (tx as f32 + 1.0) * ts - pad;
    let v1 = (ty as f32 + 1.0) * ts - pad;
    let mut v = Vec::new();
    let mut t = Vec::new();
    let mut n = Vec::new();
    let mut c = Vec::new();
    let uvs = [u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0];
    let white = [255u8; 4];
    if block.is_item() || !block.is_solid() {
        // Thin card, Minecraft held-item silhouette.
        let (x0, x1) = (-0.08, 0.08);
        let (y0, y1) = (-0.08, 0.18);
        let (z0, z1) = (-0.02, 0.02);
        push_quad(
            &mut v,
            &mut n,
            &mut c,
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            [0.0, 0.0, 1.0],
            white,
        );
        push_quad(
            &mut v,
            &mut n,
            &mut c,
            [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            [0.0, 0.0, -1.0],
            white,
        );
        for _ in 0..2 {
            t.extend_from_slice(&uvs);
        }
    } else {
        let s = ITEM_SCALE;
        push_box(
            &mut v,
            &mut n,
            &mut c,
            [-s, -s, -s],
            [2.0 * s, 2.0 * s, 2.0 * s],
            white,
        );
        for _ in 0..6 {
            t.extend_from_slice(&uvs);
        }
    }
    Mesh::new(&v, Some(&t), Some(&n), Some(&c))
}

pub fn arm_model(swing: f32, bob: f32, aspect: f32) -> Mat4 {
    let punch = swing * swing * (3.0 - 2.0 * swing);
    // Tuned against a Minecraft screenshot: at 16:9 the arm's top edge lands
    // at NDC y -0.42 and it spans x +0.36..+0.74, running off the bottom of
    // the frame so it reads as attached to the player rather than floating.
    //
    // A fixed view-space X would drift toward the centre on a wide window,
    // because horizontal NDC scales with 1/aspect. Offsetting by
    // `0.247 * aspect` holds the arm the same distance from the right edge
    // on 4:3 through 32:9.
    Mat4::from_translation(Vec3::new(
        0.46 + 0.247 * aspect + bob * 0.04,
        -0.71 + bob * 0.05,
        -1.24,
    )) * Mat4::from_rotation_x(0.52 + punch * 0.55)
        * Mat4::from_rotation_y(0.39)
        * Mat4::from_rotation_z(0.36 - punch * 0.45)
}

pub fn item_model(swing: f32, bob: f32, aspect: f32) -> Mat4 {
    // Ride just past the hand end of the arm.
    arm_model(swing, bob, aspect)
        * Mat4::from_translation(Vec3::new(0.0, 0.02, -ARM_FORWARD - 0.04))
        * Mat4::from_rotation_y(0.5)
        * Mat4::from_rotation_x(-0.15)
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    shader: &Shader,
    atlas: &renderer::Texture2D,
    arm: &Mesh,
    item: Option<&Mesh>,
    world_mvp: &Mat4,
    selected_skin: u8,
    aspect: f32,
    swing: f32,
    bob: f32,
    light: glam::Vec4,
) {
    // `gbuffer.wgsl` computes `uMVP * (uModel * position)`, so uMVP must be
    // the view-projection ALONE, exactly as the chunk and entity passes bind
    // it. Folding the model matrix into uMVP as well applies the arm's
    // transform twice and lands it nowhere near where it was placed.
    //
    // The arm is authored directly in view space, so its projection is the
    // whole of its "view-projection". Deliberately not the world projection:
    // see `projection`.
    let proj = projection(aspect);
    atlas.bind(0);
    shader.bind();
    let loc_diff = shader.get_uniform_location("colDiffuse");
    shader.set_vec4(loc_diff, tint(hud::skin_preview_color(selected_skin)));
    shader.set_vec4(shader.get_uniform_location("uColor"), light);
    shader.set_mat4(shader.get_uniform_location("uMVP"), &proj);
    shader.set_mat4(
        shader.get_uniform_location("uModel"),
        &arm_model(swing, bob, aspect),
    );
    arm.draw();
    if let Some(item) = item {
        // The held block carries its own atlas texture, so it must not be
        // tinted with the player's skin tone.
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
        shader.set_mat4(
            shader.get_uniform_location("uModel"),
            &item_model(swing, bob, aspect),
        );
        item.draw();
    }
    // Leave the shared uniforms as this pass found them. The chunk passes
    // that follow bind their own uModel but inherit uMVP, so leaving the
    // arm's view-space projection bound would draw the transparent layer
    // through it.
    shader.set_mat4(shader.get_uniform_location("uMVP"), world_mvp);
    shader.set_mat4(shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
    shader.set_vec4(loc_diff, glam::Vec4::ONE);
    shader.set_vec4(shader.get_uniform_location("uColor"), glam::Vec4::ZERO);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aspect ratios from an old 4:3 panel out to a 32:9 superultrawide.
    const ASPECTS: [f32; 4] = [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0, 32.0 / 9.0];

    fn arm_corners() -> Vec<Vec3> {
        let (o, s) = arm_box();
        let mut out = Vec::new();
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    out.push(Vec3::new(
                        o[0] + s[0] * i as f32,
                        o[1] + s[1] * j as f32,
                        o[2] + s[2] * k as f32,
                    ));
                }
            }
        }
        out
    }

    /// Rebuild the arm's triangles on the CPU the way `build_arm_mesh` does.
    fn arm_triangles() -> Vec<([Vec3; 3], Vec3)> {
        let (origin, size) = arm_box();
        let mut v = Vec::new();
        let mut n = Vec::new();
        let mut c = Vec::new();
        push_box(&mut v, &mut n, &mut c, origin, size, [255; 4]);
        (0..v.len() / 9)
            .map(|i| {
                let p = |k: usize| {
                    Vec3::new(v[i * 9 + k * 3], v[i * 9 + k * 3 + 1], v[i * 9 + k * 3 + 2])
                };
                let nrm = Vec3::new(n[i * 9], n[i * 9 + 1], n[i * 9 + 2]);
                ([p(0), p(1), p(2)], nrm)
            })
            .collect()
    }

    /// Project a local arm vertex to normalised device coordinates.
    fn to_ndc(p: Vec3, swing: f32, aspect: f32) -> (f32, f32) {
        let clip = projection(aspect) * arm_model(swing, 0.0, aspect) * p.extend(1.0);
        (clip.x / clip.w, clip.y / clip.w)
    }

    /// NDC bounds of the arm at rest: (min_x, max_x, min_y, max_y).
    fn arm_bounds(aspect: f32) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for c in arm_corners() {
            let (x, y) = to_ndc(c, 0.0, aspect);
            b = (b.0.min(x), b.1.max(x), b.2.min(y), b.3.max(y));
        }
        b
    }

    #[test]
    fn the_arm_is_a_single_plain_box() {
        // 6 faces x 2 triangles x 3 vertices. The modelled hand that came
        // before this read as a claw at first-person size; keep it one box.
        let mesh = arm_triangles();
        assert_eq!(mesh.len(), 12, "the arm must be exactly one box");
    }

    #[test]
    fn the_gbuffer_shader_still_applies_the_model_matrix_itself() {
        // `draw` binds uMVP as the projection alone because the shader does
        // `uMVP * (uModel * position)`. If that ever changes, the arm silently
        // moves and every placement number in this file becomes wrong.
        let src = include_str!("../assets/shaders/gbuffer.wgsl");
        assert!(
            src.contains("u.mvp * (u.model * vec4(in.position, 1.0))"),
            "gbuffer.wgsl no longer applies uModel itself; hand::draw depends on it",
        );
    }

    #[test]
    fn draw_binds_the_view_projection_without_the_model_matrix() {
        // Folding the model matrix into uMVP as well as uModel applies the
        // arm's transform twice: the arm ends up rotated about double and
        // nowhere near the corner it was placed in.
        let src = include_str!("hand.rs");
        let body = src.split("\npub fn draw(").nth(1).expect("draw not found");
        let body = body.split("\n#[cfg(test)]").next().unwrap_or_default();
        let mvp = body
            .lines()
            .find(|l| l.contains("get_uniform_location(\"uMVP\")"))
            .expect("draw must bind uMVP");
        assert!(
            mvp.contains("&proj"),
            "uMVP must be the projection alone, got: {mvp}",
        );
        for folded in ["proj * arm", "proj * item", "projection(aspect) * arm"] {
            assert!(
                !body.contains(folded),
                "draw folds the model matrix into uMVP ({folded}); the shader already \
                 multiplies by uModel",
            );
        }
    }

    #[test]
    fn swing_moves_the_hand_from_idle() {
        let hand = Vec3::new(0.0, 0.0, -ARM_FORWARD);
        let idle = arm_model(0.0, 0.0, 16.0 / 9.0).transform_point3(hand);
        let punch = arm_model(1.0, 0.0, 16.0 / 9.0).transform_point3(hand);
        assert!(idle.distance(punch) > 0.05);
    }

    #[test]
    fn every_arm_triangle_faces_outward() {
        for (tri, nrm) in arm_triangles() {
            let geometric = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
            assert!(
                geometric.dot(nrm) > 0.99,
                "triangle {tri:?} is wound against its normal {nrm}: back-face culling \
                 would drop it and the deferred pass would light it from inside",
            );
        }
    }

    #[test]
    fn the_arm_encodes_depth_exactly_like_the_world() {
        // The arm shares the G-buffer's depth buffer with the terrain, so a
        // point at a given view-space distance must produce the same depth
        // under both projections. Only the FOV may differ. Tightening the
        // arm's near plane once made nearby terrain occlude the arm.
        for aspect in ASPECTS {
            let hand = projection(aspect);
            for world_fov in [60.0f32, 80.0, 100.0] {
                let world =
                    Mat4::perspective_rh(world_fov.to_radians(), aspect, NEAR_PLANE, FAR_PLANE);
                for z in [-0.15f32, -0.4, -0.8, -2.0, -25.0, -300.0] {
                    let p = Vec3::new(0.0, 0.0, z).extend(1.0);
                    let (h, w) = (hand * p, world * p);
                    let (hd, wd) = (h.z / h.w, w.z / w.w);
                    assert!(
                        (hd - wd).abs() < 1e-5,
                        "at z={z} the arm writes depth {hd} but the world writes {wd}; \
                         terrain would punch through the arm",
                    );
                }
            }
        }
    }

    #[test]
    fn the_arm_ignores_the_world_fov_setting() {
        // The FOV slider in `hud.rs` offers these; the arm must not move or
        // resize when the player changes it, so `projection` takes no FOV at
        // all. Guard that by checking the arm's projection really is fixed.
        for aspect in ASPECTS {
            let hand = projection(aspect);
            for world_fov in [60.0f32, 80.0, 90.0, 100.0] {
                let world =
                    Mat4::perspective_rh(world_fov.to_radians(), aspect, NEAR_PLANE, FAR_PLANE);
                if (world_fov - HAND_FOV_Y).abs() > 0.5 {
                    assert!(
                        world != hand,
                        "world FOV {world_fov} must not equal the arm projection",
                    );
                }
            }
            assert_eq!(hand, projection(aspect));
        }
    }

    #[test]
    fn the_arm_never_crosses_the_near_plane() {
        // View-space z is negative in front of the camera, so a vertex is
        // clipped once z rises above -NEAR_PLANE.
        let limit = -NEAR_PLANE - NEAR_MARGIN;
        for step in 0..=10 {
            let swing = step as f32 / 10.0;
            for aspect in ASPECTS {
                let m = arm_model(swing, 1.0, aspect);
                for c in arm_corners() {
                    let z = m.transform_point3(c).z;
                    assert!(
                        z < limit,
                        "vertex {c} reaches z={z} at swing={swing}, past the {limit} \
                         clip limit; the arm would be sliced open",
                    );
                }
                let im = item_model(swing, 1.0, aspect);
                for i in [-ITEM_SCALE, ITEM_SCALE] {
                    for j in [-ITEM_SCALE, ITEM_SCALE] {
                        for k in [-ITEM_SCALE, ITEM_SCALE] {
                            let z = im.transform_point3(Vec3::new(i, j, k)).z;
                            assert!(z < limit, "held item corner reaches z={z}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_arm_never_rides_up_over_the_horizon() {
        // Bounding only part of the arm once let it climb to the middle of
        // the screen and block the view.
        for aspect in ASPECTS {
            for step in 0..=10 {
                let swing = step as f32 / 10.0;
                for c in arm_corners() {
                    let (_, y) = to_ndc(c, swing, aspect);
                    assert!(
                        y < -0.28,
                        "arm reaches y={y} at swing={swing}, aspect={aspect}: it would \
                         cover the horizon and the crosshair",
                    );
                }
            }
        }
    }

    #[test]
    fn the_arm_runs_off_the_bottom_edge_on_every_aspect() {
        for aspect in ASPECTS {
            let (_, _, min_y, _) = arm_bounds(aspect);
            assert!(
                min_y < -1.0,
                "at aspect {aspect} the arm stops inside the frame at y={min_y}, so it \
                 reads as floating rather than attached to the player",
            );
        }
    }

    #[test]
    fn the_arm_matches_the_minecraft_reference_footprint() {
        // Measured off a Minecraft first-person screenshot: the arm's top
        // edge sits at NDC y -0.42 and it spans x +0.36..+0.74 at 16:9.
        let (min_x, max_x, _, max_y) = arm_bounds(16.0 / 9.0);
        assert!(
            (max_y + 0.42).abs() < 0.06,
            "arm top edge at {max_y}, reference has -0.42",
        );
        assert!(
            (min_x - 0.36).abs() < 0.06 && (max_x - 0.74).abs() < 0.06,
            "arm spans x {min_x}..{max_x}, reference has +0.36..+0.74",
        );
    }

    #[test]
    fn the_arm_stays_in_the_bottom_right_on_every_aspect() {
        for aspect in ASPECTS {
            let (min_x, max_x, _, max_y) = arm_bounds(aspect);
            assert!(
                min_x > 0.15 && max_x < 0.95,
                "arm drifts out of the right-hand corner at aspect {aspect}: x {min_x}..{max_x}",
            );
            assert!(
                (-0.55..=-0.30).contains(&max_y),
                "arm top edge at {max_y} on aspect {aspect}, want roughly -0.42",
            );
        }
    }

    /// Every body vertex in body-local space, legs swung to both extremes.
    fn body_points() -> Vec<Vec3> {
        let mut out = Vec::new();
        for i in [-TORSO_HALF_W, TORSO_HALF_W] {
            for j in [HIP_HEIGHT, TORSO_TOP] {
                for k in [TORSO_CENTER_Z - TORSO_HALF_D, TORSO_CENTER_Z + TORSO_HALF_D] {
                    out.push(Vec3::new(i, j, k));
                }
            }
        }
        for step in 0..=8 {
            let swing = -0.45 + 0.9 * step as f32 / 8.0;
            for side in [-1.0f32, 1.0] {
                let m =
                    Mat4::from_translation(Vec3::new(side * LEG_HALF_W, HIP_HEIGHT, LEG_CENTER_Z))
                        * Mat4::from_rotation_x(swing);
                for i in [-LEG_HALF_W, LEG_HALF_W] {
                    for j in [-HIP_HEIGHT, 0.0] {
                        for k in [-LEG_HALF_D, LEG_HALF_D] {
                            out.push(m.transform_point3(Vec3::new(i, j, k)));
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn the_body_stays_clear_of_the_near_plane() {
        // The camera sits inside the head, so the body is the one piece of
        // geometry that is always within arm's reach of the eye.
        let eye = Vec3::new(0.0, EYE_HEIGHT, 0.0);
        for p in body_points() {
            let d = p.distance(eye);
            assert!(
                d > NEAR_PLANE + NEAR_MARGIN,
                "body point {p} is {d} from the eye, inside the near plane",
            );
        }
    }

    #[test]
    fn the_body_stays_out_of_view_when_looking_straight_ahead() {
        // At pitch 0 a point below the eye is visible once its drop is less
        // than its horizontal distance times tan(fov/2). Checked at 100, the
        // widest the FOV slider goes, and for every yaw: the worst case is
        // the player facing straight at the point.
        let tan = (100.0f32 / 2.0).to_radians().tan();
        for p in body_points() {
            let drop = EYE_HEIGHT - p.y;
            let radius = p.x.hypot(p.z);
            assert!(
                drop >= radius * tan,
                "body point {p} drops {drop} over a radius of {radius}, so it hangs in \
                 the view at FOV 100 while walking",
            );
        }
    }

    fn torso_triangles() -> Vec<([Vec3; 3], Vec3)> {
        let mut v = Vec::new();
        let mut n = Vec::new();
        let mut c = Vec::new();
        push_torso(&mut v, &mut n, &mut c, [255; 4]);
        (0..v.len() / 9)
            .map(|i| {
                let p = |k: usize| {
                    Vec3::new(v[i * 9 + k * 3], v[i * 9 + k * 3 + 1], v[i * 9 + k * 3 + 2])
                };
                let nrm = Vec3::new(n[i * 9], n[i * 9 + 1], n[i * 9 + 2]);
                ([p(0), p(1), p(2)], nrm)
            })
            .collect()
    }

    /// Möller–Trumbore. Hits in (0, 1) along p0→p1 count.
    fn segment_hits_tri(p0: Vec3, p1: Vec3, tri: [Vec3; 3]) -> bool {
        let dir = p1 - p0;
        let e1 = tri[1] - tri[0];
        let e2 = tri[2] - tri[0];
        let pvec = dir.cross(e2);
        let det = e1.dot(pvec);
        if det.abs() < 1e-8 {
            return false;
        }
        let inv = 1.0 / det;
        let tvec = p0 - tri[0];
        let u = tvec.dot(pvec) * inv;
        if !(0.0..=1.0).contains(&u) {
            return false;
        }
        let qvec = tvec.cross(e1);
        let v = dir.dot(qvec) * inv;
        if v < 0.0 || u + v > 1.0 {
            return false;
        }
        let t = e2.dot(qvec) * inv;
        (1e-4..=1.0 - 1e-4).contains(&t)
    }

    fn look_down_ndc(p: Vec3, pitch: f32, fov: f32, aspect: f32) -> (f32, f32) {
        let eye = Vec3::new(0.0, EYE_HEIGHT, 0.0);
        let look = Vec3::new(0.0, pitch.sin(), pitch.cos());
        let view = Mat4::look_at_rh(eye, eye + look, Vec3::Y);
        let proj = Mat4::perspective_rh(fov.to_radians(), aspect, NEAR_PLANE, FAR_PLANE);
        let clip = proj * view * p.extend(1.0);
        (clip.x / clip.w, clip.y / clip.w)
    }

    #[test]
    fn every_torso_triangle_faces_outward() {
        for (tri, nrm) in torso_triangles() {
            let geometric = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
            assert!(
                geometric.dot(nrm) > 0.99,
                "torso triangle {tri:?} is wound against its normal {nrm}",
            );
        }
    }

    #[test]
    fn the_torso_has_no_lid() {
        for (tri, nrm) in torso_triangles() {
            assert!(
                nrm.y < 0.5,
                "torso triangle {tri:?} faces up ({nrm}): looking down would show the \
                 top of the chest instead of the legs",
            );
        }
    }

    #[test]
    fn the_legs_sit_under_the_torso() {
        assert_eq!(
            LEG_CENTER_Z, TORSO_CENTER_Z,
            "legs in front of the torso is the disconnected-body look",
        );
    }

    #[test]
    fn looking_down_shows_the_feet_and_not_the_top_of_the_torso() {
        let eye = Vec3::new(0.0, EYE_HEIGHT, 0.0);
        let tris: Vec<_> = torso_triangles().into_iter().map(|(t, _)| t).collect();
        for side in [-1.0f32, 1.0] {
            let foot = Vec3::new(side * LEG_HALF_W, 0.0, LEG_CENTER_Z);
            for tri in &tris {
                assert!(
                    !segment_hits_tri(eye, foot, *tri),
                    "torso face {tri:?} occludes the foot at {foot}",
                );
            }
        }
    }

    #[test]
    fn looking_down_puts_the_feet_on_screen() {
        // Pitch clamp in main is ±1.56. At a steep look-down the feet must
        // land inside the frame, not off the bottom behind a chest lid.
        let pitch = -1.4;
        for fov in [70.0f32, 100.0] {
            for side in [-1.0f32, 1.0] {
                let foot = Vec3::new(side * LEG_HALF_W, 0.0, LEG_CENTER_Z);
                let (x, y) = look_down_ndc(foot, pitch, fov, 16.0 / 9.0);
                assert!(
                    x.abs() < 1.0 && y.abs() < 1.0,
                    "foot {foot} projects to NDC ({x}, {y}) at fov {fov}, off-screen",
                );
            }
        }
    }

    #[test]
    fn the_legs_only_swing_when_the_player_moves() {
        assert_eq!(leg_swing(0.0, 1.7), 0.0, "legs must be still when standing");
        let moving: f32 = (0..20)
            .map(|i| leg_swing(4.0, i as f32 * 0.1).abs())
            .fold(0.0, f32::max);
        assert!(moving > 0.3, "legs barely swing at walking speed: {moving}");
    }

    #[test]
    fn the_body_faces_the_way_the_camera_looks() {
        // `look_dir` in main is (cos(pitch) sin(yaw), _, cos(pitch) cos(yaw)),
        // and body-local forward is +Z, so the yaw applies unnegated.
        for yaw in [0.0f32, 1.0, 2.5, -2.0] {
            let fwd = Mat4::from_rotation_y(yaw).transform_vector3(Vec3::Z);
            let look = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            assert!(
                fwd.dot(look) > 0.999,
                "body faces {fwd} but the camera looks along {look}",
            );
        }
    }

    #[test]
    fn the_hand_end_aims_up_and_inward_toward_the_crosshair() {
        let m = arm_model(0.0, 0.0, 16.0 / 9.0);
        let base = m.transform_point3(Vec3::ZERO);
        let aim = m.transform_point3(Vec3::new(0.0, 0.0, -1.0)) - base;
        assert!(
            aim.z < -0.5,
            "hand end must point away from the camera, got {aim}"
        );
        assert!(
            aim.y > 0.05,
            "arm must tilt up toward the crosshair, got {aim}"
        );
        assert!(
            aim.x < -0.05,
            "right arm must angle inward to screen centre, got {aim}"
        );
    }
}
