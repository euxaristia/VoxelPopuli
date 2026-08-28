use glfw::{Action, GamepadAxis, GamepadButton, JoystickId, Key};
mod atlas;
mod atlas_table;
mod block;
mod chunk;
mod crafting;
mod explosion;
mod hud;
mod inventory;
mod item;
mod java_compat;
mod mining;
mod mob;
mod noise;
mod player;
mod profiler;
mod renderer;
mod save;
mod vibrant;
mod village;
mod world;

use crate::hud::*;
use crate::inventory::*;
use crate::player::Player;
use crate::save::{GameSave, GameSettings, SAVE_FILE};
use crate::world::{CLOUD_HEIGHT, World};
use block::BlockType;
use glam::{Mat4, Vec2, Vec3};
use profiler::FrameProfiler;

// The system allocator's per-call overhead on Windows dominates
// MeshSnapshot::capture (~1.2ms/chunk just to allocate its ~249KB of
// scratch buffers, up to 64x/frame during chunk streaming -- see the
// flying-forward stutter investigation). mimalloc's thread-local free
// lists make that churn far cheaper.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(PartialEq)]
pub enum GameState {
    Loading,
    Playing,
    Paused,
}

// shader imports
use renderer::{RenderTexture2D, Shader, Texture2D};

// const WINDOW_WIDTH: u32 = 1920;
// const WINDOW_HEIGHT: u32 = 1080;
const RENDER_WIDTH: i32 = 1125;
const RENDER_HEIGHT: i32 = 633;

fn render_target_size_for_framebuffer(width: i32, height: i32) -> (i32, i32) {
    // Minimized or degenerate framebuffers (e.g. 3840x0) would otherwise
    // produce an extreme aspect and an absurdly wide target texture.
    if width <= 0 || height <= 0 {
        return (RENDER_WIDTH, RENDER_HEIGHT);
    }
    let aspect = width as f32 / height as f32;
    let pixel_budget = (RENDER_WIDTH * RENDER_HEIGHT) as f32;
    let target_height = (pixel_budget / aspect).sqrt().round().max(1.0) as i32;
    let target_width = (target_height as f32 * aspect).round().max(1.0) as i32;
    // Never exceed the actual framebuffer size
    (target_width.min(width), target_height.min(height))
}

#[cfg(test)]
mod render_target_tests {
    use super::*;

    fn aspect_error(size: (i32, i32), width: i32, height: i32) -> f32 {
        (size.0 as f32 / size.1 as f32 - width as f32 / height as f32).abs()
    }

    #[test]
    fn render_target_keeps_default_resolution_for_default_aspect() {
        assert_eq!(
            render_target_size_for_framebuffer(RENDER_WIDTH, RENDER_HEIGHT),
            (RENDER_WIDTH, RENDER_HEIGHT)
        );
    }

    #[test]
    fn render_target_tracks_tall_framebuffer_aspect() {
        let size = render_target_size_for_framebuffer(1280, 1380);
        assert!(aspect_error(size, 1280, 1380) < 0.002);
    }

    #[test]
    fn render_target_tracks_wide_framebuffer_aspect() {
        let size = render_target_size_for_framebuffer(2560, 1080);
        assert!(aspect_error(size, 2560, 1080) < 0.002);
    }

    #[test]
    fn render_target_falls_back_for_degenerate_framebuffers() {
        assert_eq!(
            render_target_size_for_framebuffer(3840, 0),
            (RENDER_WIDTH, RENDER_HEIGHT)
        );
        assert_eq!(
            render_target_size_for_framebuffer(0, 0),
            (RENDER_WIDTH, RENDER_HEIGHT)
        );
        assert_eq!(
            render_target_size_for_framebuffer(-1, 720),
            (RENDER_WIDTH, RENDER_HEIGHT)
        );
    }

    #[test]
    fn render_target_never_exceeds_framebuffer() {
        let (w, h) = render_target_size_for_framebuffer(320, 200);
        assert!(w <= 320 && h <= 200);
        assert!(w >= 1 && h >= 1);
    }
}

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

fn load_shader(path: &str) -> String {
    std::fs::read_to_string(format!("assets/shaders/{path}"))
        .unwrap_or_else(|e| panic!("Failed to load shader {path}: {e}"))
}

fn seed_from_text(text: &str) -> i64 {
    let trimmed = text.trim();
    if let Ok(seed) = trimmed.parse::<i64>() {
        return seed;
    }

    let mut hash: i32 = 0;
    for unit in trimmed.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(unit as i32);
    }
    hash as i64
}

fn resolve_world_seed() -> i64 {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(seed) = arg.strip_prefix("--seed=") {
            return seed_from_text(seed);
        }
        if arg == "--seed"
            && let Some(seed) = args.next()
        {
            return seed_from_text(&seed);
        }
    }

    if let Ok(seed) = std::env::var("VOXELPOPULI_SEED")
        && !seed.trim().is_empty()
    {
        return seed_from_text(&seed);
    }

    rand::random::<i64>()
}

fn draw_screen_quad(shader: &Shader, color: glam::Vec4) {
    let v = [
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0,
        0.0,
    ];
    let mesh = renderer::Mesh::new(&v, None, None, None);
    shader.bind();
    shader.set_vec4(shader.get_uniform_location("uColor"), color);
    renderer::set_depth_test(false);
    mesh.draw();
    renderer::set_depth_test(true);
}

