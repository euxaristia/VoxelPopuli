// wgpu backend that keeps the old GL-style immediate API: shaders hold
// persistent uniform values, meshes draw against whatever state is set,
// and the frame is recorded into passes that submit on end_frame.
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Bytes per chunk slot in the GPU voxel pool (one u8 per voxel, per array).
const CHUNK_VOXELS: u64 = (CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH) as u64;

// One uniform layout shared by every shader. Uniform "locations" are byte
// offsets into this block, so get_uniform_location/set_* keep GL semantics.
// Field offsets must match the Uniforms struct in assets/shaders/*.wgsl.
const UNIFORM_SIZE: usize = 224;
// Dynamic-offset slots must be aligned to the device limit (256 everywhere).
const UNIFORM_STRIDE: usize = 256;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

// Deferred G-buffer attachments (Vibrant Visuals geometry pass).
//   albedo    RGB base color, A alpha coverage. sRGB so the hardware does
//             the encode on write and the decode on read, which keeps dark
//             albedo out of the banding an 8-bit linear store would cause.
//   normal    world-space normal, XYZ; float, because an 8-bit encode
//             visibly facets smooth specular highlights.
//   mers      metalness / emissive / roughness / subsurface, one byte each,
//             matching the packed MERS texture channel order.
//   lighting  the terms the mesher already baked per vertex: R sky
//             visibility, G block light, B ambient occlusion.
const GBUFFER_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const GBUFFER_NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const GBUFFER_MERS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const GBUFFER_LIGHTING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
// Scene radiance before tone mapping. Illuminance is authored in lux, so
// the lighting pass routinely writes values in the thousands.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

fn uniform_offset(name: &str) -> i32 {
    match name {
        "uMVP" => 0,
        "uModel" => 64,
        "colDiffuse" => 128,
        "skyCol" => 144,
        "uColor" => 160,
        "sunDir" => 176,
        "uTime" => 188,
        "viewPos" => 192,
        "time" => 204,
        "uScreenSize" => 208,
        "uBodyType" => 216,
        "uHdrScale" => 220,
        // Unknown names behave like GL's -1 location: sets are ignored.
        _ => -1,
    }
}

fn cast_slice<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => return v,
            std::task::Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct StateFlags {
    blend: bool,
    depth_test: bool,
    depth_write: bool,
    cull: bool,
    polygon_offset: bool,
}

#[derive(Clone, PartialEq)]
enum Target {
    Surface,
    Offscreen {
        color: wgpu::TextureView,
        depth: wgpu::TextureView,
    },
    /// Deferred geometry pass: four color attachments plus depth.
    GBuffer {
        albedo: wgpu::TextureView,
        normal: wgpu::TextureView,
        mers: wgpu::TextureView,
        lighting: wgpu::TextureView,
        depth: wgpu::TextureView,
    },
    /// The lighting resolve. No depth attachment: the resolve writes every
    /// pixel, and it *samples* the G-buffer depth, which a pass may not do
    /// while also holding that texture as a depth attachment.
    HdrResolve {
        color: wgpu::TextureView,
    },
    /// Scene radiance in lux, with the G-buffer depth bound so forward
    /// transparents test against the geometry that was already resolved.
    Hdr {
        color: wgpu::TextureView,
        depth: wgpu::TextureView,
    },
}

/// The attachment layout of a target. Pipelines are compatible across
/// targets of the same kind, so this and not the view identity is what
/// keys the pipeline cache.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum TargetKind {
    Surface,
    Offscreen,
    GBuffer,
    Hdr,
}

impl Target {
    fn kind(&self) -> TargetKind {
        match self {
            Target::Surface => TargetKind::Surface,
            Target::Offscreen { .. } => TargetKind::Offscreen,
            Target::GBuffer { .. } => TargetKind::GBuffer,
            Target::HdrResolve { .. } | Target::Hdr { .. } => TargetKind::Hdr,
        }
    }

    fn color_views(&self, frame_view: &wgpu::TextureView) -> Vec<wgpu::TextureView> {
        match self {
            Target::Surface => vec![frame_view.clone()],
            Target::Offscreen { color, .. }
            | Target::Hdr { color, .. }
            | Target::HdrResolve { color } => vec![color.clone()],
            Target::GBuffer {
                albedo,
                normal,
                mers,
                lighting,
                ..
            } => vec![
                albedo.clone(),
                normal.clone(),
                mers.clone(),
                lighting.clone(),
            ],
        }
    }

    fn depth_view<'a>(
        &'a self,
        surface_depth: &'a wgpu::TextureView,
    ) -> Option<&'a wgpu::TextureView> {
        match self {
            Target::Surface => Some(surface_depth),
            Target::Offscreen { depth, .. }
            | Target::GBuffer { depth, .. }
            | Target::Hdr { depth, .. } => Some(depth),
            Target::HdrResolve { .. } => None,
        }
    }
}

impl TargetKind {
    /// Formats of each color attachment, in binding order. `surface` is
    /// passed in because the swapchain format is chosen at init.
    fn color_formats(self, surface: wgpu::TextureFormat) -> Vec<wgpu::TextureFormat> {
        match self {
            TargetKind::Surface => vec![surface],
            TargetKind::Offscreen => vec![OFFSCREEN_FORMAT],
            TargetKind::GBuffer => vec![
                GBUFFER_ALBEDO_FORMAT,
                GBUFFER_NORMAL_FORMAT,
                GBUFFER_MERS_FORMAT,
                GBUFFER_LIGHTING_FORMAT,
            ],
            TargetKind::Hdr => vec![HDR_FORMAT],
        }
    }
}

// A pooled vertex buffer with a stable identity for rebind elision.
#[derive(Clone)]
struct PoolBuffer {
    id: u64,
    buffer: wgpu::Buffer,
}

struct DrawRec {
    shader: Arc<ShaderInner>,
    flags: StateFlags,
    uniform_offset: u32,
    texture: wgpu::BindGroup,
    texture_id: u64,
    buffer: PoolBuffer,
    vertex_count: u32,
    // When set, the draw's vertex count comes from this GPU-written
    // DrawIndirectArgs buffer (compute-meshed chunks) instead of
    // `vertex_count`, which the CPU never knew.
    indirect: Option<wgpu::Buffer>,
}

// A fullscreen step: the deferred lighting resolve, tone mapping, and the
// post passes that follow them. These bring their own pipeline and bind
// groups instead of going through the GL-style state machine, because they
// read several textures at once and have no vertex buffer.
struct FullscreenRec {
    pipeline: Arc<wgpu::RenderPipeline>,
    bind_groups: Vec<wgpu::BindGroup>,
}

// Steps are one ordered list so a fullscreen resolve lands between the
// draws that precede and follow it, rather than being appended to the pass.
enum Step {
    Draw(DrawRec),
    Fullscreen(FullscreenRec),
}

struct PassRec {
    target: Target,
    clear_color: Option<[f64; 4]>,
    clear_depth: bool,
    steps: Vec<Step>,
}

