use glfw::{Action, Context, Key};
use std::ffi::c_void;

mod block;
mod chunk;
mod noise;
mod renderer;
mod world;

use block::BlockType;
use world::World;
use renderer::Shader;
use glam::{Vec2, Vec3, Mat4};

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

const PS1_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;
layout(location = 2) in vec3 vertexNormal;
layout(location = 3) in vec4 vertexColor;

uniform mat4 uMVP;
uniform mat4 uModel;
uniform float uPrecision;

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
    if (pos.w > 0.0) {
        vec2 snappedPos = pos.xy / pos.w;
        snappedPos = floor(snappedPos * uPrecision + 0.5) / uPrecision;
        pos.xy = snappedPos * pos.w;
    }
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
    
    // Dynamic Lighting
    float diff = max(0.0, dot(fragNormal, sunDir));
    float ambient = max(0.05, 0.4 * sunDir.y + 0.1);
    float light = ambient + diff * max(0.0, sunDir.y * 0.9);
    
    vec3 baseColor = texelColor.rgb * fragColor.rgb * colDiffuse.rgb;
    vec3 color = baseColor * light;
    
    // Vibrancy and Tone Mapping
    float sunY = max(0.0, sunDir.y);
    color *= mix(vec3(1.2, 0.8, 0.6), vec3(1.0, 1.0, 1.05), sunY);
    
    // Saturation boost
    float gray = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(gray), color, 1.25);
    
    // PS1 color depth
    vec4 c = vec4(color, texelColor.a * fragColor.a * colDiffuse.a);
    c.rgb = floor(c.rgb * 31.0 + 0.5) / 31.0;
    finalColor = c;
}
"#;

const FLAT_VS: &str = r#"
#version 330 core
layout(location = 0) in vec3 vertexPosition;
layout(location = 3) in vec4 vertexColor;
uniform mat4 uMVP;
out vec4 fragColor;
void main() {
    fragColor = vertexColor;
    gl_Position = uMVP * vec4(vertexPosition, 1.0);
}
"#;

const FLAT_FS: &str = r#"
#version 330 core
in vec4 fragColor;
out vec4 finalColor;
void main() {
    finalColor = fragColor;
}
"#;

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

    let mut v = Vec::new();
    let mut c = Vec::new();

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
    };

    add_quad(sun_pos, sun_c, size);
    add_quad(moon_pos, moon_c, size * 0.8);

    let mesh = renderer::Mesh::new(&v, None, None, Some(&c));
    shader.bind();
    shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
    unsafe {
        gl::Disable(gl::DEPTH_TEST);
        gl::Disable(gl::CULL_FACE);
        mesh.draw();
        gl::Enable(gl::CULL_FACE);
        gl::Enable(gl::DEPTH_TEST);
    }
}

struct Player {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
    air_seconds: f32,
    health: i32,
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
                    if Self::is_point_in_block(world, Vec3::new(pos.x + x_off, pos.y + y, pos.z + z_off)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn update(&mut self, world: &World, move_input: Vec3, dt: f32, is_sprinting: bool) {
        let waist_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 0.9).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        let feet_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 0.1).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        
        let mut mv = move_input;
        if mv.length_squared() > 0.1 {
            mv = mv.normalize();
            let speed = if waist_in_w { 3.5 } else if is_sprinting { 8.5 } else { 4.5 };
            mv *= speed;
        }

        self.velocity.x = mv.x;
        self.velocity.z = mv.z;
        self.velocity.y -= (if waist_in_w { 14.0 } else { 28.0 }) * dt;

        // Apply dy
        let dy = self.velocity.y * dt;
        self.position.y += dy;
        if Self::check_collision(world, self.position) {
            if self.velocity.y < 0.0 {
                self.grounded = true;
            }
            self.position.y -= dy;
            self.velocity.y = 0.0;
        } else if self.velocity.y != 0.0 {
            self.grounded = false;
        }

        // Apply dx
        let dx = self.velocity.x * dt;
        self.position.x += dx;
        if Self::check_collision(world, self.position) {
            self.position.x -= dx;
        }

        // Apply dz
        let dz = self.velocity.z * dt;
        self.position.z += dz;
        if Self::check_collision(world, self.position) {
            self.position.z -= dz;
        }

