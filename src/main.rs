use glfw::{Action, Context, GamepadAxis, GamepadButton, JoystickId, Key};
mod block;
mod chunk;
mod crafting;
mod explosion;
mod item;
mod mining;
mod noise;
mod renderer;
mod world;

use crate::world::{VIEW_DISTANCE, World};
use block::BlockType;
use glam::{Mat4, Vec2, Vec3};

#[derive(PartialEq)]
pub enum GameState {
    Loading,
    Playing,
    Paused,
}

// ── Inventory ──────────────────────────────────────────────────────────────
// Slot layout (45 total):
//   0-8   hotbar           9-35  main inventory (3×9)
//   36-39 armor (visual)  40-43  crafting 2×2     44  crafting output

#[derive(Clone, Copy, PartialEq)]
struct ItemStack {
    block: BlockType,
    count: u32,
    durability: Option<u16>,
}
impl ItemStack {
    fn new(b: BlockType, n: u32) -> Self {
        Self {
            block: b,
            count: n,
            durability: None,
        }
    }
    fn new_tool(b: BlockType) -> Self {
        let dur = item::tool_properties(b).map(|t| t.durability);
        Self {
            block: b,
            count: 1,
            durability: dur,
        }
    }
}

fn stack_max(b: BlockType) -> u32 {
    item::max_stack_size(b)
}

/// Add items when mining – hotbar first, then main inventory.
fn inv_add(slots: &mut [Option<ItemStack>; 45], block: BlockType, mut amt: u32) {
    let sm = stack_max(block);
    for pass in 0..2u8 {
        for i in (0..9).chain(9..36) {
            if amt == 0 {
                return;
            }
            match slots[i] {
                Some(s) if s.block == block && s.count < sm && pass == 0 => {
                    let add = (sm - s.count).min(amt);
                    slots[i] = Some(ItemStack::new(block, s.count + add));
                    amt -= add;
                }
                None if pass == 1 => {
                    let add = amt.min(sm);
                    slots[i] = Some(ItemStack::new(block, add));
                    amt -= add;
                }
                _ => {}
            }
        }
    }
}

/// Add a tool item to inventory (preserves durability).
#[allow(dead_code)]
fn inv_add_tool(slots: &mut [Option<ItemStack>; 45], tool: ItemStack) {
    // Tools stack to 1, so just find an empty slot
    for i in (0..9).chain(9..36) {
        if slots[i].is_none() {
            slots[i] = Some(tool);
            return;
        }
    }
}

/// Full MC 1.0 slot-click mechanics.
fn inv_click(
    slots: &mut [Option<ItemStack>; 45],
    cursor: &mut Option<ItemStack>,
    slot: usize,
    right: bool,
    shift: bool,
) {
    // Crafting output: pick up only (no placing)
    if slot == 44 {
        if !right
            && cursor.is_none()
            && let Some(output) = slots[44].take()
        {
            *cursor = Some(output);
            // Consume one from each crafting input
            for slot in slots.iter_mut().take(44).skip(40) {
                if let Some(s) = slot {
                    s.count -= 1;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
            // Re-check recipe
            update_craft_output_2x2(slots);
        }
        return;
    }
    // Armor slots: simple swap for now
    if (36..40).contains(&slot) {
        if !shift {
            std::mem::swap(&mut slots[slot], cursor);
        }
        return;
    }
    if shift {
        if let Some(s) = slots[slot] {
            slots[slot] = None;
            let sm = stack_max(s.block);
            let (a, b) = if slot < 9 {
                (9usize, 36usize)
            } else {
                (0usize, 9usize)
            };
            let mut rem = s.count;
            for slot_ref in slots.iter_mut().take(b).skip(a) {
                if rem == 0 {
                    break;
                }
                if let Some(d) = *slot_ref
                    && d.block == s.block
                    && d.count < sm
                {
                    let add = (sm - d.count).min(rem);
                    *slot_ref = Some(ItemStack::new(s.block, d.count + add));
                    rem -= add;
                }
            }
            for slot_ref in slots.iter_mut().take(b).skip(a) {
                if rem == 0 {
                    break;
                }
                if slot_ref.is_none() {
                    let n = rem.min(sm);
                    *slot_ref = Some(ItemStack::new(s.block, n));
                    rem -= n;
                }
            }
            if rem > 0 {
                slots[slot] = Some(ItemStack::new(s.block, rem));
            }
        }
        // Update crafting output if we touched crafting slots
        if (40..44).contains(&slot) {
            update_craft_output_2x2(slots);
        }
        return;
    }
    if right {
        if cursor.is_none() {
            if let Some(s) = slots[slot] {
                let half = s.count.div_ceil(2);
                *cursor = Some(ItemStack::new(s.block, half));
                let left = s.count - half;
                slots[slot] = if left > 0 {
                    Some(ItemStack::new(s.block, left))
                } else {
                    None
                };
            }
        } else {
            let held = cursor.unwrap();
            let sm = stack_max(held.block);
            let ok = match slots[slot] {
                None => true,
                Some(d) => d.block == held.block && d.count < sm,
            };
            if ok {
                match slots[slot] {
                    None => {
                        slots[slot] = Some(ItemStack::new(held.block, 1));
                    }
                    Some(d) => {
                        slots[slot] = Some(ItemStack::new(d.block, d.count + 1));
                    }
                }
                let nc = held.count - 1;
                *cursor = if nc > 0 {
                    Some(ItemStack::new(held.block, nc))
                } else {
                    None
                };
            }
        }
    } else {
        match (*cursor, slots[slot]) {
            (None, _) => {
                *cursor = slots[slot].take();
            }
            (Some(h), None) => {
                slots[slot] = Some(h);
                *cursor = None;
            }
            (Some(h), Some(d)) if h.block == d.block => {
                let sm = stack_max(d.block);
                let add = (sm - d.count).min(h.count);
                slots[slot] = Some(ItemStack::new(d.block, d.count + add));
                let nc = h.count - add;
                *cursor = if nc > 0 {
                    Some(ItemStack::new(h.block, nc))
                } else {
                    None
                };
            }
            _ => {
                std::mem::swap(&mut slots[slot], cursor);
            }
        }
    }
    // Update crafting output if we touched crafting slots
    if (40..44).contains(&slot) {
        update_craft_output_2x2(slots);
    }
}

/// Check 2x2 crafting grid and update output slot.
fn update_craft_output_2x2(slots: &mut [Option<ItemStack>; 45]) {
    let grid: Vec<Option<BlockType>> = (40..44).map(|i| slots[i].map(|s| s.block)).collect();
    if let Some((block, count)) = crafting::find_recipe(&grid, 2, 2) {
        if block.is_tool() {
            slots[44] = Some(ItemStack::new_tool(block));
        } else {
            slots[44] = Some(ItemStack::new(block, count as u32));
        }
    } else {
        slots[44] = None;
    }
}

/// Check 3x3 crafting grid and update output slot.
fn update_craft_output_3x3(slots: &mut [Option<ItemStack>; 10]) {
    let grid: Vec<Option<BlockType>> = (0..9).map(|i| slots[i].map(|s| s.block)).collect();
    if let Some((block, count)) = crafting::find_recipe(&grid, 3, 3) {
        if block.is_tool() {
            slots[9] = Some(ItemStack::new_tool(block));
        } else {
            slots[9] = Some(ItemStack::new(block, count as u32));
        }
    } else {
        slots[9] = None;
    }
}

/// Returns (x, y, w, h) of a slot in the inventory panel.
fn slot_rect(slot: usize, px: f32, py: f32) -> (f32, f32, f32, f32) {
    let ss = 36.0f32;
    let st = 38.0f32; // size, stride
    match slot {
        0..=8 => (px + 10.0 + slot as f32 * st, py + 294.0, ss, ss),
        9..=35 => {
            let i = slot - 9;
            (
                px + 10.0 + (i % 9) as f32 * st,
                py + 168.0 + (i / 9) as f32 * st,
                ss,
                ss,
            )
        }
        36..=39 => (px + 10.0, py + 10.0 + (slot - 36) as f32 * st, ss, ss),
        40..=43 => {
            let i = slot - 40;
            (
                px + 195.0 + (i % 2) as f32 * st,
                py + 28.0 + (i / 2) as f32 * st,
                ss,
                ss,
            )
        }
        44 => (px + 304.0, py + 42.0, ss, ss),
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

fn slot_at_pos(mx: f32, my: f32, px: f32, py: f32) -> Option<usize> {
    for s in (0..45).filter(|&s| !(36..40).contains(&s)) {
        // skip armor for now
        let (x, y, w, h) = slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    None
}
/// Crafting table UI layout.
/// Returns (x,y,w,h) for crafting table slots relative to panel origin.
/// Slots 0-8: 3x3 grid, 9: output, 100-135: inventory (mapped to inv 0-35), 136-144: hotbar overlay
/// We use 100+ offsets to distinguish from craft_table_slots indices.
fn ct_slot_rect(slot: usize, px: f32, py: f32) -> (f32, f32, f32, f32) {
    let ss = 36.0f32;
    let st = 38.0f32;
    match slot {
        // 3x3 crafting grid
        0..=8 => {
            let col = slot % 3;
            let row = slot / 3;
            (
                px + 16.0 + col as f32 * st,
                py + 18.0 + row as f32 * st,
                ss,
                ss,
            )
        }
        // Output slot
        9 => (px + 200.0, py + 52.0, ss, ss),
        // Main inventory (slots 9-35 of inv_slots, mapped as 100+)
        100..=126 => {
            let i = slot - 100;
            (
                px + 10.0 + (i % 9) as f32 * st,
                py + 168.0 + (i / 9) as f32 * st,
                ss,
                ss,
            )
        }
        // Hotbar (slots 0-8 of inv_slots, mapped as 127+)
        127..=135 => {
            let i = slot - 127;
            (px + 10.0 + i as f32 * st, py + 294.0, ss, ss)
        }
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

fn ct_slot_at_pos(mx: f32, my: f32, px: f32, py: f32) -> Option<usize> {
    // Check 3x3 grid + output
    for s in 0..=9 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    // Check inventory area (inv slots 9-35 → ct slots 100-126)
    for s in 100..=126 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    // Check hotbar (inv slots 0-8 → ct slots 127-135)
    for s in 127..=135 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    None
}

/// Handle click in crafting table UI.
fn ct_click(
    ct_slots: &mut [Option<ItemStack>; 10],
    inv_slots: &mut [Option<ItemStack>; 45],
    cursor: &mut Option<ItemStack>,
    ct_slot: usize,
    right: bool,
) {
    if ct_slot == 9 {
        // Output slot: pick up only
        if !right
            && cursor.is_none()
            && let Some(output) = ct_slots[9].take()
        {
            *cursor = Some(output);
            for slot in ct_slots.iter_mut().take(9) {
                if let Some(s) = slot {
                    s.count -= 1;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
            update_craft_output_3x3(ct_slots);
        }
        return;
    }

    // Map ct_slot to actual slot reference
    let slot_ref: &mut Option<ItemStack> = if ct_slot <= 8 {
        &mut ct_slots[ct_slot]
    } else if (100..=126).contains(&ct_slot) {
        &mut inv_slots[ct_slot - 100 + 9]
    } else if (127..=135).contains(&ct_slot) {
        &mut inv_slots[ct_slot - 127]
    } else {
        return;
    };

    if right {
        if cursor.is_none() {
            if let Some(s) = *slot_ref {
                let half = s.count.div_ceil(2);
                *cursor = Some(ItemStack::new(s.block, half));
                let left = s.count - half;
                *slot_ref = if left > 0 {
                    Some(ItemStack::new(s.block, left))
                } else {
                    None
                };
            }
        } else {
            let held = cursor.unwrap();
            let sm = stack_max(held.block);
            let ok = match *slot_ref {
                None => true,
                Some(d) => d.block == held.block && d.count < sm,
            };
            if ok {
                match *slot_ref {
                    None => *slot_ref = Some(ItemStack::new(held.block, 1)),
                    Some(d) => *slot_ref = Some(ItemStack::new(d.block, d.count + 1)),
                }
                let nc = held.count - 1;
                *cursor = if nc > 0 {
                    Some(ItemStack::new(held.block, nc))
                } else {
                    None
                };
            }
        }
    } else {
        match (*cursor, *slot_ref) {
            (None, _) => {
                *cursor = slot_ref.take();
            }
            (Some(h), None) => {
                *slot_ref = Some(h);
                *cursor = None;
            }
            (Some(h), Some(d)) if h.block == d.block => {
                let sm = stack_max(d.block);
                let add = (sm - d.count).min(h.count);
                *slot_ref = Some(ItemStack::new(d.block, d.count + add));
                let nc = h.count - add;
                *cursor = if nc > 0 {
                    Some(ItemStack::new(h.block, nc))
                } else {
                    None
                };
            }
            _ => {
                std::mem::swap(slot_ref, cursor);
            }
        }
    }

    // Update crafting output if a crafting grid slot was touched
    if ct_slot <= 8 {
        update_craft_output_3x3(ct_slots);
    }
}

/// Return crafting table items to inventory on close.
fn ct_close(ct_slots: &mut [Option<ItemStack>; 10], inv_slots: &mut [Option<ItemStack>; 45]) {
    for slot in ct_slots.iter_mut().take(9) {
        if let Some(s) = slot.take() {
            inv_add(inv_slots, s.block, s.count);
        }
    }
    ct_slots[9] = None;
}

// shader imports
use renderer::{RenderTexture2D, Shader, Texture2D};

// const WINDOW_WIDTH: u32 = 1920;
// const WINDOW_HEIGHT: u32 = 1080;
const RENDER_WIDTH: i32 = 750;
const RENDER_HEIGHT: i32 = 422;

#[derive(Debug)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

impl WindowState {
    fn load() -> Self {
        let path = "window.cfg";
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut width = 1280;
            let mut height = 720;
            let mut x = 100;
            let mut y = 100;
            let mut maximized = false;
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    match parts[0] {
                        "width" => width = parts[1].parse().unwrap_or(width),
                        "height" => height = parts[1].parse().unwrap_or(height),
                        "x" => x = parts[1].parse().unwrap_or(x),
                        "y" => y = parts[1].parse().unwrap_or(y),
                        "maximized" => maximized = parts[1].parse().unwrap_or(maximized),
                        _ => {}
                    }
                }
            }
            Self {
                width,
                height,
                x,
                y,
                maximized,
            }
        } else {
            Self {
                width: 1280,
                height: 720,
                x: 100,
                y: 100,
                maximized: false,
            }
        }
    }

    fn save(&self) {
        let content = format!(
            "width:{}\nheight:{}\nx:{}\ny:{}\nmaximized:{}\n",
            self.width, self.height, self.x, self.y, self.maximized
        );
        let _ = std::fs::write("window.cfg", content);
    }
}

const PS1_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 2) in vec3 vertexNormal;
layout(location = 3) in vec4 vertexColor;

uniform mat4 uMVP;
uniform mat4 uModel;
uniform float uTime;

out vec4 fragColor;
out vec2 fragTexCoord;
out vec3 fragPos;
out vec3 fragNormal;

void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;
    
    vec3 pos = vertexPosition;
    // Simple wave animation for water (alpha 240/255 ~= 0.94)
    if (vertexColor.a > 0.940 && vertexColor.a < 0.942) {
        pos.y += sin(uTime * 1.5 + vertexPosition.x * 0.8 + vertexPosition.z * 0.8) * 0.08 - 0.05;
    }
    
    fragPos = (uModel * vec4(pos, 1.0)).xyz;
    fragNormal = normalize((uModel * vec4(vertexNormal, 0.0)).xyz);
    
    gl_Position = uMVP * vec4(pos, 1.0);
}
"#;

const PS1_FS: &str = r#"
#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
in vec3 fragPos;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec4 colDiffuse;
uniform vec3 sunDir;
uniform vec3 viewPos;
uniform float time;
uniform vec4 skyCol;

out vec4 finalColor;

void main() {
    vec4 texelColor = texture(texture0, fragTexCoord);
    if (texelColor.a < 0.1) discard;
    
    // Global ambient based on sun position (time of day)
    float sunY = max(0.0, sunDir.y);
    float timeLight = 0.15 + (sunY * 0.85); // 0.15 at night, 1.0 at noon
    
    vec3 baseColor = texelColor.rgb * fragColor.rgb * colDiffuse.rgb;
    vec3 ambient = texelColor.rgb * 0.12; // minimum ambient so caves aren't pure black
    vec3 color = max(baseColor * timeLight, ambient);
    
    color *= mix(vec3(1.0, 0.9, 0.8), vec3(1.0, 1.0, 1.05), sunY); // slight tinting
    
    // Final color output without artificial quantization
    vec4 c = vec4(color, texelColor.a * fragColor.a * colDiffuse.a);
    finalColor = c;
}
"#;

const WATER_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 2) in vec3 vertexNormal;
layout(location = 3) in vec4 vertexColor;

uniform mat4 uMVP;
uniform mat4 uModel;
uniform float uTime;

out vec4 fragColor;
out vec2 fragTexCoord;
out vec3 fragPos;
out vec3 fragNormal;

void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;

    vec3 pos = vertexPosition;
    // Slight vertex animation for waves
    float wave = sin(uTime * 2.5 + pos.x * 1.5 + pos.z * 1.5) * 0.05;
    pos.y += wave;

    fragPos = (uModel * vec4(pos, 1.0)).xyz;
    fragNormal = normalize((uModel * vec4(vertexNormal, 0.0)).xyz);

    gl_Position = uMVP * vec4(pos, 1.0);
}
"#;

const WATER_FS: &str = r#"
#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
in vec3 fragPos;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec3 sunDir;
uniform vec3 viewPos;
uniform float uTime;
uniform vec4 skyCol;

out vec4 finalColor;

void main() {
    vec4 texelColor = texture(texture0, fragTexCoord);

    vec3 N = normalize(fragNormal);
    vec3 V = normalize(viewPos - fragPos);
    vec3 L = normalize(sunDir);
    vec3 R = reflect(-L, N);

    // Ambient + Diffuse
    float diff = max(dot(N, L), 0.0);
    vec3 diffuse = texelColor.rgb * fragColor.rgb * (diff * 0.7 + 0.3);

    // Specular (Sparkle/Highlights)
    float spec = pow(max(dot(V, R), 0.0), 32.0);
    vec3 specular = vec3(1.0) * spec * 0.6;

    // Fresnel-ish transparency
    float fresnel = pow(1.0 - max(dot(N, V), 0.0), 3.0);
    float alpha = mix(fragColor.a, 1.0, fresnel * 0.5);

    // Simple shore foam / depth effect simulation
    vec3 color = mix(diffuse, vec3(0.1, 0.4, 0.8), 0.2) + specular;

    finalColor = vec4(color, alpha);
}
"#;

const FLAT_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 3) in vec4 vertexColor;
uniform mat4 uMVP;
out vec4 fragColor;
out vec2 fragTexCoord;
void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;
    gl_Position = uMVP * vec4(vertexPosition, 1.0);
}
"#;