impl PassRec {
    fn new(target: Target) -> Self {
        Self {
            target,
            clear_color: None,
            clear_depth: false,
            steps: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    fn draws(&self) -> impl Iterator<Item = &DrawRec> {
        self.steps.iter().filter_map(|s| match s {
            Step::Draw(draw) => Some(draw),
            Step::Fullscreen(_) => None,
        })
    }

    fn push_draw(&mut self, draw: DrawRec) {
        self.steps.push(Step::Draw(draw));
    }

    fn last_draw(&self) -> Option<&DrawRec> {
        self.draws().last()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipeKey {
    shader_id: usize,
    flags: StateFlags,
    kind: TargetKind,
}

// Persistent GPU-resident mirror of World's chunk pool, so chunk meshing can
// read neighbor voxels directly instead of gathering a CPU-side snapshot per
// dispatch (see hashed-splashing-haven plan). Populated in M1; not yet read
// by any compute shader.
struct GpuVoxelPool {
    blocks: wgpu::Buffer,
    light: wgpu::Buffer,
    liquid: wgpu::Buffer,
    slot_meta: wgpu::Buffer,
    bytes_uploaded: u64,
    upload_count: u64,
}

// M3 test state (hashed-splashing-haven plan): one compute-meshed chunk's
// vertex buffer plus the DrawIndirectArgs the compute shader filled in,
// rendered every frame alongside the normal CPU-meshed world.
#[allow(dead_code)]
struct GpuMeshM3 {
    vertices: PoolBuffer,
    indirect: wgpu::Buffer,
    // TEMPORARY debug: vertex count read back at setup, so the draw can be
    // switched to a plain non-indirect draw of the same buffer to isolate
    // whether garbage rendering comes from the draw_indirect path.
    debug_vertex_count: u32,
    // TEMPORARY debug: full copy of the vertex buffer taken at setup, so a
    // frame-time readback can detect the buffer being clobbered afterwards.
    debug_snapshot: Vec<u8>,
}

// TEMPORARY debug toggle for the M3 garbage-rendering bug: true bypasses
// draw_indirect and issues draw(0..debug_vertex_count) on the same buffer.
#[allow(dead_code)]
const M3_DEBUG_DIRECT_DRAW: bool = true;

struct Ctx {
    instance: wgpu::Instance,
    raw_display_handle: wgpu::rwh::RawDisplayHandle,
    raw_window_handle: wgpu::rwh::RawWindowHandle,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    surface_depth: wgpu::TextureView,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    // Deferred rendering resources. The layouts, the lighting pipeline and
    // the tone-map module outlive any particular resolution; the targets
    // themselves live in `deferred` and are rebuilt on resize.
    deferred_uniform_layout: wgpu::BindGroupLayout,
    gbuffer_layout: wgpu::BindGroupLayout,
    hdr_layout: wgpu::BindGroupLayout,
    lighting_pipeline: Arc<wgpu::RenderPipeline>,
    tonemap_module: wgpu::ShaderModule,
    tonemap_layout: wgpu::PipelineLayout,
    deferred: Option<Deferred>,
    sampler: wgpu::Sampler,
    white_texture: wgpu::BindGroup,
    pipelines: HashMap<PipeKey, wgpu::RenderPipeline>,
    ubo: Option<(wgpu::Buffer, wgpu::BindGroup, usize)>,
    // Vertex buffers are pooled by power-of-two size class: creating GPU
    // buffers is far too slow for the per-frame UI meshes this game makes.
    // Dropped meshes park their buffers in `retired` until the frame is
    // submitted or discarded, then return to the pool, so recorded draws
    // cannot observe a later mesh's data.
    buffer_pool: HashMap<u64, Vec<PoolBuffer>>,
    retired: Vec<PoolBuffer>,
    // Bumped once per end_frame; invalidates cached per-shader uniform slots.
    frame_stamp: u64,
    // Frame state
    passes: Vec<PassRec>,
    uniform_arena: Vec<u8>,
    state: StateFlags,
    bound_shader: Option<Arc<ShaderInner>>,
    bound_texture: Option<(u64, wgpu::BindGroup)>,
    gpu_voxel_pool: Option<GpuVoxelPool>,
    #[allow(dead_code)]
    gpu_mesh_m3: Option<GpuMeshM3>,
    // TEMPORARY debug: second M3 test mesh, tinted by position instead of
    // real AO/light, rendered at a different y_offset in the SAME frame as
    // the real one so a single screenshot can compare both with zero
    // run-to-run camera/timing variance.
    #[allow(dead_code)]
    gpu_mesh_m3_tinted: Option<GpuMeshM3>,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> R) -> R {
    CTX.with(|c| f(c.borrow_mut().as_mut().expect("renderer not initialized")))
}

fn make_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        // TEXTURE_BINDING so the deferred lighting pass can read depth back
        // to rebuild world position.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    make_depth_texture(device, width, height).create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn init<W: wgpu::rwh::HasWindowHandle + wgpu::rwh::HasDisplayHandle>(
    window: &W,
    width: i32,
    height: i32,
) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let raw_display_handle = window.display_handle().expect("display handle").as_raw();
    let raw_window_handle = window.window_handle().expect("window handle").as_raw();
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })
            .expect("create surface")
    };
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .expect("no suitable GPU adapter");
    let info = adapter.get_info();
    println!("Renderer: {} ({:?})", info.name, info.backend);
    let adapter_limits = adapter.limits();
    println!("Adapter limits: {:#?}", adapter_limits);
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::empty(),
        required_limits: adapter_limits,
        ..Default::default()
    }))
    .expect("request device");

    let caps = surface.get_capabilities(&adapter);
    // The GL renderer used a non-sRGB default framebuffer; match it so
    // colors and the hand-tuned lighting stay identical.
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(caps.formats[0]);
    // The GL version ran without vsync; prefer the lowest-latency
    // uncapped mode the surface actually supports.
    let present_mode = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox]
        .into_iter()
        .find(|m| caps.present_modes.contains(m))
        .unwrap_or(wgpu::PresentMode::Fifo);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: width.max(1) as u32,
        height: height.max(1) as u32,
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);
    let surface_depth = make_depth(&device, config.width, config.height);

    let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(UNIFORM_SIZE as u64),
            },
            count: None,
        }],
    });
    let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("main"),
        bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
        immediate_size: 0,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("nearest-repeat"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    // --- Deferred layouts and pipelines -------------------------------
    let deferred_uniform_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("deferred uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(DEFERRED_UNIFORM_SIZE),
                },
                count: None,
            }],
        });
    // Every G-buffer read is a textureLoad at the fragment's own pixel, so
    // these are unfilterable and there is no sampler.
    let color_attachment_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let gbuffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gbuffer"),
        entries: &[
            color_attachment_entry(0),
            color_attachment_entry(1),
            color_attachment_entry(2),
            color_attachment_entry(3),
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let hdr_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("hdr scene"),
        entries: &[color_attachment_entry(0)],
    });

    let load_shader = |name: &str| -> wgpu::ShaderModule {
        let path = format!("assets/shaders/{name}");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to load shader {path}: {e}"));
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    };
    let lighting_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("deferred lighting"),
        bind_group_layouts: &[Some(&deferred_uniform_layout), Some(&gbuffer_layout)],
        immediate_size: 0,
    });
    let tonemap_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tonemap"),
        bind_group_layouts: &[Some(&deferred_uniform_layout), Some(&hdr_layout)],
        immediate_size: 0,
    });
    let lighting_pipeline = Arc::new(build_fullscreen_pipeline(
        &device,
        &lighting_layout,
        &load_shader("deferred_lighting.wgsl"),
        &[HDR_FORMAT],
        false,
    ));
    let tonemap_module = load_shader("tonemap.wgsl");

    let ctx = Ctx {
        white_texture: make_texture_bind_group(
            &device,
            &queue,
            &texture_layout,
            &sampler,
            &[255, 255, 255, 255],
            1,
            1,
        )
        .1,
        instance,
        raw_display_handle,
        raw_window_handle,
        device,
        queue,
        surface,
        config,
        surface_depth,
        uniform_layout,
        texture_layout,
        pipeline_layout,
        deferred_uniform_layout,
        gbuffer_layout,
        hdr_layout,
        lighting_pipeline,
        tonemap_module,
        tonemap_layout,
        deferred: None,
        sampler,
        pipelines: HashMap::new(),
        ubo: None,
        buffer_pool: HashMap::new(),
        retired: Vec::new(),
        frame_stamp: 0,
        passes: vec![PassRec::new(Target::Surface)],
        uniform_arena: Vec::new(),
        // Matches the gl::Enable defaults the GL version set at startup.
        state: StateFlags {
            blend: true,
            depth_test: true,
            depth_write: true,
            cull: true,
            polygon_offset: false,
        },
        bound_shader: None,
        bound_texture: None,
        gpu_voxel_pool: None,
        gpu_mesh_m3: None,
        gpu_mesh_m3_tinted: None,
    };
    CTX.with(|c| *c.borrow_mut() = Some(ctx));
}

/// Allocates the persistent GPU voxel pool sized for `pool_size` chunk slots.
/// Call once, after `init`, before any chunk sync calls.
pub fn gpu_pool_init(pool_size: usize) {
    with_ctx(|c| {
        let voxel_buf_size = pool_size as u64 * CHUNK_VOXELS;
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let blocks = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_voxel_pool_blocks"),
            size: voxel_buf_size,
            usage,
            mapped_at_creation: false,
        });
        let light = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_voxel_pool_light"),
            size: voxel_buf_size,
            usage,
            mapped_at_creation: false,
        });
        let liquid = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_voxel_pool_liquid"),
            size: voxel_buf_size,
            usage,
            mapped_at_creation: false,
        });
        let slot_meta = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_voxel_pool_slot_meta"),
            size: pool_size as u64 * 8,
            usage,
            mapped_at_creation: false,
        });
        println!(
            "GPU voxel pool: {} slots, {:.1} MiB total",
            pool_size,
            (voxel_buf_size * 3 + pool_size as u64 * 8) as f64 / (1024.0 * 1024.0)
        );
        c.gpu_voxel_pool = Some(GpuVoxelPool {
            blocks,
            light,
            liquid,
            slot_meta,
            bytes_uploaded: 0,
            upload_count: 0,
        });
    });
}