        let head_in_w = world.get_block(self.position.x.floor() as i32, (self.position.y + 1.6).floor() as i32, self.position.z.floor() as i32) == BlockType::Water;
        if head_in_w {
            self.air_seconds = (self.air_seconds - dt).max(0.0);
        } else {
            self.air_seconds = (self.air_seconds + dt * 3.0).min(15.0);
        }
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    #[cfg(target_os = "macos")]
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));

    let (mut window, events) = glfw.create_window(WINDOW_WIDTH, WINDOW_HEIGHT, "VoxelPopuli Rust", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    window.set_key_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_cursor_mode(glfw::CursorMode::Disabled);

    gl::load_with(|s| window.get_proc_address(s).map_or(std::ptr::null(), |p| p as *const _));

    unsafe {
        gl::Enable(gl::DEPTH_TEST);
        gl::Enable(gl::CULL_FACE);
        gl::CullFace(gl::BACK);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let mut world = World::new();
    world.generate_atlas();
    world.init_celestial();

    let shader = Shader::new(PS1_VS, PS1_FS).expect("Failed to compile PS1 shader");
    let flat_shader = Shader::new(FLAT_VS, FLAT_FS).expect("Failed to compile FLAT shader");

    let mut player = Player {
        position: Vec3::new(32.5, 164.0, 32.5),
        velocity: Vec3::ZERO,
        grounded: false,
        air_seconds: 15.0,
        health: 20,
    };
    let mut camera_angle = Vec2::new(std::f32::consts::PI, 0.0);
    let mut last_cursor_pos = window.get_cursor_pos();

    let mut last_time = glfw.get_time();

    while !window.should_close() {
        let current_time = glfw.get_time();
        let delta_time = (current_time - last_time) as f32;
        last_time = current_time;

        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                }
                glfw::WindowEvent::Key(Key::Space, _, Action::Press, _) => {
                    let waist_in_w = world.get_block(player.position.x.floor() as i32, (player.position.y + 0.9).floor() as i32, player.position.z.floor() as i32) == BlockType::Water;
                    let feet_in_w = world.get_block(player.position.x.floor() as i32, (player.position.y + 0.1).floor() as i32, player.position.z.floor() as i32) == BlockType::Water;
                    
                    if waist_in_w {
                        player.velocity.y = 4.0;
                    } else if player.grounded {
                        player.velocity.y = 9.0;
                        player.grounded = false;
                    } else if feet_in_w {
                        player.velocity.y = 5.0;
                    }
                }
                glfw::WindowEvent::CursorPos(x, y) => {
                    let dx = (x - last_cursor_pos.0) as f32;
                    let dy = (y - last_cursor_pos.1) as f32;
                    last_cursor_pos = (x, y);

                    camera_angle.x -= dx * 0.003;
                    camera_angle.y -= dy * 0.003;
                    camera_angle.y = camera_angle.y.clamp(-1.56, 1.56);
                }
                _ => {}
            }
        }

        // Basic movement
        let forward = Vec3::new(camera_angle.x.sin(), 0.0, camera_angle.x.cos());
        let right = forward.cross(Vec3::Y);
        let mut move_dir = Vec3::ZERO;

        if window.get_key(Key::W) == Action::Press { move_dir += forward; }
        if window.get_key(Key::S) == Action::Press { move_dir -= forward; }
        if window.get_key(Key::A) == Action::Press { move_dir -= right; }
        if window.get_key(Key::D) == Action::Press { move_dir += right; }

        if move_dir.length_squared() > 0.1 {
            move_dir = move_dir.normalize();
        }

        let is_sprinting = window.get_key(Key::LeftControl) == Action::Press;
        
        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
        let look_dir = Vec3::new(
            camera_angle.y.cos() * camera_angle.x.sin(),
            camera_angle.y.sin(),
            camera_angle.y.cos() * camera_angle.x.cos(),
        );

        // Mouse interaction
        if window.get_mouse_button(glfw::MouseButtonLeft) == Action::Press {
            let res = world.raycast(eye_pos, look_dir, 8.0);
            if res.hit {
                world.set_block(res.x, res.y, res.z, BlockType::Air);
            }
        }
        if window.get_mouse_button(glfw::MouseButtonRight) == Action::Press {
            let res = world.raycast(eye_pos, look_dir, 8.0);
            if res.hit {
                let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
                if world.get_block(nx, ny, nz) == BlockType::Air {
                    world.set_block(nx, ny, nz, BlockType::Stone); // Default to stone for now
                }
            }
        }

        player.update(&world, move_dir, delta_time, is_sprinting);
        world.update(player.position, current_time as f32);
        world.update_clouds(player.position, current_time as f32);

        let sun_angle = (current_time as f32 / 1200.0) * 2.0 * std::f32::consts::PI;
        let sun_y = sun_angle.sin();

        let mut sky_c = glam::Vec4::new(0.5, 0.8, 0.9, 1.0); // Default skyblue
        
        if sun_y > 0.3 {
            sky_c = glam::Vec4::new(135.0/255.0, 206.0/255.0, 235.0/255.0, 1.0);
        } else if sun_y > -0.2 {
            let mut t = (sun_y + 0.2) / 0.5;
            t = t.clamp(0.0, 1.0);
            
            if t < 0.5 {
                let u = t * 2.0;
                sky_c.x = 255.0/255.0 * u + 135.0/255.0 * (1.0 - u);
                sky_c.y = 160.0/255.0 * u + 100.0/255.0 * (1.0 - u);
                sky_c.z = 180.0/255.0 * u + 150.0/255.0 * (1.0 - u);
            } else {
                let u = (t - 0.5) * 2.0;
                sky_c.x = 135.0/255.0 * u + 255.0/255.0 * (1.0 - u);
                sky_c.y = 206.0/255.0 * u + 160.0/255.0 * (1.0 - u);
                sky_c.z = 235.0/255.0 * u + 180.0/255.0 * (1.0 - u);
            }
        } else if sun_y > -0.5 {
            let t = (sun_y + 0.5) / 0.3;
            sky_c.x = 30.0/255.0 * t + 10.0/255.0 * (1.0 - t);
            sky_c.y = 30.0/255.0 * t + 10.0/255.0 * (1.0 - t);
            sky_c.z = 80.0/255.0 * t + 50.0/255.0 * (1.0 - t);
        } else {
            sky_c = glam::Vec4::new(5.0/255.0, 5.0/255.0, 15.0/255.0, 1.0);
        }

        unsafe {
            gl::ClearColor(sky_c.x, sky_c.y, sky_c.z, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        // Camera matrices
        let (width, height) = window.get_framebuffer_size();
        let aspect = width as f32 / height.max(1) as f32;
        let projection = Mat4::perspective_rh_gl(75.0_f32.to_radians(), aspect, 0.01, 1000.0);
        let view = Mat4::look_at_rh(eye_pos, eye_pos + look_dir, Vec3::Y);
        let mvp = projection * view;
        let model_mat = Mat4::IDENTITY;

        shader.bind();
        let loc_mvp = shader.get_uniform_location("uMVP");
        let loc_model = shader.get_uniform_location("uModel");
        let loc_precision = shader.get_uniform_location("uPrecision");
        let loc_sundir = shader.get_uniform_location("sunDir");
        let loc_col_diffuse = shader.get_uniform_location("colDiffuse");
        let loc_viewpos = shader.get_uniform_location("viewPos");
        let loc_skycol = shader.get_uniform_location("skyCol");

        shader.set_mat4(loc_mvp, &mvp);
        shader.set_mat4(loc_model, &model_mat);
        shader.set_float(loc_precision, 960.0);
        shader.set_vec3(loc_sundir, glam::Vec3::new(0.0, sun_y, sun_angle.cos()));
        shader.set_vec4(loc_col_diffuse, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
        shader.set_vec3(loc_viewpos, eye_pos);
        shader.set_vec4(loc_skycol, sky_c);

        // Render celestial
        world.render_stars(player.position, current_time as f32, &flat_shader, &mvp);
        draw_sun_moon(&flat_shader, player.position, current_time as f32, mvp);

        // Render world here
        shader.bind();
        world.render_opaque();
        
        // Render clouds
        flat_shader.bind();
        flat_shader.set_mat4(flat_shader.get_uniform_location("uMVP"), &mvp);
        world.render_clouds();
        
        shader.bind();
        world.render_transparent();

        window.swap_buffers();
    }
}
