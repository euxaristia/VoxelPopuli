use crate::block::BlockType;
use crate::item;
use crate::renderer::{Mesh, Shader, Texture2D};
use glam::{Vec2, Vec4};

const RENDER_DISTANCE_OPTIONS: [i32; 4] = [4, 8, 12, 16];

const VERTICES_PER_QUAD: usize = 6;

struct SolidQuadBatch {
    vertices: Vec<f32>,
    colors: Vec<u8>,
}

impl SolidQuadBatch {
    fn with_capacity(quads: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(quads * VERTICES_PER_QUAD * 3),
            colors: Vec::with_capacity(quads * VERTICES_PER_QUAD * 4),
        }
    }

    fn push(&mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
        self.vertices.extend_from_slice(&[
            x,
            y,
            0.0,
            x + w,
            y,
            0.0,
            x + w,
            y + h,
            0.0,
            x,
            y,
            0.0,
            x + w,
            y + h,
            0.0,
            x,
            y + h,
            0.0,
        ]);
        for _ in 0..VERTICES_PER_QUAD {
            self.colors.extend_from_slice(&color);
        }
    }

    fn vertex_count(&self) -> usize {
        self.vertices.len() / 3
    }

    fn draw(self, shader: &Shader, screen_width: f32, screen_height: f32) {
        if self.vertex_count() == 0 {
            return;
        }
        let mesh = Mesh::new(&self.vertices, None, None, Some(&self.colors));
        shader.bind();
        shader.set_vec2(
            shader.get_uniform_location("uScreenSize"),
            Vec2::new(screen_width, screen_height),
        );
        mesh.draw();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseSubMenu {
    Main,
    Settings,
    WorldInfo,
    Profile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PauseClick {
    Resume,
    OpenSettings,
    OpenWorldInfo,
    OpenProfile,
    SaveQuit,
    Back,
    SetRenderDistance(i32),
    SetFov(f32),
    ToggleFancy,
    ExportJava,
    SelectSkin(u8),
}

pub const SKIN_NAMES: [&str; 4] = ["Classic Steve", "Alex", "PS1 Voxel Classic", "Custom Hero"];

pub fn skin_preview_color(selected_skin: u8) -> [u8; 4] {
    match selected_skin {
        1 => [220, 180, 140, 255],
        2 => [160, 90, 70, 255],
        3 => [80, 140, 200, 255],
        _ => [200, 150, 100, 255],
    }
}

fn in_rect(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

fn centered_panel(sw: f32, sh: f32, panel_w: f32, panel_h: f32) -> (f32, f32, f32, f32) {
    ((sw - panel_w) / 2.0, (sh - panel_h) / 2.0, panel_w, panel_h)
}

fn settings_panel(sw: f32, sh: f32) -> (f32, f32, f32, f32) {
    centered_panel(sw, sh, 700.0, 500.0)
}

fn world_info_panel(sw: f32, sh: f32) -> (f32, f32, f32, f32) {
    centered_panel(sw, sh, 680.0, 460.0)
}

fn profile_panel(sw: f32, sh: f32) -> (f32, f32, f32, f32) {
    centered_panel(sw, sh, 640.0, 440.0)
}

pub fn pause_click(menu: PauseSubMenu, sw: f32, sh: f32, mx: f32, my: f32) -> Option<PauseClick> {
    match menu {
        PauseSubMenu::Main => {
            let btn_w = 340.0;
            let btn_h = 44.0;
            let btn_x = 100.0;
            let btn_y_start = sh / 2.0 - 130.0;
            for i in 0..5 {
                let y = btn_y_start + i as f32 * (btn_h + 10.0);
                if in_rect(mx, my, btn_x, y, btn_w, btn_h) {
                    return Some(match i {
                        0 => PauseClick::Resume,
                        1 => PauseClick::OpenSettings,
                        2 => PauseClick::OpenWorldInfo,
                        3 => PauseClick::OpenProfile,
                        _ => PauseClick::SaveQuit,
                    });
                }
            }
            let sbtn_s = 44.0;
            for i in 0..3 {
                let sx = btn_x + i as f32 * (sbtn_s + 10.0);
                let sy = sh - 70.0;
                if in_rect(mx, my, sx, sy, sbtn_s, sbtn_s) {
                    return Some(match i {
                        1 => PauseClick::OpenSettings,
                        2 => PauseClick::OpenProfile,
                        _ => return None,
                    });
                }
            }
            None
        }
        PauseSubMenu::Settings => {
            let (panel_x, panel_y, _, _) = settings_panel(sw, sh);
            for (i, d) in RENDER_DISTANCE_OPTIONS.iter().enumerate() {
                let bx = panel_x + 340.0 + i as f32 * 75.0;
                if in_rect(mx, my, bx, panel_y + 82.0, 65.0, 32.0) {
                    return Some(PauseClick::SetRenderDistance(*d));
                }
            }
            let fovs = [60.0, 70.0, 80.0, 90.0, 100.0];
            for (i, f) in fovs.iter().enumerate() {
                let bx = panel_x + 340.0 + i as f32 * 65.0;
                if in_rect(mx, my, bx, panel_y + 152.0, 58.0, 32.0) {
                    return Some(PauseClick::SetFov(*f));
                }
            }
            if in_rect(mx, my, panel_x + 340.0, panel_y + 222.0, 140.0, 32.0) {
                return Some(PauseClick::ToggleFancy);
            }
            if in_rect(mx, my, panel_x + 230.0, panel_y + 420.0, 240.0, 44.0) {
                return Some(PauseClick::Back);
            }
            None
        }
        PauseSubMenu::WorldInfo => {
            let (panel_x, panel_y, _, _) = world_info_panel(sw, sh);
            if in_rect(mx, my, panel_x + 40.0, panel_y + 220.0, 320.0, 44.0) {
                return Some(PauseClick::ExportJava);
            }
            if in_rect(mx, my, panel_x + 220.0, panel_y + 380.0, 240.0, 44.0) {
                return Some(PauseClick::Back);
            }
            None
        }
        PauseSubMenu::Profile => {
            let (panel_x, panel_y, _, _) = profile_panel(sw, sh);
            for i in 0..SKIN_NAMES.len() {
                let sy = panel_y + 130.0 + i as f32 * 50.0;
                if in_rect(mx, my, panel_x + 40.0, sy, 340.0, 40.0) {
                    return Some(PauseClick::SelectSkin(i as u8));
                }
            }
            if in_rect(mx, my, panel_x + 200.0, panel_y + 360.0, 240.0, 44.0) {
                return Some(PauseClick::Back);
            }
            None
        }
    }
}

pub fn draw_rect(
    shader: &Shader,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [u8; 4],
    screen_width: f32,
    screen_height: f32,
) {
    let verts = [
        x,
        y,
        0.0,
        x + w,
        y,
        0.0,
        x + w,
        y + h,
        0.0,
        x,
        y,
        0.0,
        x + w,
        y + h,
        0.0,
        x,
        y + h,
        0.0,
    ];
    // ui.wgsl reads the tint from the per-vertex color attribute, not a uniform.
    let mut c = Vec::new();
    for _ in 0..6 {
        c.extend_from_slice(&color);
    }
    let mesh = Mesh::new(&verts, None, None, Some(&c));
    shader.bind();
    shader.set_vec2(
        shader.get_uniform_location("uScreenSize"),
        Vec2::new(screen_width, screen_height),
    );
    mesh.draw();
}

pub fn draw_texture_quad(texture: &Texture2D) {
    let v = [
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0,
        0.0,
    ];
    let t = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let mesh = Mesh::new(&v, Some(&t), None, None);
    texture.bind(0);
    mesh.draw();
}

#[allow(clippy::too_many_arguments)]
pub fn draw_texture_ui_uv(
    texture: &Texture2D,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u: f32,
    v: f32,
    uw: f32,
    vh: f32,
    shader: &Shader,
    screen_width: f32,
    screen_height: f32,
    tint: [u8; 4],
) {
    // The atlases are pixel art sampled with Nearest. Landing a quad on a
    // fractional pixel makes the sampler pick a different phase per glyph, so
    // strokes come out uneven. Snap the rect to whole pixels.
    let x = x.round();
    let y = y.round();
    let w = w.round().max(1.0);
    let h = h.round().max(1.0);
    let verts = [
        x,
        y,
        0.0,
        x + w,
        y,
        0.0,
        x + w,
        y + h,
        0.0,
        x,
        y,
        0.0,
        x + w,
        y + h,
        0.0,
        x,
        y + h,
        0.0,
    ];
    let texs = [
        u,
        v,
        u + uw,
        v,
        u + uw,
        v + vh,
        u,
        v,
        u + uw,
        v + vh,
        u,
        v + vh,
    ];
    let mesh = Mesh::new(&verts, Some(&texs), None, None);
    shader.bind();
    shader.set_vec2(
        shader.get_uniform_location("uScreenSize"),
        Vec2::new(screen_width, screen_height),
    );
    shader.set_vec4(
        shader.get_uniform_location("uColor"),
        Vec4::new(
            tint[0] as f32 / 255.0,
            tint[1] as f32 / 255.0,
            tint[2] as f32 / 255.0,
            tint[3] as f32 / 255.0,
        ),
    );
    texture.bind(0);
    mesh.draw();
}

pub const TEXT_WHITE: [u8; 4] = [255, 255, 255, 255];
pub const TEXT_SHADOW: [u8; 4] = [45, 45, 45, 255];

/// Horizontal advance per character, in pixels, for a run drawn at `size`.
/// The font cells are 16x16 with the glyph ink sitting in the left 12.
pub fn text_advance(size: f32) -> f32 {
    (size * 0.55).round().max(1.0)
}

/// Width of `text` when drawn at `size`, in pixels.
pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().filter(|c| (' '..'\u{80}').contains(c)).count() as f32 * text_advance(size)
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    texture: &Texture2D,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    shader: &Shader,
    sw: f32,
    sh: f32,
) {
    draw_text_tinted(texture, text, x, y, size, shader, sw, sh, TEXT_WHITE);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text_tinted(
    texture: &Texture2D,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    shader: &Shader,
    sw: f32,
    sh: f32,
    tint: [u8; 4],
) {
    // Whole-pixel advance, so every glyph in the run lands on the same
    // sampling phase instead of drifting by a fraction of a pixel each step.
    let advance = text_advance(size);
    let mut cur_x = x.round();
    let y = y.round();
    for c in text.chars() {
        let i = c as u32;
        if !(32..128).contains(&i) {
            continue;
        }
        let idx = i - 32;
        let col = (idx % 16) as f32;
        let row = (idx / 16) as f32;
        let uw = 1.0 / 16.0;
        let vh = 1.0 / 8.0;
        draw_texture_ui_uv(
            texture,
            cur_x,
            y,
            size,
            size,
            col * uw,
            row * vh,
            uw,
            vh,
            shader,
            sw,
            sh,
            tint,
        );
        cur_x += advance;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_debug_overlay(
    ui_shader: &Shader,
    tex_shader: &Shader,
    font_tex: &Texture2D,
    lines: &[String],
    profiler: &crate::profiler::FrameProfiler,
    sw: f32,
    sh: f32,
) {
    let margin = 10.0;
    let font_size = 13.0;
    let line_height = 16.0;
    let padding = 8.0;

    let mut max_len: usize = 0;
    for l in lines {
        max_len = max_len.max(l.len());
    }

    let box_w = (max_len as f32 * font_size * 0.55) + padding * 2.0;
    let box_h = (lines.len() as f32 * line_height) + padding * 2.0;

    draw_rect(
        ui_shader,
        margin,
        margin,
        box_w,
        box_h,
        [0, 0, 0, 180],
        sw,
        sh,
    );

    let mut cur_y = margin + padding;
    for l in lines {
        draw_text(
            font_tex,
            l,
            margin + padding,
            cur_y,
            font_size,
            tex_shader,
            sw,
            sh,
        );
        cur_y += line_height;
    }

    let graph_x = margin;
    let graph_y = margin + box_h + 12.0;
    let graph_w = (360.0_f32).min(sw - 32.0);
    let graph_h = 70.0;

    draw_rect(
        ui_shader,
        graph_x,
        graph_y,
        graph_w,
        graph_h,
        [0, 0, 0, 160],
        sw,
        sh,
    );

    let bar_w = graph_w / 120.0;
    for i in 0..profiler.history_count {
        let idx = (profiler.history_idx + 120 - profiler.history_count + i) % 120;
        let ms = profiler.frame_history[idx];
        let height = (ms / 33.3 * graph_h).clamp(2.0, graph_h);

        let color = if ms <= 8.33 {
            [68, 255, 68, 220]
        } else if ms <= 16.67 {
            [255, 235, 59, 220]
        } else {
            [244, 67, 54, 230]
        };

        draw_rect(
            ui_shader,
            graph_x + i as f32 * bar_w,
            graph_y + (graph_h - height),
            bar_w.max(1.0),
            height,
            color,
            sw,
            sh,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_item_icon(
    atlas: &Texture2D,
    tex_shader: &Shader,
    block: BlockType,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    sw: f32,
    sh: f32,
) {
    let (tx, ty) = item::atlas_uv(block);
    let ts = 1.0 / 16.0;
    let u = tx as f32 * ts;
    let v = ty as f32 * ts;
    draw_texture_ui_uv(
        atlas, x, y, w, h, u, v, ts, ts, tex_shader, sw, sh, TEXT_WHITE,
    );
}

pub fn draw_heart(
    shader: &Shader,
    x: f32,
    y: f32,
    size: f32,
    screen_width: f32,
    screen_height: f32,
    empty: bool,
) {
    let color = if empty {
        [40, 40, 40, 200]
    } else {
        [220, 20, 20, 255]
    };
    let border = [20, 20, 20, 255];
    let mut batch = SolidQuadBatch::with_capacity(64);

    for px in 0..9 {
        for py in 0..9 {
            let fill = matches!(
                (px, py),
                (1..=3, 0)
                    | (5..=7, 0)
                    | (0..=8, 1..=3)
                    | (1..=7, 4)
                    | (2..=6, 5)
                    | (3..=5, 6)
                    | (4..=4, 7)
            );
            if fill {
                let bx = x + (px as f32 / 9.0) * size;
                let by = y + (py as f32 / 9.0) * size;
                let bw = size / 9.0;

                let is_border = matches!(
                    (px, py),
                    (1..=3, 0)
                        | (5..=7, 0)
                        | (4, 1)
                        | (0, 1..=3)
                        | (8, 1..=3)
                        | (1, 4)
                        | (7, 4)
                        | (2, 5)
                        | (6, 5)
                        | (3, 6)
                        | (5, 6)
                        | (4, 7)
                );

                let is_highlight = match (px, py) {
                    (1, 1) | (2, 1) | (1, 2) => !empty,
                    _ => false,
                };

                let p_color = if is_border {
                    border
                } else if is_highlight {
                    [255, 255, 255, 255]
                } else {
                    color
                };
                batch.push(bx, by, bw, bw, p_color);
            }
        }
    }
    batch.draw(shader, screen_width, screen_height);
}

pub fn draw_bubble(
    shader: &Shader,
    x: f32,
    y: f32,
    size: f32,
    screen_width: f32,
    screen_height: f32,
    empty: bool,
) {
    let color = if empty {
        [20, 20, 40, 150]
    } else {
        [60, 150, 255, 200]
    };
    let border = [20, 20, 40, 255];
    let mut batch = SolidQuadBatch::with_capacity(64);

    for px in 0..8 {
        for py in 0..8 {
            let dist_sq = (px as f32 - 3.5).powi(2) + (py as f32 - 3.5).powi(2);
            if dist_sq <= 16.0 {
                let bx = x + (px as f32 / 8.0) * size;
                let by = y + (py as f32 / 8.0) * size;
                let bw = size / 8.0;

                let is_border = dist_sq > 9.0;
                let p_color = if is_border {
                    border
                } else if px == 2 && py == 2 && !empty {
                    [255, 255, 255, 255]
                } else {
                    color
                };

                batch.push(bx, by, bw, bw, p_color);
            }
        }
    }
    batch.draw(shader, screen_width, screen_height);
}

pub fn draw_drumstick(
    shader: &Shader,
    x: f32,
    y: f32,
    size: f32,
    screen_width: f32,
    screen_height: f32,
    val: i32,
) {
    let fill_color = [190, 110, 40, 255];
    let bone_color = [230, 220, 200, 255];
    let border_color = [60, 30, 10, 255];
    let empty_color = [40, 25, 15, 180];
    let mut batch = SolidQuadBatch::with_capacity(64);

    for px in 0..8 {
        for py in 0..8 {
            let is_meat = matches!((px, py), (2..=6, 1..=5) | (3..=5, 6));
            let is_bone = matches!((px, py), (0..=1, 6..=7) | (1, 5));
            if is_meat || is_bone {
                let bx = x + (px as f32 / 8.0) * size;
                let by = y + (py as f32 / 8.0) * size;
                let bw = size / 8.0;

                let is_right_half = px >= 4;
                let is_filled = match val {
                    2 => true,
                    1 => !is_right_half,
                    _ => false,
                };

                let color = if is_filled {
                    if is_bone { bone_color } else { fill_color }
                } else {
                    empty_color
                };
                let p_color = if px == 0 || px == 7 || py == 0 || py == 7 {
                    border_color
                } else {
                    color
                };
                batch.push(bx, by, bw, bw, p_color);
            }
        }
    }
    batch.draw(shader, screen_width, screen_height);
}

pub fn draw_armor(
    shader: &Shader,
    x: f32,
    y: f32,
    size: f32,
    screen_width: f32,
    screen_height: f32,
    val: i32,
) {
    let metal_color = [190, 200, 210, 255];
    let border_color = [30, 35, 40, 255];
    let empty_color = [30, 30, 30, 160];
    let mut batch = SolidQuadBatch::with_capacity(64);

    for px in 0..8 {
        for py in 0..8 {
            let is_shape = matches!((px, py), (0..=7, 1..=4) | (1..=6, 5..=6) | (2..=5, 7))
                && !matches!((px, py), (3..=4, 1..=2));

            if is_shape {
                let bx = x + (px as f32 / 8.0) * size;
                let by = y + (py as f32 / 8.0) * size;
                let bw = size / 8.0;

                let is_right_half = px >= 4;
                let is_filled = match val {
                    2 => true,
                    1 => !is_right_half,
                    _ => false,
                };

                let color = if is_filled { metal_color } else { empty_color };
                let p_color = if px == 0 || px == 7 || py == 1 || py == 7 {
                    border_color
                } else {
                    color
                };
                batch.push(bx, by, bw, bw, p_color);
            }
        }
    }
    batch.draw(shader, screen_width, screen_height);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_quad_batch_encodes_six_vertices_and_colors_per_quad() {
        let mut batch = SolidQuadBatch::with_capacity(2);
        batch.push(2.0, 3.0, 5.0, 7.0, [10, 20, 30, 40]);
        batch.push(-1.0, 4.0, 2.0, 3.0, [50, 60, 70, 80]);

        assert_eq!(batch.vertex_count(), 12);
        assert_eq!(batch.vertices.len(), 12 * 3);
        assert_eq!(batch.colors.len(), 12 * 4);
        assert_eq!(
            &batch.vertices[..18],
            &[
                2.0, 3.0, 0.0, 7.0, 3.0, 0.0, 7.0, 10.0, 0.0, 2.0, 3.0, 0.0, 7.0, 10.0, 0.0, 2.0,
                10.0, 0.0,
            ]
        );
        assert_eq!(&batch.colors[..24], &[10, 20, 30, 40].repeat(6));
        assert_eq!(&batch.colors[24..], &[50, 60, 70, 80].repeat(6));
    }

    #[test]
    fn pause_click_matches_drawn_settings_and_profile_hitboxes() {
        let sw = 1280.0;
        let sh = 720.0;
        let (sx, sy, _, _) = settings_panel(sw, sh);
        assert_eq!(
            pause_click(
                PauseSubMenu::Settings,
                sw,
                sh,
                sx + 340.0 + 2.0 * 75.0 + 4.0,
                sy + 90.0
            ),
            Some(PauseClick::SetRenderDistance(12))
        );
        assert_eq!(
            pause_click(
                PauseSubMenu::Settings,
                sw,
                sh,
                sx + 340.0 + 65.0 + 4.0,
                sy + 160.0
            ),
            Some(PauseClick::SetFov(70.0))
        );
        assert_eq!(
            pause_click(PauseSubMenu::Settings, sw, sh, sx + 250.0, sy + 430.0),
            Some(PauseClick::Back)
        );

        let (px, py, _, _) = profile_panel(sw, sh);
        assert_eq!(
            pause_click(PauseSubMenu::Profile, sw, sh, px + 50.0, py + 135.0),
            Some(PauseClick::SelectSkin(0))
        );
        assert_eq!(
            pause_click(PauseSubMenu::Profile, sw, sh, px + 50.0, py + 185.0),
            Some(PauseClick::SelectSkin(1))
        );
        assert_eq!(
            pause_click(
                PauseSubMenu::WorldInfo,
                sw,
                sh,
                world_info_panel(sw, sh).0 + 50.0,
                world_info_panel(sw, sh).1 + 230.0
            ),
            Some(PauseClick::ExportJava)
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_bedrock_button(
    ui_shader: &Shader,
    tex_shader: &Shader,
    font_tex: &Texture2D,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: &str,
    sw: f32,
    sh: f32,
) {
    draw_rect(
        ui_shader,
        x - 2.0,
        y - 2.0,
        w + 4.0,
        h + 4.0,
        [20, 20, 20, 255],
        sw,
        sh,
    );
    draw_rect(ui_shader, x, y, w, h, [198, 198, 198, 255], sw, sh);
    draw_rect(
        ui_shader,
        x + 2.0,
        y + 2.0,
        w - 4.0,
        2.0,
        [255, 255, 255, 255],
        sw,
        sh,
    );
    draw_rect(
        ui_shader,
        x + 2.0,
        y + 2.0,
        2.0,
        h - 4.0,
        [255, 255, 255, 255],
        sw,
        sh,
    );
    draw_rect(
        ui_shader,
        x + 2.0,
        y + h - 4.0,
        w - 4.0,
        2.0,
        [130, 130, 130, 255],
        sw,
        sh,
    );
    draw_rect(
        ui_shader,
        x + w - 4.0,
        y + 2.0,
        2.0,
        h - 4.0,
        [130, 130, 130, 255],
        sw,
        sh,
    );

    let font_size = 18.0;
    let txt_w = text_width(text, font_size);
    let tx = x + (w - txt_w) / 2.0;
    let ty = y + (h - font_size) / 2.0;
    // Offset copy underneath is a drop shadow, so it has to be dark. Drawing it
    // in the same white as the label just smears the glyphs.
    draw_text_tinted(
        font_tex,
        text,
        tx + 2.0,
        ty + 2.0,
        font_size,
        tex_shader,
        sw,
        sh,
        TEXT_SHADOW,
    );
    draw_text(font_tex, text, tx, ty, font_size, tex_shader, sw, sh);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_pause_menu(
    ui_shader: &Shader,
    tex_shader: &Shader,
    font_tex: &Texture2D,
    sw: f32,
    sh: f32,
    sub_menu: PauseSubMenu,
    world_seed: u64,
    mobs_count: usize,
    export_status: &str,
    selected_skin: u8,
    render_dist: i32,
    fov: f32,
    fancy_gfx: bool,
) {
    draw_rect(ui_shader, 0.0, 0.0, sw, sh, [0, 0, 0, 180], sw, sh);

    match sub_menu {
        PauseSubMenu::Main => {
            let btn_w = 340.0;
            let btn_h = 44.0;
            let btn_x = 100.0;
            let btn_y_start = sh / 2.0 - 130.0;

            let labels = [
                "Resume Game",
                "Settings",
                "World Info & Export",
                "Profile & Skins",
                "Save & Quit",
            ];
            for (i, label) in labels.iter().enumerate() {
                let y = btn_y_start + i as f32 * (btn_h + 10.0);
                draw_bedrock_button(
                    ui_shader, tex_shader, font_tex, btn_x, y, btn_w, btn_h, label, sw, sh,
                );
            }

            let sbtn_s = 44.0;
            for i in 0..3 {
                let sx = btn_x + i as f32 * (sbtn_s + 10.0);
                let sy = sh - 70.0;
                draw_rect(
                    ui_shader,
                    sx - 2.0,
                    sy - 2.0,
                    sbtn_s + 4.0,
                    sbtn_s + 4.0,
                    [20, 20, 20, 255],
                    sw,
                    sh,
                );
                draw_rect(
                    ui_shader,
                    sx,
                    sy,
                    sbtn_s,
                    sbtn_s,
                    [198, 198, 198, 255],
                    sw,
                    sh,
                );

                if i == 0 {
                    draw_rect(
                        ui_shader,
                        sx + 10.0,
                        sy + 15.0,
                        24.0,
                        14.0,
                        [60, 60, 60, 255],
                        sw,
                        sh,
                    );
                } else if i == 1 {
                    draw_rect(
                        ui_shader,
                        sx + 10.0,
                        sy + 18.0,
                        24.0,
                        16.0,
                        [60, 60, 60, 255],
                        sw,
                        sh,
                    );
                    draw_rect(
                        ui_shader,
                        sx + 18.0,
                        sy + 12.0,
                        8.0,
                        6.0,
                        [60, 60, 60, 255],
                        sw,
                        sh,
                    );
                } else if i == 2 {
                    draw_rect(
                        ui_shader,
                        sx + 17.0,
                        sy + 10.0,
                        12.0,
                        12.0,
                        [60, 60, 60, 255],
                        sw,
                        sh,
                    );
                    draw_rect(
                        ui_shader,
                        sx + 12.0,
                        sy + 24.0,
                        22.0,
                        14.0,
                        [60, 60, 60, 255],
                        sw,
                        sh,
                    );
                }
            }

            let panel_w = 380.0;
            let panel_h = sh - 100.0;
            let panel_x = sw - panel_w - 40.0;
            let panel_y = 50.0;

            draw_rect(
                ui_shader,
                panel_x - 2.0,
                panel_y - 2.0,
                panel_w + 4.0,
                panel_h + 4.0,
                [20, 20, 20, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x,
                panel_y,
                panel_w,
                panel_h,
                [49, 49, 49, 230],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "World Session",
                panel_x + 20.0,
                panel_y + 15.0,
                20.0,
                tex_shader,
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x + 10.0,
                panel_y + 45.0,
                panel_w - 20.0,
                2.0,
                [80, 80, 80, 255],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "Bedrock / Java Multiplayer: Active",
                panel_x + 20.0,
                panel_y + 65.0,
                15.0,
                tex_shader,
                sw,
                sh,
            );
            draw_text(
                font_tex,
                "Players in My World",
                panel_x + 20.0,
                panel_y + 110.0,
                16.0,
                tex_shader,
                sw,
                sh,
            );

            let py = panel_y + 140.0;
            draw_rect(
                ui_shader,
                panel_x + 10.0,
                py,
                panel_w - 20.0,
                50.0,
                [60, 60, 60, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x + 20.0,
                py + 10.0,
                30.0,
                30.0,
                skin_preview_color(selected_skin),
                sw,
                sh,
            );
            draw_text(
                font_tex,
                "Player (Host)",
                panel_x + 60.0,
                py + 17.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
        }

        PauseSubMenu::Settings => {
            let (panel_x, panel_y, panel_w, panel_h) = settings_panel(sw, sh);

            draw_rect(
                ui_shader,
                panel_x - 2.0,
                panel_y - 2.0,
                panel_w + 4.0,
                panel_h + 4.0,
                [20, 20, 20, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x,
                panel_y,
                panel_w,
                panel_h,
                [40, 42, 46, 245],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "SETTINGS - GRAPHICS & VIDEO",
                panel_x + 30.0,
                panel_y + 25.0,
                22.0,
                tex_shader,
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x + 20.0,
                panel_y + 60.0,
                panel_w - 40.0,
                2.0,
                [80, 80, 80, 255],
                sw,
                sh,
            );

            let rdist_label = format!("Render Distance: {} Chunks", render_dist);
            draw_text(
                font_tex,
                &rdist_label,
                panel_x + 40.0,
                panel_y + 90.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
            for (i, d) in RENDER_DISTANCE_OPTIONS.iter().enumerate() {
                let bx = panel_x + 340.0 + i as f32 * 75.0;
                let active = *d == render_dist;
                let col = if active {
                    [40, 160, 80, 255]
                } else {
                    [100, 100, 100, 255]
                };
                draw_rect(ui_shader, bx, panel_y + 82.0, 65.0, 32.0, col, sw, sh);
                draw_text(
                    font_tex,
                    &format!("{}c", d),
                    bx + 20.0,
                    panel_y + 90.0,
                    16.0,
                    tex_shader,
                    sw,
                    sh,
                );
            }

            let fov_label = format!("Field of View (FOV): {:.0} deg", fov);
            draw_text(
                font_tex,
                &fov_label,
                panel_x + 40.0,
                panel_y + 160.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
            let fovs = [60.0, 70.0, 80.0, 90.0, 100.0];
            for (i, f) in fovs.iter().enumerate() {
                let bx = panel_x + 340.0 + i as f32 * 65.0;
                let active = (*f - fov).abs() < 1.0;
                let col = if active {
                    [40, 160, 80, 255]
                } else {
                    [100, 100, 100, 255]
                };
                draw_rect(ui_shader, bx, panel_y + 152.0, 58.0, 32.0, col, sw, sh);
                draw_text(
                    font_tex,
                    &format!("{:.0}", f),
                    bx + 12.0,
                    panel_y + 160.0,
                    16.0,
                    tex_shader,
                    sw,
                    sh,
                );
            }

            let gfx_label = format!(
                "Graphics Quality: {}",
                if fancy_gfx { "FANCY" } else { "FAST" }
            );
            draw_text(
                font_tex,
                &gfx_label,
                panel_x + 40.0,
                panel_y + 230.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
            let gfx_col = if fancy_gfx {
                [40, 160, 80, 255]
            } else {
                [100, 100, 100, 255]
            };
            draw_rect(
                ui_shader,
                panel_x + 340.0,
                panel_y + 222.0,
                140.0,
                32.0,
                gfx_col,
                sw,
                sh,
            );
            draw_text(
                font_tex,
                if fancy_gfx {
                    "Fancy (AO/Clouds)"
                } else {
                    "Fast"
                },
                panel_x + 348.0,
                panel_y + 230.0,
                14.0,
                tex_shader,
                sw,
                sh,
            );

            draw_bedrock_button(
                ui_shader,
                tex_shader,
                font_tex,
                panel_x + 230.0,
                panel_y + 420.0,
                240.0,
                44.0,
                "Back to Pause Menu",
                sw,
                sh,
            );
        }

        PauseSubMenu::WorldInfo => {
            let (panel_x, panel_y, panel_w, panel_h) = world_info_panel(sw, sh);

            draw_rect(
                ui_shader,
                panel_x - 2.0,
                panel_y - 2.0,
                panel_w + 4.0,
                panel_h + 4.0,
                [20, 20, 20, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x,
                panel_y,
                panel_w,
                panel_h,
                [40, 42, 46, 245],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "WORLD INFO & ANVIL EXPORT",
                panel_x + 30.0,
                panel_y + 25.0,
                22.0,
                tex_shader,
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x + 20.0,
                panel_y + 60.0,
                panel_w - 40.0,
                2.0,
                [80, 80, 80, 255],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                &format!("World Seed: {}", world_seed),
                panel_x + 40.0,
                panel_y + 90.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
            draw_text(
                font_tex,
                &format!("Active Mobs in Memory: {}", mobs_count),
                panel_x + 40.0,
                panel_y + 130.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );
            draw_text(
                font_tex,
                "Format: Minecraft Java 1.17 MCA (Anvil 16x16x256)",
                panel_x + 40.0,
                panel_y + 170.0,
                16.0,
                tex_shader,
                sw,
                sh,
            );

            draw_bedrock_button(
                ui_shader,
                tex_shader,
                font_tex,
                panel_x + 40.0,
                panel_y + 220.0,
                320.0,
                44.0,
                "Export Java 1.17 World",
                sw,
                sh,
            );

            if !export_status.is_empty() {
                draw_text(
                    font_tex,
                    export_status,
                    panel_x + 40.0,
                    panel_y + 280.0,
                    16.0,
                    tex_shader,
                    sw,
                    sh,
                );
            }

            draw_bedrock_button(
                ui_shader,
                tex_shader,
                font_tex,
                panel_x + 220.0,
                panel_y + 380.0,
                240.0,
                44.0,
                "Back to Pause Menu",
                sw,
                sh,
            );
        }

        PauseSubMenu::Profile => {
            let (panel_x, panel_y, panel_w, panel_h) = profile_panel(sw, sh);

            draw_rect(
                ui_shader,
                panel_x - 2.0,
                panel_y - 2.0,
                panel_w + 4.0,
                panel_h + 4.0,
                [20, 20, 20, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x,
                panel_y,
                panel_w,
                panel_h,
                [40, 42, 46, 245],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "PROFILE & PLAYER SKINS",
                panel_x + 30.0,
                panel_y + 25.0,
                22.0,
                tex_shader,
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                panel_x + 20.0,
                panel_y + 60.0,
                panel_w - 40.0,
                2.0,
                [80, 80, 80, 255],
                sw,
                sh,
            );

            draw_text(
                font_tex,
                "Select Active Player Skin:",
                panel_x + 40.0,
                panel_y + 90.0,
                18.0,
                tex_shader,
                sw,
                sh,
            );

            for (i, name) in SKIN_NAMES.iter().enumerate() {
                let sy = panel_y + 130.0 + i as f32 * 50.0;
                let is_sel = selected_skin == i as u8;
                let bg_col = if is_sel {
                    [40, 160, 80, 255]
                } else {
                    [70, 75, 82, 255]
                };
                draw_rect(ui_shader, panel_x + 40.0, sy, 340.0, 40.0, bg_col, sw, sh);
                draw_text(
                    font_tex,
                    name,
                    panel_x + 60.0,
                    sy + 10.0,
                    18.0,
                    tex_shader,
                    sw,
                    sh,
                );
            }

            draw_bedrock_button(
                ui_shader,
                tex_shader,
                font_tex,
                panel_x + 200.0,
                panel_y + 360.0,
                240.0,
                44.0,
                "Back to Pause Menu",
                sw,
                sh,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_stack_item(
    atlas: &Texture2D,
    tex_shader: &Shader,
    ui_shader: &Shader,
    font_texture: &Texture2D,
    s: &crate::inventory::ItemStack,
    rx: f32,
    ry: f32,
    slot_size: f32,
    sw: f32,
    sh: f32,
) {
    let icon_size = slot_size - 8.0;
    draw_item_icon(
        atlas,
        tex_shader,
        s.block,
        rx + 4.0,
        ry + 4.0,
        icon_size,
        icon_size,
        sw,
        sh,
    );

    // Numeric stack count text (e.g. 64, 16)
    if s.count > 1 {
        let count_str = format!("{}", s.count);
        let font_sz = (slot_size * 0.38).clamp(10.0, 14.0);
        let tw = text_width(&count_str, font_sz);
        let tx = rx + slot_size - tw - 2.0;
        let ty = ry + slot_size - font_sz - 1.0;
        draw_text_tinted(
            font_texture,
            &count_str,
            tx + 1.0,
            ty + 1.0,
            font_sz,
            tex_shader,
            sw,
            sh,
            TEXT_SHADOW,
        );
        draw_text_tinted(
            font_texture,
            &count_str,
            tx,
            ty,
            font_sz,
            tex_shader,
            sw,
            sh,
            TEXT_WHITE,
        );
    }

    // Durability bar for tools
    if let (Some(dur), Some(tool_props)) = (s.durability, item::tool_properties(s.block)) {
        if tool_props.durability > 0 {
            let max_dur = tool_props.durability as f32;
            let pct = (dur as f32 / max_dur).clamp(0.0, 1.0);
            let bar_max_w = slot_size - 8.0;
            let bar_w = (bar_max_w * pct).round();
            let bar_y = ry + slot_size - 6.0;
            let red = if pct > 0.5 {
                ((1.0 - pct) * 2.0 * 255.0) as u8
            } else {
                255
            };
            let green = if pct > 0.5 {
                255
            } else {
                (pct * 2.0 * 255.0) as u8
            };
            draw_rect(
                ui_shader,
                rx + 4.0,
                bar_y,
                bar_max_w,
                2.0,
                [0, 0, 0, 255],
                sw,
                sh,
            );
            draw_rect(
                ui_shader,
                rx + 4.0,
                bar_y,
                bar_w,
                2.0,
                [red, green, 0, 255],
                sw,
                sh,
            );
        }
    }
}