fn draw_sun_moon(shader: &Shader, player_pos: Vec3, time: f32, mvp: Mat4, overlay: bool) {
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

    // The forward path draws the sky before the world, so it must not
    // depth test; the deferred path draws it after the resolve and does.
    renderer::set_depth_test(!overlay);
    renderer::set_cull(false);

    shader.set_int(shader.get_uniform_location("uBodyType"), 1); // 1 = Sun
    sun_mesh.draw();

    shader.set_int(shader.get_uniform_location("uBodyType"), 2); // 2 = Moon
    moon_mesh.draw();

    shader.set_int(shader.get_uniform_location("uBodyType"), 0); // Reset
    renderer::set_cull(true);
    renderer::set_depth_test(true);
}

fn args_force_new_world(args: &[String]) -> bool {
    args.iter().any(|arg| {
        arg == "--seed"
            || arg.starts_with("--seed=")
            || arg == "--import-world"
            || arg.starts_with("--import-world=")
    })
}

fn main() {
    // Entry point
    let args: Vec<String> = std::env::args().collect();
    let mut loaded_save = if args_force_new_world(&args) {
        None
    } else {
        GameSave::read_from(std::path::Path::new(SAVE_FILE)).ok()
    };
    let world_seed = loaded_save
        .as_ref()
        .map(|save| save.seed as i64)
        .unwrap_or_else(resolve_world_seed);
    if let Some(config) = java_compat::ExportConfig::from_args() {
        match java_compat::export_classic_java_world(world_seed as u64, &config) {
            Ok(summary) => {
                println!(
                    "Exported {} chunks across {} region file(s) as {} to {}",
                    summary.chunks,
                    summary.regions,
                    java_compat::TARGET_NAME,
                    config.output_dir.display()
                );
            }
            Err(err) => {
                eprintln!("Failed to export Java 1.17 world: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    let mut import_world_path = None;
    if let Some(import_idx) = args.iter().position(|a| a == "--import-world") {
        let Some(path) = args.get(import_idx + 1) else {
            eprintln!("--import-world requires a world directory path");
            std::process::exit(1);
        };
        import_world_path = Some(std::path::PathBuf::from(path));
    } else if let Some(save) = &loaded_save {
        import_world_path = save.import_world.clone();
    }
    if let Some(path) = &import_world_path {
        match java_compat::validate_classic_java_world(path) {
            Ok(regions) => {
                println!(
                    "Streaming Minecraft Java chunks from {regions} region file(s) in {}",
                    path.display()
                );
            }
            Err(err) => {
                eprintln!(
                    "Failed to import Minecraft Java world from {}: {err}",
                    path.display()
                );
                std::process::exit(1);
            }
        }
    }

    let state = WindowState::load();
    let window_title = format!("VoxelPopuli Rust - Seed {world_seed}");
    let mut glfw = glfw::init(glfw::log_errors).unwrap();
    // Add DualSense mappings for Linux
    glfw.update_gamepad_mappings("030000004c050000e60d000011010000,PS5 Controller,a:b0,b:b1,back:b8,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b10,leftshoulder:b4,leftstick:b11,lefttrigger:a3,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:a4,rightx:a2,righty:a5,start:b9,x:b2,y:b3,platform:Linux,\n\
                                  050000004c050000e60d0000ff070000,PS5 Controller,a:b0,b:b1,back:b8,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b10,leftshoulder:b4,leftstick:b11,lefttrigger:a3,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:a4,rightx:a2,righty:a5,start:b9,x:b2,y:b3,platform:Linux,");
    // wgpu owns the GPU; tell GLFW not to create a GL context.
    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw.window_hint(glfw::WindowHint::AutoIconify(false));
    let (mut window, events) = glfw
        .create_window(
            state.width,
            state.height,
            &window_title,
            glfw::WindowMode::Windowed,
        )
        .expect("Failed to create GLFW window.");
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
    let (init_fb_w, init_fb_h) = window.get_framebuffer_size();
    renderer::init(&*window, init_fb_w, init_fb_h);
    let mut world = World::new(world_seed as u64);
    if let Some(path) = import_world_path.clone() {
        world.set_import_world(path);
    }
    if let Some(save) = &loaded_save {
        world.install_edits(&save.edits);
    }
    world.generate_atlas();
    world.init_celestial();
    match village::nearest_village(world_seed as u64, 32, 32, 8) {
        Some(v) => {
            let dist = (((v.center_x - 32).pow(2) + (v.center_z - 32).pow(2)) as f64).sqrt();
            println!(
                "Nearest village: X={} Z={} — {:.0} blocks from spawn{}",
                v.center_x,
                v.center_z,
                dist,
                if v.desert { " (desert)" } else { "" }
            );
        }
        None => println!("No village within 8 regions of spawn for this seed"),
    }
    let shader = Shader::new(&load_shader("ps1.wgsl")).expect("Failed to compile PS1 shader");
    // Deferred geometry pass: same uniforms as ps1, four color attachments.
    let gbuffer_shader = Shader::with_outputs(&load_shader("gbuffer.wgsl"), 4)
        .expect("Failed to compile G-buffer shader");
    let vibrant_pack = vibrant::VibrantPack::load_default();
    for warning in &vibrant_pack.warnings {
        eprintln!("Vibrant Visuals pack: {warning}");
    }
    let water_shader =
        Shader::new(&load_shader("water.wgsl")).expect("Failed to compile water shader");
    let flat_shader =
        Shader::new(&load_shader("flat.wgsl")).expect("Failed to compile FLAT shader");
    let ui_shader = Shader::new(&load_shader("ui.wgsl")).expect("Failed to compile UI shader");
    let texture_shader =
        Shader::new(&load_shader("texture.wgsl")).expect("Failed to compile TEXTURE shader");
    let texture_ui_shader =
        Shader::new(&load_shader("ui_texture.wgsl")).expect("Failed to compile UI TEXTURE shader");
    let color_shader =
        Shader::new(&load_shader("color.wgsl")).expect("Failed to compile COLOR shader");
    let (initial_fb_width, initial_fb_height) = window.get_framebuffer_size();
    let (target_width, target_height) =
        render_target_size_for_framebuffer(initial_fb_width, initial_fb_height);
    let mut target = RenderTexture2D::new(target_width, target_height);
    let font_texture = Texture2D::from_file("assets/font.png");

    let mut is_fullscreen = false;
    let mut win_pos = (state.x, state.y);
    let mut win_size = (state.width, state.height);
    let mut show_debug_overlay = false;

    let mut game_state = GameState::Loading;
    let mut spawn_y = 150.0;
    let mut inv_slots = create_starting_inventory();
    let mut inv_cursor: Option<ItemStack> = None;
    let mut player = Player::new(spawn_y);
    let mut camera_angle = Vec2::new(std::f32::consts::PI, 0.0);
    let mut last_cursor_pos = window.get_cursor_pos();
    let mut mining_state = mining::MiningState::new();
    let mut left_mouse_held = false;
    let mut right_mouse_held = false;
    let mut bow_charge = 0.0f32;
    let mut crafting_table_open = false;
    let mut craft_table_slots = [None::<ItemStack>; 10]; // 0-8: 3x3 grid, 9: output
    let mut pause_sub_menu = PauseSubMenu::Main;
    let settings = loaded_save
        .as_ref()
        .map(|save| save.settings.clamped())
        .unwrap_or_default();
    let mut render_dist_setting = settings.view_distance;
    let mut fov_setting = settings.fov;
    let mut fancy_gfx_setting = settings.fancy_graphics;
    let mut export_status_msg = String::new();
    let mut selected_skin = settings.selected_skin;
    world.set_view_distance(render_dist_setting);
    let mut placement_lock: Option<LinearPlacementLock> = None;
    let mut place_repeat_timer = 0.0f32;
    let mut prev_gp_lt = -1.0f32;
    let mut prev_gp_dpad_right = false;
    let mut prev_gp_dpad_left = false;
    let mut prev_gp_rb = false;
    let mut prev_gp_lb = false;
    let mut last_time = glfw.get_time();
    let mut fps_last_time = last_time;
    let mut fps_frames: u32 = 0;
    let mut current_fps: f32 = 0.0;
    let mut profiler = FrameProfiler::new();
    while !window.should_close() {
        let frame_start = std::time::Instant::now();
        let current_time = glfw.get_time();
        let delta_time = (current_time - last_time) as f32;
        last_time = current_time;

        fps_frames += 1;
        let fps_elapsed = current_time - fps_last_time;
        if fps_elapsed >= 1.0 {
            current_fps = (fps_frames as f64 / fps_elapsed) as f32;
            fps_frames = 0;
            fps_last_time = current_time;
            window.set_title(&format!(
                "VoxelPopuli Rust - Seed {} - {:.0} FPS",
                world_seed, current_fps
            ));
        }
        glfw.poll_events();

        if game_state == GameState::Loading {
            // Update world incrementally
            world.update(Vec3::new(32.5, 0.0, 32.5), current_time as f32);
            if !world.is_loading {
                if let Some(save) = loaded_save.take() {
                    player = save.restore_player();
                    inv_slots = save.inventory;
                    camera_angle = save.camera_angle;
                    spawn_y = player.position.y;
                } else {
                    for y in (0..255).rev() {
                        if world.get_block(32, y, 32) != BlockType::Air {
                            spawn_y = y as f32 + 2.0;
                            break;
                        }
                    }
                    player.position = Vec3::new(32.5, spawn_y, 32.5);
                }
                game_state = GameState::Playing;
            }

            let (win_sw, win_sh) = window.get_size();
            let sw = win_sw as f32;
            let sh = win_sh as f32;

            // Render Loading Screen
            renderer::clear(0.117, 0.117, 0.117, 1.0); // Exact #1e1e1e
            renderer::set_depth_test(false);
            renderer::set_cull(false);
            renderer::set_blend(true);

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

            // Progress Fill (tracks fully meshed & GPU ready chunks)
            let total_chunks =
                (world.view_distance * 2 + 1) as f32 * (world.view_distance * 2 + 1) as f32;
            let ready_chunks = world
                .chunks
                .iter()
                .flatten()
                .filter(|c| !c.dirty && !c.meshing_in_progress)
                .count();
            let progress = (ready_chunks as f32 / total_chunks).clamp(0.0, 1.0);
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

            renderer::set_depth_test(true);
            let (fb_w, fb_h) = window.get_framebuffer_size();
            renderer::end_frame(fb_w, fb_h);
            continue;
        }
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                // Surface size follows the framebuffer in renderer::end_frame.
                glfw::WindowEvent::Size(..) => {}
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    if game_state == GameState::Paused {
                        if pause_sub_menu != PauseSubMenu::Main {
                            pause_sub_menu = PauseSubMenu::Main;
                        } else {
                            game_state = GameState::Playing;
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        }
                    } else if game_state == GameState::Playing {
                        if crafting_table_open {
                            ct_close(&mut craft_table_slots, &mut inv_slots);
                            crafting_table_open = false;
                            player.inventory_open = false;
                            if let Some(c) = inv_cursor {
                                let remaining = inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = (remaining > 0).then(|| c.with_count(remaining));
                            }
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        } else if player.inventory_open {
                            player.inventory_open = false;
                            if let Some(c) = inv_cursor {
                                let remaining = inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = (remaining > 0).then(|| c.with_count(remaining));
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
                            let remaining = inv_add(&mut inv_slots, c.block, c.count);
                            inv_cursor = (remaining > 0).then(|| c.with_count(remaining));
                        }
                        window.set_cursor_mode(glfw::CursorMode::Disabled);
                    } else {
                        player.inventory_open = !player.inventory_open;
                        if player.inventory_open {
                            window.set_cursor_mode(glfw::CursorMode::Normal);
                        } else {
                            if let Some(c) = inv_cursor {
                                let remaining = inv_add(&mut inv_slots, c.block, c.count);
                                inv_cursor = (remaining > 0).then(|| c.with_count(remaining));
                            }
                            window.set_cursor_mode(glfw::CursorMode::Disabled);
                        }
                    }
                }
                glfw::WindowEvent::Key(Key::F3, _, Action::Press, _) => {
                    show_debug_overlay = !show_debug_overlay;
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
                        let sw = inner_win_size.0 as f32;
                        let sh = inner_win_size.1 as f32;
                        let mx = last_cursor_pos.0 as f32;
                        let my = last_cursor_pos.1 as f32;
                        if let Some(click) = pause_click(pause_sub_menu, sw, sh, mx, my) {
                            match click {
                                PauseClick::Resume => {
                                    game_state = GameState::Playing;
                                    window.set_cursor_mode(glfw::CursorMode::Disabled);
                                }
                                PauseClick::OpenSettings => {
                                    pause_sub_menu = PauseSubMenu::Settings;
                                }
                                PauseClick::OpenWorldInfo => {
                                    pause_sub_menu = PauseSubMenu::WorldInfo;
                                }
                                PauseClick::OpenProfile => {
                                    pause_sub_menu = PauseSubMenu::Profile;
                                }
                                PauseClick::SaveQuit => {
                                    window.set_should_close(true);
                                }
                                PauseClick::Back => {
                                    pause_sub_menu = PauseSubMenu::Main;
                                }
                                PauseClick::SetRenderDistance(distance) => {
                                    render_dist_setting = distance;
                                    world.set_view_distance(distance);
                                }
                                PauseClick::SetFov(fov) => {
                                    fov_setting = fov;
                                }
                                PauseClick::ToggleFancy => {
                                    fancy_gfx_setting = !fancy_gfx_setting;
                                }
                                PauseClick::ExportJava => {
                                    let config = java_compat::ExportConfig {
                                        output_dir: std::path::PathBuf::from("java17_world"),
                                        radius: 4,
                                    };
                                    match java_compat::export_classic_java_chunks(
                                        world.seed,
                                        &config,
                                        |cx, cz| world.chunk_for_export(cx, cz),
                                    ) {
                                        Ok(summary) => {
                                            export_status_msg =
                                                format!("Exported {} live chunks!", summary.chunks)
                                        }
                                        Err(e) => export_status_msg = format!("Export failed: {e}"),
                                    }
                                }
                                PauseClick::SelectSkin(skin) => {
                                    selected_skin = skin;
                                }
                            }
                        }
                    } else if game_state == GameState::Playing && !player.inventory_open {
                        if left {
                            left_mouse_held = action == Action::Press;
                            if action == Action::Press && player.attack_cooldown <= 0.0 {
                                let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
                                let look_dir = Vec3::new(
                                    camera_angle.y.cos() * camera_angle.x.sin(),
                                    camera_angle.y.sin(),
                                    camera_angle.y.cos() * camera_angle.x.cos(),
                                );
                                let held = inv_slots[player.selected_slot]
                                    .map(|s| s.block)
                                    .unwrap_or(BlockType::Air);
                                if world.try_melee(eye_pos, look_dir, item::melee_damage(held), 4.0)
                                {
                                    player.attack_cooldown = 0.4;
                                    left_mouse_held = false;
                                    mining_state.reset();
                                }
                            }
                            if action == Action::Release {
                                mining_state.reset();
                            }
                        }
                        if right {
                            if action == Action::Press {
                                right_mouse_held = true;
                                place_repeat_timer = 0.0;
                            } else if action == Action::Release {
                                right_mouse_held = false;
                                placement_lock = None;
                                place_repeat_timer = 0.0;
                                if bow_charge > 0.1
                                    && inv_slots[player.selected_slot]
                                        .map_or(false, |s| s.block == BlockType::Bow)
                                {
                                    if let Some(arrow_idx) = inv_slots.iter().position(|slot| {
                                        slot.map_or(false, |s| s.block == BlockType::Arrow)
                                    }) {
                                        let s = inv_slots[arrow_idx].as_mut().unwrap();
                                        s.count -= 1;
                                        if s.count == 0 {
                                            inv_slots[arrow_idx] = None;
                                        }
                                        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
                                        let look_dir = Vec3::new(
                                            camera_angle.y.cos() * camera_angle.x.sin(),
                                            camera_angle.y.sin(),
                                            camera_angle.y.cos() * camera_angle.x.cos(),
                                        )
                                        .normalize();
                                        let speed = bow_charge * 30.0;
                                        world.arrows.push(crate::block::ArrowEntity {
                                            position: eye_pos + look_dir * 0.5,
                                            velocity: look_dir * speed,
                                            life: 60.0,
                                            in_ground: false,
                                            damage: (bow_charge * 9.0 + 1.0).round(),
                                            is_critical: bow_charge >= 1.0,
                                            from_player: true,
                                        });
                                    }
                                }
                                bow_charge = 0.0;
                            }
                        }

                        if right && action == Action::Press {
                            if let Some(s) = &mut inv_slots[player.selected_slot] {
                                if let Some(food_props) = item::food_properties(s.block)
                                    && player.eat_food(food_props)
                                {
                                    s.count -= 1;
                                    if s.count == 0 {
                                        inv_slots[player.selected_slot] = None;
                                    }
                                    continue;
                                }
                                if item::armor_properties(s.block).is_some() {
                                    let armor_item = s.block;
                                    s.count -= 1;
                                    if s.count == 0 {
                                        inv_slots[player.selected_slot] = None;
                                    }
                                    if let Some(old_armor) = player.equip_armor(armor_item) {
                                        inv_slots[player.selected_slot] =
                                            Some(ItemStack::new(old_armor, 1));
                                    }
                                    continue;
                                }
                            }

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
                                if target_block == BlockType::Bed {
                                    player.spawn_point = Some(Vec3::new(
                                        res.x as f32,
                                        (res.y + 1) as f32,
                                        res.z as f32,
                                    ));
                                    continue;
                                }
                            }
                            if res.hit {
                                if try_place_block_with_lock(
                                    &mut world,
                                    &mut inv_slots,
                                    player.selected_slot,
                                    eye_pos,
                                    look_dir,
                                    &player,
                                    &mut placement_lock,
                                ) {
                                    place_repeat_timer = 0.20;
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
                    if try_place_block_with_lock(
                        &mut world,
                        &mut inv_slots,
                        player.selected_slot,
                        gp_eye,
                        gp_look,
                        &player,
                        &mut placement_lock,
                    ) {
                        place_repeat_timer = 0.20;
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
            if right_mouse_held
                && inv_slots[player.selected_slot].map_or(false, |s| s.block == BlockType::Bow)
                && inv_slots
                    .iter()
                    .any(|slot| slot.map_or(false, |s| s.block == BlockType::Arrow))
            {
                bow_charge = (bow_charge + delta_time * 1.5).min(1.0);
            } else if !right_mouse_held {
                bow_charge = 0.0;
            }

            player.update(
                &world,
                move_dir,
                delta_time,
                is_sprinting,
                is_jumping,
                is_sneaking,
                current_time,
            );
            let t_wu = std::time::Instant::now();
            let earned_xp = world.update(player.position, delta_time as f32);
            profiler.world_update_ms = t_wu.elapsed().as_secs_f32() * 1000.0;
            if earned_xp > 0 {
                player.add_xp(earned_xp);
            }
            for (block, count) in world.pending_drops.drain(..) {
                inv_add(&mut inv_slots, block, count);
            }
            if world.pending_hurt > 0 {
                player.take_damage(world.pending_hurt);
                world.pending_hurt = 0;
            }
            if player.health <= 0 {
                player.respawn(Vec3::new(32.5, spawn_y, 32.5));
            }

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
                    let target_b = world.get_block(mined.x, mined.y, mined.z);
                    let xp_val = match target_b {
                        BlockType::CoalOre => 1,
                        BlockType::DiamondOre => 5,
                        BlockType::LapisOre => 3,
                        _ => 0,
                    };
                    if xp_val > 0 {
                        world.xp_orbs.push(crate::block::XpOrbEntity {
                            position: Vec3::new(
                                mined.x as f32 + 0.5,
                                mined.y as f32 + 0.5,
                                mined.z as f32 + 0.5,
                            ),
                            velocity: Vec3::new(
                                (rand::random::<f32>() - 0.5) * 1.5,
                                2.5,
                                (rand::random::<f32>() - 0.5) * 1.5,
                            ),
                            xp_value: xp_val,
                            life: 300.0,
                        });
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

            // Per-frame block placement repeat update (Bedrock-style linear placement lock)
            let gp_lt_held = gp_gs
                .as_ref()
                .is_some_and(|gs| gs.get_axis(GamepadAxis::AxisLeftTrigger) > 0.5);
            let is_placing = (right_mouse_held || gp_lt_held) && !player.inventory_open;
            if is_placing {
                place_repeat_timer -= delta_time;
                if place_repeat_timer <= 0.0 {
                    try_place_block_with_lock(
                        &mut world,
                        &mut inv_slots,
                        player.selected_slot,
                        eye_pos,
                        look_dir,
                        &player,
                        &mut placement_lock,
                    );
                    place_repeat_timer = 0.20;
                }
            } else {
                placement_lock = None;
                place_repeat_timer = 0.0;
            }
        }

        let dusk_time = current_time as f32 + 570.0;
        if fancy_gfx_setting {
            world.update_clouds(player.position, dusk_time);
        }
        let (sun_angle, sun_y) = {
            let a = (dusk_time / 1200.0) * 2.0 * std::f32::consts::PI;
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
        let (framebuffer_width, framebuffer_height) = window.get_framebuffer_size();
        let (target_width, target_height) =
            render_target_size_for_framebuffer(framebuffer_width, framebuffer_height);
        if target.texture.width != target_width || target.texture.height != target_height {
            target = RenderTexture2D::new(target_width, target_height);
        }

        let aspect = target.texture.width as f32 / target.texture.height as f32;
        // perspective_rh maps depth to wgpu's [0, 1] range (GL used [-1, 1]).
        let projection = Mat4::perspective_rh(fov_setting.to_radians(), aspect, 0.1, 1000.0);
        let view = Mat4::look_at_rh(eye_pos, eye_pos + look_dir, Vec3::Y);
        let mvp = projection * view;

        // Vibrant Visuals deferred path. Fancy graphics off keeps the
        // original forward renderer, which is also the fallback if the
        // deferred targets could not be built.
        renderer::deferred_resize(target.texture.width, target.texture.height);
        let deferred = fancy_gfx_setting && renderer::deferred_ready();
        let deferred_uniforms = vibrant::frame::build_uniforms(
            &vibrant_pack,
            &vibrant::frame::FrameInput {
                day_fraction: vibrant::frame::day_fraction(dusk_time),
                camera_pos: eye_pos,
                view_proj: mvp,
            },
        );
        // Forward draws that land in the HDR target have to be lifted into
        // the same lux-scaled space the lighting pass writes. Set before
        // any sky or world draw: leftover staging from a previous deferred
        // frame would otherwise blow out the forward path, and stars would
        // miss the scale if they drew first.
        let hdr_scale = if deferred {
            vibrant::frame::hdr_scale(&deferred_uniforms)
        } else {
            1.0
        };
        shader.set_float(shader.get_uniform_location("uHdrScale"), hdr_scale);
        flat_shader.set_float(flat_shader.get_uniform_location("uHdrScale"), hdr_scale);
        water_shader.set_float(water_shader.get_uniform_location("uHdrScale"), hdr_scale);
        // Opaque geometry writes the G-buffer when deferred, and shades
        // itself when not. Both shaders read the same uniform names.
        let world_shader = if deferred { &gbuffer_shader } else { &shader };

        if deferred {
            renderer::deferred_begin_geometry();
        } else {
            target.bind();
            renderer::clear(sky_c.x, sky_c.y, sky_c.z, 1.0);
            world.render_stars(player.position, dusk_time, &flat_shader, &mvp);
            draw_sun_moon(&flat_shader, player.position, dusk_time, mvp, true);
        }
        world_shader.bind();
        world_shader.set_mat4(world_shader.get_uniform_location("uMVP"), &mvp);
        world_shader.set_mat4(world_shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
        world_shader.set_float(
            world_shader.get_uniform_location("uTime"),
            current_time as f32,
        );
        world_shader.set_vec2(
            world_shader.get_uniform_location("uResolution"),
            glam::Vec2::new(target.texture.width as f32, target.texture.height as f32),
        );
        world_shader.set_vec3(world_shader.get_uniform_location("sunDir"), sun_dir);
        world_shader.set_vec4(
            world_shader.get_uniform_location("colDiffuse"),
            glam::Vec4::ONE,
        );
        world_shader.set_vec3(world_shader.get_uniform_location("viewPos"), eye_pos);
        world_shader.set_vec4(world_shader.get_uniform_location("skyCol"), sky_c);
        let frustum = world::Frustum::from_matrix(&mvp);
        world.compute_visible_chunks(&frustum);
        world.render_opaque(world_shader, &frustum);

        // Opaque effects belong in the G-buffer alongside the terrain.
        world.render_explosives(world_shader, &mvp, current_time as f32);
        world.render_arrows(world_shader);
        world.render_xp_orbs(world_shader, current_time as f32);
        world.render_mobs(world_shader, player.position);

        // Resolve the G-buffer, then draw everything that cannot be
        // deferred -- blended decals, particles and the sky discs --
        // forward into the lit HDR target.
        if deferred {
            renderer::deferred_resolve(&deferred_uniforms);
            world.render_stars(player.position, dusk_time, &flat_shader, &mvp);
            // Depth-tested here, unlike the forward path where the sky is
            // drawn first and painted over by the terrain.
            draw_sun_moon(&flat_shader, player.position, dusk_time, mvp, false);
        }
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_mat4(shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
        shader.set_float(shader.get_uniform_location("uTime"), current_time as f32);
        shader.set_vec3(shader.get_uniform_location("sunDir"), sun_dir);
        shader.set_vec4(shader.get_uniform_location("colDiffuse"), glam::Vec4::ONE);
        shader.set_vec3(shader.get_uniform_location("viewPos"), eye_pos);
        shader.set_vec4(shader.get_uniform_location("skyCol"), sky_c);

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
            renderer::set_blend(true);
            renderer::set_depth_write(false);
            renderer::set_polygon_offset(true);
            let normals = vec![0.0f32; verts.len()];
            let colors = vec![255u8; (verts.len() / 3) * 4];
            let crack_mesh = renderer::Mesh::new(&verts, Some(&uvs), Some(&normals), Some(&colors));
            crack_mesh.draw();
            renderer::set_polygon_offset(false);
            renderer::set_depth_write(true);
            renderer::set_blend(false);
        }

        world.render_particles(&shader, &mvp);

        // Transparency render order depends on whether the camera is above or below
        // the cloud layer, so each transparent layer is drawn back-to-front.
        let above_clouds = eye_pos.y > CLOUD_HEIGHT;

        if fancy_gfx_setting && !above_clouds {
            world.render_clouds(&flat_shader, &mvp);
        }

        shader.bind();
        world.render_transparent(&shader, &frustum);

        water_shader.bind();
        water_shader.set_mat4(water_shader.get_uniform_location("uMVP"), &mvp);
        water_shader.set_float(
            water_shader.get_uniform_location("uTime"),
            current_time as f32,
        );
        water_shader.set_vec3(water_shader.get_uniform_location("sunDir"), sun_dir);
        water_shader.set_vec3(water_shader.get_uniform_location("viewPos"), eye_pos);
        water_shader.set_vec4(water_shader.get_uniform_location("skyCol"), sky_c);
        world.render_water(&water_shader, &frustum);

        if fancy_gfx_setting && above_clouds {
            world.render_clouds(&flat_shader, &mvp);
        }
        if deferred {
            // Tone map the lit HDR scene down onto the offscreen target the
            // UI and the final blit already expect.
            target.bind();
            renderer::deferred_tonemap();
        }
        RenderTexture2D::unbind();
        renderer::clear(0.0, 0.0, 0.0, 1.0);
        texture_shader.bind();
        draw_texture_quad(&target.texture);

        // Liquid screen overlay (below HUD)
        let cam_block = world.get_block(
            eye_pos.x.floor() as i32,
            eye_pos.y.floor() as i32,
            eye_pos.z.floor() as i32,
        );
        if cam_block == BlockType::Water || cam_block == BlockType::Lava {
            renderer::set_blend(true);
            renderer::set_depth_test(false);
            let tint = if cam_block == BlockType::Lava {
                glam::Vec4::new(0.65, 0.16, 0.02, 0.65)
            } else {
                glam::Vec4::new(0.05, 0.05, 0.2, 0.55)
            };
            draw_screen_quad(&color_shader, tint);
            renderer::set_depth_test(true);
        }

        renderer::set_depth_test(false);
        renderer::set_blend(true);
        renderer::set_cull(false);

        let draw_stack_item = |atlas: &Texture2D,
                               tex_shader: &Shader,
                               ui_shader: &Shader,
                               font_texture: &Texture2D,
                               s: &ItemStack,
                               rx: f32,
                               ry: f32,
                               slot_size: f32,
                               sw: f32,
                               sh: f32| {
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
        };
        ui_shader.bind();
        ui_shader.set_vec2(
            ui_shader.get_uniform_location("uScreenSize"),
            glam::Vec2::new(framebuffer_width as f32, framebuffer_height as f32),
        );
        let (sw, sh) = (framebuffer_width as f32, framebuffer_height as f32);
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
                draw_stack_item(
                    world.atlas.as_ref().unwrap(),
                    &texture_ui_shader,
                    &ui_shader,
                    &font_texture,
                    s,
                    rx,
                    ry,
                    36.0,
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

        // F3 debug overlay
        if show_debug_overlay {
            let biome =
                chunk::terrain_height_and_biome(player.position.x, player.position.z, world.seed).1;
            let yaw_deg = camera_angle.x.to_degrees().rem_euclid(360.0);
            let pitch_deg = camera_angle.y.to_degrees();
            let compass = [
                "South",
                "Southeast",
                "East",
                "Northeast",
                "North",
                "Northwest",
                "West",
                "Southwest",
            ][(((yaw_deg + 22.5) / 45.0) as usize) % 8];
            let cx = (player.position.x / chunk::CHUNK_WIDTH as f32).floor() as i32;
            let cz = (player.position.z / chunk::CHUNK_DEPTH as f32).floor() as i32;
            let stats = profiler.stats();
            let loaded_chunks = world.chunks.iter().flatten().count();
            let lines = [
                format!(
                    "VoxelPopuli: {:.0} FPS (avg: {:.0} | min: {:.0} | 1% low: {:.0})",
                    current_fps, stats.avg_fps, stats.min_fps, stats.low_1_fps
                ),
                format!(
                    "Frame Time: {:.2}ms (P95: {:.2}ms | P99: {:.2}ms)",
                    stats.avg_ms, stats.p95_ms, stats.p99_ms
                ),
                format!(
                    "Subsystems: Render {:.1}ms | World {:.1}ms | Mesh {:.1}ms",
                    profiler.render_ms, profiler.world_update_ms, profiler.mesh_dispatch_ms
                ),
                format!(
                    "XYZ: {:.3} / {:.3} / {:.3}",
                    player.position.x, player.position.y, player.position.z
                ),
                format!(
                    "Block: {} {} {}",
                    player.position.x.floor() as i32,
                    player.position.y.floor() as i32,
                    player.position.z.floor() as i32
                ),
                format!(
                    "Chunk Pool: {cx} {cz} ({} loaded, {} dirty, {} in-flight)",
                    loaded_chunks, world.dirty_count, world.meshing_in_flight
                ),
                format!("Mobs: {} active (48 cap)", world.mobs.len()),
                format!("Facing: {compass} (yaw {yaw_deg:.1}, pitch {pitch_deg:.1})"),
                format!("Biome: {biome:?}"),
                format!("Seed: {}", world.seed),
            ];
            draw_debug_overlay(
                &ui_shader,
                &texture_ui_shader,
                &font_texture,
                &lines,
                &profiler,
                sw,
                sh,
            );
        }

        // ── Crafting Table overlay (3x3) ─────────────────────────────────────
        if crafting_table_open {
            draw_screen_quad(&color_shader, glam::Vec4::new(0.0, 0.0, 0.0, 0.6));
            renderer::set_depth_test(false);
            renderer::set_blend(true);
            renderer::set_cull(false);
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
            renderer::set_depth_test(false);
            renderer::set_blend(true);
            renderer::set_cull(false);
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
                    if s.count > 1 {
                        let count_str = format!("{}", s.count);
                        let font_sz = 13.0;
                        let tw = text_width(&count_str, font_sz);
                        let tx = sx + ss - tw - 2.0;
                        let ty = sy + ss - font_sz - 1.0;
                        draw_text_tinted(
                            &font_texture,
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
                            &font_texture,
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
                    if let (Some(dur), Some(tool_props)) =
                        (s.durability, item::tool_properties(s.block))
                    {
                        if tool_props.durability > 0 {
                            let max_dur = tool_props.durability as f32;
                            let pct = (dur as f32 / max_dur).clamp(0.0, 1.0);
                            let bar_max_w = ss - 8.0;
                            let bar_w = (bar_max_w * pct).round();
                            let bar_y = sy + ss - 6.0;
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
                                shader,
                                sx + 4.0,
                                bar_y,
                                bar_max_w,
                                2.0,
                                [0, 0, 0, 255],
                                sw,
                                sh,
                            );
                            draw_rect(
                                shader,
                                sx + 4.0,
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

        // Draw Health, Armor, Food, and Oxygen HUD
        if !player.inventory_open {
            let heart_size = 24.0;
            let icon_size = 24.0;
            let bubble_size = 20.0;
            let spacing = 4.0;
            let w_off = (10.0 * (heart_size + spacing)) / 2.0;

            // Health (left side of center)
            let hx_start = sw / 2.0 - w_off - 80.0;
            let hy = sh - 80.0;
            for i in 0..10 {
                let rx = hx_start + i as f32 * (heart_size + spacing);
                let heart_val = (player.health - i * 2).clamp(0, 2);
                draw_heart(&ui_shader, rx, hy, heart_size, sw, sh, heart_val == 0);
            }

            // Armor Bar (above health bar, if total defense > 0)
            let total_defense = player.total_armor_defense();
            if total_defense > 0 {
                let ay = hy - 26.0;
                for i in 0..10 {
                    let rx = hx_start + i as f32 * (icon_size + spacing);
                    let armor_val = (total_defense - i * 2).clamp(0, 2);
                    draw_armor(&ui_shader, rx, ay, icon_size, sw, sh, armor_val);
                }
            }

            // Food Drumsticks (right side of center)
            let fx_start = sw / 2.0 + 80.0;
            let fy = hy;
            for i in 0..10 {
                let rx = fx_start + i as f32 * (icon_size + spacing);
                let food_val = (player.hunger - i * 2).clamp(0, 2);
                draw_drumstick(&ui_shader, rx, fy, icon_size, sw, sh, food_val);
            }

            // Experience Bar (above hotbar)
            let xp_bar_w = 440.0;
            let xp_bar_h = 8.0;
            let xpx = sw / 2.0 - xp_bar_w / 2.0;
            let xpy = sh - 52.0;
            draw_rect(
                &ui_shader,
                xpx - 1.0,
                xpy - 1.0,
                xp_bar_w + 2.0,
                xp_bar_h + 2.0,
                [20, 20, 20, 255],
                sw,
                sh,
            );
            draw_rect(
                &ui_shader,
                xpx,
                xpy,
                xp_bar_w,
                xp_bar_h,
                [35, 45, 30, 230],
                sw,
                sh,
            );
            let fill_w = (xp_bar_w * player.xp_progress.clamp(0.0, 1.0)).round();
            if fill_w > 0.0 {
                draw_rect(
                    &ui_shader,
                    xpx,
                    xpy,
                    fill_w,
                    xp_bar_h,
                    [110, 230, 50, 255],
                    sw,
                    sh,
                );
            }
            if player.xp_level > 0 {
                let lvl_str = format!("{}", player.xp_level);
                let txt_w = lvl_str.len() as f32 * 10.0;
                draw_text(
                    &font_texture,
                    &lvl_str,
                    sw / 2.0 - txt_w / 2.0,
                    xpy - 12.0,
                    14.0,
                    &texture_ui_shader,
                    sw,
                    sh,
                );
            }

            // Oxygen bubbles (above food bar)
            let head_in_w = world.get_block(
                eye_pos.x.floor() as i32,
                eye_pos.y.floor() as i32,
                eye_pos.z.floor() as i32,
            ) == BlockType::Water;
            if head_in_w || player.air_seconds < 15.0 {
                let ox_start = fx_start;
                let oy = fy - 26.0;
                let bubbles_left = (player.air_seconds / 1.5).ceil() as i32;

                for i in 0..10 {
                    let rx = ox_start + i as f32 * (bubble_size + spacing);
                    let empty = i >= bubbles_left;
                    draw_bubble(&ui_shader, rx, oy, bubble_size, sw, sh, empty);
                }
            }

            if bow_charge > 0.0 {
                let bar_w = 60.0;
                let bar_h = 6.0;
                let bx = sw / 2.0 - bar_w / 2.0;
                let by = sh / 2.0 + 30.0;
                draw_rect(
                    &ui_shader,
                    bx - 1.0,
                    by - 1.0,
                    bar_w + 2.0,
                    bar_h + 2.0,
                    [20, 20, 20, 255],
                    sw,
                    sh,
                );
                draw_rect(
                    &ui_shader,
                    bx,
                    by,
                    bar_w * bow_charge,
                    bar_h,
                    [60, 220, 80, 255],
                    sw,
                    sh,
                );
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
                pause_sub_menu,
                world_seed as u64,
                world.mobs.len(),
                &export_status_msg,
                selected_skin,
                render_dist_setting,
                fov_setting,
                fancy_gfx_setting,
            );
        }

        renderer::set_depth_test(true);
        renderer::set_cull(true);
        renderer::end_frame(framebuffer_width, framebuffer_height);

        profiler.record_frame(frame_start.elapsed());
        profiler.log_telemetry_if_needed(
            world.chunks.iter().flatten().count(),
            world.dirty_count,
            world.meshing_in_flight,
            world.mobs.len(),
        );
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
    let settings = GameSettings {
        view_distance: render_dist_setting,
        fov: fov_setting,
        fancy_graphics: fancy_gfx_setting,
        selected_skin,
    }
    .clamped();
    if let Ok(save) = GameSave::capture(
        &world,
        &player,
        &inv_slots,
        camera_angle,
        settings,
        import_world_path.as_deref(),
    ) {
        if let Err(err) = save.write_to(std::path::Path::new(SAVE_FILE)) {
            eprintln!("Failed to save world: {err}");
        }
    }
}
