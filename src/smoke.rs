use crate::block::BlockType;
use crate::container::{Container, Furnace};
use crate::inventory::{INVENTORY_SLOT_COUNT, ItemStack};
use crate::renderer::{self, RenderTexture2D, Shader, Texture2D};

/// An opaque near-plane surface must not erase the arm or held item.
pub fn hand_occlusion(
    shader: &Shader,
    atlas: &Texture2D,
    arm: &renderer::Mesh,
    width: i32,
    height: i32,
) -> Result<(), String> {
    use glam::{Mat4, Vec3, Vec4};
    let target = RenderTexture2D::new(800, 450);
    let wall = renderer::Mesh::new(
        &[
            -1., -1., 0.01, 1., -1., 0.01, 1., 1., 0.01, -1., -1., 0.01, 1., 1., 0.01, -1., 1.,
            0.01,
        ],
        Some(&[0.5, 0.5].repeat(6)),
        None,
        Some(&[255, 255, 255, 255].repeat(6)),
    );
    let white = Texture2D::from_data(&[255, 255, 255, 255], 1, 1);
    let item = crate::hand::build_item_mesh(BlockType::Stone);
    for (pose, swing, held) in [
        ("idle", 0., None),
        ("punch", 1., None),
        ("item", 0.5, Some(&item)),
    ] {
        let mut reference = None;
        for occluded in [false, true] {
            target.bind();
            renderer::clear(0., 0., 0., 1.);
            renderer::set_depth_test(true);
            renderer::set_depth_write(true);
            renderer::set_cull(false);
            renderer::set_blend(false);
            shader.bind();
            shader.set_float(shader.get_uniform_location("uFogDensity"), 0.);
            shader.set_float(shader.get_uniform_location("uHdrScale"), 1.);
            shader.set_int(shader.get_uniform_location("uHdrOutput"), 0);
            shader.set_vec3(shader.get_uniform_location("sunDir"), Vec3::Y);
            shader.set_vec3(shader.get_uniform_location("viewPos"), Vec3::ZERO);
            shader.set_vec4(
                shader.get_uniform_location("uColor"),
                Vec4::new(1., 0., 1., 1.),
            );
            if occluded {
                white.bind(0);
                shader.set_mat4(shader.get_uniform_location("uMVP"), &Mat4::IDENTITY);
                shader.set_mat4(shader.get_uniform_location("uModel"), &Mat4::IDENTITY);
                shader.set_vec4(
                    shader.get_uniform_location("colDiffuse"),
                    Vec4::new(0.1, 0.3, 0.7, 1.),
                );
                wall.draw();
            }
            crate::hand::draw(
                shader,
                atlas,
                arm,
                held,
                &Mat4::IDENTITY,
                0,
                800. / 450.,
                swing,
                0.,
                Vec4::new(1., 0., 1., 1.),
            );
            RenderTexture2D::unbind();
            renderer::end_frame(width, height);
            let path = std::path::Path::new("target/test-artifacts")
                .join(format!("hand-{pose}-wall-{occluded}.png"));
            target.save_png(&path)?;
            let pixels = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
            if let Some(baseline) = reference.as_ref() {
                let baseline: &image::RgbaImage = baseline;
                assert_ne!(
                    baseline.get_pixel(0, 0),
                    pixels.get_pixel(0, 0),
                    "wall was not rendered"
                );
                let mut arm_pixels = 0;
                for (before, after) in baseline.pixels().zip(pixels.pixels()) {
                    if before.0[..3].iter().any(|&v| v > 5) {
                        assert_eq!(before, after, "nearby surface hid the {pose} viewmodel");
                        arm_pixels += 1;
                    }
                }
                assert!(arm_pixels > 1000, "viewmodel was absent");
                println!(
                    "Hand {pose}: {arm_pixels} pixels remain visible against a near-plane wall"
                );
            } else {
                reference = Some(pixels);
            }
        }
    }
    Ok(())
}

pub fn hand_input(frame: u32) -> Option<glfw::WindowEvent> {
    let action = match frame % 72 {
        3 | 27 => glfw::Action::Press,
        4 | 61 => glfw::Action::Release,
        _ => return None,
    };
    Some(glfw::WindowEvent::MouseButton(
        glfw::MouseButtonLeft,
        action,
        glfw::Modifiers::empty(),
    ))
}

pub fn hand_capture(frame: u32) -> Option<String> {
    let pose = match frame % 72 {
        0 => "idle",
        12 => "punch",
        23 => "returned",
        36 => "held-first",
        55 => "held-repeat",
        71 => "released",
        _ => return None,
    };
    let mode = if frame < 72 { "fancy" } else { "fast" };
    Some(format!("hand-{mode}-{pose}"))
}

pub fn verify_hand_frame(frame: u32, amount: f32) {
    match frame % 72 {
        0 | 23 | 71 => assert_eq!(amount, 0.0, "the arm did not return to rest"),
        12 | 36 | 55 => assert!(
            amount > 0.9,
            "clicking/holding attack did not punch: frame={frame}, amount={amount}"
        ),
        _ => unreachable!(),
    }
}

