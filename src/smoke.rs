use crate::block::BlockType;
use crate::container::{Container, Furnace};
use crate::inventory::{INVENTORY_SLOT_COUNT, ItemStack};
use crate::renderer::{self, RenderTexture2D, Shader, Texture2D};

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