const FLAT_FS: &str = r#"
#version 330 core
in vec4 fragColor;
in vec2 fragTexCoord;
out vec4 finalColor;
uniform int uBodyType; // 0=flat, 1=sun, 2=moon
void main() {
    if (uBodyType > 0) {
        vec2 uv = fragTexCoord * 8.0;
        ivec2 iuv = ivec2(floor(uv));
        if (iuv.x < 0 || iuv.x > 7 || iuv.y < 0 || iuv.y > 7) discard;
        
        int shape[8] = int[](60, 126, 255, 255, 255, 255, 126, 60);
        int rowMask = shape[7 - iuv.y];
        if ((rowMask & (1 << (7 - iuv.x))) == 0) discard;
        
        vec4 color = fragColor;
        if (uBodyType == 2) {
            // Craters on the moon
            // 0, 0, 10, 4, 24, 24, 68, 0 roughly mapping to darker craters on MC moon
            int craters[8] = int[](0, 20, 36, 64, 8, 16, 2, 0); 
            int craterMask = craters[7 - iuv.y];
            if ((craterMask & (1 << (7 - iuv.x))) != 0) {
                color.rgb *= 0.65;
            }
        }
        finalColor = color;
    } else {
        finalColor = fragColor;
    }
}
"#;

const UI_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 3) in vec4 vertexColor;
uniform vec2 uScreenSize;
out vec4 fragColor;
void main() {
    vec2 pos = (vertexPosition.xy / uScreenSize) * 2.0 - 1.0;
    gl_Position = vec4(pos.x, -pos.y, 0.0, 1.0);
    fragColor = vertexColor;
}
"#;

const UI_FS: &str = r#"
#version 330 core
in vec4 fragColor;
out vec4 finalColor;
void main() {
    finalColor = fragColor;
}
"#;

const TEXTURE_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
out vec2 fragTexCoord;
void main() {
    fragTexCoord = vertexTexCoord;
    gl_Position = vec4(vertexPosition, 1.0);
}
"#;

const TEXTURE_FS: &str = r#"
#version 330 core
in vec2 fragTexCoord;
out vec4 finalColor;
uniform sampler2D texture0;
void main() {
    finalColor = texture(texture0, fragTexCoord);
}
"#;

const COLOR_FS: &str = r#"
#version 330 core
out vec4 finalColor;
uniform vec4 uColor;
void main() {
    finalColor = uColor;
}
"#;

const UI_TEXTURE_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
uniform vec2 uScreenSize;
out vec2 fragTexCoord;
void main() {
    vec2 pos = (vertexPosition.xy / uScreenSize) * 2.0 - 1.0;
    gl_Position = vec4(pos.x, -pos.y, 0.0, 1.0);
    fragTexCoord = vertexTexCoord;
}
"#;

const UI_TEXTURE_FS: &str = r#"
#version 330 core
in vec2 fragTexCoord;
out vec4 finalColor;
uniform sampler2D texture0;
void main() {
    finalColor = texture(texture0, fragTexCoord);
}
"#;

#[allow(clippy::too_many_arguments)]
fn draw_rect(
    shader: &Shader,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [u8; 4],
    screen_width: f32,
    screen_height: f32,
) {
    let v = [
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
    let mut c = Vec::new();
    for _ in 0..6 {
        c.extend_from_slice(&color);
    }
    let mesh = renderer::Mesh::new(&v, None, None, Some(&c));
    shader.bind();
    shader.set_vec2(
        shader.get_uniform_location("uScreenSize"),
        glam::Vec2::new(screen_width, screen_height),
    );
    mesh.draw();
}

fn draw_texture_quad(texture: &Texture2D) {
    let v = [
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0,
        0.0,
    ];
    let t = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];
    let mesh = renderer::Mesh::new(&v, Some(&t), None, None);
    texture.bind(0);
    mesh.draw();
}