pub fn water_lighting(
    shader: &Shader,
    atlas: &Texture2D,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let target = RenderTexture2D::new(128, 128);
    let vertices = [
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0,
        0.0,
    ];
    let uv = [13.5 / 16.0, 12.5 / 16.0].repeat(6);
    let normals = [0.0, 1.0, 0.0].repeat(6);
    let mut brightness = Vec::new();
    for (name, sun_y, lighting) in [
        ("day", 1.0, [255, 0, 255, 180]),
        ("night", -1.0, [255, 0, 255, 180]),
        ("torch", -1.0, [0, 255, 255, 180]),
    ] {
        target.bind();
        renderer::clear(0.0, 0.0, 0.0, 1.0);
        renderer::set_depth_test(false);
        renderer::set_cull(false);
        renderer::set_blend(false);
        atlas.bind(0);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &glam::Mat4::IDENTITY);
        shader.set_mat4(shader.get_uniform_location("uModel"), &glam::Mat4::IDENTITY);
        shader.set_vec3(
            shader.get_uniform_location("sunDir"),
            glam::Vec3::new(0.0, sun_y, 0.0),
        );
        shader.set_vec3(
            shader.get_uniform_location("viewPos"),
            glam::Vec3::new(0.0, 1.0, 3.0),
        );
        shader.set_vec4(
            shader.get_uniform_location("skyCol"),
            glam::Vec4::new(0.53, 0.75, 0.85, 1.0),
        );
        shader.set_float(shader.get_uniform_location("uTime"), 0.0);
        shader.set_float(shader.get_uniform_location("uHdrScale"), 1.0);
        shader.set_int(shader.get_uniform_location("uHdrOutput"), 0);
        shader.set_float(shader.get_uniform_location("uFogDensity"), 0.0);
        let mesh = renderer::Mesh::new(
            &vertices,
            Some(&uv),
            Some(&normals),
            Some(&lighting.repeat(6)),
        );
        mesh.draw();
        RenderTexture2D::unbind();
        renderer::end_frame(width, height);
        let path = std::path::Path::new("target/test-artifacts").join(format!("water-{name}.png"));
        target.save_png(&path)?;
        let screenshot = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
        let [r, g, b, _] = screenshot.get_pixel(64, 64).0;
        if g as f32 <= r as f32 * 1.3 || (g as f32) < b as f32 * 0.55 {
            return Err(format!(
                "Water lighting changed its hue in {name}: {r},{g},{b}"
            ));
        }
        brightness.push(r as u32 + g as u32 + b as u32);
        println!("Water {name}: RGB={r},{g},{b}");
    }
    if brightness[1] >= brightness[0] / 2 || brightness[2] <= brightness[1] {
        return Err(format!(
            "Water ignored daylight/block lighting: {brightness:?}"
        ));
    }
    Ok(())
}

pub fn lighting_time(frame: u32) -> f32 {
    [300.0, 582.0, 900.0, 18.0][(frame / 8) as usize]
}

pub fn lighting_capture(frame: u32) -> Option<String> {
    (frame % 8 == 7).then(|| {
        format!(
            "lighting-{}",
            ["noon", "sunset", "midnight", "sunrise"][(frame / 8) as usize]
        )
    })
}

pub fn capture_lighting(scene: &RenderTexture2D, name: &str) -> Result<(), String> {
    let path =
        std::path::Path::new("target/test-artifacts").join(format!("world-{name}-scene.png"));
    scene.save_png(&path)?;
    println!("Rendered {}", frame_capture_path(name)?.display());
    Ok(())
}

pub fn verify_lighting_cycle() -> Result<(), String> {
    let mut brightness = Vec::new();
    for name in ["noon", "sunset", "midnight", "sunrise"] {
        let capture = image::open(frame_capture_path(&format!("lighting-{name}"))?)
            .map_err(|e| e.to_string())?
            .to_rgb8();
        // Sample terrain/water away from the sky, arm, and HUD.
        let pixels: Vec<_> = capture
            .enumerate_pixels()
            .filter(|(x, y, _)| {
                *x < capture.width() / 2
                    && *y > capture.height() / 2
                    && *y < capture.height() * 4 / 5
            })
            .map(|(_, _, p)| p)
            .collect();
        let average = pixels
            .iter()
            .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
            .sum::<f32>()
            / pixels.len() as f32;
        if average < 8.0 {
            return Err(format!("{name} terrain is unreadable: average={average}"));
        }
        brightness.push(average);
        println!("Lighting {name}: terrain luminance={average:.1}");
    }
    if brightness[2] >= brightness[0] * 0.65 {
        return Err(format!("Exposure flattened day and night: {brightness:?}"));
    }
    Ok(())
}