/// Uploads one chunk's blocks/light/liquid into its GPU pool slot, replacing
/// whatever chunk previously occupied it. `slot` must come from the same
/// ring-buffer indexing scheme the caller uses for `pool_size` in
/// `gpu_pool_init` (see `World::gpu_pool_index`).
pub fn gpu_pool_upload_chunk(
    slot: u32,
    chunk_x: i32,
    chunk_z: i32,
    blocks: &[u8],
    light: &[u8],
    liquid: &[u8],
) {
    with_ctx(|c| {
        let Some(pool) = c.gpu_voxel_pool.as_mut() else {
            return;
        };
        let offset = slot as u64 * CHUNK_VOXELS;
        c.queue.write_buffer(&pool.blocks, offset, blocks);
        c.queue.write_buffer(&pool.light, offset, light);
        c.queue.write_buffer(&pool.liquid, offset, liquid);
        let meta: [i32; 2] = [chunk_x, chunk_z];
        c.queue
            .write_buffer(&pool.slot_meta, slot as u64 * 8, cast_slice(&meta));
        pool.bytes_uploaded += (blocks.len() + light.len() + liquid.len() + 8) as u64;
        pool.upload_count += 1;
        if pool.upload_count % 64 == 0 {
            println!(
                "GPU voxel pool: {} chunks synced, {:.2} MiB uploaded this session",
                pool.upload_count,
                pool.bytes_uploaded as f64 / (1024.0 * 1024.0)
            );
        }
    });
}

/// Decoded vertex read back from `gpu_mesh_test_run`'s output buffer.
#[allow(dead_code)]
pub struct GpuTestVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}

#[allow(dead_code)]
const M2_MAX_VERTICES: u64 = 200_000;
#[allow(dead_code)]
const M2_WORDS_PER_VERTEX: u64 = 9;

// Shared M2/M3 compute setup: loads chunk_mesh_test.wgsl and builds its
// bind group layout + pipeline. Validation errors (shader compile, layout
// mismatch) are surfaced via error scope rather than panicking.
#[allow(dead_code)]
fn mesh_test_compute_pipeline(
    c: &Ctx,
) -> Result<(wgpu::BindGroupLayout, wgpu::ComputePipeline), String> {
    let scope = c.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let source = std::fs::read_to_string("assets/shaders/chunk_mesh_test.wgsl")
        .map_err(|e| format!("read chunk_mesh_test.wgsl: {e}"))?;
    let module = c.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("chunk_mesh_test"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    if let Some(e) = block_on(scope.pop()) {
        return Err(e.to_string());
    }

    let storage_entry = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let bgl = c
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_mesh_test_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(1, true),  // blocks_pool
                storage_entry(2, true),  // light_pool
                storage_entry(3, true),  // slot_meta
                storage_entry(4, true),  // atlas_table
                storage_entry(5, false), // out_vertices
                storage_entry(6, false), // vertex_counter / DrawIndirectArgs
            ],
        });
    let pipeline_layout = c
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_mesh_test_pl"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
    let pipeline = c
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu_mesh_test_pipeline"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
    Ok((bgl, pipeline))
}

#[allow(dead_code)]
fn mesh_test_params_buffer(
    c: &Ctx,
    chunk_x: i32,
    chunk_z: i32,
    pool_width: i32,
    y_offset: f32,
    debug_tint: u32,
) -> wgpu::Buffer {
    let params: [i32; 5] = [
        chunk_x,
        chunk_z,
        pool_width,
        y_offset.to_bits() as i32,
        debug_tint as i32,
    ];
    let params_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu_mesh_test_params"),
        size: 20,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    c.queue.write_buffer(&params_buf, 0, cast_slice(&params));
    params_buf
}

#[allow(dead_code)]
fn mesh_test_atlas_buffer(c: &Ctx) -> wgpu::Buffer {
    let atlas_data = crate::atlas_table::build_atlas_table();
    let atlas_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu_mesh_test_atlas"),
        size: (atlas_data.len() * std::mem::size_of::<crate::atlas_table::AtlasEntry>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    c.queue.write_buffer(&atlas_buf, 0, cast_slice(&atlas_data));
    atlas_buf
}

/// M2 test harness (hashed-splashing-haven plan): dispatches the standard-
/// block-only compute mesher (assets/shaders/chunk_mesh_test.wgsl) for one
/// chunk already resident in the GPU voxel pool, reads the result back to
/// CPU, and returns decoded vertices for comparison against the CPU mesher.
/// Not part of the real rendering path — dev-only, run once.
#[allow(dead_code)]
pub fn gpu_mesh_test_run(
    chunk_x: i32,
    chunk_z: i32,
    pool_width: i32,
) -> Result<Vec<GpuTestVertex>, String> {
    with_ctx(|c| {
        let Some(pool) = c.gpu_voxel_pool.as_ref() else {
            return Err("GPU voxel pool not initialized".to_string());
        };
        let blocks_buf = pool.blocks.clone();
        let light_buf = pool.light.clone();
        let slot_meta_buf = pool.slot_meta.clone();

        let (bgl, pipeline) = mesh_test_compute_pipeline(c)?;
        let params_buf = mesh_test_params_buffer(c, chunk_x, chunk_z, pool_width, 0.0, 0);
        let atlas_buf = mesh_test_atlas_buffer(c);

        let out_size = M2_MAX_VERTICES * M2_WORDS_PER_VERTEX * 4;
        let out_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_test_out"),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let counter_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_test_counter"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        c.queue.write_buffer(&counter_buf, 0, &0u32.to_le_bytes());

        let bind_group = c.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_mesh_test_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: blocks_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: slot_meta_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: atlas_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: out_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: counter_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = c
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_mesh_test_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_mesh_test_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        let readback_counter = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_test_counter_readback"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&counter_buf, 0, &readback_counter, 0, 4);
        c.queue.submit(std::iter::once(encoder.finish()));

        let count = {
            let slice = readback_counter.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            c.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
            u32::from_le_bytes(data[0..4].try_into().unwrap())
        };
        readback_counter.unmap();

        if count as u64 > M2_MAX_VERTICES {
            return Err(format!(
                "GPU mesh test overflowed the output buffer: {count} vertices > {M2_MAX_VERTICES} capacity"
            ));
        }

        let out_bytes = count as u64 * M2_WORDS_PER_VERTEX * 4;
        let readback_verts = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_test_verts_readback"),
            size: out_bytes.max(4),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder2 = c
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_mesh_test_encoder2"),
            });
        encoder2.copy_buffer_to_buffer(&out_buf, 0, &readback_verts, 0, out_bytes.max(4));
        c.queue.submit(std::iter::once(encoder2.finish()));

        let vertices = {
            let slice = readback_verts.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            c.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
            let words: &[u32] = bytemuck_words(&data);
            (0..count as usize)
                .map(|i| {
                    let w = &words[i * 9..i * 9 + 9];
                    let color_word = w[8];
                    GpuTestVertex {
                        pos: [
                            f32::from_bits(w[0]),
                            f32::from_bits(w[1]),
                            f32::from_bits(w[2]),
                        ],
                        uv: [f32::from_bits(w[3]), f32::from_bits(w[4])],
                        normal: [
                            f32::from_bits(w[5]),
                            f32::from_bits(w[6]),
                            f32::from_bits(w[7]),
                        ],
                        color: [
                            (color_word & 0xFF) as u8,
                            ((color_word >> 8) & 0xFF) as u8,
                            ((color_word >> 16) & 0xFF) as u8,
                            ((color_word >> 24) & 0xFF) as u8,
                        ],
                    }
                })
                .collect()
        };
        readback_verts.unmap();

        Ok(vertices)
    })
}

#[allow(dead_code)]
fn bytemuck_words(bytes: &[u8]) -> &[u32] {
    unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, bytes.len() / 4) }
}

// TEMPORARY debug helper for the M3 garbage-rendering bug: blocking copy of
// a GPU buffer's first `size` bytes back to the CPU.
#[allow(dead_code)]
fn read_back_buffer(c: &Ctx, buf: &wgpu::Buffer, size: u32) -> Result<Vec<u8>, String> {
    let rb = c.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("m3_debug_readback"),
        size: size as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = c.device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(buf, 0, &rb, 0, size as u64);
    c.queue.submit(std::iter::once(enc.finish()));
    let slice = rb.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    c.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    let data = {
        let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
        mapped.to_vec()
    };
    rb.unmap();
    Ok(data)
}

