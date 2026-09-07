use crate::container;
use crate::hud::*;
use crate::inventory::{INVENTORY_SLOT_COUNT, ItemStack};
use crate::renderer::{self, Shader, Texture2D};

#[allow(clippy::too_many_arguments)]
pub fn draw_container(
    container: &container::Container,
    inv_slots: &[Option<ItemStack>; INVENTORY_SLOT_COUNT],
    cursor: Option<ItemStack>,
    cursor_position: (f32, f32),
    atlas: &Texture2D,
    ui_shader: &Shader,
    texture_ui_shader: &Shader,
    font_texture: &Texture2D,
    sw: f32,
    sh: f32,
) {
    renderer::set_depth_test(false);
    renderer::set_blend(true);
    renderer::set_cull(false);
    let (px, py) = (sw / 2.0 - 190.0, sh / 2.0 - 170.0);
    draw_rect(
        ui_shader,
        px - 2.0,
        py - 2.0,
        384.0,
        344.0,
        [55, 55, 55, 255],
        sw,
        sh,
    );
    draw_rect(
        ui_shader,
        px,
        py,
        380.0,
        340.0,
        [198, 182, 161, 255],
        sw,
        sh,
    );
    let title = if matches!(container, container::Container::Chest(_)) {
        "Chest"
    } else {
        "Furnace"
    };
    draw_text_tinted(
        font_texture,
        title,
        px + 10.0,
        py + 9.0,
        16.0,
        texture_ui_shader,
        sw,
        sh,
        [55, 48, 40, 255],
    );
    draw_text_tinted(
        font_texture,
        "Inventory",
        px + 10.0,
        py + 150.0,
        14.0,
        texture_ui_shader,
        sw,
        sh,
        [55, 48, 40, 255],
    );
    let (mx, my) = cursor_position;
    let hovered = container.slot_at_pos(mx, my, px, py);
    for slot in (0..container.slots().len()).chain(100..136) {
        let (rx, ry, w, h) = container.slot_rect(slot, px, py);
        draw_rect(
            ui_shader,
            rx - 1.0,
            ry - 1.0,
            w + 2.0,
            h + 2.0,
            [55, 55, 55, 255],
            sw,
            sh,
        );
        let bg = if hovered == Some(slot) {
            [185, 175, 158, 255]
        } else {
            [140, 130, 118, 255]
        };
        draw_rect(ui_shader, rx, ry, w, h, bg, sw, sh);
        let stack = if slot >= 100 {
            inv_slots[slot - 100]
        } else {
            container.slots()[slot]
        };
        if let Some(stack) = stack {
            draw_stack_item(
                atlas,
                texture_ui_shader,
                ui_shader,
                font_texture,
                &stack,
                rx,
                ry,
                w,
                sw,
                sh,
            );
        }
    }
    if let container::Container::Furnace(furnace) = container {
        draw_rect(
            ui_shader,
            px + 90.0,
            py + 78.0,
            36.0,
            14.0,
            [75, 65, 55, 255],
            sw,
            sh,
        );
        let fuel = if furnace.burn_total > 0.0 {
            furnace.burn_remaining / furnace.burn_total
        } else {
            0.0
        };
        draw_rect(
            ui_shader,
            px + 90.0,
            py + 78.0,
            36.0 * fuel,
            14.0,
            [245, 145, 45, 255],
            sw,
            sh,
        );
        draw_rect(
            ui_shader,
            px + 160.0,
            py + 78.0,
            62.0,
            14.0,
            [75, 65, 55, 255],
            sw,
            sh,
        );
        draw_rect(
            ui_shader,
            px + 160.0,
            py + 78.0,
            62.0 * furnace.progress / container::SMELT_SECONDS,
            14.0,
            [120, 200, 90, 255],
            sw,
            sh,
        );
        for (label, x, y) in [
            ("Input", 43.0, 41.0),
            ("Fuel", 43.0, 119.0),
            ("Output", 239.0, 47.0),
        ] {
            draw_text_tinted(
                font_texture,
                label,
                px + x,
                py + y,
                12.0,
                texture_ui_shader,
                sw,
                sh,
                [55, 48, 40, 255],
            );
        }
    }
    draw_text(
        font_texture,
        "Shift-click: transfer   Right-click: split   E: close",
        px,
        py + 350.0,
        13.0,
        texture_ui_shader,
        sw,
        sh,
    );
    if let Some(stack) = cursor {
        draw_stack_item(
            atlas,
            texture_ui_shader,
            ui_shader,
            font_texture,
            &stack,
            mx - 18.0,
            my - 18.0,
            36.0,
            sw,
            sh,
        );
    }
}