/// Controlled GPU probes exercise the production shaders, attachments, and tone mapper.
pub fn shader_lighting(
    gbuffer: &Shader,
    forward: &Shader,
    flat: &Shader,
    width: i32,
    height: i32,
) -> Result<(), String> {
    use glam::{Mat4, Vec3, Vec4};
    use renderer::DeferredUniforms;
    renderer::deferred_resize(128, 128);
    let target = RenderTexture2D::new(128, 128);
    let texture = Texture2D::from_data(&[128, 128, 128, 255], 1, 1);
    let vertices = [
        -1.0, -1.0, 0.5, 1.0, -1.0, 0.5, 1.0, 1.0, 0.5, -1.0, -1.0, 0.5, 1.0, 1.0, 0.5, -1.0, 1.0,
        0.5,
    ];
    let uv = [0.5, 0.5].repeat(6);
    let normals = [0.0, 1.0, 0.0].repeat(6);
    let mut samples = Vec::new();
    for (name, distance, sky, density, mode) in [
        ("linear-albedo", 1.0, 0, 0.0, 0),
        (
            "near-haze",
            1.0,
            255,
            crate::vibrant::frame::HAZE_DENSITY,
            0,
        ),
        (
            "far-haze",
            600.0,
            255,
            crate::vibrant::frame::HAZE_DENSITY,
            0,
        ),
        (
            "sealed-cave",
            600.0,
            0,
            crate::vibrant::frame::HAZE_DENSITY,
            0,
        ),
        ("forward-linear", 1.0, 255, 0.0, 1),
        ("flat-linear", 1.0, 255, 0.0, 2),
        ("hidden-sun", 1.0, 0, 0.0, 3),
        ("hidden-sun-no-glare", 1.0, 0, 0.0, 4),
    ] {
        let transform = Mat4::from_translation(Vec3::new(0.0, 0.0, -distance - 0.5));
        let mut uniforms = DeferredUniforms {
            inv_view_proj: transform.to_cols_array(),
            camera_pos_exposure: [0.0, 0.0, 0.0, 1.0],
            sun_direction_illuminance: [0.0, -1.0, 0.0, std::f32::consts::PI],
            moon_direction_illuminance: [0.0; 4],
            ambient_color_illuminance: [1.0, 1.0, 1.0, std::f32::consts::PI],
            sky_params: [0.0, 0.0, 0.0, density],
            horizon_color: [0.2, 0.4, 0.7, 0.0],
            zenith_color: [0.2, 0.4, 0.7, 0.0],
            atmosphere: [1.0, 0.0, 0.0, 16.0],
            ..DeferredUniforms::default()
        };
        if mode >= 3 {
            let down = Vec3::new(0.0, -1.0, -1.0).normalize();
            uniforms.inv_view_proj =
                Mat4::from_translation(Vec3::new(0.0, -1.0, -2.0)).to_cols_array();
            uniforms.sun_direction_illuminance = [down.x, down.y, down.z, 0.0];
            uniforms.moon_direction_illuminance = [-down.x, -down.y, -down.z, std::f32::consts::PI];
            uniforms.atmosphere[1] = if mode == 3 { 10.0 } else { 0.0 };
        }
        renderer::set_depth_test(true);
        renderer::set_depth_write(true);
        renderer::set_cull(false);
        renderer::set_blend(false);
        renderer::deferred_begin_geometry();
        let shader = match mode {
            1 => forward,
            2 => flat,
            _ => gbuffer,
        };
        texture.bind(0);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &transform.inverse());
        shader.set_mat4(shader.get_uniform_location("uModel"), &transform);
        shader.set_vec4(shader.get_uniform_location("colDiffuse"), Vec4::ONE);
        shader.set_vec4(shader.get_uniform_location("uColor"), Vec4::ZERO);
        shader.set_vec3(shader.get_uniform_location("sunDir"), Vec3::Y);
        shader.set_vec3(shader.get_uniform_location("viewPos"), Vec3::ZERO);
        shader.set_float(shader.get_uniform_location("uHdrScale"), 1.0);
        shader.set_int(shader.get_uniform_location("uHdrOutput"), 1);
        shader.set_float(shader.get_uniform_location("uFogDensity"), 0.0);
        shader.set_int(shader.get_uniform_location("uBodyType"), 0);
        let colors = if mode == 2 {
            [128, 128, 128, 255]
        } else {
            [sky, 0, 255, 255]
        };
        let mesh = renderer::Mesh::new(
            &vertices,
            Some(&uv),
            Some(&normals),
            Some(&colors.repeat(6)),
        );
        if mode == 0 {
            mesh.draw();
        }
        renderer::deferred_resolve(&uniforms);
        if mode == 1 || mode == 2 {
            shader.bind();
            if mode == 2 {
                shader.set_mat4(shader.get_uniform_location("uMVP"), &Mat4::IDENTITY);
            }
            mesh.draw();
        }
        target.bind();
        renderer::deferred_tonemap();
        RenderTexture2D::unbind();
        renderer::end_frame(width, height);
        let path = std::path::Path::new("target/test-artifacts").join(format!("shader-{name}.png"));
        target.save_png(&path)?;
        let capture = image::open(&path).map_err(|e| e.to_string())?.to_rgb8();
        let rgb = capture.get_pixel(64, 64).0;
        println!("Shader {name}: {rgb:?}");
        samples.push(rgb);
    }
    // sRGB 128 decodes to ~0.216, which ACES + the display transfer maps to 154.
    for i in [0, 4, 5] {
        if !(151..=158).contains(&samples[i][1]) {
            return Err(format!("sRGB was not decoded before lighting: {samples:?}"));
        }
    }
    if samples[1] != samples[0]
        || samples[3] != samples[0]
        || samples[2][2].saturating_sub(samples[2][0]) < 30
    {
        return Err(format!(
            "Haze failed distance/sky-visibility checks: {samples:?}"
        ));
    }
    if samples[6] != samples[7] {
        return Err(format!(
            "A hidden sun still produces glare: {:?} vs {:?}",
            samples[6], samples[7]
        ));
    }
    // Exercise the production sky draw functions with a foreground occluder.
    let mut sky_world = crate::world::World::new(42);
    sky_world.star_model = Some(renderer::Mesh::new(&vertices, None, None, Some(&[255; 24])));
    sky_world.cloud_model = Some(renderer::Mesh::new(&vertices, None, None, Some(&[255; 24])));
    let foreground = [
        -1.0, -1.0, 0.2, 0.0, -1.0, 0.2, 0.0, 1.0, 0.2, -1.0, -1.0, 0.2, 0.0, 1.0, 0.2, -1.0, 1.0,
        0.2,
    ];
    let occluder = renderer::Mesh::new(&foreground, None, None, Some(&[20, 30, 40, 255].repeat(6)));
    for clouds in [false, true] {
        target.bind();
        renderer::clear(0.0, 0.0, 0.0, 1.0);
        renderer::set_depth_test(true);
        renderer::set_depth_write(true);
        renderer::set_cull(false);
        renderer::set_blend(false);
        flat.bind();
        flat.set_mat4(flat.get_uniform_location("uMVP"), &Mat4::IDENTITY);
        flat.set_float(flat.get_uniform_location("uHdrScale"), 1.0);
        flat.set_int(flat.get_uniform_location("uHdrOutput"), 0);
        flat.set_int(flat.get_uniform_location("uBodyType"), 0);
        flat.set_vec4(
            flat.get_uniform_location("colDiffuse"),
            Vec4::new(0.1, 0.2, 0.3, 1.0),
        );
        occluder.draw();
        if clouds {
            sky_world.render_clouds(flat, &Mat4::IDENTITY);
        } else {
            sky_world.render_stars(Vec3::ZERO, 900.0, flat, &Mat4::IDENTITY);
        }
        RenderTexture2D::unbind();
        renderer::end_frame(width, height);
        let name = if clouds {
            "cloud-tint"
        } else {
            "star-occlusion"
        };
        let path = std::path::Path::new("target/test-artifacts").join(format!("shader-{name}.png"));
        target.save_png(&path)?;
        let capture = image::open(&path).map_err(|e| e.to_string())?.to_rgb8();
        let covered = capture.get_pixel(32, 64).0;
        let visible = capture.get_pixel(96, 64).0;
        if covered != [20, 30, 40]
            || (!clouds && visible != [255; 3])
            || (clouds && !(24..=27).contains(&visible[0]))
        {
            return Err(format!(
                "{name} failed: occluded={covered:?}, visible={visible:?}"
            ));
        }
        println!("Shader {name}: occluded={covered:?}, visible={visible:?}");
    }
    Ok(())
}