/// M3 test harness (hashed-splashing-haven plan): compute-meshes one chunk
/// straight into a GPU-resident vertex buffer plus a DrawIndirectArgs buffer,
/// no CPU readback. The shader's vertex counter *is* the indirect args
/// buffer: its atomicAdd lands on word 0 (vertex_count) and the other three
/// words are pre-seeded to {instance_count: 1, first_vertex: 0,
/// first_instance: 0}. `gpu_mesh_m3_draw` then renders it every frame via
/// draw_indirect, offset `y_offset` above its CPU-meshed twin for visual
/// comparison. Dev-only, run once.
#[allow(dead_code)]
pub fn gpu_mesh_m3_setup(
    chunk_x: i32,
    chunk_z: i32,
    pool_width: i32,
    y_offset: f32,
    debug_tint: u32,
    store_tinted: bool,
) -> Result<(), String> {
    with_ctx(|c| {
        let Some(pool) = c.gpu_voxel_pool.as_ref() else {
            return Err("GPU voxel pool not initialized".to_string());
        };
        let blocks_buf = pool.blocks.clone();
        let light_buf = pool.light.clone();
        let slot_meta_buf = pool.slot_meta.clone();

        let (bgl, pipeline) = mesh_test_compute_pipeline(c)?;
        // TEMPORARY debug: debug_tint=1 makes write_vertex override color with
        // a position-derived rainbow instead of real AO/light, to visually
        // distinguish "wrong shape" from "right shape, wrong shading".
        let params_buf =
            mesh_test_params_buffer(c, chunk_x, chunk_z, pool_width, y_offset, debug_tint);
        let atlas_buf = mesh_test_atlas_buffer(c);

        let scope = c.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let out_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_m3_verts"),
            size: M2_MAX_VERTICES * M2_WORDS_PER_VERTEX * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let indirect_buf = c.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_mesh_m3_indirect"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let seed: [u32; 4] = [0, 1, 0, 0];
        c.queue.write_buffer(&indirect_buf, 0, cast_slice(&seed));

        let bind_group = c.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_mesh_m3_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: blocks_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: slot_meta_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: atlas_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: out_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: indirect_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = c
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_mesh_m3_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_mesh_m3_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        c.queue.submit(std::iter::once(encoder.finish()));
        if let Some(e) = block_on(scope.pop()) {
            return Err(e.to_string());
        }

        // Temporary M3 debug readback: dump the DrawIndirectArgs the compute
        // shader produced plus sample vertices at the start, end, and one
        // past the end of the written region, to pinpoint whether garbage
        // rendering comes from bad args or bad vertex data.
        let debug_vertex_count = {
            let args_rb = c.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_mesh_m3_args_rb"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut enc = c.device.create_command_encoder(&Default::default());
            enc.copy_buffer_to_buffer(&indirect_buf, 0, &args_rb, 0, 16);
            c.queue.submit(std::iter::once(enc.finish()));
            let slice = args_rb.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            c.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            let args: Vec<u32> = {
                let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
                bytemuck_words(&data).to_vec()
            };
            args_rb.unmap();
            println!(
                "[M3 debug] indirect args: vertex_count={} instance_count={} first_vertex={} first_instance={}",
                args[0], args[1], args[2], args[3]
            );

            let count = args[0] as u64;
            let sample_at = |c: &Ctx, vert_index: u64| -> Result<[f32; 4], String> {
                let rb = c.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu_mesh_m3_vert_rb"),
                    size: M2_WORDS_PER_VERTEX * 4,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut enc = c.device.create_command_encoder(&Default::default());
                enc.copy_buffer_to_buffer(
                    &out_buf,
                    vert_index * M2_WORDS_PER_VERTEX * 4,
                    &rb,
                    0,
                    M2_WORDS_PER_VERTEX * 4,
                );
                c.queue.submit(std::iter::once(enc.finish()));
                let slice = rb.slice(..);
                slice.map_async(wgpu::MapMode::Read, |_| {});
                c.device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .map_err(|e| e.to_string())?;
                let words: Vec<u32> = {
                    let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
                    bytemuck_words(&data).to_vec()
                };
                rb.unmap();
                Ok([
                    f32::from_bits(words[0]),
                    f32::from_bits(words[1]),
                    f32::from_bits(words[2]),
                    f32::from_bits(words[8]), // color word reinterpreted, 0.0 = black/zero
                ])
            };
            for idx in [0, count / 2, count.saturating_sub(1), count] {
                let v = sample_at(c, idx)?;
                println!(
                    "[M3 debug] vertex[{idx}]: pos=({:.2}, {:.2}, {:.2}) color_word_bits={}",
                    v[0], v[1], v[2], v[3]
                );
            }
            count
        };

        // TEMPORARY debug: full buffer snapshot + quad-structure validation.
        // Every quad is emitted as 6 contiguous vertices [A,B,C,A,C,D] (or
        // [A,C,B,A,D,C] for bottom faces), so position 0 must equal position
        // 3 and position 2 must equal 4 (or 1 equal 5). A violation means
        // triangles are being assembled across quad boundaries, which the
        // M2 sorted-multiset compare is blind to.
        let snapshot = read_back_buffer(c, &out_buf, (debug_vertex_count * 36) as u32)?;
        {
            let words = bytemuck_words(&snapshot);
            let pos =
                |vi: usize| -> [u32; 3] { [words[vi * 9], words[vi * 9 + 1], words[vi * 9 + 2]] };
            let mut bad = 0usize;
            for g in 0..(debug_vertex_count as usize / 6) {
                let b = g * 6;
                let ok =
                    pos(b) == pos(b + 3) && (pos(b + 2) == pos(b + 4) || pos(b + 1) == pos(b + 5));
                if !ok {
                    if bad < 3 {
                        let p = |vi: usize| -> [f32; 3] {
                            let w = pos(vi);
                            [
                                f32::from_bits(w[0]),
                                f32::from_bits(w[1]),
                                f32::from_bits(w[2]),
                            ]
                        };
                        println!(
                            "[M3 debug] quad {g} malformed: p0={:?} p1={:?} p2={:?} p3={:?} p4={:?} p5={:?}",
                            p(b),
                            p(b + 1),
                            p(b + 2),
                            p(b + 3),
                            p(b + 4),
                            p(b + 5)
                        );
                    }
                    bad += 1;
                }
            }
            println!(
                "[M3 debug] quad structure at setup: {bad} malformed of {}",
                debug_vertex_count / 6
            );

            // TEMPORARY debug: a real face's 6 vertices should all fit within
            // a ~1x1x1 cube (plus the shared y_offset). Any quad whose corners
            // spread far wider than that has a wrong vertex somewhere, even if
            // the p0==p3/p2==p4 echo check above didn't catch it (echo only
            // proves two corners agree with each other, not that either is
            // actually correct).
            let posf = |vi: usize| -> [f32; 3] {
                let w = pos(vi);
                [
                    f32::from_bits(w[0]),
                    f32::from_bits(w[1]),
                    f32::from_bits(w[2]),
                ]
            };
            let mut wild = 0usize;
            for g in 0..(debug_vertex_count as usize / 6) {
                let b = g * 6;
                let corners: Vec<[f32; 3]> = (0..6).map(|k| posf(b + k)).collect();
                let mut min = corners[0];
                let mut max = corners[0];
                for cnr in &corners[1..] {
                    for a in 0..3 {
                        min[a] = min[a].min(cnr[a]);
                        max[a] = max[a].max(cnr[a]);
                    }
                }
                let extent = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
                let max_extent = extent[0].max(extent[1]).max(extent[2]);
                if max_extent > 2.0 {
                    if wild < 5 {
                        println!(
                            "[M3 debug] quad {g} wild extent {max_extent:.1}: corners={corners:?}"
                        );
                    }
                    wild += 1;
                }
            }
            println!(
                "[M3 debug] quads with >2.0 bounding extent: {wild} of {}",
                debug_vertex_count / 6
            );

            // TEMPORARY debug: real unpacked RGBA byte stats (not the
            // misleading f32::from_bits reinterpretation used in the earlier
            // sample prints), to see what colors are actually being written
            // instead of guessing from "looks dark" screenshots.
            if debug_tint == 0 {
                let rgba = |vi: usize| -> [u8; 4] {
                    let w = words[vi * 9 + 8];
                    [w as u8, (w >> 8) as u8, (w >> 16) as u8, (w >> 24) as u8]
                };
                let n = debug_vertex_count as usize;
                let mut min_c = [255u8; 4];
                let mut max_c = [0u8; 4];
                let mut sum = [0u64; 4];
                let mut hist_buckets = [0usize; 8]; // 0-31,32-63,...,224-255 on max(r,g,b)
                for vi in 0..n {
                    let c = rgba(vi);
                    for k in 0..4 {
                        min_c[k] = min_c[k].min(c[k]);
                        max_c[k] = max_c[k].max(c[k]);
                        sum[k] += c[k] as u64;
                    }
                    let brightest = c[0].max(c[1]).max(c[2]);
                    hist_buckets[(brightest as usize) / 32] += 1;
                }
                let avg: Vec<f64> = sum.iter().map(|s| *s as f64 / n.max(1) as f64).collect();
                println!(
                    "[M3 debug] color stats over {n} vertices: min={min_c:?} max={max_c:?} avg_rgba={avg:.1?}"
                );
                println!(
                    "[M3 debug] brightness histogram (max(r,g,b), 8 buckets of 32): {hist_buckets:?}"
                );
            }
        }

        let mesh = GpuMeshM3 {
            vertices: PoolBuffer {
                id: BUFFER_IDS.fetch_add(1, Ordering::Relaxed) as u64,
                buffer: out_buf,
            },
            indirect: indirect_buf,
            debug_vertex_count: debug_vertex_count as u32,
            debug_snapshot: snapshot,
        };
        if store_tinted {
            c.gpu_mesh_m3_tinted = Some(mesh);
        } else {
            c.gpu_mesh_m3 = Some(mesh);
        }
        Ok(())
    })
}

