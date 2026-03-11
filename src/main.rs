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
        if (pos.w > 0.1) {
            snappedPos = floor(snappedPos * uPrecision + 0.5) / uPrecision;
        }
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

struct Player {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
}

impl Player {
    fn update(&mut self, world: &World, move_dir: Vec3, dt: f32) {
        let speed = 15.0;
        let gravity = -30.0;
        let jump_force = 10.0;

        self.velocity.x = move_dir.x * speed;
        self.velocity.z = move_dir.z * speed;
        self.velocity.y += gravity * dt;

        let next_pos = self.position + self.velocity * dt;
        
        // Simple voxel collision (check only for Air/Water)
        let is_solid = |pos: Vec3| {
            let b = world.get_block(pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32);
            b != BlockType::Air && b != BlockType::Water
        };

        // Y collision
        let mut final_pos = self.position;
        if is_solid(Vec3::new(final_pos.x, next_pos.y, final_pos.z)) || 
           is_solid(Vec3::new(final_pos.x, next_pos.y + 1.8, final_pos.z)) {
            if self.velocity.y < 0.0 {
                self.grounded = true;
            }
            self.velocity.y = 0.0;
        } else {
            final_pos.y = next_pos.y;
            self.grounded = false;
        }

        // X collision
        if !is_solid(Vec3::new(next_pos.x, final_pos.y + 0.1, final_pos.z)) &&
           !is_solid(Vec3::new(next_pos.x, final_pos.y + 1.7, final_pos.z)) {
            final_pos.x = next_pos.x;
        }

        // Z collision
        if !is_solid(Vec3::new(final_pos.x, final_pos.y + 0.1, next_pos.z)) &&
           !is_solid(Vec3::new(final_pos.x, final_pos.y + 1.7, next_pos.z)) {
            final_pos.z = next_pos.z;
        }

        self.position = final_pos;
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

    let shader = Shader::new(PS1_VS, PS1_FS).expect("Failed to compile PS1 shader");

    let mut player = Player {
        position: Vec3::new(0.0, 150.0, 0.0),
        velocity: Vec3::ZERO,
        grounded: false,
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
                    if player.grounded {
                        player.velocity.y = 10.0;
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

        player.update(&world, move_dir, delta_time);
        world.update(player.position, current_time as f32);

        unsafe {
            gl::ClearColor(0.5, 0.8, 0.9, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        // Camera matrices
        let (width, height) = window.get_framebuffer_size();
        let aspect = width as f32 / height.max(1) as f32;
        let projection = Mat4::perspective_rh_gl(75.0_f32.to_radians(), aspect, 0.01, 1000.0);
        let eye_pos = player.position + Vec3::new(0.0, 1.6, 0.0);
        let target = eye_pos + Vec3::new(
            camera_angle.y.cos() * camera_angle.x.sin(),
            camera_angle.y.sin(),
            camera_angle.y.cos() * camera_angle.x.cos(),
        );
        let view = Mat4::look_at_rh(eye_pos, target, Vec3::Y);
        let mvp = projection * view;
        let model_mat = Mat4::IDENTITY;

        let sun_angle = (current_time as f32 / 1200.0) * 2.0 * std::f32::consts::PI;
        let sun_y = sun_angle.sin();

        shader.bind();
        let loc_mvp = shader.get_uniform_location("uMVP");
        let loc_model = shader.get_uniform_location("uModel");
        let loc_precision = shader.get_uniform_location("uPrecision");
        let loc_sundir = shader.get_uniform_location("sunDir");
        let loc_col_diffuse = shader.get_uniform_location("colDiffuse");
        let loc_viewpos = shader.get_uniform_location("viewPos");

        shader.set_mat4(loc_mvp, &mvp);
        shader.set_mat4(loc_model, &model_mat);
        shader.set_float(loc_precision, 240.0);
        shader.set_vec3(loc_sundir, glam::Vec3::new(0.0, sun_y, sun_angle.cos()));
        shader.set_vec4(loc_col_diffuse, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
        shader.set_vec3(loc_viewpos, eye_pos);

        // Render world here
        world.render();

        window.swap_buffers();
    }
}