pub fn frame_capture_path(mode: &str) -> Result<std::path::PathBuf, String> {
    let directory = std::path::Path::new("target/test-artifacts");
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    Ok(directory.join(format!("world-{mode}-frame.png")))
}

pub fn capture_world(scene: &RenderTexture2D, mode: &str) -> Result<(), String> {
    for name in ["scene", "frame"] {
        let path =
            std::path::Path::new("target/test-artifacts").join(format!("world-{mode}-{name}.png"));
        if name == "scene" {
            scene.save_png(&path)?;
        }
        let screenshot = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
        let visible = screenshot
            .pixels()
            .filter(|p| p[0].max(p[1]).max(p[2]) > 12)
            .count();
        // This fixed coastal seed must show both sky and vegetation. A lit
        // cave wall or the HUD alone cannot satisfy the rendering check.
        let sky = screenshot
            .pixels()
            .filter(|p| p[2].saturating_sub(p[0]) > 20 && p[2] >= p[1])
            .count();
        let vegetation = screenshot
            .pixels()
            .filter(|p| p[1].saturating_sub(p[0]) > 10 && p[1].saturating_sub(p[2]) > 10)
            .count();
        let total = (screenshot.width() * screenshot.height()) as usize;
        if visible < total / 2 || sky < total / 50 || vegetation < total / 50 {
            return Err(format!(
                "{mode} {name} does not show the surface world: visible={visible}, sky={sky}, vegetation={vegetation}, total={total}"
            ));
        }
        println!("Rendered {}: {visible} visible pixels", path.display());
    }
    Ok(())
}