/// Records an indirect draw of the M3 compute-meshed chunk using the
/// currently bound shader/texture/state, exactly like `Mesh::draw` except
/// the vertex count lives in the GPU-written DrawIndirectArgs buffer.
/// No-op until `gpu_mesh_m3_setup` has run.
#[allow(dead_code)]
pub fn gpu_mesh_m3_draw() {
    gpu_mesh_m3_draw_impl(false);
}

// TEMPORARY debug: draws the tinted comparison copy (see gpu_mesh_m3_tinted).
#[allow(dead_code)]
pub fn gpu_mesh_m3_draw_tinted() {
    gpu_mesh_m3_draw_impl(true);
}

#[allow(dead_code)]
fn gpu_mesh_m3_draw_impl(tinted: bool) {
    with_ctx(|c| {
        let Some(m3) = (if tinted {
            &c.gpu_mesh_m3_tinted
        } else {
            &c.gpu_mesh_m3
        }) else {
            return;
        };
        // TEMPORARY debug: on the 120th recorded frame, read the vertex
        // buffer back and byte-compare against the setup-time snapshot to
        // detect the buffer being clobbered after setup.
        static M3_DEBUG_FRAME: AtomicUsize = AtomicUsize::new(0);
        let frame = if tinted {
            usize::MAX
        } else {
            M3_DEBUG_FRAME.fetch_add(1, Ordering::Relaxed)
        };
        if frame == 120 {
            match read_back_buffer(c, &m3.vertices.buffer, m3.debug_vertex_count * 36) {
                Ok(now) => {
                    if now == m3.debug_snapshot {
                        println!("[M3 debug] frame-120 buffer identical to setup snapshot");
                    } else {
                        let first = now
                            .iter()
                            .zip(&m3.debug_snapshot)
                            .position(|(a, b)| a != b)
                            .unwrap_or(0);
                        let diff = now
                            .iter()
                            .zip(&m3.debug_snapshot)
                            .filter(|(a, b)| a != b)
                            .count();
                        println!(
                            "[M3 debug] frame-120 buffer CLOBBERED: {diff} bytes differ, first at byte {first} (vertex {} word {})",
                            first / 36,
                            (first % 36) / 4
                        );
                    }
                }
                Err(e) => println!("[M3 debug] frame-120 readback failed: {e}"),
            }
        }
        let buffer = m3.vertices.clone();
        let indirect = m3.indirect.clone();
        let debug_vertex_count = m3.debug_vertex_count;
        let Some(shader) = c.bound_shader.clone() else {
            return;
        };
        let uniform_offset = if !shader.dirty.get() && shader.cached_frame.get() == c.frame_stamp {
            shader.cached_offset.get()
        } else {
            let offset = c.uniform_arena.len() as u32;
            c.uniform_arena.extend_from_slice(&*shader.staging.borrow());
            c.uniform_arena.resize(offset as usize + UNIFORM_STRIDE, 0);
            shader.dirty.set(false);
            shader.cached_offset.set(offset);
            shader.cached_frame.set(c.frame_stamp);
            offset
        };
        let (texture_id, texture) = c
            .bound_texture
            .clone()
            .unwrap_or_else(|| (0, c.white_texture.clone()));
        let flags = c.state;
        // TEMPORARY debug: on the first recorded frame, print this draw's
        // record-time state next to the previous draw in the pass (the last
        // CPU chunk mesh) to rule out any pipeline/bind mismatch.
        if frame == 0 {
            let fmt = |f: &StateFlags| {
                format!(
                    "blend={} depth_test={} depth_write={} cull={} poly_off={}",
                    f.blend, f.depth_test, f.depth_write, f.cull, f.polygon_offset
                )
            };
            if let Some(prev) = c.passes.last().unwrap().last_draw() {
                println!(
                    "[M3 debug] prev draw: shader={} {} uoff={} tex={} buf={} count={}",
                    prev.shader.id,
                    fmt(&prev.flags),
                    prev.uniform_offset,
                    prev.texture_id,
                    prev.buffer.id,
                    prev.vertex_count
                );
            }
            println!(
                "[M3 debug] m3 draw:   shader={} {} uoff={} tex={} buf={} count={}",
                shader.id,
                fmt(&flags),
                uniform_offset,
                texture_id,
                buffer.id,
                debug_vertex_count
            );
        }
        c.passes.last_mut().unwrap().push_draw(DrawRec {
            shader,
            flags,
            uniform_offset,
            texture,
            texture_id,
            buffer,
            vertex_count: if M3_DEBUG_DIRECT_DRAW {
                debug_vertex_count
            } else {
                0
            },
            indirect: if M3_DEBUG_DIRECT_DRAW {
                None
            } else {
                Some(indirect)
            },
        });
    });
}

fn make_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    data: &[u8],
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, bind_group)
}

// GL-style dynamic state. These affect draws recorded after the call.
pub fn set_blend(on: bool) {
    with_ctx(|c| c.state.blend = on);
}
pub fn set_depth_test(on: bool) {
    with_ctx(|c| c.state.depth_test = on);
}
pub fn set_depth_write(on: bool) {
    with_ctx(|c| c.state.depth_write = on);
}
pub fn set_cull(on: bool) {
    with_ctx(|c| c.state.cull = on);
}
pub fn set_polygon_offset(on: bool) {
    with_ctx(|c| c.state.polygon_offset = on);
}

/// Clears color and depth of the current render target, like
/// glClearColor + glClear(COLOR|DEPTH).
pub fn clear(r: f32, g: f32, b: f32, a: f32) {
    with_ctx(|c| {
        let target = c.passes.last().unwrap().target.clone();
        let pass = if c.passes.last().unwrap().is_empty() {
            c.passes.last_mut().unwrap()
        } else {
            c.passes.push(PassRec::new(target));
            c.passes.last_mut().unwrap()
        };
        pass.clear_color = Some([r as f64, g as f64, b as f64, a as f64]);
        pass.clear_depth = true;
    });
}

fn switch_target(target: Target) {
    with_ctx(|c| switch_target_inner(c, target));
}

fn switch_target_inner(c: &mut Ctx, target: Target) {
    let last = c.passes.last().unwrap();
    if last.target == target {
        return;
    }
    if last.is_empty() && last.clear_color.is_none() {
        c.passes.last_mut().unwrap().target = target;
        return;
    }
    c.passes.push(PassRec::new(target));
}

static SHADER_IDS: AtomicUsize = AtomicUsize::new(0);