#[allow(clippy::too_many_arguments)]
fn draw_texture_ui_uv(
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
    let mesh = renderer::Mesh::new(&verts, Some(&texs), None, None);
    shader.bind();
    shader.set_vec2(
        shader.get_uniform_location("uScreenSize"),
        glam::Vec2::new(screen_width, screen_height),
    );
    texture.bind(0);
    mesh.draw();
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    texture: &Texture2D,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    shader: &Shader,
    sw: f32,
    sh: f32,
) {
    let mut cur_x = x;
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
        );
        cur_x += size * 0.55;
    }
}

fn draw_screen_quad(shader: &Shader, color: glam::Vec4) {
    let v = [
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0,
        0.0,
    ];
    let mesh = renderer::Mesh::new(&v, None, None, None);
    shader.bind();
    shader.set_vec4(shader.get_uniform_location("uColor"), color);
    unsafe {
        gl::Disable(gl::DEPTH_TEST);
        mesh.draw();
        gl::Enable(gl::DEPTH_TEST);
    }
}

fn draw_sun_moon(shader: &Shader, player_pos: Vec3, time: f32, mvp: Mat4) {
    let dist = 450.0;
    let size = 60.0;
    let angle = (time / 1200.0) * 2.0 * std::f32::consts::PI;
    let sun_y = angle.sin();
    let sun_dir = Vec3::new(0.0, sun_y, angle.cos());
    let sun_pos = player_pos + sun_dir * dist;
    let moon_angle = angle + std::f32::consts::PI;
    let moon_dir = Vec3::new(0.0, moon_angle.sin(), moon_angle.cos());
    let moon_pos = player_pos + moon_dir * dist;
    let sunrise_mult = if sun_y > -0.3 && sun_y < 0.3 {
        0.6
    } else {
        1.0
    };
    let sun_c = [
        (255.0 * sunrise_mult) as u8,
        (200.0 * sunrise_mult) as u8,
        (150.0 * sunrise_mult) as u8,
        255,
    ];
    let moon_c = [220, 225, 255, 255];

    let create_mesh = |pos: Vec3, color: [u8; 4], q_size: f32| -> renderer::Mesh {
        let mut v = Vec::new();
        let mut c = Vec::new();
        let mut t = Vec::new();
        let look = (player_pos - pos).normalize();
        let r = look.cross(Vec3::Y).normalize();
        let u = r.cross(look).normalize();
        let v0 = pos + (r * -q_size) + (u * -q_size);
        let v1 = pos + (r * q_size) + (u * -q_size);
        let v2 = pos + (r * q_size) + (u * q_size);
        let v3 = pos + (r * -q_size) + (u * q_size);
        v.extend_from_slice(&[v0.x, v0.y, v0.z, v1.x, v1.y, v1.z, v2.x, v2.y, v2.z]);
        v.extend_from_slice(&[v0.x, v0.y, v0.z, v2.x, v2.y, v2.z, v3.x, v3.y, v3.z]);
        for _ in 0..6 {
            c.extend_from_slice(&color);
        }
        t.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
        renderer::Mesh::new(&v, Some(&t), None, Some(&c))
    };

    let sun_mesh = create_mesh(sun_pos, sun_c, size);
    let moon_mesh = create_mesh(moon_pos, moon_c, size * 0.8);

    shader.bind();
    shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);

    unsafe {
        gl::Disable(gl::DEPTH_TEST);
        gl::Disable(gl::CULL_FACE);
    }

    shader.set_int(shader.get_uniform_location("uBodyType"), 1); // 1 = Sun
    sun_mesh.draw();

    shader.set_int(shader.get_uniform_location("uBodyType"), 2); // 2 = Moon
    moon_mesh.draw();

    shader.set_int(shader.get_uniform_location("uBodyType"), 0); // Reset
    unsafe {
        gl::Enable(gl::CULL_FACE);
        gl::Enable(gl::DEPTH_TEST);
    }
}

/// Draw a block/item icon in the UI at (x,y) with size (w,h) using atlas texture.
#[allow(clippy::too_many_arguments)]
fn draw_item_icon(
    atlas: &renderer::Texture2D,
    tex_shader: &renderer::Shader,
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
    draw_texture_ui_uv(atlas, x, y, w, h, u, v, ts, ts, tex_shader, sw, sh);
}

struct Player {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
    air_seconds: f32,
    inventory_open: bool,
    selected_slot: usize,
    health: i32,
    flying: bool,
    last_space_release: f64,
    space_was_pressed: bool,
}
impl Player {
    fn is_point_in_block(world: &World, p: Vec3) -> bool {
        let b = world.get_block(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        b != BlockType::Air && b != BlockType::Water
    }
    fn check_collision(world: &World, pos: Vec3) -> bool {
        let w = 0.22;
        let h = 1.75;
        for x_off in [-w, w] {
            for z_off in [-w, w] {
                for yi in 0..=2 {
                    let y = 0.1 + yi as f32 * ((h - 0.1) / 2.0);
                    if Self::is_point_in_block(
                        world,
                        Vec3::new(pos.x + x_off, pos.y + y, pos.z + z_off),
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }
    pub fn intersects_block(&self, bx: i32, by: i32, bz: i32) -> bool {
        let w = 0.3; // Slightly larger for safety
        let player_min = self.position - Vec3::new(w, 0.0, w);
        let player_max = self.position + Vec3::new(w, 1.8, w);
        let block_min = Vec3::new(bx as f32, by as f32, bz as f32);
        let block_max = block_min + Vec3::ONE;

        player_max.x > block_min.x
            && player_min.x < block_max.x
            && player_max.y > block_min.y
            && player_min.y < block_max.y
            && player_max.z > block_min.z
            && player_min.z < block_max.z
    }
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        world: &World,
        move_input: Vec3,
        dt: f32,
        is_sprinting: bool,
        is_jumping: bool,
        is_sneaking: bool,
        current_time: f64,
    ) {
        if self.inventory_open {
            return;
        }

        // Double-tap space detection for toggling flight
        let space_just_pressed = is_jumping && !self.space_was_pressed;
        if space_just_pressed && !self.grounded {
            let time_since_last = current_time - self.last_space_release;
            if time_since_last < 0.35 {
                self.flying = !self.flying;
                self.velocity.y = 0.0;
            }
        }
        if !is_jumping && self.space_was_pressed {
            // Space was just released
            self.last_space_release = current_time;
        }
        self.space_was_pressed = is_jumping;

        // Landing disables flying
        if self.grounded && self.flying {
            self.flying = false;
        }

        let waist_in_w = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 0.9).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let feet_in_w = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 0.1).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let head_in_w = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 1.6).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let in_water = waist_in_w || feet_in_w;

        // Apply movement input
        let mut mv = move_input;
        if mv.length_squared() > 0.1 {
            mv = mv.normalize();
            let speed = if self.flying {
                10.92 // MC creative fly speed
            } else if in_water {
                2.0
            } else if is_sprinting {
                5.612
            } else {
                4.317
            };
            mv *= speed;
        }

        if self.flying {
            // Flying physics: direct control, high drag, no gravity
            let fly_drag = 0.09f32.powf(dt);
            self.velocity.x = self.velocity.x * (1.0 - fly_drag) + mv.x * fly_drag;
            self.velocity.z = self.velocity.z * (1.0 - fly_drag) + mv.z * fly_drag;

            // Vertical: space up, shift down, else hover
            let fly_speed = 7.8;
            if is_jumping {
                self.velocity.y = fly_speed;
            } else if is_sneaking {
                self.velocity.y = -fly_speed;
            } else {
                self.velocity.y *= 0.6f32.powf(dt * 20.0); // Smooth stop
            }
        } else {
            // Normal horizontal velocity
            let friction_multiplier = if in_water {
                0.8
            } else if !self.grounded {
                0.98
            } else {
                0.6
            };
            self.velocity.x = self.velocity.x * (1.0 - friction_multiplier * dt * 20.0)
                + mv.x * friction_multiplier * dt * 20.0;
            self.velocity.z = self.velocity.z * (1.0 - friction_multiplier * dt * 20.0)
                + mv.z * friction_multiplier * dt * 20.0;

            // Vertical velocity and gravity (MC 1.0 physics)
            if in_water {
                if is_jumping {
                    self.velocity.y += 0.04 * 20.0;
                    if self.velocity.y > 2.0 {
                        self.velocity.y = 2.0;
                    }
                } else {
                    self.velocity.y -= 0.02 * 20.0;
                    if self.velocity.y < -2.0 {
                        self.velocity.y = -2.0;
                    }
                }
            } else {
                self.velocity.y -= 32.0 * dt;
                let fall_drag = 0.98f32.powf(dt * 20.0);
                self.velocity.y *= fall_drag;
                if is_jumping && self.grounded {
                    self.velocity.y = 8.4;
                    self.grounded = false;
                }
            }
        }

        let dy = self.velocity.y * dt;
        self.position.y += dy;
        if Self::check_collision(world, self.position) {
            if self.velocity.y <= 0.0 {
                self.grounded = true;
            }
            self.position.y -= dy;
            self.velocity.y = 0.0;
        } else if self.velocity.y != 0.0 {
            self.grounded = false;
        }

        let dx = self.velocity.x * dt;
        self.position.x += dx;
        if Self::check_collision(world, self.position) {
            self.position.x -= dx;
        }

        let dz = self.velocity.z * dt;
        self.position.z += dz;
        if Self::check_collision(world, self.position) {
            self.position.z -= dz;
        }

        if head_in_w {
            self.air_seconds = (self.air_seconds - dt).max(0.0);
        } else {
            self.air_seconds = (self.air_seconds + dt * 3.0).min(15.0);
        }

        if self.air_seconds <= 0.0 {
            // Drowning damage (not fully implemented, just draining health for now)
            // real implementation would need a timer
        }
    }
}

fn draw_heart(
    shader: &Shader,
    x: f32,
    y: f32,
    size: f32,
    screen_width: f32,
    screen_height: f32,
    empty: bool,
) {
    // Simple blocky heart shape (10x9 grid roughly)
    // 01100110
    // 11111111
    // 11111111
    // 01111110
    // 00111100
    // 00011000

    let color = if empty {
        [40, 40, 40, 200]
    } else {
        [220, 20, 20, 255]
    };
    let border = [20, 20, 20, 255];

    // Draw background/border
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
                draw_rect(shader, bx, by, bw, bw, p_color, screen_width, screen_height);
            }
        }
    }
}

fn draw_bubble(
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

                draw_rect(shader, bx, by, bw, bw, p_color, screen_width, screen_height);
            }
        }
    }
}

// explode function moved to explosion.rs

// Cleanup old particle code