/// Render the production container UI without opening a game or touching a save.
pub fn container_ui() -> Result<(), String> {
    let mut glfw = glfw::init(glfw::log_errors).map_err(|e| e.to_string())?;
    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw.window_hint(glfw::WindowHint::Visible(false));
    let (window, _) = glfw
        .create_window(
            900,
            700,
            "Container UI smoke test",
            glfw::WindowMode::Windowed,
        )
        .ok_or("Could not create the test window")?;
    let _renderer = renderer::init(&*window, 900, 700);
    let ui = Shader::new(&crate::load_shader("ui.wgsl"))?;
    let textured = Shader::new(&crate::load_shader("ui_texture.wgsl"))?;
    let atlas = Texture2D::from_data(&crate::atlas::generate_atlas_data(), 256, 256);
    let font = Texture2D::from_file("assets/font.png");
    let target = RenderTexture2D::new(900, 700);
    let mut inventory = [None; INVENTORY_SLOT_COUNT];
    inventory[0] = Some(ItemStack::new_tool(BlockType::StonePickaxe));
    inventory[1] = Some(ItemStack::new(BlockType::OakLog, 12));
    inventory[9] = Some(ItemStack::new(BlockType::RawBeef, 3));
    let mut chest = [None; crate::container::CHEST_SLOTS];
    chest[0] = Some(ItemStack::new(BlockType::Cobblestone, 64));
    chest[1] = Some(ItemStack::new(BlockType::IronIngot, 12));
    chest[10] = Some(ItemStack {
        durability: Some(27),
        ..ItemStack::new_tool(BlockType::IronPickaxe)
    });
    let mut furnace = Furnace {
        slots: [
            Some(ItemStack::new(BlockType::IronOre, 8)),
            Some(ItemStack::new(BlockType::Coal, 4)),
            None,
        ],
        ..Furnace::default()
    };
    furnace.tick(15.0);
    let directory = std::path::Path::new("target/test-artifacts");
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    for (name, container) in [
        ("chest", Container::Chest(Box::new(chest))),
        ("furnace", Container::Furnace(furnace)),
    ] {
        target.bind();
        renderer::clear(0.12, 0.16, 0.20, 1.0);
        crate::container_ui::draw_container(
            &container,
            &inventory,
            None,
            (315.0, 270.0),
            &atlas,
            &ui,
            &textured,
            &font,
            900.0,
            700.0,
        );
        RenderTexture2D::unbind();
        renderer::end_frame(900, 700);
        let path = directory.join(format!("{name}.png"));
        target.save_png(&path)?;
        let screenshot = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
        if screenshot.get_pixel(450, 350).0 == screenshot.get_pixel(0, 0).0 {
            return Err(format!("{name} panel was not rendered"));
        }
        println!("Rendered {}", path.display());
    }
    let mut world = crate::world::World::new(42);
    world.insert_chunk(crate::chunk::Chunk::new(0, 0, 42));
    world.set_block(2, 90, 2, BlockType::Chest);
    assert!(world.open_container((2, 90, 2)));
    world.containers.get_mut(&(2, 90, 2)).unwrap().click(
        &mut inventory,
        &mut None,
        100,
        false,
        true,
    );
    let player = crate::player::Player::new(91.0);
    let save = crate::save::GameSave::capture(
        &world,
        &player,
        &inventory,
        glam::Vec2::ZERO,
        crate::save::GameSettings::default(),
        None,
        Some(ItemStack::new(BlockType::Coal, 2)),
        &[None; 10],
    )
    .map_err(|e| e.to_string())?;
    let save_path = directory.join("container-smoke.vps");
    save.write_to(&save_path).map_err(|e| e.to_string())?;
    let restored = crate::save::GameSave::read_from(&save_path).map_err(|e| e.to_string())?;
    assert_eq!(restored, save);
    world.containers = restored.containers.into_iter().collect();
    world.set_block(2, 90, 2, BlockType::Air);
    assert_eq!(world.dropped_items.len(), 1);
    assert_eq!(
        world.dropped_items[0].stack,
        ItemStack::new_tool(BlockType::StonePickaxe)
    );
    println!("World container interaction, save, reload, and break passed");
    Ok(())
}

