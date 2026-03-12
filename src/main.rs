use glfw::{Action, Context, Key};
mod block;
mod chunk;
mod noise;
mod renderer;
mod world;

use block::BlockType;
use world::World;
use renderer::{Shader, Texture2D, RenderTexture2D};
use glam::{Vec2, Vec3, Mat4};

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;
const RENDER_WIDTH: i32 = 960;
const RENDER_HEIGHT: i32 = 540;

const PS1_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 2) in vec3 vertexNormal;
layout(location = 3) in vec4 vertexColor;

uniform mat4 uMVP;
uniform mat4 uModel;
uniform mat4 uProjection;
uniform mat4 uModelView;

out vec4 fragColor;
out vec2 fragTexCoord;
out vec3 fragPos;
out vec3 fragNormal;

void main() {
    fragColor = vertexColor;
    fragTexCoord = vertexTexCoord;
    fragPos = (uModel * vec4(vertexPosition, 1.0)).xyz;
    fragNormal = normalize((uModel * vec4(vertexNormal, 0.0)).xyz);
    
    vec4 pos = uMVP * vec4(vertexPosition, 1.0);
    gl_Position = pos;
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
    vec3 color = baseColor * timeLight;
    
    color *= mix(vec3(1.0, 0.9, 0.8), vec3(1.0, 1.0, 1.05), sunY); // slight tinting
    
    // Final color output without artificial quantization
    vec4 c = vec4(color, texelColor.a * fragColor.a * colDiffuse.a);
    finalColor = c;
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
uniform bool uIsCircle;
void main() {
    if (uIsCircle) {
        vec2 uv = fragTexCoord * 2.0 - 1.0;
        float d = dot(uv, uv);
        if (d > 1.0) discard;
        float falloff = 1.0 - smoothstep(0.8, 1.0, d);
        finalColor = vec4(fragColor.rgb, fragColor.a * falloff);
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

fn draw_rect(shader: &Shader, x: f32, y: f32, w: f32, h: f32, color: [u8; 4], screen_width: f32, screen_height: f32) {
    let v = [
        x, y, 0.0,  x + w, y, 0.0,  x + w, y + h, 0.0,
        x, y, 0.0,  x + w, y + h, 0.0,  x, y + h, 0.0,
    ];
    let mut c = Vec::new();
    for _ in 0..6 { c.extend_from_slice(&color); }
    let mesh = renderer::Mesh::new(&v, None, None, Some(&c));
    shader.bind();
    shader.set_vec2(shader.get_uniform_location("uScreenSize"), glam::Vec2::new(screen_width, screen_height));
    mesh.draw();
}

fn draw_texture_quad(texture: &Texture2D) {
    let v = [
        -1.0, -1.0, 0.0,  1.0, -1.0, 0.0,  1.0,  1.0, 0.0,
        -1.0, -1.0, 0.0,  1.0,  1.0, 0.0, -1.0,  1.0, 0.0,
    ];
    let t = [
        0.0, 0.0,  1.0, 0.0,  1.0, 1.0,
        0.0, 0.0,  1.0, 1.0,  0.0, 1.0,
    ];
    let mesh = renderer::Mesh::new(&v, Some(&t), None, None);
    texture.bind(0);
    mesh.draw();
}

fn draw_screen_quad(shader: &Shader, color: glam::Vec4) {
    let v = [
        -1.0, -1.0, 0.0,  1.0, -1.0, 0.0,  1.0,  1.0, 0.0,
        -1.0, -1.0, 0.0,  1.0,  1.0, 0.0, -1.0,  1.0, 0.0,
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
    let sunrise_mult = if sun_y > -0.3 && sun_y < 0.3 { 0.6 } else { 1.0 };
    let sun_c = [ (255.0 * sunrise_mult) as u8, (200.0 * sunrise_mult) as u8, (150.0 * sunrise_mult) as u8, 255 ];
    let moon_c = [ 220, 225, 255, 255 ];
    let mut v = Vec::new(); let mut c = Vec::new(); let mut t = Vec::new();
    let mut add_quad = |pos: Vec3, color: [u8; 4], q_size: f32| {
        let look = (player_pos - pos).normalize();
        let r = look.cross(Vec3::Y).normalize();
        let u = r.cross(look).normalize();
        let v0 = pos + (r * -q_size) + (u * -q_size);
        let v1 = pos + (r * q_size) + (u * -q_size);
        let v2 = pos + (r * q_size) + (u * q_size);
        let v3 = pos + (r * -q_size) + (u * q_size);
        v.extend_from_slice(&[v0.x, v0.y, v0.z, v1.x, v1.y, v1.z, v2.x, v2.y, v2.z]);
        v.extend_from_slice(&[v0.x, v0.y, v0.z, v2.x, v2.y, v2.z, v3.x, v3.y, v3.z]);
        for _ in 0..6 { c.extend_from_slice(&color); }
        t.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    };
    add_quad(sun_pos, sun_c, size); add_quad(moon_pos, moon_c, size * 0.8);
    let mesh = renderer::Mesh::new(&v, Some(&t), None, Some(&c));
    shader.bind(); shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
    shader.set_int(shader.get_uniform_location("uIsCircle"), 1);
    unsafe { gl::Disable(gl::DEPTH_TEST); gl::Disable(gl::CULL_FACE); mesh.draw(); gl::Enable(gl::CULL_FACE); gl::Enable(gl::DEPTH_TEST); }
}

fn get_block_ui_color(block: BlockType) -> [u8; 4] {
    match block {
        BlockType::Stone => [140, 140, 140, 255],
        BlockType::Dirt => [130, 90, 60, 255],
        BlockType::Grass => [100, 180, 50, 255],
        BlockType::OakLog => [100, 70, 40, 255],
        BlockType::OakLeaves => [40, 120, 30, 255],
        BlockType::Bedrock => [60, 60, 60, 255],
        BlockType::Water => [40, 80, 200, 255],
        BlockType::Sand => [220, 210, 160, 255],
        BlockType::Gravel => [120, 120, 130, 255],
        BlockType::CoalOre => [100, 100, 100, 255],
        _ => [0, 0, 0, 0],
    }
}

struct Player { position: Vec3, velocity: Vec3, grounded: bool, air_seconds: f32, inventory_open: bool, selected_slot: usize, health: i32 }
impl Player {
    fn is_point_in_block(world: &World, p: Vec3) -> bool {
        let b = world.get_block(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        b != BlockType::Air && b != BlockType::Water
    }
    fn check_collision(world: &World, pos: Vec3) -> bool {
        let w = 0.22; let h = 1.75;
        for x_off in [-w, w] {
            for z_off in [-w, w] {
                for yi in 0..=2 {
                    let y = 0.1 + yi as f32 * ((h - 0.1) / 2.0);
                    if Self::is_point_in_block(world, Vec3::new(pos.x + x_off, pos.y + y, pos.z + z_off)) { return true; }
                }
            }
        }
        false
    }
    fn update(&mut self, world: &World, move_input: Vec3, dt: f32, is_sprinting: bool, is_jumping: bool) {
        if self.inventory_open { return; }
        let waist_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 0.9).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        let feet_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 0.1).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        let mut mv = move_input;
        if mv.length_squared() > 0.1 {
            mv = mv.normalize(); let speed = if waist_in_w || feet_in_w { 3.5 } else if is_sprinting { 8.5 } else { 4.5 }; mv *= speed;
        }
        self.velocity.x = mv.x; self.velocity.z = mv.z; 
        
        if waist_in_w || feet_in_w {
            if is_jumping {
                self.velocity.y += 24.0 * dt;
                if self.velocity.y > 6.0 { self.velocity.y = 6.0; }
            } else {
                if waist_in_w {
                    self.velocity.y -= 10.0 * dt;
                    if self.velocity.y < -3.5 { self.velocity.y = -3.5; }
                } else {
                    self.velocity.y -= 28.0 * dt;
                }
            }
        } else {
            self.velocity.y -= 28.0 * dt;
            if is_jumping && self.grounded {
                self.velocity.y = 9.0;
                self.grounded = false;
            }
        }

        let dy = self.velocity.y * dt; self.position.y += dy;
        if Self::check_collision(world, self.position) {
            if self.velocity.y < 0.0 { self.grounded = true; }
            self.position.y -= dy; self.velocity.y = 0.0;
        } else if self.velocity.y != 0.0 {
            self.grounded = false;
        }
        let dx = self.velocity.x * dt; self.position.x += dx;
        if Self::check_collision(world, self.position) { self.position.x -= dx; }
        let dz = self.velocity.z * dt; self.position.z += dz;
        if Self::check_collision(world, self.position) { self.position.z -= dz; }
        let head_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 1.6).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        if head_in_w { self.air_seconds = (self.air_seconds - dt).max(0.0); }
        else { self.air_seconds = (self.air_seconds + dt * 3.0).min(15.0); }
        if self.air_seconds <= 0.0 {
            // Drowning damage (not fully implemented, just draining health for now)
            // real implementation would need a timer
        }
    }
}

fn draw_heart(shader: &Shader, x: f32, y: f32, size: f32, screen_width: f32, screen_height: f32, empty: bool) {
    let mut v: Vec<f32> = Vec::new();
    let mut c: Vec<u8> = Vec::new();
    
    // Simple blocky heart shape (10x9 grid roughly)
    // 01100110
    // 11111111
    // 11111111
    // 01111110
    // 00111100
    // 00011000
    
    let color = if empty { [40, 40, 40, 200] } else { [220, 20, 20, 255] };
    let border = [20, 20, 20, 255];
    
    // Draw background/border
    for px in 0..9 {
        for py in 0..9 {
            let fill = match (px, py) {
                (1..=3, 0) | (5..=7, 0) => true,
                (0..=8, 1..=3) => true,
                (1..=7, 4) => true,
                (2..=6, 5) => true,
                (3..=5, 6) => true,
                (4..=4, 7) => true,
                _ => false
            };
            if fill {
                let bx = x + (px as f32 / 9.0) * size;
                let by = y + (py as f32 / 9.0) * size;
                let bw = size / 9.0;
                
                let is_border = match (px, py) {
                    (1..=3, 0) | (5..=7, 0) => true,
                    (4, 1) => true,
                    (0, 1..=3) | (8, 1..=3) => true,
                    (1, 4) | (7, 4) => true,
                    (2, 5) | (6, 5) => true,
                    (3, 6) | (5, 6) => true,
                    (4, 7) => true,
                    _ => false
                };
                
                let is_highlight = match (px, py) {
                    (1, 1) | (2, 1) | (1, 2) => !empty,
                    _ => false
                };
                
                let p_color = if is_border { border } else if is_highlight { [255, 255, 255, 255] } else { color };
                draw_rect(shader, bx, by, bw, bw, p_color, screen_width, screen_height);
            }
        }
    }
}

fn draw_bubble(shader: &Shader, x: f32, y: f32, size: f32, screen_width: f32, screen_height: f32, empty: bool) {
    let color = if empty { [20, 20, 40, 150] } else { [60, 150, 255, 200] };
    let border = [20, 20, 40, 255];
    
    for px in 0..8 {
        for py in 0..8 {
            let dist_sq = (px as f32 - 3.5).powi(2) + (py as f32 - 3.5).powi(2);
            if dist_sq <= 16.0 {
                let bx = x + (px as f32 / 8.0) * size;
                let by = y + (py as f32 / 8.0) * size;
                let bw = size / 8.0;
                
                let is_border = dist_sq > 9.0;
                let p_color = if is_border { border } else if px == 2 && py == 2 && !empty { [255, 255, 255, 255] } else { color };
                
                draw_rect(shader, bx, by, bw, bw, p_color, screen_width, screen_height);
            }
        }
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    #[cfg(target_os = "macos")]
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    let (mut window, events) = glfw.create_window(WINDOW_WIDTH, WINDOW_HEIGHT, "VoxelPopuli Rust", glfw::WindowMode::Windowed).expect("Failed to create GLFW window.");
    window.make_current(); window.set_key_polling(true); window.set_cursor_pos_polling(true); window.set_size_polling(true); window.set_mouse_button_polling(true); window.set_cursor_mode(glfw::CursorMode::Disabled);
    gl::load_with(|s| window.get_proc_address(s).map_or(std::ptr::null(), |p| p as *const _));
    unsafe {
        gl::Enable(gl::DEPTH_TEST); gl::DepthFunc(gl::LEQUAL);
        gl::Enable(gl::CULL_FACE); gl::CullFace(gl::BACK);
        gl::Enable(gl::BLEND); gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
    let mut world = World::new(); world.generate_atlas(); world.init_celestial();
    let shader = Shader::new(PS1_VS, PS1_FS).expect("Failed to compile PS1 shader");
    let flat_shader = Shader::new(FLAT_VS, FLAT_FS).expect("Failed to compile FLAT shader");
    let ui_shader = Shader::new(UI_VS, UI_FS).expect("Failed to compile UI shader");
    let texture_shader = Shader::new(TEXTURE_VS, TEXTURE_FS).expect("Failed to compile TEXTURE shader");
    let color_shader = Shader::new(TEXTURE_VS, COLOR_FS).expect("Failed to compile COLOR shader");
    let target = RenderTexture2D::new(RENDER_WIDTH, RENDER_HEIGHT);
    let mut spawn_y = 150.0;
    world.update(Vec3::new(32.5, 0.0, 32.5), 0.0); 
    for y in (0..255).rev() { if world.get_block(32, y, 32) != BlockType::Air { spawn_y = y as f32 + 2.0; break; } }
    let inventory_blocks = [
        BlockType::Stone, BlockType::Dirt, BlockType::Grass, BlockType::OakLog, 
        BlockType::OakLeaves, BlockType::Sand, BlockType::Gravel, BlockType::CoalOre,
        BlockType::Water, BlockType::Bedrock
    ];
    let mut hotbar = [BlockType::Air; 10];
    for i in 0..inventory_blocks.len().min(10) { hotbar[i] = inventory_blocks[i]; }
    let mut player = Player { position: Vec3::new(32.5, spawn_y, 32.5), velocity: Vec3::ZERO, grounded: false, air_seconds: 15.0, inventory_open: false, selected_slot: 0, health: 20 };
    let mut camera_angle = Vec2::new(std::f32::consts::PI, 0.0);
    let mut last_cursor_pos = window.get_cursor_pos();
    let mut last_time = glfw.get_time();
    let mut fps_last_time = last_time;
    let mut fps_frames: u32 = 0;
    let mut fps: f32 = 0.0;
    while !window.should_close() {
        let current_time = glfw.get_time(); 
        let delta_time = (current_time - last_time) as f32; 
        last_time = current_time;

        fps_frames += 1;
        let fps_elapsed = current_time - fps_last_time;
        if fps_elapsed >= 1.0 {
            fps = (fps_frames as f64 / fps_elapsed) as f32;
            fps_frames = 0;
            fps_last_time = current_time;
            window.set_title(&format!("VoxelPopuli Rust - {:.0} FPS", fps));
            println!("FPS: {:.0}", fps);
        }
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Size(width, height) => { unsafe { gl::Viewport(0, 0, width, height); } }
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => { window.set_should_close(true) }
                glfw::WindowEvent::Key(Key::E, _, Action::Press, _) => {
                    player.inventory_open = !player.inventory_open;
                    if player.inventory_open { window.set_cursor_mode(glfw::CursorMode::Normal); }
                    else { window.set_cursor_mode(glfw::CursorMode::Disabled); last_cursor_pos = window.get_cursor_pos(); }
                }

                glfw::WindowEvent::CursorPos(x, y) => {
                    if !player.inventory_open {
                        let dx = (x - last_cursor_pos.0) as f32; let dy = (y - last_cursor_pos.1) as f32;
                        last_cursor_pos = (x, y); camera_angle.x -= dx * 0.003; camera_angle.y -= dy * 0.003;
                        camera_angle.y = camera_angle.y.clamp(-1.56, 1.56);
                    }
                }
                glfw::WindowEvent::MouseButton(button, Action::Press, _) => {
                    if player.inventory_open {
                        if button == glfw::MouseButtonLeft {
                            let (mx, my) = window.get_cursor_pos(); let (sw, sh) = (WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
                            for i in 0..inventory_blocks.len() {
                                let rx = (sw / 2.0 - 175.0) + (i % 5) as f32 * 70.0; let ry = (sh / 2.0 - 70.0) + (i / 5) as f32 * 70.0;
                                if mx >= rx as f64 && mx <= (rx + 60.0) as f64 && my >= ry as f64 && my <= (ry + 60.0) as f64 { hotbar[player.selected_slot] = inventory_blocks[i]; }
                            }
                        }
                    } else {
                        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
                        let look_dir = Vec3::new(camera_angle.y.cos() * camera_angle.x.sin(), camera_angle.y.sin(), camera_angle.y.cos() * camera_angle.x.cos());
                        if button == glfw::MouseButtonLeft {
                            let res = world.raycast(eye_pos, look_dir, 8.0); if res.hit { world.set_block(res.x, res.y, res.z, BlockType::Air); }
                        } else if button == glfw::MouseButtonRight {
                            let res = world.raycast(eye_pos, look_dir, 8.0);
                            if res.hit {
                                let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
                                if world.get_block(nx, ny, nz) == BlockType::Air {
                                    let b = hotbar[player.selected_slot]; if b != BlockType::Air { world.set_block(nx, ny, nz, b); }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let forward = Vec3::new(camera_angle.x.sin(), 0.0, camera_angle.x.cos());
        let right = forward.cross(Vec3::Y);
        let mut move_dir = Vec3::ZERO;
        if !player.inventory_open {
            if window.get_key(Key::W) == Action::Press { move_dir += forward; }
            if window.get_key(Key::S) == Action::Press { move_dir -= forward; }
            if window.get_key(Key::A) == Action::Press { move_dir -= right; }
            if window.get_key(Key::D) == Action::Press { move_dir += right; }
        }
        let is_sprinting = window.get_key(Key::LeftControl) == Action::Press;
        let is_jumping = window.get_key(Key::Space) == Action::Press;
        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
        let look_dir = Vec3::new(camera_angle.y.cos() * camera_angle.x.sin(), camera_angle.y.sin(), camera_angle.y.cos() * camera_angle.x.cos());
        player.update(&world, move_dir, delta_time, is_sprinting, is_jumping);
        world.update(player.position, current_time as f32);
        world.update_clouds(player.position, current_time as f32);
        let (sun_angle, sun_y) = { let a = (current_time as f32 / 1200.0) * 2.0 * std::f32::consts::PI; (a, a.sin()) };
        let sun_dir = glam::Vec3::new(0.0, sun_y, sun_angle.cos());
        let sky_c = {
            if sun_y > 0.3 { glam::Vec4::new(135.0/255.0, 206.0/255.0, 235.0/255.0, 1.0) }
            else if sun_y > -0.2 {
                let t = ((sun_y + 0.2) / 0.5).clamp(0.0, 1.0);
                if t < 0.5 { let u = t * 2.0; glam::Vec4::new((255.0*u+135.0*(1.0-u))/255.0, (160.0*u+100.0*(1.0-u))/255.0, (180.0*u+150.0*(1.0-u))/255.0, 1.0) }
                else { let u = (t-0.5)*2.0; glam::Vec4::new((135.0*u+255.0*(1.0-u))/255.0, (206.0*u+160.0*(1.0-u))/255.0, (235.0*u+180.0*(1.0-u))/255.0, 1.0) }
            } else if sun_y > -0.5 { let t = (sun_y+0.5)/0.3; glam::Vec4::new((30.0*t+10.0*(1.0-t))/255.0, (30.0*t+10.0*(1.0-t))/255.0, (80.0*t+50.0*(1.0-t))/255.0, 1.0) }
            else { glam::Vec4::new(5.0/255.0, 5.0/255.0, 15.0/255.0, 1.0) }
        };
        target.bind();
        unsafe { gl::ClearColor(sky_c.x, sky_c.y, sky_c.z, 1.0); gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT); }
        let aspect = RENDER_WIDTH as f32 / RENDER_HEIGHT as f32;
        let projection = Mat4::perspective_rh_gl(75.0_f32.to_radians(), aspect, 0.1, 1000.0);
        let view = Mat4::look_at_rh(eye_pos, eye_pos + look_dir, Vec3::Y);
        let mvp = projection * view;
        world.render_stars(player.position, current_time as f32, &flat_shader, &mvp);
        draw_sun_moon(&flat_shader, player.position, current_time as f32, mvp);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_mat4(shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
        shader.set_mat4(shader.get_uniform_location("uProjection"), &projection);
        shader.set_mat4(shader.get_uniform_location("uModelView"), &view);
        shader.set_vec2(shader.get_uniform_location("uResolution"), glam::Vec2::new(RENDER_WIDTH as f32, RENDER_HEIGHT as f32));
        shader.set_vec3(shader.get_uniform_location("sunDir"), sun_dir);
        shader.set_vec4(shader.get_uniform_location("colDiffuse"), glam::Vec4::ONE);
        shader.set_vec3(shader.get_uniform_location("viewPos"), eye_pos);
        shader.set_vec4(shader.get_uniform_location("skyCol"), sky_c);
        let frustum = world::Frustum::from_matrix(&mvp);
        world.render_opaque(&frustum); world.render_clouds(&flat_shader, &mvp);
        shader.bind(); world.render_transparent(&frustum);
        RenderTexture2D::unbind();
        let (win_width, win_height) = window.get_framebuffer_size();
        unsafe { gl::Viewport(0, 0, win_width, win_height); gl::ClearColor(0.0, 0.0, 0.0, 1.0); gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT); }
        texture_shader.bind(); draw_texture_quad(&target.texture);
        unsafe { gl::Disable(gl::DEPTH_TEST); gl::Enable(gl::BLEND); gl::Disable(gl::CULL_FACE); }
        ui_shader.bind(); ui_shader.set_vec2(ui_shader.get_uniform_location("uScreenSize"), glam::Vec2::new(win_width as f32, win_height as f32));
        let (sw, sh) = (win_width as f32, win_height as f32); let hbx = (sw - 400.0) / 2.0;
        for i in 0..10 {
            let rx = hbx + i as f32 * 40.0; let ry = sh - 45.0;
            let color = if player.selected_slot == i { [255, 255, 255, 255] } else { [100, 100, 100, 255] };
            draw_rect(&ui_shader, rx, ry, 35.0, 35.0, color, sw, sh);
            let b = hotbar[i]; if b != BlockType::Air { 
                let mut block_color = get_block_ui_color(b);
                block_color[3] = 255; // Force maximum opacity
                draw_rect(&ui_shader, rx + 5.0, ry + 5.0, 25.0, 25.0, block_color, sw, sh); 
            }
        }
        if player.inventory_open {
            draw_rect(&ui_shader, 0.0, 0.0, sw, sh, [0, 0, 0, 180], sw, sh);
            for i in 0..inventory_blocks.len() {
                let rx = (sw / 2.0 - 175.0) + (i % 5) as f32 * 70.0; let ry = (sh / 2.0 - 70.0) + (i / 5) as f32 * 70.0;
                draw_rect(&ui_shader, rx, ry, 60.0, 60.0, [255, 255, 255, 80], sw, sh);
                draw_rect(&ui_shader, rx + 12.0, ry + 12.0, 36.0, 36.0, get_block_ui_color(inventory_blocks[i]), sw, sh);
            }
        } else { draw_rect(&ui_shader, sw/2.0 - 2.0, sh/2.0 - 2.0, 4.0, 4.4, [255, 255, 255, 255], sw, sh); }

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
            let head_in_w = world.get_block(eye_pos.x.floor() as i32, eye_pos.y.floor() as i32, eye_pos.z.floor() as i32) == BlockType::Water;
            if head_in_w || player.air_seconds < 15.0 {
                let ox_start = sw / 2.0 + 80.0;
                let oy = sh - 78.0;
                let bubbles_left = (player.air_seconds / 1.5).ceil() as i32;
                
                for i in 0..10 {
                    let rx = ox_start + i as f32 * (bubble_size + spacing);
                    let empty = i >= bubbles_left;
                    draw_bubble(&ui_shader, rx, oy, bubble_size, sw, sh, empty);
                }
            }
        }

        let cam_block = world.get_block(eye_pos.x.floor() as i32, eye_pos.y.floor() as i32, eye_pos.z.floor() as i32);
        if cam_block == BlockType::Water {
            unsafe { gl::Enable(gl::BLEND); gl::Disable(gl::DEPTH_TEST); }
            draw_screen_quad(&color_shader, glam::Vec4::new(0.0, 0.47, 0.95, 0.6));
            unsafe { gl::Enable(gl::DEPTH_TEST); }
        }
        unsafe { gl::Enable(gl::DEPTH_TEST); gl::Enable(gl::CULL_FACE); }
        window.swap_buffers();
    }
}
