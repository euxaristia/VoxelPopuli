// wgpu backend that keeps the old GL-style immediate API: shaders hold
// persistent uniform values, meshes draw against whatever state is set,
// and the frame is recorded into passes that submit on end_frame.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// One uniform layout shared by every shader. Uniform "locations" are byte
// offsets into this block, so get_uniform_location/set_* keep GL semantics.
// Field offsets must match the Uniforms struct in assets/shaders/*.wgsl.
const UNIFORM_SIZE: usize = 224;
// Dynamic-offset slots must be aligned to the device limit (256 everywhere).
const UNIFORM_STRIDE: usize = 256;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

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
}

struct PassRec {
    target: Target,
    clear_color: Option<[f64; 4]>,
    clear_depth: bool,
    draws: Vec<DrawRec>,
}

impl PassRec {
    fn new(target: Target) -> Self {
        Self {
            target,
            clear_color: None,
            clear_depth: false,
            draws: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipeKey {
    shader_id: usize,
    flags: StateFlags,
    format: wgpu::TextureFormat,
}

struct Ctx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    surface_depth: wgpu::TextureView,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    white_texture: wgpu::BindGroup,
    pipelines: HashMap<PipeKey, wgpu::RenderPipeline>,
    ubo: Option<(wgpu::Buffer, wgpu::BindGroup, usize)>,
    // Vertex buffers are pooled by power-of-two size class: creating GPU
    // buffers is far too slow for the per-frame UI meshes this game makes.
    // Dropped meshes park their buffers in `retired` until the frame is
    // submitted, then return to the pool, so in-flight draws stay valid.
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
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> R) -> R {
    CTX.with(|c| f(c.borrow_mut().as_mut().expect("renderer not initialized")))
}

fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn init<W: wgpu::rwh::HasWindowHandle + wgpu::rwh::HasDisplayHandle>(
    window: &W,
    width: i32,
    height: i32,
) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(
                    window.display_handle().expect("display handle").as_raw(),
                ),
                raw_window_handle: window.window_handle().expect("window handle").as_raw(),
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
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
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
        device,
        queue,
        surface,
        config,
        surface_depth,
        uniform_layout,
        texture_layout,
        pipeline_layout,
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
    };
    CTX.with(|c| *c.borrow_mut() = Some(ctx));
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
        let pass = if c.passes.last().unwrap().draws.is_empty() {
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
    with_ctx(|c| {
        let last = c.passes.last().unwrap();
        if last.target == target {
            return;
        }
        if last.draws.is_empty() && last.clear_color.is_none() {
            c.passes.last_mut().unwrap().target = target;
            return;
        }
        c.passes.push(PassRec::new(target));
    });
}

static SHADER_IDS: AtomicUsize = AtomicUsize::new(0);

struct ShaderInner {
    id: usize,
    module: wgpu::ShaderModule,
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
    pub fn new(source: &str) -> Result<Self, String> {
        with_ctx(|c| {
            let scope = c.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = c
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
            match block_on(scope.pop()) {
                Some(e) => Err(e.to_string()),
                None => Ok(Self {
                    inner: Arc::new(ShaderInner {
                        id: SHADER_IDS.fetch_add(1, Ordering::Relaxed),
                        module,
                        staging: RefCell::new([0; UNIFORM_SIZE]),
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
                    c.uniform_arena
                        .resize(offset as usize + UNIFORM_STRIDE, 0);
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
            c.passes.last_mut().unwrap().draws.push(DrawRec {
                shader,
                flags,
                uniform_offset,
                texture,
                texture_id,
                buffer: self.buffer.clone(),
                vertex_count: self.vertex_count as u32,
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

pub struct RenderTexture2D {
    pub texture: Texture2D,
    depth: wgpu::TextureView,
}

impl RenderTexture2D {
    pub fn new(width: i32, height: i32) -> Self {
        let texture = Texture2D::from_data(
            &vec![0u8; (width * height * 4) as usize],
            width,
            height,
        );
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
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: VERTEX_STRIDE as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Unorm8x4
        ],
    })];
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
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: flags.blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
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
            Cst::Success(f) | Cst::Suboptimal(f) => f,
            Cst::Outdated | Cst::Lost => {
                c.surface.configure(&c.device, &c.config);
                match c.surface.get_current_texture() {
                    Cst::Success(f) | Cst::Suboptimal(f) => f,
                    _ => return,
                }
            }
            Cst::Timeout | Cst::Occluded | Cst::Validation => return,
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
            for draw in &pass.draws {
                let target_format = match pass.target {
                    Target::Surface => c.config.format,
                    Target::Offscreen { .. } => OFFSCREEN_FORMAT,
                };
                let key = PipeKey {
                    shader_id: draw.shader.id,
                    flags: draw.flags,
                    format: target_format,
                };
                if !c.pipelines.contains_key(&key) {
                    let pipeline = build_pipeline(
                        &c.device,
                        &c.pipeline_layout,
                        &draw.shader.module,
                        draw.flags,
                        target_format,
                    );
                    c.pipelines.insert(key, pipeline);
                }
            }
        }

        let mut encoder = c
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        for pass in &passes {
            if pass.draws.is_empty() && pass.clear_color.is_none() {
                continue;
            }
            let (color_view, depth_view, target_format) = match &pass.target {
                Target::Surface => (&frame_view, &c.surface_depth, c.config.format),
                Target::Offscreen { color, depth } => (color, depth, OFFSCREEN_FORMAT),
            };
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match pass.clear_color {
                            Some([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if pass.clear_depth {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
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
            for draw in &pass.draws {
                let key = PipeKey {
                    shader_id: draw.shader.id,
                    flags: draw.flags,
                    format: target_format,
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
                rpass.draw(0..draw.vertex_count, 0..1);
            }
        }
        c.queue.submit([encoder.finish()]);
        c.queue.present(frame);
        // The frame is submitted; retired mesh buffers are safe to reuse.
        let retired = std::mem::take(&mut c.retired);
        for pooled in retired {
            c.buffer_pool
                .entry(pooled.buffer.size())
                .or_default()
                .push(pooled);
        }
    });
}