/// Captures every production model with animated and resting poses.
pub fn mobs() -> Result<(), String> {
    use crate::mob::{Mob, MobKind};
    use glam::{Mat4, Vec3, Vec4};
    let mut glfw = glfw::init(glfw::log_errors).map_err(|e| e.to_string())?;
    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw.window_hint(glfw::WindowHint::Visible(false));
    let (window, _) = glfw
        .create_window(
            1600,
            1100,
            "Creature rendering smoke test",
            glfw::WindowMode::Windowed,
        )
        .ok_or("Could not create the test window")?;
    let _renderer = renderer::init(&*window, 1600, 1100);
    let shader = Shader::new(&crate::load_shader("ps1.wgsl"))?;
    let ui = Shader::new(&crate::load_shader("ui.wgsl"))?;
    let text = Shader::new(&crate::load_shader("ui_texture.wgsl"))?;
    let font = Texture2D::from_file("assets/font.png");
    let visuals = crate::mob_visuals::MobVisuals::new();
    let target = RenderTexture2D::new(1600, 1100);
    std::fs::create_dir_all("target/test-artifacts").map_err(|e| e.to_string())?;
    for (page, entries) in MobKind::ALL.chunks(20).enumerate() {
        for pose in 0..3 {
            target.bind();
            renderer::clear(0.105, 0.14, 0.155, 1.0);
            renderer::set_depth_test(true);
            renderer::set_depth_write(true);
            renderer::set_cull(true);
            renderer::set_blend(false);
            for (i, &kind) in entries.iter().enumerate() {
                let mut mob = Mob::new(kind, Vec3::ZERO, Vec3::ZERO, 0);
                mob.yaw = std::f32::consts::FRAC_PI_2;
                mob.walk_phase = if pose == 0 { 0.0 } else { 1.2 };
                mob.walk_blend = if pose == 1 { 1.0 } else { 0.0 };
                let visual_height = crate::mob_visuals::render_height(&mob);
                let size = visual_height.max(kind.species().width * 1.3);
                let eye = if pose == 2 {
                    Vec3::new(size * 2.0, size * 0.65, 0.0)
                } else {
                    Vec3::new(size * 0.9, size * 0.78, size * 1.75)
                };
                let view = glam::camera::rh::view::look_at_mat4(
                    eye,
                    Vec3::Y * visual_height * 0.48,
                    Vec3::Y,
                );
                let proj = glam::camera::rh::proj::directx::perspective(
                    45f32.to_radians(),
                    1.35,
                    0.01,
                    100.0,
                );
                let clip = Mat4::from_translation(Vec3::new(
                    -0.8 + (i % 5) as f32 * 0.4,
                    0.64 - (i / 5) as f32 * 0.47,
                    0.0,
                )) * Mat4::from_scale(Vec3::new(0.19, 0.195, 1.0));
                shader.bind();
                shader.set_mat4(shader.get_uniform_location("uMVP"), &(clip * proj * view));
                shader.set_vec4(
                    shader.get_uniform_location("uColor"),
                    Vec4::new(1.0, 0.0, 1.0, 1.0),
                );
                shader.set_vec3(shader.get_uniform_location("sunDir"), Vec3::Y);
                shader.set_vec3(shader.get_uniform_location("viewPos"), eye);
                shader.set_float(shader.get_uniform_location("uFogDensity"), 0.0);
                shader.set_float(shader.get_uniform_location("uHdrScale"), 1.0);
                shader.set_int(shader.get_uniform_location("uHdrOutput"), 0);
                visuals.draw(
                    &mob,
                    &shader,
                    pose as f32 * 0.22,
                    if pose == 2 { eye * 100.0 } else { eye },
                );
            }
            renderer::set_depth_test(false);
            renderer::set_cull(false);
            renderer::set_blend(true);
            crate::hud::draw_text(
                &font,
                &format!(
                    "OVERWORLD / {} CREATURES / {}",
                    MobKind::ALL.len(),
                    match pose {
                        0 => "REST",
                        1 => "MOVEMENT",
                        _ => "SIDE PROFILE",
                    }
                ),
                32.0,
                25.0,
                24.0,
                &text,
                1600.0,
                1100.0,
            );
            for (i, &kind) in entries.iter().enumerate() {
                crate::hud::draw_text(
                    &font,
                    kind.species().name,
                    22.0 + (i % 5) as f32 * 320.0,
                    294.0 + (i / 5) as f32 * 258.5,
                    17.0,
                    &text,
                    1600.0,
                    1100.0,
                );
            }
            RenderTexture2D::unbind();
            renderer::end_frame(1600, 1100);
            let path = format!("target/test-artifacts/mobs-{}-{pose}.png", page + 1);
            target.save_png(std::path::Path::new(&path))?;
            let capture = image::open(&path).map_err(|e| e.to_string())?.to_rgb8();
            for (i, kind) in entries.iter().enumerate() {
                let x = (i % 5) * 320;
                let y = 85 + (i / 5) * 258;
                let colors: std::collections::HashSet<_> = (x + 35..x + 285)
                    .step_by(3)
                    .flat_map(|px| (y + 10..y + 205).step_by(3).map(move |py| (px, py)))
                    .map(|(px, py)| capture.get_pixel(px as u32, py as u32).0)
                    .collect();
                if colors.len() < 3 {
                    return Err(format!("Missing model pixels for {kind:?}"));
                }
            }
            println!("Rendered {path}");
        }
    }
    target.bind();
    renderer::clear(0.1, 0.12, 0.15, 1.0);
    crate::creature_ui::Catalogue::default().draw(&ui, &text, &font, 1600.0, 1100.0, 12);
    RenderTexture2D::unbind();
    renderer::end_frame(1600, 1100);
    target.save_png(std::path::Path::new(
        "target/test-artifacts/creature-catalogue.png",
    ))?;
    // Consecutive frames, not just disconnected poses, expose wrong joint axes.
    let sequence = RenderTexture2D::new(800, 450);
    let mut dolphin = Mob::new(MobKind::Dolphin, Vec3::ZERO, Vec3::ZERO, 0);
    dolphin.yaw = std::f32::consts::FRAC_PI_2;
    let eye = Vec3::new(2.3, 1.0, 2.2);
    let mvp =
        glam::camera::rh::proj::directx::perspective(42f32.to_radians(), 800.0 / 450.0, 0.05, 30.0)
            * glam::camera::rh::view::look_at_mat4(eye, Vec3::new(0.0, 0.3, -0.15), Vec3::Y);
    for frame in 0..64 {
        sequence.bind();
        renderer::clear(0.09, 0.19, 0.23, 1.0);
        renderer::set_depth_test(true);
        renderer::set_depth_write(true);
        renderer::set_cull(true);
        renderer::set_blend(false);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_vec4(
            shader.get_uniform_location("uColor"),
            Vec4::new(1.0, 0.0, 1.0, 1.0),
        );
        shader.set_vec3(shader.get_uniform_location("sunDir"), Vec3::Y);
        visuals.draw(
            &dolphin,
            &shader,
            frame as f32 * std::f32::consts::PI / 64.0,
            eye,
        );
        RenderTexture2D::unbind();
        renderer::end_frame(1600, 1100);
        sequence.save_png(std::path::Path::new(&format!(
            "target/test-artifacts/dolphin-motion-{frame:02}.png"
        )))?;
    }
    verify_mob_simulation();
    mob_scene()?;
    Ok(())
}