// A fresh shader's uniforms are zero except uHdrScale, which is 1.0: the
// forward path is display-referred and must not be scaled. Only draws
// aimed at the HDR target raise it.
fn initial_uniform_staging() -> [u8; UNIFORM_SIZE] {
    let mut staging = [0u8; UNIFORM_SIZE];
    let offset = uniform_offset("uHdrScale") as usize;
    staging[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
    staging
}

struct ShaderInner {
    id: usize,
    module: wgpu::ShaderModule,
    // How many @location outputs the fragment entry point writes. Targets
    // past this many are left unwritten rather than validated against a
    // shader that has nothing to put in them.
    outputs: usize,
    staging: RefCell<[u8; UNIFORM_SIZE]>,
    // Draws reuse the last uniform slot while no set_* happened this frame.
    dirty: std::cell::Cell<bool>,
    cached_offset: std::cell::Cell<u32>,
    cached_frame: std::cell::Cell<u64>,
}

pub struct Shader {
    inner: Arc<ShaderInner>,
}

impl Shader {
    /// Compiles a shader whose fragment stage writes one color attachment.
    pub fn new(source: &str) -> Result<Self, String> {
        Self::with_outputs(source, 1)
    }

    /// Compiles a shader that writes `outputs` color attachments, for the
    /// deferred geometry pass.
    pub fn with_outputs(source: &str, outputs: usize) -> Result<Self, String> {
        with_ctx(|c| {
            let scope = c.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = c.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            match block_on(scope.pop()) {
                Some(e) => Err(e.to_string()),
                None => Ok(Self {
                    inner: Arc::new(ShaderInner {
                        id: SHADER_IDS.fetch_add(1, Ordering::Relaxed),
                        module,
                        outputs,
                        staging: RefCell::new(initial_uniform_staging()),
                        dirty: std::cell::Cell::new(true),
                        cached_offset: std::cell::Cell::new(0),
                        cached_frame: std::cell::Cell::new(u64::MAX),
                    }),
                }),
            }
        })
    }

    pub fn bind(&self) {
        with_ctx(|c| c.bound_shader = Some(self.inner.clone()));
    }

    #[allow(dead_code)]
    pub fn unbind() {}

    pub fn get_uniform_location(&self, name: &str) -> i32 {
        uniform_offset(name)
    }

    fn write(&self, location: i32, bytes: &[u8]) {
        if location < 0 {
            return;
        }
        let start = location as usize;
        self.inner.staging.borrow_mut()[start..start + bytes.len()].copy_from_slice(bytes);
        self.inner.dirty.set(true);
    }

    pub fn set_int(&self, location: i32, value: i32) {
        self.write(location, &value.to_le_bytes());
    }

    pub fn set_float(&self, location: i32, value: f32) {
        self.write(location, &value.to_le_bytes());
    }

    pub fn set_vec2(&self, location: i32, value: glam::Vec2) {
        self.write(location, cast_slice(value.as_ref()));
    }

    pub fn set_vec3(&self, location: i32, value: glam::Vec3) {
        self.write(location, cast_slice(value.as_ref()));
    }

    pub fn set_vec4(&self, location: i32, value: glam::Vec4) {
        self.write(location, cast_slice(value.as_ref()));
    }

    pub fn set_mat4(&self, location: i32, value: &glam::Mat4) {
        self.write(location, cast_slice(value.as_ref()));
    }
}

static TEXTURE_IDS: AtomicUsize = AtomicUsize::new(1);

pub struct Texture2D {
    id: u64,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pub width: i32,
    pub height: i32,
}

impl Texture2D {
    pub fn from_data(data: &[u8], width: i32, height: i32) -> Self {
        let (texture, bind_group) = with_ctx(|c| {
            make_texture_bind_group(
                &c.device,
                &c.queue,
                &c.texture_layout,
                &c.sampler,
                data,
                width as u32,
                height as u32,
            )
        });
        Self {
            id: TEXTURE_IDS.fetch_add(1, Ordering::Relaxed) as u64,
            texture,
            bind_group,
            width,
            height,
        }
    }

    pub fn from_file(path: &str) -> Self {
        let img = image::open(path).expect("Failed to load image").to_rgba8();
        let (width, height) = img.dimensions();
        Self::from_data(&img, width as i32, height as i32)
    }

    #[allow(dead_code)]
    pub fn update(&self, x: i32, y: i32, width: i32, height: i32, data: &[u8]) {
        with_ctx(|c| {
            c.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: x as u32,
                        y: y as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width as u32),
                    rows_per_image: Some(height as u32),
                },
                wgpu::Extent3d {
                    width: width as u32,
                    height: height as u32,
                    depth_or_array_layers: 1,
                },
            );
        });
    }

    pub fn bind(&self, _slot: u32) {
        with_ctx(|c| c.bound_texture = Some((self.id, self.bind_group.clone())));
    }
}

// Interleaved vertex: position (12) + uv (8) + normal (12) + color (4).
const VERTEX_STRIDE: usize = 36;

pub struct Mesh {
    buffer: PoolBuffer,
    pub vertex_count: i32,
}

impl Mesh {
    pub fn new(
        vertices: &[f32],
        texcoords: Option<&[f32]>,
        normals: Option<&[f32]>,
        colors: Option<&[u8]>,
    ) -> Self {
        let vertex_count = vertices.len() / 3;
        let pos_bytes = cast_slice(vertices);
        let uv_bytes = texcoords.map(cast_slice);
        let norm_bytes = normals.map(cast_slice);
        // One interleaved buffer per mesh keeps it to a single bind per draw.
        // GL leaves missing attributes at the default (0, 0, 0, 1); fill the
        // same defaults (the buffer starts zeroed, so only alpha needs work).
        let mut data = vec![0u8; vertex_count.max(1) * VERTEX_STRIDE];
        for i in 0..vertex_count {
            let out = &mut data[i * VERTEX_STRIDE..(i + 1) * VERTEX_STRIDE];
            out[0..12].copy_from_slice(&pos_bytes[i * 12..i * 12 + 12]);
            if let Some(uv) = uv_bytes {
                out[12..20].copy_from_slice(&uv[i * 8..i * 8 + 8]);
            }
            if let Some(norm) = norm_bytes {
                out[20..32].copy_from_slice(&norm[i * 12..i * 12 + 12]);
            }
            match colors {
                Some(c) => out[32..36].copy_from_slice(&c[i * 4..i * 4 + 4]),
                None => out[35] = 255,
            }
        }
        let buffer = with_ctx(|c| pooled_buffer(c, &data));
        Self {
            buffer,
            vertex_count: vertex_count as i32,
        }
    }

    pub fn draw(&self) {
        if self.vertex_count == 0 {
            return;
        }
        with_ctx(|c| {
            let Some(shader) = c.bound_shader.clone() else {
                return;
            };
            // Reuse the previous uniform slot when nothing changed since the
            // last draw this frame (chunk draws share identical uniforms).
            let uniform_offset =
                if !shader.dirty.get() && shader.cached_frame.get() == c.frame_stamp {
                    shader.cached_offset.get()
                } else {
                    let offset = c.uniform_arena.len() as u32;
                    c.uniform_arena.extend_from_slice(&*shader.staging.borrow());
                    c.uniform_arena.resize(offset as usize + UNIFORM_STRIDE, 0);
                    shader.dirty.set(false);
                    shader.cached_offset.set(offset);
                    shader.cached_frame.set(c.frame_stamp);
                    offset
                };
            let (texture_id, texture) = c
                .bound_texture
                .clone()
                .unwrap_or_else(|| (0, c.white_texture.clone()));
            let flags = c.state;
            c.passes.last_mut().unwrap().push_draw(DrawRec {
                shader,
                flags,
                uniform_offset,
                texture,
                texture_id,
                buffer: self.buffer.clone(),
                vertex_count: self.vertex_count as u32,
                indirect: None,
            });
        });
    }
}

static BUFFER_IDS: AtomicUsize = AtomicUsize::new(0);

fn pooled_buffer(c: &mut Ctx, bytes: &[u8]) -> PoolBuffer {
    let class = (bytes.len().next_power_of_two().max(256)) as u64;
    let buffer = c
        .buffer_pool
        .get_mut(&class)
        .and_then(Vec::pop)
        .unwrap_or_else(|| PoolBuffer {
            id: BUFFER_IDS.fetch_add(1, Ordering::Relaxed) as u64,
            buffer: c.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: class,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        });
    if !bytes.is_empty() {
        c.queue.write_buffer(&buffer.buffer, 0, bytes);
    }
    buffer
}

fn recycle_retired_buffers(c: &mut Ctx) {
    for pooled in std::mem::take(&mut c.retired) {
        c.buffer_pool
            .entry(pooled.buffer.size())
            .or_default()
            .push(pooled);
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        // Park the buffer until end_frame; draws recorded this frame still
        // reference it and queued writes must not clobber it.
        CTX.with(|c| {
            if let Some(ctx) = c.borrow_mut().as_mut() {
                ctx.retired.push(self.buffer.clone());
            }
        });
    }
}

// ---------------------------------------------------------------------
// Deferred rendering (Vibrant Visuals)
//
// The frame runs: geometry into the G-buffer, a fullscreen lighting
// resolve into an HDR target, then tone mapping down to whatever target
// the UI draws onto. The two fullscreen steps bring their own pipelines
// and bind groups because they sample several textures at once, which the
// one-texture immediate-mode path cannot express.
// ---------------------------------------------------------------------

/// Everything the lighting and tone-map shaders need for one frame.
/// Laid out to match `Deferred` in the WGSL; keep the two in step.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DeferredUniforms {
    /// Inverse view-projection, for rebuilding world position from depth.
    pub inv_view_proj: [f32; 16],
    /// xyz camera position, w exposure multiplier.
    pub camera_pos_exposure: [f32; 4],
    /// xyz direction *towards* the sun, w illuminance in lux.
    pub sun_direction_illuminance: [f32; 4],
    /// Linear sun color, w unused.
    pub sun_color: [f32; 4],
    /// xyz direction towards the moon, w illuminance in lux.
    pub moon_direction_illuminance: [f32; 4],
    pub moon_color: [f32; 4],
    /// Linear ambient color, w illuminance in lux.
    pub ambient_color_illuminance: [f32; 4],
    /// x sky intensity, y emissive desaturation, z block-light lux,
    /// w unused.
    pub sky_params: [f32; 4],
    /// Linear sky colors driving both the sky itself and indirect light.
    pub zenith_color: [f32; 4],
    pub horizon_color: [f32; 4],
    /// Rayleigh strength, sun Mie, moon Mie, sun glare shape.
    pub atmosphere: [f32; 4],
    /// horizon_blend_stops: min, start, mie_start, max.
    pub horizon_stops: [f32; 4],
    /// Linear color of block light (torches and the like), w unused.
    pub block_light_color: [f32; 4],
}

