use crate::hud::{draw_rect, draw_text, draw_text_tinted};
use crate::mob_catalog::{Habitat, MobKind, Temper};
use crate::renderer::{Shader, Texture2D};

pub const PAGE_SIZE: usize = 16;
pub struct Catalogue {
    pub page: usize,
    pub natural_only: bool,
    pub notice: String,
}
impl Default for Catalogue {
    fn default() -> Self {
        Self {
            page: 0,
            natural_only: true,
            notice: "Choose a creature to spawn in front of you.".into(),
        }
    }
}
#[derive(Debug, PartialEq)]
pub enum Click {
    Spawn(MobKind),
    Previous,
    Next,
    Filter,
    ClearNearby,
    Close,
}

impl Catalogue {
    pub fn entries(&self) -> Vec<MobKind> {
        let mut entries: Vec<_> = MobKind::ALL
            .iter()
            .copied()
            .filter(|k| !self.natural_only || k.species().habitat != Habitat::Special)
            .collect();
        entries.sort_by_key(|k| k.species().name);
        entries
    }
    fn transform(sw: f32, sh: f32) -> (f32, f32, f32) {
        let scale = (sw / 960.0).min(sh / 650.0).min(1.0);
        (
            (sw - 900.0 * scale) * 0.5,
            (sh - 590.0 * scale) * 0.5,
            scale,
        )
    }
    pub fn click(&self, sw: f32, sh: f32, mx: f32, my: f32) -> Option<Click> {
        let (ox, oy, s) = Self::transform(sw, sh);
        let (x, y) = ((mx - ox) / s, (my - oy) / s);
        let hit = |rx: f32, ry: f32, w: f32, h: f32| x >= rx && x < rx + w && y >= ry && y < ry + h;
        if hit(722.0, 26.0, 150.0, 34.0) {
            return Some(Click::Close);
        }
        if hit(24.0, 84.0, 300.0, 32.0) {
            return Some(Click::Filter);
        }
        if hit(632.0, 84.0, 240.0, 32.0) {
            return Some(Click::ClearNearby);
        }
        if hit(24.0, 530.0, 136.0, 34.0) && self.page > 0 {
            return Some(Click::Previous);
        }
        let entries = self.entries();
        if hit(740.0, 530.0, 136.0, 34.0) && (self.page + 1) * PAGE_SIZE < entries.len() {
            return Some(Click::Next);
        }
        for (i, &kind) in entries
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .enumerate()
        {
            if hit(
                24.0 + (i % 4) as f32 * 216.0,
                132.0 + (i / 4) as f32 * 82.0,
                204.0,
                72.0,
            ) {
                return Some(Click::Spawn(kind));
            }
        }
        None
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        ui: &Shader,
        textured: &Shader,
        font: &Texture2D,
        sw: f32,
        sh: f32,
        count: usize,
    ) {
        let (ox, oy, s) = Self::transform(sw, sh);
        let rect = |x, y, w, h, c| draw_rect(ui, ox + x * s, oy + y * s, w * s, h * s, c, sw, sh);
        let label = |text: &str, x, y, size, color| {
            draw_text_tinted(
                font,
                text,
                ox + x * s,
                oy + y * s,
                size * s,
                textured,
                sw,
                sh,
                color,
            )
        };
        draw_rect(ui, 0.0, 0.0, sw, sh, [8, 15, 19, 195], sw, sh);
        rect(0.0, 0.0, 900.0, 590.0, [28, 39, 43, 255]);
        rect(0.0, 0.0, 900.0, 4.0, [119, 187, 132, 255]);
        label("CREATURES", 24.0, 26.0, 25.0, [242, 238, 221, 255]);
        label(
            &format!("Sandbox  /  {count} of 48 active"),
            24.0,
            61.0,
            14.0,
            [176, 191, 190, 255],
        );
        rect(722.0, 26.0, 150.0, 34.0, [58, 75, 79, 255]);
        label("Resume [Esc]", 733.0, 35.0, 16.0, [242, 238, 221, 255]);
        rect(24.0, 84.0, 300.0, 32.0, [58, 79, 68, 255]);
        rect(632.0, 84.0, 240.0, 32.0, [86, 60, 54, 255]);
        label(
            "Clear nearby creatures",
            643.0,
            92.0,
            15.0,
            [242, 217, 195, 255],
        );
        label(
            if self.natural_only {
                "Existing habitats  >"
            } else {
                "All Overworld creatures  >"
            },
            36.0,
            92.0,
            16.0,
            [223, 238, 212, 255],
        );
        let entries = self.entries();
        for (i, &kind) in entries
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .enumerate()
        {
            let x = 24.0 + (i % 4) as f32 * 216.0;
            let y = 132.0 + (i / 4) as f32 * 82.0;
            let sp = kind.species();
            rect(x, y, 204.0, 72.0, [43, 57, 61, 255]);
            rect(x, y, 4.0, 72.0, [sp.coat[0], sp.coat[1], sp.coat[2], 255]);
            label(sp.name, x + 13.0, y + 14.0, 15.0, [243, 238, 219, 255]);
            let temperament = match sp.temper {
                Temper::Passive => "Passive",
                Temper::Neutral => "Neutral",
                Temper::Hostile => "Hostile",
            };
            label(
                temperament,
                x + 13.0,
                y + 42.0,
                12.0,
                if sp.temper == Temper::Hostile {
                    [224, 147, 129, 255]
                } else {
                    [159, 191, 178, 255]
                },
            );
        }
        label(&self.notice, 24.0, 480.0, 16.0, [233, 210, 145, 255]);
        rect(24.0, 530.0, 136.0, 34.0, [58, 75, 79, 255]);
        rect(740.0, 530.0, 136.0, 34.0, [58, 75, 79, 255]);
        label("< Previous", 34.0, 539.0, 16.0, [242, 238, 221, 255]);
        label("Next >", 777.0, 539.0, 16.0, [242, 238, 221, 255]);
        draw_text(
            font,
            &format!("{} / {}", self.page + 1, entries.len().div_ceil(PAGE_SIZE)),
            ox + 420.0 * s,
            oy + 539.0 * s,
            16.0 * s,
            textured,
            sw,
            sh,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_hitboxes_scale_and_all_pages_are_reachable() {
        for (w, h) in [(960.0, 650.0), (1920.0, 1080.0), (640.0, 480.0)] {
            let mut menu = Catalogue::default();
            let (x, y, s) = Catalogue::transform(w, h);
            assert_eq!(
                menu.click(w, h, x + 40.0 * s, y + 150.0 * s),
                Some(Click::Spawn(menu.entries()[0]))
            );
            assert_eq!(
                menu.click(w, h, x + 650.0 * s, y + 95.0 * s),
                Some(Click::ClearNearby)
            );
            assert!(
                menu.entries()
                    .iter()
                    .all(|k| k.species().habitat != Habitat::Special)
            );
            menu.natural_only = false;
            assert_eq!(menu.entries().len(), MobKind::ALL.len());
            menu.page = menu.entries().len().div_ceil(PAGE_SIZE) - 1;
            assert_eq!(menu.click(w, h, x + 780.0 * s, y + 545.0 * s), None);
            assert_eq!(
                menu.click(w, h, x + 780.0 * s, y + 40.0 * s),
                Some(Click::Close)
            );
        }
    }
}
