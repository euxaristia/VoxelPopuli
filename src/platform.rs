//! Services whose implementations differ between the desktop and browser.
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;

pub fn args() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args().collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        vec!["VoxelPopuli".into()]
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use rayon::spawn;

// World limits dispatches to one job per frame on the web. Results still
// travel through the existing channels and retain their stale-job checks.
#[cfg(target_arch = "wasm32")]
pub fn spawn(job: impl FnOnce() + 'static) {
    job();
}

pub fn read(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let path = path.as_ref().to_string_lossy().replace('\\', "/");
        let bytes: &[u8] = match path.as_str() {
            "assets/font.png" => include_bytes!("../assets/font.png"),
            "assets/shaders/celestial.wgsl" => include_bytes!("../assets/shaders/celestial.wgsl"),
            "assets/shaders/ps1.wgsl" => include_bytes!("../assets/shaders/ps1.wgsl"),
            "assets/shaders/gbuffer.wgsl" => include_bytes!("../assets/shaders/gbuffer.wgsl"),
            "assets/shaders/water.wgsl" => include_bytes!("../assets/shaders/water.wgsl"),
            "assets/shaders/flat.wgsl" => include_bytes!("../assets/shaders/flat.wgsl"),
            "assets/shaders/ui.wgsl" => include_bytes!("../assets/shaders/ui.wgsl"),
            "assets/shaders/texture.wgsl" => include_bytes!("../assets/shaders/texture.wgsl"),
            "assets/shaders/ui_texture.wgsl" => include_bytes!("../assets/shaders/ui_texture.wgsl"),
            "assets/shaders/color.wgsl" => include_bytes!("../assets/shaders/color.wgsl"),
            "assets/shaders/deferred_lighting.wgsl" => {
                include_bytes!("../assets/shaders/deferred_lighting.wgsl")
            }
            "assets/shaders/tonemap.wgsl" => include_bytes!("../assets/shaders/tonemap.wgsl"),
            "assets/shaders/ao.wgsl" => include_bytes!("../assets/shaders/ao.wgsl"),
            "assets/shaders/atmosphere.wgsl" => include_bytes!("../assets/shaders/atmosphere.wgsl"),
            "assets/shaders/atmosphere_composite.wgsl" => {
                include_bytes!("../assets/shaders/atmosphere_composite.wgsl")
            }
            "assets/shaders/bloom_composite.wgsl" => {
                include_bytes!("../assets/shaders/bloom_composite.wgsl")
            }
            "assets/shaders/bloom_down.wgsl" => include_bytes!("../assets/shaders/bloom_down.wgsl"),
            "assets/shaders/bloom_extract.wgsl" => {
                include_bytes!("../assets/shaders/bloom_extract.wgsl")
            }
            "assets/shaders/bloom_up.wgsl" => include_bytes!("../assets/shaders/bloom_up.wgsl"),
            "assets/shaders/cinematic_common.wgsl" => {
                include_bytes!("../assets/shaders/cinematic_common.wgsl")
            }
            "assets/shaders/cinematic_glass.wgsl" => {
                include_bytes!("../assets/shaders/cinematic_glass.wgsl")
            }
            "assets/shaders/cinematic_lighting.wgsl" => {
                include_bytes!("../assets/shaders/cinematic_lighting.wgsl")
            }
            "assets/shaders/cinematic_water.wgsl" => {
                include_bytes!("../assets/shaders/cinematic_water.wgsl")
            }
            "assets/shaders/post_common.wgsl" => {
                include_bytes!("../assets/shaders/post_common.wgsl")
            }
            "assets/shaders/shadow.wgsl" => include_bytes!("../assets/shaders/shadow.wgsl"),
            "assets/shaders/temporal.wgsl" => include_bytes!("../assets/shaders/temporal.wgsl"),
            "assets/shaders/water_depth.wgsl" => {
                include_bytes!("../assets/shaders/water_depth.wgsl")
            }
            "assets/shaders/chunk_mesh_test.wgsl" => {
                include_bytes!("../assets/shaders/chunk_mesh_test.wgsl")
            }
            _ => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, path)),
        };
        Ok(bytes.to_vec())
    }
}

pub fn read_to_string(path: impl AsRef<std::path::Path>) -> std::io::Result<String> {
    String::from_utf8(read(path)?)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}