impl Default for DeferredUniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: glam::Mat4::IDENTITY.to_cols_array(),
            camera_pos_exposure: [0.0, 0.0, 0.0, 1.0],
            sun_direction_illuminance: [0.0, 1.0, 0.0, 100_000.0],
            sun_color: [1.0, 1.0, 1.0, 0.0],
            moon_direction_illuminance: [0.0, -1.0, 0.0, 0.27],
            moon_color: [1.0, 1.0, 1.0, 0.0],
            ambient_color_illuminance: [1.0, 1.0, 1.0, 0.02],
            sky_params: [1.0, 0.1, 0.0, 0.0],
            zenith_color: [0.0, 0.24, 0.37, 0.0],
            horizon_color: [0.56, 0.71, 1.0, 0.0],
            atmosphere: [1.0, 1.0, 0.0, 4.0],
            horizon_stops: [0.0, 0.25, 0.5, 0.25],
            block_light_color: [1.0, 0.78, 0.5, 0.0],
        }
    }
}

const DEFERRED_UNIFORM_SIZE: u64 = std::mem::size_of::<DeferredUniforms>() as u64;

/// The G-buffer, the HDR scene target, and the pipelines that resolve
/// them. Rebuilt whenever the render resolution changes.
struct Deferred {
    width: u32,
    height: u32,
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    mers: wgpu::TextureView,
    lighting: wgpu::TextureView,
    // Kept as the texture, not a view: the geometry pass needs a render
    // view and the lighting pass needs a sampleable one.
    depth: wgpu::Texture,
    hdr: wgpu::TextureView,
    // Sampled by the tone-map pass.
    #[allow(dead_code)]
    hdr_texture: wgpu::Texture,
    uniform_buffer: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    gbuffer_group: wgpu::BindGroup,
    hdr_group: wgpu::BindGroup,
    lighting_pipeline: Arc<wgpu::RenderPipeline>,
    // One tone-map pipeline per output kind: the game renders to the
    // low-res offscreen target, but a full-res path would go to the
    // surface, and the two formats need different pipelines.
    tonemap_pipelines: HashMap<TargetKind, Arc<wgpu::RenderPipeline>>,
    tonemap_module: wgpu::ShaderModule,
    tonemap_layout: wgpu::PipelineLayout,
}

fn make_attachment(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn view_of(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Sets the render resolution of the deferred targets, building them on
/// first use and rebuilding them on resize.
pub fn deferred_resize(width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let (width, height) = (width as u32, height as u32);
    with_ctx(|c| {
        if c.deferred
            .as_ref()
            .is_some_and(|d| d.width == width && d.height == height)
        {
            return;
        }
        c.deferred = Some(build_deferred(c, width, height));
    });
}

fn build_deferred(c: &Ctx, width: u32, height: u32) -> Deferred {
    let device = &c.device;
    let albedo = make_attachment(
        device,
        "gbuffer albedo",
        width,
        height,
        GBUFFER_ALBEDO_FORMAT,
    );
    let normal = make_attachment(
        device,
        "gbuffer normal",
        width,
        height,
        GBUFFER_NORMAL_FORMAT,
    );
    let mers = make_attachment(device, "gbuffer mers", width, height, GBUFFER_MERS_FORMAT);
    let lighting = make_attachment(
        device,
        "gbuffer lighting",
        width,
        height,
        GBUFFER_LIGHTING_FORMAT,
    );
    let hdr_texture = make_attachment(device, "hdr scene", width, height, HDR_FORMAT);
    let depth = make_depth_texture(device, width, height);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("deferred uniforms"),
        size: DEFERRED_UNIFORM_SIZE,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("deferred uniforms"),
        layout: &c.deferred_uniform_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    let albedo_view = view_of(&albedo);
    let normal_view = view_of(&normal);
    let mers_view = view_of(&mers);
    let lighting_view = view_of(&lighting);
    let depth_view = view_of(&depth);
    let hdr_view = view_of(&hdr_texture);

    // Both fullscreen passes read their inputs 1:1 with textureLoad, so
    // neither needs a sampler.
    let gbuffer_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gbuffer"),
        layout: &c.gbuffer_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&mers_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&lighting_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&depth_view),
            },
        ],
    });
    let hdr_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hdr scene"),
        layout: &c.hdr_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&hdr_view),
        }],
    });

    Deferred {
        width,
        height,
        albedo: albedo_view,
        normal: normal_view,
        mers: mers_view,
        lighting: lighting_view,
        depth,
        hdr: hdr_view,
        hdr_texture,
        uniform_buffer,
        uniform_group,
        gbuffer_group,
        hdr_group,
        lighting_pipeline: c.lighting_pipeline.clone(),
        tonemap_pipelines: HashMap::new(),
        tonemap_module: c.tonemap_module.clone(),
        tonemap_layout: c.tonemap_layout.clone(),
    }
}

/// Directs subsequent draws into the G-buffer. Clears it first: a stale
/// G-buffer would light last frame's geometry where nothing was drawn.
pub fn deferred_begin_geometry() {
    with_ctx(|c| {
        let Some(d) = c.deferred.as_ref() else {
            return;
        };
        let target = Target::GBuffer {
            albedo: d.albedo.clone(),
            normal: d.normal.clone(),
            mers: d.mers.clone(),
            lighting: d.lighting.clone(),
            depth: view_of(&d.depth),
        };
        switch_target_inner(c, target);
        let pass = c.passes.last_mut().unwrap();
        pass.clear_color = Some([0.0, 0.0, 0.0, 0.0]);
        pass.clear_depth = true;
    });
}

/// Resolves the G-buffer into the HDR target with the given lighting.
pub fn deferred_resolve(uniforms: &DeferredUniforms) {
    with_ctx(|c| {
        let Some(d) = c.deferred.as_ref() else {
            return;
        };
        c.queue
            .write_buffer(&d.uniform_buffer, 0, cast_slice(&[*uniforms]));
        let hdr = d.hdr.clone();
        let depth = view_of(&d.depth);
        let step = Step::Fullscreen(FullscreenRec {
            pipeline: d.lighting_pipeline.clone(),
            bind_groups: vec![d.uniform_group.clone(), d.gbuffer_group.clone()],
        });
        switch_target_inner(c, Target::HdrResolve { color: hdr.clone() });
        // The lighting resolve replaces the HDR target wholesale, so it
        // does not need a clear of its own.
        c.passes.last_mut().unwrap().steps.push(step);
        // Forward draws after the resolve need the geometry depth, which
        // this pass could not hold while the resolve sampled it.
        switch_target_inner(c, Target::Hdr { color: hdr, depth });
    });
}

/// Tone maps the HDR target onto whatever target is currently bound.
pub fn deferred_tonemap() {
    with_ctx(|c| {
        let Some(d) = c.deferred.as_ref() else {
            return;
        };
        let kind = c.passes.last().unwrap().target.kind();
        let pipeline = match d.tonemap_pipelines.get(&kind) {
            Some(pipeline) => pipeline.clone(),
            None => {
                let pipeline = Arc::new(build_fullscreen_pipeline(
                    &c.device,
                    &d.tonemap_layout,
                    &d.tonemap_module,
                    &kind.color_formats(c.config.format),
                    true,
                ));
                c.deferred
                    .as_mut()
                    .unwrap()
                    .tonemap_pipelines
                    .insert(kind, pipeline.clone());
                pipeline
            }
        };
        let d = c.deferred.as_ref().unwrap();
        let step = Step::Fullscreen(FullscreenRec {
            pipeline,
            bind_groups: vec![d.uniform_group.clone(), d.hdr_group.clone()],
        });
        c.passes.last_mut().unwrap().steps.push(step);
    });
}

/// True once the deferred targets exist, so callers can fall back to the
/// forward path on a device where they could not be built.
pub fn deferred_ready() -> bool {
    with_ctx(|c| c.deferred.is_some())
}