fn verify_mob_simulation() {
    use crate::chunk::Chunk;
    use crate::mob::{Mob, MobKind};
    use crate::world::{MOB_CAP, World};
    use glam::Vec3;
    let mut world = World::new(42);
    assert!(
        world
            .spawn_mob(MobKind::Pig, Vec3::new(8.0, 120.0, 8.0), 0)
            .is_err()
    );
    let mut chunk = Chunk::new(0, 0, 42);
    for column in chunk.blocks.iter_mut() {
        for (y, row) in column.iter_mut().enumerate().take(145).skip(110) {
            row.fill(if y == 110 {
                BlockType::Stone
            } else {
                BlockType::Air
            });
        }
    }
    world.insert_chunk(chunk);
    assert!(
        world
            .spawn_mob(MobKind::Cod, Vec3::new(8.0, 120.0, 8.0), 0)
            .is_err()
    );
    for x in 5..11 {
        for z in 5..11 {
            for y in 115..125 {
                world.set_block(x, y, z, BlockType::Water);
            }
        }
    }
    assert!(
        world
            .spawn_mob(MobKind::Cod, Vec3::new(8.0, 120.0, 8.0), 0)
            .is_ok()
    );
    assert!(
        world
            .spawn_mob(MobKind::Bee, Vec3::new(3.0, 130.0, 3.0), 0)
            .is_ok()
    );
    for _ in 0..100 {
        world.update_mobs(Vec3::new(2.0, 120.0, 2.0), 0.1, BlockType::Air);
    }
    let cod = world.mobs.iter().find(|m| m.kind == MobKind::Cod).unwrap();
    assert_eq!(
        world.get_block(
            cod.position.x.floor() as i32,
            cod.position.y.floor() as i32,
            cod.position.z.floor() as i32
        ),
        BlockType::Water,
        "fish escaped the pond"
    );
    assert!(
        world
            .mobs
            .iter()
            .find(|m| m.kind == MobKind::Bee)
            .unwrap()
            .position
            .y
            > 128.0,
        "flying mob fell to the ground"
    );
    while world.mobs.len() < MOB_CAP {
        world
            .mobs
            .push(Mob::new(MobKind::Pig, Vec3::ZERO, Vec3::ZERO, 0));
    }
    assert_eq!(
        world.spawn_mob(MobKind::Bat, Vec3::new(3.0, 140.0, 3.0), 0),
        Err("Creature limit reached (48).")
    );
    world.mobs.clear();
    let player = Vec3::new(4.0, 111.0, 3.0);
    let zombie = Vec3::new(3.0, 111.0, 3.0);
    world.mobs.push(Mob::new(MobKind::Husk, zombie, zombie, 0));
    world.set_block(3, 112, 3, BlockType::Stone);
    world.pending_hurt = 0;
    world.update_mobs(player, 0.1, BlockType::Air);
    assert_eq!(world.pending_hurt, 0, "melee hit through a wall");
    world.set_block(3, 112, 3, BlockType::Air);
    world.mobs[0].position = zombie;
    world.mobs[0].attack_cooldown = 0.0;
    world.update_mobs(player, 0.1, BlockType::Air);
    assert!(
        world.pending_hurt > 0,
        "unobstructed hostile did not attack"
    );
    world.mobs.clear();
    for kind in [MobKind::Cow, MobKind::Sheep, MobKind::Cow] {
        let p = Vec3::new(3.5, 111.0, 3.5);
        world.mobs.push(Mob::new(kind, p, p, 0));
    }
    for _ in 0..100 {
        world.update_mobs(Vec3::new(8.0, 111.0, 3.5), 0.1, BlockType::Wheat);
        for (i, a) in world.mobs.iter().enumerate() {
            for b in &world.mobs[i + 1..] {
                let d = (a.position - b.position).abs();
                let width = a.half_width() + b.half_width() - 0.01;
                assert!(
                    d.x >= width
                        || d.z >= width
                        || a.position.y >= b.position.y + b.height()
                        || b.position.y >= a.position.y + a.height(),
                    "herd walked through itself"
                );
            }
        }
    }
    println!("Creature simulation: herd separation, collision, water, flight and capacity passed");
}