fn draw_pause_menu(
    ui_shader: &Shader,
    tex_shader: &Shader,
    font_tex: &Texture2D,
    sw: f32,
    sh: f32,
) {
    // 1. Full-screen tint
    draw_rect(ui_shader, 0.0, 0.0, sw, sh, [0, 0, 0, 160], sw, sh);

    // 2. Buttons (Left Column)
    let btn_w = 340.0;
    let btn_h = 44.0;
    let btn_x = 100.0;
    let btn_y_start = sh / 2.0 - 100.0;

    let labels = ["Resume Game", "Settings", "Marketplace", "Save & Quit"];
    for (i, label) in labels.iter().enumerate() {
        let y = btn_y_start + i as f32 * (btn_h + 12.0);
        // Shadow/Border
        draw_rect(
            ui_shader,
            btn_x - 2.0,
            y - 2.0,
            btn_w + 4.0,
            btn_h + 4.0,
            [20, 20, 20, 255],
            sw,
            sh,
        );
        // Base
        draw_rect(
            ui_shader,
            btn_x,
            y,
            btn_w,
            btn_h,
            [198, 198, 198, 255],
            sw,
            sh,
        );
        // Inset
        draw_rect(
            ui_shader,
            btn_x + 2.0,
            y + 2.0,
            btn_w - 4.0,
            btn_h - 4.0,
            [110, 110, 110, 255],
            sw,
            sh,
        );
        let text_sz = 18.0;
        let text_x = btn_x + (btn_w - label.len() as f32 * text_sz * 0.55) / 2.0;
        draw_text(
            font_tex,
            label,
            text_x,
            y + 13.0,
            text_sz,
            tex_shader,
            sw,
            sh,
        );
    }

    // Small buttons bottom left icons
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

        // Simple Icons
        if i == 0 {
            // Chat
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
            draw_rect(
                ui_shader,
                sx + 12.0,
                sy + 17.0,
                6.0,
                4.0,
                [255, 255, 255, 100],
                sw,
                sh,
            );
        } else if i == 1 {
            // Camera
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
            draw_rect(
                ui_shader,
                sx + 19.0,
                sy + 23.0,
                6.0,
                6.0,
                [255, 255, 255, 100],
                sw,
                sh,
            );
        } else if i == 2 {
            // Profile
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

    // 3. Right Panel (Friends / Session)
    let panel_w = 380.0;
    let panel_h = sh - 100.0;
    let panel_x = sw - panel_w - 40.0;
    let panel_y = 50.0;

    // Panel background
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

    // Header
    draw_text(
        font_tex,
        "Friends (12)",
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
        "Invite to Game",
        panel_x + 60.0,
        panel_y + 70.0,
        18.0,
        tex_shader,
        sw,
        sh,
    );
    draw_text(
        font_tex,
        "Players in My World",
        panel_x + 20.0,
        panel_y + 120.0,
        16.0,
        tex_shader,
        sw,
        sh,
    );

    // Player row
    let py = panel_y + 150.0;
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
    // Head icon
    draw_rect(
        ui_shader,
        panel_x + 20.0,
        py + 10.0,
        30.0,
        30.0,
        [200, 150, 100, 255],
        sw,
        sh,
    );
    draw_text(
        font_tex,
        "Grog Grog",
        panel_x + 60.0,
        py + 17.0,
        18.0,
        tex_shader,
        sw,
        sh,
    );

    // Bottom centered text
    draw_text(
        font_tex,
        "Game is paused",
        sw / 2.0 - 60.0,
        sh - 40.0,
        18.0,
        tex_shader,
        sw,
        sh,
    );
}

fn main() {
    // Entry point
    let state = WindowState::load();
    let mut glfw = glfw::init(glfw::log_errors).unwrap();
    // Add DualSense mappings for Linux
    glfw.update_gamepad_mappings("030000004c050000e60d000011010000,PS5 Controller,a:b0,b:b1,back:b8,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b10,leftshoulder:b4,leftstick:b11,lefttrigger:a3,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:a4,rightx:a2,righty:a5,start:b9,x:b2,y:b3,platform:Linux,\n\
                                  050000004c050000e60d0000ff070000,PS5 Controller,a:b0,b:b1,back:b8,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b10,leftshoulder:b4,leftstick:b11,lefttrigger:a3,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:a4,rightx:a2,righty:a5,start:b9,x:b2,y:b3,platform:Linux,");
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(
        glfw::OpenGlProfileHint::Core,
    ));
    glfw.window_hint(glfw::WindowHint::AutoIconify(false));
    #[cfg(target_os = "macos")]
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    let (mut window, events) = glfw
        .create_window(
            state.width,
            state.height,
            "VoxelPopuli Rust",
            glfw::WindowMode::Windowed,
        )
        .expect("Failed to create GLFW window.");
    window.make_current();
    window.set_pos(state.x, state.y);
    if state.maximized {
        window.maximize();
    }
    window.set_key_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_size_polling(true);
    window.set_mouse_button_polling(true);
    window.set_scroll_polling(true);
    window.set_cursor_mode(glfw::CursorMode::Disabled);
    gl::load_with(|s| {
        window
            .get_proc_address(s)
            .map_or(std::ptr::null(), |p| p as *const _)
    });
    unsafe {
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LEQUAL);
        gl::Enable(gl::CULL_FACE);
        gl::CullFace(gl::BACK);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
    let mut world = World::new();
    world.generate_atlas();
    world.init_celestial();
    let shader = Shader::new(PS1_VS, PS1_FS).expect("Failed to compile PS1 shader");
    let water_shader = Shader::new(WATER_VS, WATER_FS).expect("Failed to compile water shader");
    let flat_shader = Shader::new(FLAT_VS, FLAT_FS).expect("Failed to compile FLAT shader");
    let ui_shader = Shader::new(UI_VS, UI_FS).expect("Failed to compile UI shader");
    let texture_shader =
        Shader::new(TEXTURE_VS, TEXTURE_FS).expect("Failed to compile TEXTURE shader");
    let texture_ui_shader =
        Shader::new(UI_TEXTURE_VS, UI_TEXTURE_FS).expect("Failed to compile UI TEXTURE shader");
    let color_shader = Shader::new(TEXTURE_VS, COLOR_FS).expect("Failed to compile COLOR shader");
    let target = RenderTexture2D::new(RENDER_WIDTH, RENDER_HEIGHT);
    let font_texture = Texture2D::from_file("assets/font.png");

    let mut is_fullscreen = false;
    let mut win_pos = (state.x, state.y);
    let mut win_size = (state.width, state.height);

    let mut game_state = GameState::Loading;
    let mut spawn_y = 150.0;
    let mut inv_slots = [None::<ItemStack>; 45];
    inv_slots[0] = Some(ItemStack::new(BlockType::Nuke, 64));
    inv_slots[1] = Some(ItemStack::new(BlockType::FlintAndSteel, 1));
    let mut inv_cursor: Option<ItemStack> = None;
    let mut player = Player {
        position: Vec3::new(32.5, spawn_y, 32.5),
        velocity: Vec3::ZERO,
        grounded: false,
        air_seconds: 15.0,
        inventory_open: false,
        selected_slot: 0,
        health: 20,
        flying: false,
        last_space_release: 0.0,
        space_was_pressed: false,
    };
    let mut camera_angle = Vec2::new(std::f32::consts::PI, 0.0);
    let mut last_cursor_pos = window.get_cursor_pos();
    let mut mining_state = mining::MiningState::new();
    let mut left_mouse_held = false;
    let mut crafting_table_open = false;
    let mut craft_table_slots = [None::<ItemStack>; 10]; // 0-8: 3x3 grid, 9: output
    let mut prev_gp_lt = -1.0f32;
    let mut prev_gp_dpad_right = false;
    let mut prev_gp_dpad_left = false;
    let mut prev_gp_rb = false;
    let mut prev_gp_lb = false;
    let mut last_time = glfw.get_time();
    let mut fps_last_time = last_time;
    let mut fps_frames: u32 = 0;
    while !window.should_close() {
        let current_time = glfw.get_time();
        let delta_time = (current_time - last_time) as f32;
        last_time = current_time;

        fps_frames += 1;
        let fps_elapsed = current_time - fps_last_time;
        if fps_elapsed >= 1.0 {
            let fps = (fps_frames as f64 / fps_elapsed) as f32;
            fps_frames = 0;
            fps_last_time = current_time;
            window.set_title(&format!("VoxelPopuli Rust - {:.0} FPS", fps));
        }
        glfw.poll_events();

        if game_state == GameState::Loading {
            // Update world incrementally
            world.update(Vec3::new(32.5, 0.0, 32.5), current_time as f32);
            if !world.is_loading {
                for y in (0..255).rev() {
                    if world.get_block(32, y, 32) != BlockType::Air {
                        spawn_y = y as f32 + 2.0;
                        break;
                    }
                }
                player.position = Vec3::new(32.5, spawn_y, 32.5);
                game_state = GameState::Playing;
            }

            let (win_sw, win_sh) = window.get_size();
            let sw = win_sw as f32;
            let sh = win_sh as f32;

            // Render Loading Screen
            unsafe {
                gl::Viewport(0, 0, win_sw, win_sh);
                gl::ClearColor(0.117, 0.117, 0.117, 1.0); // Exact #1e1e1e
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
                gl::Disable(gl::DEPTH_TEST);
                gl::Disable(gl::CULL_FACE);
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            }

            // 1. Logo removed as requested

            // 2. Spinner (8-segment pulse)
            let spin_x = sw / 2.0;
            let spin_y = sh / 2.0 + 10.0;
            let spin_radius = 16.0;
            for i in 0..8 {
                let angle = (i as f32 / 8.0) * 2.0 * std::f32::consts::PI;
                let t_offset = (current_time * 8.0) as usize % 8;
                let brightness = if i == t_offset {
                    255
                } else if i == (t_offset + 7) % 8 {
                    180
                } else if i == (t_offset + 6) % 8 {
                    100
                } else {
                    40
                };
                let sx = spin_x + angle.cos() * spin_radius;
                let sy = spin_y + angle.sin() * spin_radius;
                draw_rect(
                    &ui_shader,
                    sx - 2.0,
                    sy - 2.0,
                    4.0,
                    4.0,
                    [255, 255, 255, brightness],
                    sw,
                    sh,
                );
            }

            // 3. Progress Bar (Thin & Clean)
            let bar_w = (sw * 0.45).min(500.0);
            let bar_h = 4.0;
            let bar_x = (sw - bar_w) / 2.0;
            let bar_y = sh / 2.0 + 60.0;

            // Border (1px white)
            draw_rect(
                &ui_shader,
                bar_x - 1.0,
                bar_y - 1.0,
                bar_w + 2.0,
                bar_h + 2.0,
                [255, 255, 255, 255],
                sw,
                sh,
            );
            // BG (Inside border)
            draw_rect(
                &ui_shader,
                bar_x,
                bar_y,
                bar_w,
                bar_h,
                [30, 30, 30, 255],
                sw,
                sh,
            );

            // Progress Fill
            let total_chunks = (VIEW_DISTANCE * 2 + 1) as f32 * (VIEW_DISTANCE * 2 + 1) as f32;
            let progress = (world.chunks_generated_count as f32 / total_chunks).clamp(0.0, 1.0);
            draw_rect(
                &ui_shader,
                bar_x,
                bar_y,
                bar_w * progress,
                bar_h,
                [255, 255, 255, 255],
                sw,
                sh,
            );

            // 4. Percentage Text
            let pct = (progress * 100.0) as i32;
            let pct_str = format!("{}%", pct);
            draw_text(
                &font_texture,
                &pct_str,
                sw / 2.0 - 15.0,
                bar_y + 15.0,
                1.5,
                &texture_ui_shader,
                sw,
                sh,
            );

            unsafe {
                gl::Enable(gl::DEPTH_TEST);
            }
            window.swap_buffers();
            continue;
        }
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Size(width, height) => unsafe {
                    gl::Viewport(0, 0, width, height);
                },
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    if game_state == GameState::Paused {
                        game_state = GameState::Playing;
                        window.set_cursor_mode(glfw::CursorMode::Disabled);
                    } else if game_state == GameState::Playing {
                        if crafting_table_open {
                            ct_close(&mut craft_table_slots, &mut inv_slots);
                            crafting_table_open = false;
                            player.inventory_open = false;
                            if let Some(c) = inv_cursor {
                                inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = None;
                            }
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        } else if player.inventory_open {
                            player.inventory_open = false;
                            if let Some(c) = inv_cursor {
                                inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = None;
                            }
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        } else {
                            game_state = GameState::Paused;
                            window.set_cursor_mode(glfw::CursorMode::Normal);
                        }
                    }
                }
                glfw::WindowEvent::Key(Key::E, _, Action::Press, _) => {
                    if game_state != GameState::Playing {
                        continue;
                    }
                    if crafting_table_open {
                        ct_close(&mut craft_table_slots, &mut inv_slots);
                        crafting_table_open = false;
                        player.inventory_open = false;
                        if let Some(c) = inv_cursor {
                            inv_add(&mut inv_slots, c.block, c.count);
                            inv_cursor = None;
                        }
                        window.set_cursor_mode(glfw::CursorMode::Disabled);
                    } else {
                        player.inventory_open = !player.inventory_open;
                        if player.inventory_open {
                            window.set_cursor_mode(glfw::CursorMode::Normal);
                        } else {
                            if let Some(c) = inv_cursor {
                                inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = None;
                            }
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        }
                    }
                }
                glfw::WindowEvent::Key(Key::F11, _, Action::Press, _) => {
                    is_fullscreen = !is_fullscreen;
                    if is_fullscreen {
                        win_pos = window.get_pos();
                        let (w, h) = window.get_size();
                        win_size = (w as u32, h as u32);
                        glfw.with_primary_monitor(|_, m| {
                            if let Some(monitor) = m {
                                let video_mode = monitor.get_video_mode().unwrap();
                                #[cfg(target_os = "linux")]
                                {
                                    window.set_decorated(false);
                                    window.set_monitor(
                                        glfw::WindowMode::Windowed,
                                        0,
                                        0,
                                        video_mode.width,
                                        video_mode.height,
                                        Some(video_mode.refresh_rate),
                                    );
                                }
                                #[cfg(not(target_os = "linux"))]
                                {
                                    window.set_monitor(
                                        glfw::WindowMode::FullScreen(monitor),
                                        0,
                                        0,
                                        video_mode.width,
                                        video_mode.height,
                                        Some(video_mode.refresh_rate),
                                    );
                                }
                            }
                        });
                    } else {
                        #[cfg(target_os = "linux")]
                        {
                            window.set_decorated(true);
                        }
                        window.set_monitor(
                            glfw::WindowMode::Windowed,
                            win_pos.0,
                            win_pos.1,
                            win_size.0,
                            win_size.1,
                            None,
                        );
                    }
                }
                // Hotbar selection: keys 1-9
                glfw::WindowEvent::Key(Key::Num1, _, Action::Press, _) => {
                    player.selected_slot = 0;
                }
                glfw::WindowEvent::Key(Key::Num2, _, Action::Press, _) => {
                    player.selected_slot = 1;
                }
                glfw::WindowEvent::Key(Key::Num3, _, Action::Press, _) => {
                    player.selected_slot = 2;
                }
                glfw::WindowEvent::Key(Key::Num4, _, Action::Press, _) => {
                    player.selected_slot = 3;
                }
                glfw::WindowEvent::Key(Key::Num5, _, Action::Press, _) => {
                    player.selected_slot = 4;
                }
                glfw::WindowEvent::Key(Key::Num6, _, Action::Press, _) => {
                    player.selected_slot = 5;
                }
                glfw::WindowEvent::Key(Key::Num7, _, Action::Press, _) => {
                    player.selected_slot = 6;
                }
                glfw::WindowEvent::Key(Key::Num8, _, Action::Press, _) => {
                    player.selected_slot = 7;
                }
                glfw::WindowEvent::Key(Key::Num9, _, Action::Press, _) => {
                    player.selected_slot = 8;
                }
                // Q: drop cursor item (inv open) or selected hotbar item (inv closed)
                glfw::WindowEvent::Key(Key::Q, _, Action::Press, _) => {
                    if player.inventory_open {
                        inv_cursor = None;
                    } else {
                        inv_slots[player.selected_slot] = None;
                    }
                }

                glfw::WindowEvent::Scroll(_, dy) if !player.inventory_open => {
                    if dy > 0.0 {
                        player.selected_slot = (player.selected_slot + 8) % 9;
                    } else {
                        player.selected_slot = (player.selected_slot + 1) % 9;
                    }
                }

                glfw::WindowEvent::CursorPos(x, y) => {
                    if game_state == GameState::Playing && !player.inventory_open {
                        let dx = (x - last_cursor_pos.0) as f32;
                        let dy = (y - last_cursor_pos.1) as f32;
                        camera_angle.x -= dx * 0.003;
                        camera_angle.y -= dy * 0.003;
                        camera_angle.y = camera_angle.y.clamp(-1.56, 1.56);
                    }
                    last_cursor_pos = (x, y);
                }
                glfw::WindowEvent::MouseButton(button, action, mods) => {
                    let left = button == glfw::MouseButtonLeft;
                    let right = button == glfw::MouseButtonRight;
                    let shift = mods.contains(glfw::Modifiers::Shift);

                    if crafting_table_open && action == Action::Press {
                        let (win_w, win_h) = window.get_framebuffer_size();
                        let (sw, sh) = (win_w as f32, win_h as f32);
                        let (px, py) = (sw / 2.0 - 190.0, sh / 2.0 - 170.0);
                        let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
                        if let Some(ct_slot) = ct_slot_at_pos(mx, my, px, py) {
                            ct_click(
                                &mut craft_table_slots,
                                &mut inv_slots,
                                &mut inv_cursor,
                                ct_slot,
                                right && !left,
                            );
                        }
                    } else if player.inventory_open && action == Action::Press {
                        let (win_w, win_h) = window.get_framebuffer_size();
                        let (sw, sh) = (win_w as f32, win_h as f32);
                        let (px, py) = (sw / 2.0 - 190.0, sh / 2.0 - 170.0);
                        let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
                        if let Some(slot) = slot_at_pos(mx, my, px, py) {
                            inv_click(&mut inv_slots, &mut inv_cursor, slot, right && !left, shift);
                        }
                    } else if game_state == GameState::Paused && action == Action::Press && left {
                        let inner_win_size = window.get_size();
                        let _sw = inner_win_size.0 as f32;
                        let sh = inner_win_size.1 as f32;
                        let mx = last_cursor_pos.0 as f32;
                        let my = last_cursor_pos.1 as f32;

                        let btn_w = 340.0;
                        let btn_h = 44.0;
                        let btn_x = 100.0;
                        let btn_y_start = sh / 2.0 - 100.0;

                        for i in 0..4 {
                            let y = btn_y_start + i as f32 * (btn_h + 12.0);
                            if mx >= btn_x && mx <= btn_x + btn_w && my >= y && my <= y + btn_h {
                                if i == 0 {
                                    // Resume
                                    game_state = GameState::Playing;
                                    window.set_cursor_mode(glfw::CursorMode::Disabled);
                                } else if i == 3 {
                                    // Save & Quit
                                    window.set_should_close(true);
                                }
                            }
                        }
                    } else if game_state == GameState::Playing && !player.inventory_open {
                        if left {
                            left_mouse_held = action == Action::Press;
                            if action == Action::Release {
                                mining_state.reset();
                            }
                        }
                        if right && action == Action::Press {
                            let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
                            let look_dir = Vec3::new(
                                camera_angle.y.cos() * camera_angle.x.sin(),
                                camera_angle.y.sin(),
                                camera_angle.y.cos() * camera_angle.x.cos(),
                            );
                            let res = world.raycast(eye_pos, look_dir, 8.0);
                            if res.hit {
                                // Right-click CraftingTable block → open 3x3 crafting UI
                                let target_block = world.get_block(res.x, res.y, res.z);
                                if target_block == BlockType::CraftingTable {
                                    crafting_table_open = true;
                                    player.inventory_open = true;
                                    window.set_cursor_mode(glfw::CursorMode::Normal);
                                    continue;
                                }
                            }
                            if res.hit
                                && let Some(s) = &mut inv_slots[player.selected_slot]
                            {
                                if s.block == BlockType::FlintAndSteel {
                                    let target = world.get_block(res.x, res.y, res.z);
                                    if target == BlockType::TNT {
                                        world.set_block(res.x, res.y, res.z, BlockType::Air);
                                        world.explosives.push(crate::block::ActiveExplosive {
                                            position: Vec3::new(
                                                res.x as f32,
                                                res.y as f32,
                                                res.z as f32,
                                            ),
                                            fuse: 4.0,
                                            block_type: BlockType::TNT,
                                        });
                                        continue;
                                    } else if target == BlockType::Nuke {
                                        world.set_block(res.x, res.y, res.z, BlockType::Air);
                                        world.explosives.push(crate::block::ActiveExplosive {
                                            position: Vec3::new(
                                                res.x as f32,
                                                res.y as f32,
                                                res.z as f32,
                                            ),
                                            fuse: 4.0,
                                            block_type: BlockType::Nuke,
                                        });
                                        continue;
                                    }
                                }

                                if !s.block.is_item() {
                                    let (nx, ny, nz) =
                                        (res.x + res.nx, res.y + res.ny, res.z + res.nz);
                                    if world.get_block(nx, ny, nz) == BlockType::Air
                                        && !player.intersects_block(nx, ny, nz)
                                    {
                                        world.set_block(nx, ny, nz, s.block);
                                        s.count -= 1;
                                        if s.count == 0 {
                                            inv_slots[player.selected_slot] = None;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // --- Gamepad input ---
        let mut gp_jump = false;
        let mut gp_sneak = false;
        let mut gp_sprint = false;
        let mut gp_lx = 0.0f32;
        let mut gp_ly = 0.0f32;
        let mut gp_gs: Option<glfw::GamepadState> = None;
        for i in 0..16 {
            let jid = match i {
                0 => JoystickId::Joystick1,
                1 => JoystickId::Joystick2,
                2 => JoystickId::Joystick3,
                3 => JoystickId::Joystick4,
                4 => JoystickId::Joystick5,
                5 => JoystickId::Joystick6,
                6 => JoystickId::Joystick7,
                7 => JoystickId::Joystick8,
                8 => JoystickId::Joystick9,
                9 => JoystickId::Joystick10,
                10 => JoystickId::Joystick11,
                11 => JoystickId::Joystick12,
                12 => JoystickId::Joystick13,
                13 => JoystickId::Joystick14,
                14 => JoystickId::Joystick15,
                15 => JoystickId::Joystick16,
                _ => unreachable!(),
            };
            let js = glfw.get_joystick(jid);
            if js.is_present()
                && let Some(gs) = js.get_gamepad_state()
            {
                gp_gs = Some(gs);
                break;
            }
        }

        if let Some(gs) = gp_gs {
            const DEADZONE: f32 = 0.15;
            const LOOK_SPEED: f32 = 3.0;

            // Right stick: camera look
            if game_state == GameState::Playing && !player.inventory_open {
                let rx = gs.get_axis(GamepadAxis::AxisRightX);
                let ry = gs.get_axis(GamepadAxis::AxisRightY);
                let rx = if rx.abs() > DEADZONE { rx } else { 0.0 };
                let ry = if ry.abs() > DEADZONE { ry } else { 0.0 };
                camera_angle.x -= rx * LOOK_SPEED * delta_time;
                camera_angle.y -= ry * LOOK_SPEED * delta_time;
                camera_angle.y = camera_angle.y.clamp(-1.56, 1.56);
            }

            // Left stick: movement (captured for use below)
            {
                let lx = gs.get_axis(GamepadAxis::AxisLeftX);
                let ly = gs.get_axis(GamepadAxis::AxisLeftY);
                gp_lx = if lx.abs() > DEADZONE { lx } else { 0.0 };
                gp_ly = if ly.abs() > DEADZONE { ly } else { 0.0 };
            }

            // Buttons: A=jump, B=sneak/descend, L3=sprint
            gp_jump = gs.get_button_state(GamepadButton::ButtonA) == Action::Press;
            gp_sneak = gs.get_button_state(GamepadButton::ButtonB) == Action::Press;
            gp_sprint = gs.get_button_state(GamepadButton::ButtonLeftThumb) == Action::Press;

            // Triggers: RT=break block, LT=place block (on press edge)
            let rt = gs.get_axis(GamepadAxis::AxisRightTrigger);
            let lt = gs.get_axis(GamepadAxis::AxisLeftTrigger);

            if !player.inventory_open {
                // Gamepad mining: track held state for RT (break)
                let gp_break_held = rt > 0.5;
                if !gp_break_held {
                    // RT released — reset mining if it was driven by gamepad
                    if !left_mouse_held {
                        mining_state.reset();
                    }
                }

                let gp_place = lt > 0.5 && prev_gp_lt <= 0.5;

                if gp_place {
                    let gp_eye = player.position + Vec3::new(0.0, 1.6, 0.0);
                    let gp_look = Vec3::new(
                        camera_angle.y.cos() * camera_angle.x.sin(),
                        camera_angle.y.sin(),
                        camera_angle.y.cos() * camera_angle.x.cos(),
                    );
                    if gp_place {
                        let res = world.raycast(gp_eye, gp_look, 8.0);
                        if res.hit
                            && let Some(s) = &mut inv_slots[player.selected_slot]
                        {
                            if s.block == BlockType::FlintAndSteel {
                                let target = world.get_block(res.x, res.y, res.z);
                                if target == BlockType::TNT {
                                    world.set_block(res.x, res.y, res.z, BlockType::Air);
                                    world.explosives.push(crate::block::ActiveExplosive {
                                        position: Vec3::new(
                                            res.x as f32,
                                            res.y as f32,
                                            res.z as f32,
                                        ),
                                        fuse: 4.0,
                                        block_type: BlockType::TNT,
                                    });
                                } else if target == BlockType::Nuke {
                                    world.set_block(res.x, res.y, res.z, BlockType::Air);
                                    world.explosives.push(crate::block::ActiveExplosive {
                                        position: Vec3::new(
                                            res.x as f32,
                                            res.y as f32,
                                            res.z as f32,
                                        ),
                                        fuse: 4.0,
                                        block_type: BlockType::Nuke,
                                    });
                                } else if !s.block.is_item() {
                                    let (nx, ny, nz) =
                                        (res.x + res.nx, res.y + res.ny, res.z + res.nz);
                                    if world.get_block(nx, ny, nz) == BlockType::Air {
                                        world.set_block(nx, ny, nz, s.block);
                                        s.count -= 1;
                                        if s.count == 0 {
                                            inv_slots[player.selected_slot] = None;
                                        }
                                    }
                                }
                            } else if !s.block.is_item() {
                                let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
                                if world.get_block(nx, ny, nz) == BlockType::Air {
                                    world.set_block(nx, ny, nz, s.block);
                                    s.count -= 1;
                                    if s.count == 0 {
                                        inv_slots[player.selected_slot] = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            prev_gp_lt = lt;

            // D-Pad + bumpers: hotbar slot selection
            let dpad_r = gs.get_button_state(GamepadButton::ButtonDpadRight) == Action::Press;
            let dpad_l = gs.get_button_state(GamepadButton::ButtonDpadLeft) == Action::Press;
            let rb = gs.get_button_state(GamepadButton::ButtonRightBumper) == Action::Press;
            let lb = gs.get_button_state(GamepadButton::ButtonLeftBumper) == Action::Press;

            if (dpad_r && !prev_gp_dpad_right) || (rb && !prev_gp_rb) {
                player.selected_slot = (player.selected_slot + 1) % 9;
            }
            if (dpad_l && !prev_gp_dpad_left) || (lb && !prev_gp_lb) {
                player.selected_slot = (player.selected_slot + 8) % 9;
            }

            prev_gp_dpad_right = dpad_r;
            prev_gp_dpad_left = dpad_l;
            prev_gp_rb = rb;
            prev_gp_lb = lb;
        }

        let forward = Vec3::new(camera_angle.x.sin(), 0.0, camera_angle.x.cos());
        let right = forward.cross(Vec3::Y);
        let mut move_dir = Vec3::ZERO;
        if game_state == GameState::Playing && !player.inventory_open {
            if window.get_key(Key::W) == Action::Press {
                move_dir += forward;
            }
            if window.get_key(Key::S) == Action::Press {
                move_dir -= forward;
            }
            if window.get_key(Key::A) == Action::Press {
                move_dir -= right;
            }
            if window.get_key(Key::D) == Action::Press {
                move_dir += right;
            }
            // Left stick movement
            move_dir += right * gp_lx + forward * (-gp_ly);
        }
        let is_sprinting = window.get_key(Key::LeftControl) == Action::Press || gp_sprint;
        let is_jumping = window.get_key(Key::Space) == Action::Press || gp_jump;
        let is_sneaking = window.get_key(Key::LeftShift) == Action::Press || gp_sneak;
        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
        let look_dir = Vec3::new(
            camera_angle.y.cos() * camera_angle.x.sin(),
            camera_angle.y.sin(),
            camera_angle.y.cos() * camera_angle.x.cos(),
        );

        if game_state == GameState::Playing {
            player.update(
                &world,
                move_dir,
                delta_time,
                is_sprinting,
                is_jumping,
                is_sneaking,
                current_time,
            );
            world.update(player.position, delta_time as f32);

            // Per-frame mining update
            let gp_rt_held = gp_gs
                .as_ref()
                .is_some_and(|gs| gs.get_axis(GamepadAxis::AxisRightTrigger) > 0.5);
            if (left_mouse_held || gp_rt_held) && !player.inventory_open {
                let held_item = inv_slots[player.selected_slot]
                    .as_ref()
                    .map(|s| s.block)
                    .unwrap_or(BlockType::Air);
                if let Some(mined) =
                    mining_state.update(&world, eye_pos, look_dir, held_item, delta_time)
                {
                    // Block broken — apply drop
                    if mined.drop != BlockType::Air && mined.drop_count > 0 {
                        inv_add(&mut inv_slots, mined.drop, mined.drop_count as u32);
                    }
                    world.set_block(mined.x, mined.y, mined.z, BlockType::Air);

                    // Decrement tool durability
                    if let Some(ref mut s) = inv_slots[player.selected_slot]
                        && s.block.is_tool()
                        && let Some(ref mut dur) = s.durability
                    {
                        *dur = dur.saturating_sub(1);
                        if *dur == 0 {
                            inv_slots[player.selected_slot] = None;
                        }
                    }
                }
            } else {
                mining_state.reset();
            }
        }

        world.update_clouds(player.position, current_time as f32);
        let (sun_angle, sun_y) = {
            let a = (current_time as f32 / 1200.0) * 2.0 * std::f32::consts::PI;
            (a, a.sin())
        };
        let sun_dir = glam::Vec3::new(0.0, sun_y, sun_angle.cos());
        let sky_c = {
            if sun_y > 0.3 {
                glam::Vec4::new(135.0 / 255.0, 206.0 / 255.0, 235.0 / 255.0, 1.0)
            } else if sun_y > -0.2 {
                let t = ((sun_y + 0.2) / 0.5).clamp(0.0, 1.0);
                if t < 0.5 {
                    let u = t * 2.0;
                    glam::Vec4::new(
                        (255.0 * u + 135.0 * (1.0 - u)) / 255.0,
                        (160.0 * u + 100.0 * (1.0 - u)) / 255.0,
                        (180.0 * u + 150.0 * (1.0 - u)) / 255.0,
                        1.0,
                    )
                } else {
                    let u = (t - 0.5) * 2.0;
                    glam::Vec4::new(
                        (135.0 * u + 255.0 * (1.0 - u)) / 255.0,
                        (206.0 * u + 160.0 * (1.0 - u)) / 255.0,
                        (235.0 * u + 180.0 * (1.0 - u)) / 255.0,
                        1.0,
                    )
                }
            } else if sun_y > -0.5 {
                let t = (sun_y + 0.5) / 0.3;
                glam::Vec4::new(
                    (30.0 * t + 10.0 * (1.0 - t)) / 255.0,
                    (30.0 * t + 10.0 * (1.0 - t)) / 255.0,
                    (80.0 * t + 50.0 * (1.0 - t)) / 255.0,
                    1.0,
                )
            } else {
                glam::Vec4::new(5.0 / 255.0, 5.0 / 255.0, 15.0 / 255.0, 1.0)
            }
        };
        target.bind();
        unsafe {
            gl::ClearColor(sky_c.x, sky_c.y, sky_c.z, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
        let aspect = RENDER_WIDTH as f32 / RENDER_HEIGHT as f32;
        let projection = Mat4::perspective_rh_gl(75.0_f32.to_radians(), aspect, 0.1, 1000.0);
        let view = Mat4::look_at_rh(eye_pos, eye_pos + look_dir, Vec3::Y);
        let mvp = projection * view;
        world.render_stars(player.position, current_time as f32, &flat_shader, &mvp);
        draw_sun_moon(&flat_shader, player.position, current_time as f32, mvp);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_mat4(shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
        shader.set_float(shader.get_uniform_location("uTime"), current_time as f32);
        shader.set_vec2(
            shader.get_uniform_location("uResolution"),
            glam::Vec2::new(RENDER_WIDTH as f32, RENDER_HEIGHT as f32),
        );
        shader.set_vec3(shader.get_uniform_location("sunDir"), sun_dir);
        shader.set_vec4(shader.get_uniform_location("colDiffuse"), glam::Vec4::ONE);
        shader.set_vec3(shader.get_uniform_location("viewPos"), eye_pos);
        shader.set_vec4(shader.get_uniform_location("skyCol"), sky_c);
        let frustum = world::Frustum::from_matrix(&mvp);
        world.compute_visible_chunks(&frustum);
        world.render_opaque(&frustum);

        // Render crack overlay on mining target
        if let Some((cx, cy, cz, stage)) = mining_state.crack_stage() {
            let ts = 1.0 / 16.0;
            let u = stage as f32 * ts;
            let v = 3.0 * ts; // crack textures at row 3
            let e = 0.002; // slight expansion to avoid z-fighting
            let x0 = cx as f32 - e;
            let y0 = cy as f32 - e;
            let z0 = cz as f32 - e;
            let x1 = cx as f32 + 1.0 + e;
            let y1 = cy as f32 + 1.0 + e;
            let z1 = cz as f32 + 1.0 + e;

            #[rustfmt::skip]
            let verts: [f32; 108] = [
                // -Z face
                x0,y0,z0, x1,y0,z0, x1,y1,z0, x1,y1,z0, x0,y1,z0, x0,y0,z0,
                // +Z face
                x0,y0,z1, x1,y0,z1, x1,y1,z1, x1,y1,z1, x0,y1,z1, x0,y0,z1,
                // -X face
                x0,y0,z0, x0,y0,z1, x0,y1,z1, x0,y1,z1, x0,y1,z0, x0,y0,z0,
                // +X face
                x1,y0,z0, x1,y0,z1, x1,y1,z1, x1,y1,z1, x1,y1,z0, x1,y0,z0,
                // -Y face
                x0,y0,z0, x1,y0,z0, x1,y0,z1, x1,y0,z1, x0,y0,z1, x0,y0,z0,
                // +Y face
                x0,y1,z0, x1,y1,z0, x1,y1,z1, x1,y1,z1, x0,y1,z1, x0,y1,z0,
            ];
            let u1 = u + ts;
            let v1 = v + ts;
            #[rustfmt::skip]
            let uvs: [f32; 72] = [
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
                u,v1, u1,v1, u1,v, u1,v, u,v, u,v1,
            ];
            // Bind atlas texture so the crack stage tile is sampled
            if let Some(ref atlas) = world.atlas {
                atlas.bind(0);
            }
            unsafe {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::DST_COLOR, gl::ZERO); // multiplicative darkening
                gl::DepthMask(gl::FALSE);
                gl::Enable(gl::POLYGON_OFFSET_FILL);
                gl::PolygonOffset(-1.0, -1.0);
            }
            let crack_mesh = renderer::Mesh::new(&verts, Some(&uvs), None, None);
            crack_mesh.draw();
            unsafe {
                gl::Disable(gl::POLYGON_OFFSET_FILL);
                gl::DepthMask(gl::TRUE);
                // Restore default blend func before disabling
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                gl::Disable(gl::BLEND);
            }
        }

        // Render Explosives and Particles
        world.render_explosives(&shader, &mvp, current_time as f32);
        world.render_particles(&shader, &mvp);

        world.render_clouds(&flat_shader, &mvp);
        shader.bind();
        world.render_transparent(&frustum);

        // Render Water
        water_shader.bind();
        water_shader.set_mat4(water_shader.get_uniform_location("uMVP"), &mvp);
        water_shader.set_mat4(water_shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
        water_shader.set_float(
            water_shader.get_uniform_location("uTime"),
            current_time as f32,
        );
        water_shader.set_vec3(water_shader.get_uniform_location("sunDir"), sun_dir);
        water_shader.set_vec3(water_shader.get_uniform_location("viewPos"), eye_pos);
        water_shader.set_vec4(water_shader.get_uniform_location("skyCol"), sky_c);
        world.render_water(&frustum);
        RenderTexture2D::unbind();
        let (win_width, win_height) = window.get_framebuffer_size();
        unsafe {
            gl::Viewport(0, 0, win_width, win_height);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
        texture_shader.bind();
        draw_texture_quad(&target.texture);

        // Underwater blue screen overlay (below HUD)
        let cam_block = world.get_block(
            eye_pos.x.floor() as i32,
            eye_pos.y.floor() as i32,
            eye_pos.z.floor() as i32,
        );
        if cam_block == BlockType::Water {
            unsafe {
                gl::Enable(gl::BLEND);
                gl::Disable(gl::DEPTH_TEST);
            }
            draw_screen_quad(&color_shader, glam::Vec4::new(0.05, 0.05, 0.2, 0.55));
            unsafe {
                gl::Enable(gl::DEPTH_TEST);
            }
        }

        unsafe {
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::Disable(gl::CULL_FACE);
        }
        ui_shader.bind();
        ui_shader.set_vec2(
            ui_shader.get_uniform_location("uScreenSize"),
            glam::Vec2::new(win_width as f32, win_height as f32),
        );
        let (sw, sh) = (win_width as f32, win_height as f32);
        let hbx = (sw - (9.0 * 40.0 - 4.0)) / 2.0; // 9 slots × 40px stride − last gap
        for (i, inv_slot) in inv_slots.iter().enumerate().take(9) {
            let rx = hbx + i as f32 * 40.0;
            let ry = sh - 46.0;
            // Slot background
            let bg = if player.selected_slot == i {
                [198, 198, 198, 255]
            } else {
                [85, 85, 85, 220]
            };
            draw_rect(&ui_shader, rx, ry, 36.0, 36.0, bg, sw, sh);
            // Slot border (black outline)
            draw_rect(&ui_shader, rx, ry, 36.0, 1.0, [0, 0, 0, 255], sw, sh);
            draw_rect(&ui_shader, rx, ry + 35.0, 36.0, 1.0, [0, 0, 0, 255], sw, sh);
            draw_rect(&ui_shader, rx, ry, 1.0, 36.0, [0, 0, 0, 255], sw, sh);
            draw_rect(&ui_shader, rx + 35.0, ry, 1.0, 36.0, [0, 0, 0, 255], sw, sh);
            // Selected slot: bright white border
            if player.selected_slot == i {
                draw_rect(
                    &ui_shader,
                    rx - 2.0,
                    ry - 2.0,
                    40.0,
                    2.0,
                    [255, 255, 255, 255],
                    sw,
                    sh,
                );
                draw_rect(
                    &ui_shader,
                    rx - 2.0,
                    ry + 36.0,
                    40.0,
                    2.0,
                    [255, 255, 255, 255],
                    sw,
                    sh,
                );
                draw_rect(
                    &ui_shader,
                    rx - 2.0,
                    ry - 2.0,
                    2.0,
                    40.0,
                    [255, 255, 255, 255],
                    sw,
                    sh,
                );
                draw_rect(
                    &ui_shader,
                    rx + 36.0,
                    ry - 2.0,
                    2.0,
                    40.0,
                    [255, 255, 255, 255],
                    sw,
                    sh,
                );
            }
            if let Some(s) = inv_slot {
                draw_item_icon(
                    world.atlas.as_ref().unwrap(),
                    &texture_ui_shader,
                    s.block,
                    rx + 4.0,
                    ry + 4.0,
                    28.0,
                    28.0,
                    sw,
                    sh,
                );
            }
        }
        // Crosshair (hidden when inventory is open)
        if !player.inventory_open {
            draw_rect(
                &ui_shader,
                sw / 2.0 - 2.0,
                sh / 2.0 - 2.0,
                4.0,
                4.4,
                [255, 255, 255, 255],
                sw,
                sh,
            );
        }

        // ── Crafting Table overlay (3x3) ─────────────────────────────────────
        if crafting_table_open {
            draw_screen_quad(&color_shader, glam::Vec4::new(0.0, 0.0, 0.0, 0.6));
            unsafe {
                gl::Disable(gl::DEPTH_TEST);
                gl::Enable(gl::BLEND);
                gl::Disable(gl::CULL_FACE);
            }
            ui_shader.bind();
            ui_shader.set_vec2(
                ui_shader.get_uniform_location("uScreenSize"),
                glam::Vec2::new(sw, sh),
            );

            let (px, py) = (sw / 2.0 - 190.0, sh / 2.0 - 170.0);
            let (pw, ph) = (380.0f32, 340.0f32);

            // Panel bg
            draw_rect(
                &ui_shader,
                px - 2.0,
                py - 2.0,
                pw + 4.0,
                ph + 4.0,
                [55, 55, 55, 255],
                sw,
                sh,
            );
            draw_rect(&ui_shader, px, py, pw, ph, [139, 120, 100, 255], sw, sh);
            draw_rect(
                &ui_shader,
                px + 2.0,
                py + 2.0,
                pw - 4.0,
                ph - 4.0,
                [198, 182, 161, 255],
                sw,
                sh,
            );

            // Separator
            draw_rect(
                &ui_shader,
                px + 6.0,
                py + 158.0,
                pw - 12.0,
                2.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                px + 6.0,
                py + 160.0,
                pw - 12.0,
                1.0,
                [220, 200, 180, 255],
                sw,
                sh,
            );

            // "Crafting" label area
            draw_rect(
                &ui_shader,
                px + 16.0,
                py + 4.0,
                130.0,
                12.0,
                [85, 75, 65, 120],
                sw,
                sh,
            );

            let ss = 36.0f32;
            let st = 38.0f32;
            let draw_slot = |slot_idx: usize, rx: f32, ry: f32| {
                // Slot border (black outline)
                draw_rect(
                    &ui_shader,
                    rx - 1.0,
                    ry - 1.0,
                    ss + 2.0,
                    ss + 2.0,
                    [55, 55, 55, 255],
                    sw,
                    sh,
                );
                // Slot bg
                draw_rect(&ui_shader, rx, ry, ss, ss, [140, 130, 118, 255], sw, sh);
                // Bevel
                draw_rect(&ui_shader, rx, ry, ss, 2.0, [100, 90, 80, 255], sw, sh);
                draw_rect(&ui_shader, rx, ry, 2.0, ss, [100, 90, 80, 255], sw, sh);
                let _ = (slot_idx, st); // suppress unused
            };

            // 3x3 grid slots
            for (i, ct_slot) in craft_table_slots.iter().enumerate().take(9) {
                let (rx, ry, _, _) = ct_slot_rect(i, px, py);
                draw_slot(i, rx, ry);
                if let Some(s) = ct_slot {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        rx + 4.0,
                        ry + 4.0,
                        28.0,
                        28.0,
                        sw,
                        sh,
                    );
                }
            }

            // Arrow between grid and output
            draw_rect(
                &ui_shader,
                px + 150.0,
                py + 56.0,
                40.0,
                4.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                px + 180.0,
                py + 48.0,
                4.0,
                20.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );

            // Output slot
            {
                let (rx, ry, _, _) = ct_slot_rect(9, px, py);
                draw_rect(
                    &ui_shader,
                    rx - 2.0,
                    ry - 2.0,
                    ss + 4.0,
                    ss + 4.0,
                    [55, 55, 55, 255],
                    sw,
                    sh,
                );
                draw_rect(&ui_shader, rx, ry, ss, ss, [165, 155, 140, 255], sw, sh);
                if let Some(s) = craft_table_slots[9] {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        rx + 4.0,
                        ry + 4.0,
                        28.0,
                        28.0,
                        sw,
                        sh,
                    );
                }
            }

            // Main inventory (inv_slots 9-35)
            for i in 0..27 {
                let (rx, ry, _, _) = ct_slot_rect(100 + i, px, py);
                draw_slot(100 + i, rx, ry);
                if let Some(s) = inv_slots[9 + i] {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        rx + 4.0,
                        ry + 4.0,
                        28.0,
                        28.0,
                        sw,
                        sh,
                    );
                }
            }

            // Hotbar (inv_slots 0-8)
            for (i, inv_slot) in inv_slots.iter().enumerate().take(9) {
                let (rx, ry, _, _) = ct_slot_rect(127 + i, px, py);
                draw_slot(127 + i, rx, ry);
                if let Some(s) = inv_slot {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        rx + 4.0,
                        ry + 4.0,
                        28.0,
                        28.0,
                        sw,
                        sh,
                    );
                }
            }

            // Cursor item
            let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
            if let Some(c) = inv_cursor {
                draw_item_icon(
                    world.atlas.as_ref().unwrap(),
                    &texture_ui_shader,
                    c.block,
                    mx - 14.0,
                    my - 14.0,
                    28.0,
                    28.0,
                    sw,
                    sh,
                );
            }
        }
        // ── Inventory overlay (MC 1.0 layout) ─────────────────────────────────
        else if player.inventory_open {
            draw_screen_quad(&color_shader, glam::Vec4::new(0.0, 0.0, 0.0, 0.6));
            unsafe {
                gl::Disable(gl::DEPTH_TEST);
                gl::Enable(gl::BLEND);
                gl::Disable(gl::CULL_FACE);
            }
            ui_shader.bind();
            ui_shader.set_vec2(
                ui_shader.get_uniform_location("uScreenSize"),
                glam::Vec2::new(sw, sh),
            );

            // Panel: 380×340, centered
            let (px, py) = (sw / 2.0 - 190.0, sh / 2.0 - 170.0);
            let (pw, ph) = (380.0f32, 340.0f32);

            // Panel background (MC dark gray)
            draw_rect(
                &ui_shader,
                px - 2.0,
                py - 2.0,
                pw + 4.0,
                ph + 4.0,
                [55, 55, 55, 255],
                sw,
                sh,
            );
            draw_rect(&ui_shader, px, py, pw, ph, [139, 120, 100, 255], sw, sh); // brownish MC
            draw_rect(
                &ui_shader,
                px + 2.0,
                py + 2.0,
                pw - 4.0,
                ph - 4.0,
                [198, 182, 161, 255],
                sw,
                sh,
            ); // lighter inset

            // ── Separator line between upper and lower sections
            draw_rect(
                &ui_shader,
                px + 6.0,
                py + 158.0,
                pw - 12.0,
                2.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                px + 6.0,
                py + 160.0,
                pw - 12.0,
                1.0,
                [220, 200, 180, 255],
                sw,
                sh,
            );

            // ── Player silhouette (left side of upper section)
            let plx = px + 50.0;
            let ply = py + 10.0;
            draw_rect(&ui_shader, plx, ply, 90.0, 140.0, [85, 75, 65, 120], sw, sh); // bg
            // head
            draw_rect(
                &ui_shader,
                plx + 20.0,
                ply + 4.0,
                50.0,
                50.0,
                [140, 95, 65, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                plx + 24.0,
                ply + 14.0,
                14.0,
                10.0,
                [50, 50, 50, 180],
                sw,
                sh,
            ); // eyes
            draw_rect(
                &ui_shader,
                plx + 52.0,
                ply + 14.0,
                14.0,
                10.0,
                [50, 50, 50, 180],
                sw,
                sh,
            );
            // torso
            draw_rect(
                &ui_shader,
                plx + 22.0,
                ply + 56.0,
                46.0,
                40.0,
                [65, 95, 170, 255],
                sw,
                sh,
            );
            // arms
            draw_rect(
                &ui_shader,
                plx + 6.0,
                ply + 56.0,
                14.0,
                38.0,
                [65, 95, 170, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                plx + 70.0,
                ply + 56.0,
                14.0,
                38.0,
                [65, 95, 170, 255],
                sw,
                sh,
            );
            // legs
            draw_rect(
                &ui_shader,
                plx + 22.0,
                ply + 98.0,
                20.0,
                40.0,
                [50, 60, 140, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                plx + 48.0,
                ply + 98.0,
                20.0,
                40.0,
                [50, 60, 140, 255],
                sw,
                sh,
            );

            // ── Armor slots (4 vertical, far left)
            let armor_labels = [
                [120, 140, 200, 180],
                [120, 140, 200, 180],
                [120, 140, 200, 180],
                [120, 140, 200, 180],
            ];
            for i in 0..4usize {
                let (sx, sy, sw2, sh2) = slot_rect(36 + i, px, py);
                draw_rect(&ui_shader, sx, sy, sw2, sh2, [100, 88, 78, 255], sw, sh);
                draw_rect(
                    &ui_shader,
                    sx + 2.0,
                    sy + 2.0,
                    sw2 - 4.0,
                    sh2 - 4.0,
                    [75, 65, 58, 180],
                    sw,
                    sh,
                );
                // Armor piece color hint
                let c = armor_labels[i];
                draw_rect(&ui_shader, sx + 8.0, sy + 8.0, 20.0, 20.0, c, sw, sh);
                if let Some(s) = inv_slots[36 + i] {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        sx + 4.0,
                        sy + 4.0,
                        sw2 - 8.0,
                        sh2 - 8.0,
                        sw,
                        sh,
                    );
                }
            }

            // ── Crafting 2×2 + output
            for (i, inv_slot) in inv_slots.iter().enumerate().take(44).skip(40) {
                let (sx, sy, sw2, sh2) = slot_rect(i, px, py);
                let hov = {
                    let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
                    mx >= sx && mx < sx + sw2 && my >= sy && my < sy + sh2
                };
                let bg = if hov {
                    [130, 115, 100, 255]
                } else {
                    [100, 88, 78, 255]
                };
                draw_rect(&ui_shader, sx, sy, sw2, sh2, bg, sw, sh);
                draw_rect(
                    &ui_shader,
                    sx + 2.0,
                    sy + 2.0,
                    sw2 - 4.0,
                    sh2 - 4.0,
                    [75, 65, 58, 200],
                    sw,
                    sh,
                );
                if let Some(s) = inv_slot {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        sx + 4.0,
                        sy + 4.0,
                        sw2 - 8.0,
                        sh2 - 8.0,
                        sw,
                        sh,
                    );
                }
            }
            // Arrow → between craft and output
            draw_rect(
                &ui_shader,
                px + 279.0,
                py + 45.0,
                18.0,
                8.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                px + 291.0,
                py + 38.0,
                6.0,
                22.0,
                [85, 75, 65, 255],
                sw,
                sh,
            );
            // Craft output slot (slot 44) – larger
            {
                let (sx, sy, sw2, sh2) = slot_rect(44, px, py);
                let hov = {
                    let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
                    mx >= sx && mx < sx + sw2 && my >= sy && my < sy + sh2
                };
                let bg = if hov {
                    [145, 130, 110, 255]
                } else {
                    [110, 98, 85, 255]
                };
                draw_rect(
                    &ui_shader,
                    sx - 2.0,
                    sy - 2.0,
                    sw2 + 4.0,
                    sh2 + 4.0,
                    [55, 45, 35, 255],
                    sw,
                    sh,
                );
                draw_rect(&ui_shader, sx, sy, sw2, sh2, bg, sw, sh);
                if let Some(s) = inv_slots[44] {
                    draw_item_icon(
                        world.atlas.as_ref().unwrap(),
                        &texture_ui_shader,
                        s.block,
                        sx + 4.0,
                        sy + 4.0,
                        sw2 - 8.0,
                        sh2 - 8.0,
                        sw,
                        sh,
                    );
                }
            }

            // ── Helper closure to draw a slot
            let (mx, my) = (last_cursor_pos.0 as f32, last_cursor_pos.1 as f32);
            let draw_slot = |shader: &Shader,
                             tex_shader: &Shader,
                             atlas: &Texture2D,
                             sx: f32,
                             sy: f32,
                             ss: f32,
                             item: Option<ItemStack>,
                             hov: bool,
                             sw: f32,
                             sh: f32| {
                let bg = if hov && item.is_some() {
                    [145, 130, 110, 255]
                } else {
                    [100, 88, 78, 255]
                };
                draw_rect(shader, sx, sy, ss, ss, bg, sw, sh);
                draw_rect(
                    shader,
                    sx + 2.0,
                    sy + 2.0,
                    ss - 4.0,
                    ss - 4.0,
                    [75, 65, 55, 200],
                    sw,
                    sh,
                );
                if let Some(s) = item {
                    draw_item_icon(
                        atlas,
                        tex_shader,
                        s.block,
                        sx + 4.0,
                        sy + 4.0,
                        ss - 8.0,
                        ss - 8.0,
                        sw,
                        sh,
                    );
                    // Count dots (up to 8 = 64)
                    let dots = ((s.count as f32 / 8.0).ceil() as usize).clamp(1, 8);
                    let dsz = 4.0;
                    let dgap = 2.0;
                    let dw = dots as f32 * (dsz + dgap) - dgap;
                    let dx0 = sx + ss - dw - 3.0;
                    let dy0 = sy + ss - dsz - 3.0;
                    for d in 0..dots {
                        draw_rect(
                            shader,
                            dx0 + d as f32 * (dsz + dgap),
                            dy0,
                            dsz,
                            dsz,
                            [255, 255, 255, 200],
                            sw,
                            sh,
                        );
                    }
                }
            };

            // ── Main inventory (slots 9-35, 3×9)
            for (slot, inv_slot) in inv_slots.iter().enumerate().take(36).skip(9) {
                let (sx, sy, ss, _) = slot_rect(slot, px, py);
                let hov = mx >= sx && mx < sx + ss && my >= sy && my < sy + ss;
                draw_slot(
                    &ui_shader,
                    &texture_ui_shader,
                    world.atlas.as_ref().unwrap(),
                    sx,
                    sy,
                    ss,
                    *inv_slot,
                    hov,
                    sw,
                    sh,
                );
            }
            // ── Hotbar row in inventory (slots 0-8)
            for (slot, inv_slot) in inv_slots.iter().enumerate().take(9) {
                let (sx, sy, ss, _) = slot_rect(slot, px, py);
                let hov = mx >= sx && mx < sx + ss && my >= sy && my < sy + ss;
                // Highlight selected
                if player.selected_slot == slot {
                    draw_rect(
                        &ui_shader,
                        sx - 2.0,
                        sy - 2.0,
                        ss + 4.0,
                        ss + 4.0,
                        [255, 255, 255, 180],
                        sw,
                        sh,
                    );
                }
                draw_slot(
                    &ui_shader,
                    &texture_ui_shader,
                    world.atlas.as_ref().unwrap(),
                    sx,
                    sy,
                    ss,
                    *inv_slot,
                    hov,
                    sw,
                    sh,
                );
            }

            // ── Cursor item (held on mouse)
            if let Some(c) = inv_cursor {
                draw_item_icon(
                    world.atlas.as_ref().unwrap(),
                    &texture_ui_shader,
                    c.block,
                    mx - 14.0,
                    my - 14.0,
                    28.0,
                    28.0,
                    sw,
                    sh,
                );
                // Count dots
                let dots = ((c.count as f32 / 8.0).ceil() as usize).clamp(1, 8);
                let dsz = 4.0;
                let dgap = 2.0;
                let dw = dots as f32 * (dsz + dgap) - dgap;
                for d in 0..dots {
                    draw_rect(
                        &ui_shader,
                        mx - 14.0 + 28.0 - dw + d as f32 * (dsz + dgap),
                        my + 8.0,
                        dsz,
                        dsz,
                        [255, 255, 255, 255],
                        sw,
                        sh,
                    );
                }
            }
        }

        // Draw Health and Oxygen
        if !player.inventory_open {
            let heart_size = 24.0;
            let bubble_size = 20.0;
            let spacing = 4.0;
            let w_off = (10.0 * (heart_size + spacing)) / 2.0;

            // Health (left side of center)
            let hx_start = sw / 2.0 - w_off - 80.0;
            let hy = sh - 80.0;
            for i in 0..10 {
                let rx = hx_start + i as f32 * (heart_size + spacing);
                let heart_val = (player.health - i * 2).clamp(0, 2);
                // Currently only drawing full or empty hearts
                draw_heart(&ui_shader, rx, hy, heart_size, sw, sh, heart_val == 0);
            }

            // Oxygen bubbles (right side of center)
            let head_in_w = world.get_block(
                eye_pos.x.floor() as i32,
                eye_pos.y.floor() as i32,
                eye_pos.z.floor() as i32,
            ) == BlockType::Water;
            if head_in_w || player.air_seconds < 15.0 {
                let ox_start = sw / 2.0 + 80.0;
                let oy = hy;
                let bubbles_left = (player.air_seconds / 1.5).ceil() as i32;

                for i in 0..10 {
                    let rx = ox_start + i as f32 * (bubble_size + spacing);
                    let empty = i >= bubbles_left;
                    draw_bubble(&ui_shader, rx, oy, bubble_size, sw, sh, empty);
                }
            }
        }

        if game_state == GameState::Paused {
            let (win_width, win_height) = window.get_size();
            draw_pause_menu(
                &ui_shader,
                &texture_ui_shader,
                &font_texture,
                win_width as f32,
                win_height as f32,
            );
        }

        unsafe {
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::CULL_FACE);
        }
        window.swap_buffers();
    }
    let (xw, yw) = window.get_pos();
    let (ww, hw) = window.get_size();
    let maximized = window.is_maximized();
    let state = WindowState {
        width: ww as u32,
        height: hw as u32,
        x: xw,
        y: yw,
        maximized,
    };
    state.save();
}