fn build_fullscreen_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    formats: &[wgpu::TextureFormat],
    depth: bool,
) -> wgpu::RenderPipeline {
    let targets: Vec<Option<wgpu::ColorTargetState>> = formats
        .iter()
        .enumerate()
        .map(|(i, format)| {
            (i == 0).then_some(wgpu::ColorTargetState {
                format: *format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        })
        .collect();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fullscreen"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        // The lighting resolve has no depth attachment because it *samples*
        // the G-buffer depth. Tone mapping runs on the offscreen target,
        // which does have depth, so that pipeline still declares one and
        // always-passes so it does not fight leftover geometry depth.
        depth_stencil: depth.then_some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub struct RenderTexture2D {
    pub texture: Texture2D,
    depth: wgpu::TextureView,
}

impl RenderTexture2D {
    pub fn new(width: i32, height: i32) -> Self {
        let texture =
            Texture2D::from_data(&vec![0u8; (width * height * 4) as usize], width, height);
        let depth = with_ctx(|c| make_depth(&c.device, width as u32, height as u32));
        Self { texture, depth }
    }

    pub fn bind(&self) {
        let color = self
            .texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        switch_target(Target::Offscreen {
            color,
            depth: self.depth.clone(),
        });
    }

    pub fn unbind() {
        switch_target(Target::Surface);
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    flags: StateFlags,
    formats: &[wgpu::TextureFormat],
    outputs: usize,
) -> wgpu::RenderPipeline {
    let vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: VERTEX_STRIDE as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Unorm8x4
        ],
    })];
    // Attachments the shader does not write are declared as None so a
    // single-output shader stays usable against a multi-attachment pass.
    // Blending applies only to the first attachment: the G-buffer's normal
    // and material channels are not colors and must not be mixed.
    let targets: Vec<Option<wgpu::ColorTargetState>> = formats
        .iter()
        .enumerate()
        .map(|(i, format)| {
            (i < outputs).then_some(wgpu::ColorTargetState {
                format: *format,
                blend: (flags.blend && i == 0).then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })
        })
        .collect();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &vertex_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: flags.cull.then_some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            // gl::Disable(DEPTH_TEST) turns off both testing and writing.
            depth_write_enabled: Some(flags.depth_test && flags.depth_write),
            depth_compare: Some(if flags.depth_test {
                wgpu::CompareFunction::LessEqual
            } else {
                wgpu::CompareFunction::Always
            }),
            stencil: Default::default(),
            bias: if flags.polygon_offset {
                // glPolygonOffset(-1.0, -1.0)
                wgpu::DepthBiasState {
                    constant: -1,
                    slope_scale: -1.0,
                    clamp: 0.0,
                }
            } else {
                Default::default()
            },
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Submits every pass recorded since the last end_frame and presents.
/// Replaces window.swap_buffers(); takes the current framebuffer size so
/// the surface can follow resizes.
pub fn end_frame(width: i32, height: i32) {
    with_ctx(|c| {
        let passes = std::mem::replace(&mut c.passes, vec![PassRec::new(Target::Surface)]);
        let arena = std::mem::take(&mut c.uniform_arena);
        c.frame_stamp += 1;
        if width <= 0 || height <= 0 {
            drop(passes);
            recycle_retired_buffers(c);
            return; // minimized: drop the frame
        }
        if c.config.width != width as u32 || c.config.height != height as u32 {
            c.config.width = width as u32;
            c.config.height = height as u32;
            c.surface.configure(&c.device, &c.config);
            c.surface_depth = make_depth(&c.device, c.config.width, c.config.height);
        }
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match c.surface.get_current_texture() {
            Cst::Success(f) | Cst::Suboptimal(f) => Some(f),
            Cst::Outdated => {
                c.surface.configure(&c.device, &c.config);
                match c.surface.get_current_texture() {
                    Cst::Success(f) | Cst::Suboptimal(f) => Some(f),
                    _ => None,
                }
            }
            Cst::Lost => {
                let replacement = unsafe {
                    c.instance
                        .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                            raw_display_handle: Some(c.raw_display_handle),
                            raw_window_handle: c.raw_window_handle,
                        })
                };
                match replacement {
                    Ok(surface) => {
                        surface.configure(&c.device, &c.config);
                        c.surface = surface;
                        match c.surface.get_current_texture() {
                            Cst::Success(f) | Cst::Suboptimal(f) => Some(f),
                            _ => None,
                        }
                    }
                    Err(error) => {
                        eprintln!("Failed to recreate lost render surface: {error}");
                        None
                    }
                }
            }
            Cst::Timeout | Cst::Occluded | Cst::Validation => None,
        };
        let Some(frame) = frame else {
            drop(passes);
            recycle_retired_buffers(c);
            return;
        };
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Upload this frame's uniforms; grow the shared buffer if needed.
        let needed = arena.len().max(UNIFORM_STRIDE);
        if c.ubo.as_ref().is_none_or(|(_, _, cap)| *cap < needed) {
            let buffer = c.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uniform arena"),
                size: needed as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = c.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("uniform arena"),
                layout: &c.uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(UNIFORM_SIZE as u64),
                    }),
                }],
            });
            c.ubo = Some((buffer, bind_group, needed));
        }
        if !arena.is_empty() {
            let (buffer, _, _) = c.ubo.as_ref().unwrap();
            c.queue.write_buffer(buffer, 0, &arena);
        }

        // Create any pipelines this frame needs before encoding.
        for pass in &passes {
            let kind = pass.target.kind();
            for draw in pass.draws() {
                let key = PipeKey {
                    shader_id: draw.shader.id,
                    flags: draw.flags,
                    kind,
                };
                if !c.pipelines.contains_key(&key) {
                    let pipeline = build_pipeline(
                        &c.device,
                        &c.pipeline_layout,
                        &draw.shader.module,
                        draw.flags,
                        &kind.color_formats(c.config.format),
                        draw.shader.outputs,
                    );
                    c.pipelines.insert(key, pipeline);
                }
            }
        }

        let mut encoder = c
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        for pass in &passes {
            if pass.is_empty() && pass.clear_color.is_none() {
                continue;
            }
            let kind = pass.target.kind();
            let color_views = pass.target.color_views(&frame_view);
            let depth_view = pass.target.depth_view(&c.surface_depth);
            let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = color_views
                .iter()
                .enumerate()
                .map(|(i, view)| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Only the first attachment takes the requested
                            // clear color; the G-buffer's normal and material
                            // channels clear to zero, which reads back as
                            // "nothing was drawn here".
                            load: match (pass.clear_color, i) {
                                (Some([r, g, b, a]), 0) => {
                                    wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
                                }
                                (Some(_), _) => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                (None, _) => wgpu::LoadOp::Load,
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    })
                })
                .collect();
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &color_attachments,
                depth_stencil_attachment: depth_view.map(|view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations {
                            load: if pass.clear_depth {
                                wgpu::LoadOp::Clear(1.0)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let ubo_bind = &c.ubo.as_ref().unwrap().1;
            let mut last_key = None;
            let mut last_texture = None;
            let mut last_offset = None;
            let mut last_buffer = None;
            for step in &pass.steps {
                let draw = match step {
                    Step::Draw(draw) => draw,
                    Step::Fullscreen(full) => {
                        rpass.set_pipeline(&full.pipeline);
                        for (index, group) in full.bind_groups.iter().enumerate() {
                            rpass.set_bind_group(index as u32, group, &[]);
                        }
                        // One oversized triangle covers the target with no
                        // vertex buffer; the shader derives UVs from the
                        // vertex index.
                        rpass.draw(0..3, 0..1);
                        // This step replaced the pipeline and both bind
                        // groups, so nothing cached about the previous draw
                        // still holds.
                        last_key = None;
                        last_texture = None;
                        last_offset = None;
                        last_buffer = None;
                        continue;
                    }
                };
                let key = PipeKey {
                    shader_id: draw.shader.id,
                    flags: draw.flags,
                    kind,
                };
                if last_key != Some(key) {
                    rpass.set_pipeline(&c.pipelines[&key]);
                    last_key = Some(key);
                }
                if last_offset != Some(draw.uniform_offset) {
                    rpass.set_bind_group(0, ubo_bind, &[draw.uniform_offset]);
                    last_offset = Some(draw.uniform_offset);
                }
                if last_texture != Some(draw.texture_id) {
                    rpass.set_bind_group(1, &draw.texture, &[]);
                    last_texture = Some(draw.texture_id);
                }
                if last_buffer != Some(draw.buffer.id) {
                    rpass.set_vertex_buffer(0, draw.buffer.buffer.slice(..));
                    last_buffer = Some(draw.buffer.id);
                }
                match &draw.indirect {
                    Some(args) => rpass.draw_indirect(args, 0),
                    None => rpass.draw(0..draw.vertex_count, 0..1),
                }
            }
        }
        c.queue.submit([encoder.finish()]);
        c.queue.present(frame);
        drop(passes);
        // The frame is submitted; retired mesh buffers are safe to reuse.
        recycle_retired_buffers(c);
    });
}