fn mob_scene() -> Result<(), String> {
    use crate::mob::{Mob, MobKind};
    use glam::Vec3;
    let mut world = crate::world::World::new(42);
    let mut chunk = crate::chunk::Chunk::new(0, 0, 42);
    for x in 0..16 {
        for z in 0..16 {
            for y in 100..145 {
                chunk.blocks[x][y][z] = if y == 110 {
                    BlockType::Grass
                } else {
                    BlockType::Air
                };
                chunk.light[x][y][z] = crate::chunk::pack_light(15, 0);
            }
        }
    }
    world.insert_chunk(chunk);
    world.atlas = Some(Texture2D::from_data(
        &crate::atlas::generate_atlas_data(),
        256,
        256,
    ));
    for (i, kind) in [
        MobKind::Pig,
        MobKind::Cow,
        MobKind::Sheep,
        MobKind::Zombie,
        MobKind::Skeleton,
        MobKind::Creeper,
        MobKind::Villager,
    ]
    .into_iter()
    .enumerate()
    {
        let pos = Vec3::new(2.0 + i as f32 * 1.85, 111.0, 7.0 + (i % 2) as f32);
        let mut mob = Mob::new(kind, pos, pos, 0);
        mob.yaw = std::f32::consts::FRAC_PI_2;
        mob.walk_blend = 1.0;
        mob.walk_phase = i as f32;
        world.mobs.push(mob);
    }
    let mut floor_positions = Vec::new();
    let mut floor_uv = Vec::new();
    let (tx, ty) = crate::item::atlas_uv_top(BlockType::Grass);
    let u = tx as f32 / 16.0;
    let v = ty as f32 / 16.0;
    let tile = 1.0 / 16.0;
    for x in 0..16 {
        for z in 0..16 {
            let (x, z) = (x as f32, z as f32);
            floor_positions.extend_from_slice(&[
                x,
                111.0,
                z + 1.0,
                x + 1.0,
                111.0,
                z + 1.0,
                x + 1.0,
                111.0,
                z,
                x,
                111.0,
                z + 1.0,
                x + 1.0,
                111.0,
                z,
                x,
                111.0,
                z,
            ]);
            floor_uv.extend_from_slice(&[
                u,
                v + tile,
                u + tile,
                v + tile,
                u + tile,
                v,
                u,
                v + tile,
                u + tile,
                v,
                u,
                v,
            ]);
        }
    }
    let floor = renderer::Mesh::new(
        &floor_positions,
        Some(&floor_uv),
        Some(&[0.0, 1.0, 0.0].repeat(floor_positions.len() / 3)),
        Some(&[255, 0, 255, 255].repeat(floor_positions.len() / 3)),
    );
    let fancy = Shader::with_outputs(&crate::load_shader("gbuffer.wgsl"), 4)?;
    let fast = Shader::new(&crate::load_shader("ps1.wgsl"))?;
    let target = RenderTexture2D::new(1600, 900);
    renderer::deferred_resize(1600, 900);
    let eye = Vec3::new(8.0, 114.6, 18.0);
    let mvp = glam::camera::rh::proj::directx::perspective(
        52f32.to_radians(),
        1600.0 / 900.0,
        0.05,
        100.0,
    ) * glam::camera::rh::view::look_at_mat4(eye, Vec3::new(8.0, 111.8, 7.0), Vec3::Y);
    let pack = crate::vibrant::VibrantPack::load_default();
    let uniforms = crate::vibrant::frame::build_uniforms(
        &pack,
        &crate::vibrant::frame::FrameInput {
            day_fraction: 0.0,
            camera_pos: eye,
            view_proj: mvp,
        },
    );
    for deferred in [false, true] {
        let shader = if deferred { &fancy } else { &fast };
        if deferred {
            renderer::deferred_begin_geometry();
        } else {
            target.bind();
            renderer::clear(0.47, 0.69, 0.79, 1.0);
        }
        renderer::set_depth_test(true);
        renderer::set_depth_write(true);
        renderer::set_cull(true);
        renderer::set_blend(false);
        shader.bind();
        shader.set_mat4(shader.get_uniform_location("uMVP"), &mvp);
        shader.set_vec3(shader.get_uniform_location("sunDir"), Vec3::Y);
        shader.set_vec3(shader.get_uniform_location("viewPos"), eye);
        shader.set_float(shader.get_uniform_location("uHdrScale"), 1.0);
        shader.set_int(shader.get_uniform_location("uHdrOutput"), 0);
        // Draw the ground AFTER mobs to exercise block-atlas restoration.
        world.render_mobs(shader, eye);
        floor.draw();
        if deferred {
            renderer::deferred_resolve(&uniforms);
            target.bind();
            renderer::deferred_tonemap();
        }
        RenderTexture2D::unbind();
        renderer::end_frame(1600, 1100);
        let path = format!(
            "target/test-artifacts/mob-scene-{}.png",
            if deferred { "fancy" } else { "fast" }
        );
        target.save_png(std::path::Path::new(&path))?;
        let capture = image::open(&path).map_err(|e| e.to_string())?.to_rgb8();
        let [r, g, b] = capture.get_pixel(800, 800).0;
        if g <= r.saturating_add(15) || g <= b.saturating_add(15) {
            return Err(format!(
                "Mob pass did not restore terrain state: {r},{g},{b}"
            ));
        }
        println!("Rendered {path}");
    }
    Ok(())
}
