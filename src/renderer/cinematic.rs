use super::*;
use crate::vibrant::quality::GraphicsQuality;
use glam::{Mat4, Vec2, Vec3, Vec4};

pub const VOLUME_EXTENT: [u32; 3] = [96, 64, 96];

#[derive(Clone, Copy, Debug)]
pub struct CinematicFrame {
    pub view_proj: Mat4,
    pub shadow_matrices: [Mat4; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Effects {
    view_proj: [f32; 16],
    previous_view_proj: [f32; 16],
    shadows: [[f32; 16]; 4],
    splits: [f32; 4],
    viewport: [f32; 4],
    time: [f32; 4],
    volume_origin: [f32; 4],
    active_light: [f32; 4],
    light: DeferredUniforms,
}

pub(super) struct Cinematic {
    quality: GraphicsQuality,
    width: u32,
    height: u32,
    shadow: wgpu::Texture,
    shadow_layers: Vec<wgpu::TextureView>,
    shadow_sampler: wgpu::Sampler,
    water_depth_view: wgpu::TextureView,
    volume: wgpu::Texture,
    volume_origin: [i32; 3],
    opaque: wgpu::Texture,
    opaque_depth: wgpu::Texture,
    ao: wgpu::Texture,
    atmosphere: wgpu::Texture,
    scratch: wgpu::Texture,
    history: wgpu::Texture,
    history_depth: wgpu::Texture,
    history_normal: wgpu::Texture,
    bloom: Vec<wgpu::Texture>,
    bloom_scratch: Vec<wgpu::Texture>,
    pub(super) group: wgpu::BindGroup,
    effects: Effects,
    lighting_pipeline: Arc<wgpu::RenderPipeline>,
    ao_pipeline: Arc<wgpu::RenderPipeline>,
    atmosphere_pipeline: Arc<wgpu::RenderPipeline>,
    composite_pipeline: Arc<wgpu::RenderPipeline>,
    temporal_pipeline: Arc<wgpu::RenderPipeline>,
    bloom_extract: Arc<wgpu::RenderPipeline>,
    bloom_down: Arc<wgpu::RenderPipeline>,
    bloom_up: Arc<wgpu::RenderPipeline>,
    bloom_composite: Arc<wgpu::RenderPipeline>,
    post_layout: wgpu::BindGroupLayout,
    exposure: f32,
    previous_camera: Vec3,
    history_valid: bool,
    frame: u32,
    restore_target: Option<Target>,
    finished: bool,
    capture_prefix: Option<String>,
    captures: Vec<(String, wgpu::Texture)>,
}

pub(super) fn texture_entry(
    binding: u32,
    dimension: wgpu::TextureViewDimension,
    depth: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: if depth {
                wgpu::TextureSampleType::Depth
            } else {
                wgpu::TextureSampleType::Float { filterable: false }
            },
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

pub(super) fn make_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cinematic frame resources"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Effects>() as u64),
                },
                count: None,
            },
            texture_entry(1, wgpu::TextureViewDimension::D2Array, true),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            texture_entry(3, wgpu::TextureViewDimension::D3, false),
            texture_entry(4, wgpu::TextureViewDimension::D2, false),
            texture_entry(5, wgpu::TextureViewDimension::D2, true),
            texture_entry(6, wgpu::TextureViewDimension::D2, false),
            texture_entry(7, wgpu::TextureViewDimension::D2, true),
        ],
    })
}

pub(super) fn upload_map(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    let texture = make_attachment(
        device,
        "material map",
        width,
        height,
        wgpu::TextureFormat::Rgba8Unorm,
    );
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
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    view_of(&texture)
}

pub(super) fn compose(source: &str) -> String {
    let post = if source.contains("// POST") {
        include_str!("../../assets/shaders/post_common.wgsl")
    } else {
        ""
    };
    format!(
        "{}\n{post}\n{source}",
        include_str!("../../assets/shaders/cinematic_common.wgsl")
    )
}

fn snapshot_uniform<T: Copy>(
    c: &Ctx,
    layout: &wgpu::BindGroupLayout,
    value: &T,
) -> wgpu::BindGroup {
    let buffer = c.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("immutable pass uniform"),
        size: std::mem::size_of::<T>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    c.queue.write_buffer(&buffer, 0, cast_slice(&[*value]));
    c.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("immutable pass uniform"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

fn frame_group(c: &Ctx, state: &Cinematic) -> wgpu::BindGroup {
    let buffer = c.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("immutable cinematic frame"),
        size: std::mem::size_of::<Effects>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    c.queue
        .write_buffer(&buffer, 0, cast_slice(&[state.effects]));
    let views = [
        state.shadow.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        }),
        view_of(&state.volume),
        view_of(&state.opaque),
        view_of(&state.opaque_depth),
        view_of(&state.ao),
    ];
    c.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cinematic resources"),
        layout: &c.cinematic_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&state.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&views[2]),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&views[3]),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&views[4]),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&state.water_depth_view),
            },
        ],
    })
}

fn build(c: &Ctx, quality: GraphicsQuality, width: u32, height: u32) -> Cinematic {
    let device = &c.device;
    let shadow_resolution = quality.shadow_resolution();
    let shadow = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("four stabilized directional cascades"),
        size: wgpu::Extent3d {
            width: shadow_resolution,
            height: shadow_resolution,
            depth_or_array_layers: 4,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_layers = (0..4)
        .map(|index| {
            shadow.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: index,
                array_layer_count: Some(1),
                ..Default::default()
            })
        })
        .collect();
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow PCF"),
        compare: Some(wgpu::CompareFunction::LessEqual),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let volume = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bounded block light liquid cache"),
        size: wgpu::Extent3d {
            width: VOLUME_EXTENT[0],
            height: VOLUME_EXTENT[1],
            depth_or_array_layers: VOLUME_EXTENT[2],
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let hdr = |name, w, h| make_attachment(device, name, w, h, HDR_FORMAT);
    let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post inputs"),
        entries: &[
            texture_entry(0, wgpu::TextureViewDimension::D2, false),
            texture_entry(1, wgpu::TextureViewDimension::D2, true),
            texture_entry(2, wgpu::TextureViewDimension::D2, false),
            texture_entry(3, wgpu::TextureViewDimension::D2, false),
            texture_entry(4, wgpu::TextureViewDimension::D2, true),
            texture_entry(5, wgpu::TextureViewDimension::D2, false),
            texture_entry(6, wgpu::TextureViewDimension::D2, false),
            texture_entry(7, wgpu::TextureViewDimension::D2, false),
        ],
    });
    let post_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cinematic post"),
        bind_group_layouts: &[
            Some(&c.deferred_uniform_layout),
            Some(&post_layout),
            Some(&c.cinematic_layout),
        ],
        immediate_size: 0,
    });
    let lighting_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cinematic lighting"),
        bind_group_layouts: &[
            Some(&c.deferred_uniform_layout),
            Some(&c.gbuffer_layout),
            Some(&c.cinematic_layout),
        ],
        immediate_size: 0,
    });
    let ao_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SSAO"),
        bind_group_layouts: &[Some(&c.deferred_uniform_layout), Some(&c.gbuffer_layout)],
        immediate_size: 0,
    });
    let pipeline = |name: &str, layout: &wgpu::PipelineLayout, common: bool, format| {
        let source = crate::platform::read_to_string(format!("assets/shaders/{name}"))
            .expect("embedded cinematic shader");
        let source = if common || source.starts_with("// POST") {
            compose(&source)
        } else {
            source
        };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        Arc::new(build_fullscreen_pipeline(
            device,
            layout,
            &module,
            &[format],
            false,
        ))
    };
    let mut bloom = Vec::new();
    let mut bloom_scratch = Vec::new();
    for level in 1..=5 {
        let w = (width >> level).max(1);
        let h = (height >> level).max(1);
        bloom.push(hdr("bloom pyramid", w, h));
        bloom_scratch.push(hdr("bloom upsample", w, h));
    }
    // Temporary group is replaced before any draw or pass is recorded.
    let water_depth = make_depth_texture(device, width, height);
    let water_depth_view = view_of(&water_depth);
    let zero = Effects {
        view_proj: Mat4::IDENTITY.to_cols_array(),
        previous_view_proj: Mat4::IDENTITY.to_cols_array(),
        shadows: [Mat4::IDENTITY.to_cols_array(); 4],
        splits: [16., 48., 128., 256.],
        viewport: [
            width as f32,
            height as f32,
            1.0 / shadow_resolution as f32,
            quality.effect_steps() as f32,
        ],
        time: [0.; 4],
        volume_origin: [0.; 4],
        active_light: [0., 1., 0., 0.],
        light: DeferredUniforms::default(),
    };
    let dummy = snapshot_uniform(c, &c.deferred_uniform_layout, &DeferredUniforms::default());
    let mut state = Cinematic {
        quality,
        width,
        height,
        shadow,
        shadow_layers,
        shadow_sampler,
        water_depth_view,
        volume,
        volume_origin: [i32::MIN; 3],
        opaque: hdr("immutable opaque HDR", width, height),
        opaque_depth: make_depth_texture(device, width, height),
        ao: make_attachment(
            device,
            "half resolution contact AO",
            width.div_ceil(2),
            height.div_ceil(2),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        atmosphere: hdr(
            "half resolution scattering clouds",
            width.div_ceil(2),
            height.div_ceil(2),
        ),
        scratch: hdr("cinematic ping pong", width, height),
        history: hdr("world temporal history", width, height),
        history_depth: make_depth_texture(device, width, height),
        history_normal: make_attachment(
            device,
            "history normals",
            width,
            height,
            GBUFFER_NORMAL_FORMAT,
        ),
        bloom,
        bloom_scratch,
        group: dummy,
        effects: zero,
        lighting_pipeline: pipeline(
            "cinematic_lighting.wgsl",
            &lighting_layout,
            true,
            HDR_FORMAT,
        ),
        ao_pipeline: pipeline(
            "ao.wgsl",
            &ao_layout,
            false,
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        atmosphere_pipeline: pipeline("atmosphere.wgsl", &post_pipeline_layout, true, HDR_FORMAT),
        composite_pipeline: pipeline(
            "atmosphere_composite.wgsl",
            &post_pipeline_layout,
            true,
            HDR_FORMAT,
        ),
        temporal_pipeline: pipeline("temporal.wgsl", &post_pipeline_layout, true, HDR_FORMAT),
        bloom_extract: pipeline(
            "bloom_extract.wgsl",
            &post_pipeline_layout,
            true,
            HDR_FORMAT,
        ),
        bloom_down: pipeline("bloom_down.wgsl", &post_pipeline_layout, true, HDR_FORMAT),
        bloom_up: pipeline("bloom_up.wgsl", &post_pipeline_layout, true, HDR_FORMAT),
        bloom_composite: pipeline(
            "bloom_composite.wgsl",
            &post_pipeline_layout,
            true,
            HDR_FORMAT,
        ),
        post_layout,
        exposure: 0.,
        previous_camera: Vec3::ZERO,
        history_valid: false,
        frame: 0,
        restore_target: None,
        finished: false,
        capture_prefix: None,
        captures: Vec::new(),
    };
    state.group = frame_group(c, &state);
    state
}

pub fn configure_cinematic(quality: GraphicsQuality) -> Result<(), String> {
    with_ctx(|c| {
        if quality == GraphicsQuality::Fast {
            c.cinematic = None;
            c.deferred = None;
            return Ok(());
        }
        if c.cinematic.as_ref().is_some_and(|s| s.quality == quality) {
            return Ok(());
        }
        let limits = c.device.limits();
        if quality.shadow_resolution() > limits.max_texture_dimension_2d
            || limits.max_texture_array_layers < 4
            || limits.max_sampled_textures_per_shader_stage < 12
            || limits.max_color_attachment_bytes_per_sample < 20
            || limits.max_texture_dimension_3d < 96
        {
            return Err(format!(
                "{quality:?} requires {}px shadows, four layers, 12 sampled textures and 20 attachment bytes; device limits cannot supply them",
                quality.shadow_resolution()
            ));
        }
        let (width, height) = c
            .deferred
            .as_ref()
            .map(|d| (d.width, d.height))
            .unwrap_or((1, 1));
        c.cinematic = Some(build(c, quality, width, height));
        Ok(())
    })
}

pub(super) fn resize(c: &mut Ctx, width: u32, height: u32) {
    if let Some(old) = c.cinematic.take() {
        let mut new = build(c, old.quality, width, height);
        new.volume = old.volume;
        new.volume_origin = old.volume_origin;
        new.group = frame_group(c, &new);
        c.cinematic = Some(new);
    }
}

fn halton(mut n: u32, base: u32) -> f32 {
    let mut fraction = 1.;
    let mut value = 0.;
    while n > 0 {
        fraction /= base as f32;
        value += fraction * (n % base) as f32;
        n /= base;
    }
    value
}

pub fn cascade_matrices(
    view_proj: Mat4,
    camera: Vec3,
    light: Vec3,
    near: f32,
    far: f32,
    resolution: u32,
) -> ([Mat4; 4], [f32; 4]) {
    let near = near.max(0.01);
    let far = far.max(near + 1.).min(512.);
    let inverse = view_proj.inverse();
    let mut splits = [0.; 4];
    let mut matrices = [Mat4::IDENTITY; 4];
    let mut previous = near;
    let light = light.normalize_or(Vec3::Y);
    let up = if light.y.abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let orientation = glam::camera::rh::view::look_at_mat4(Vec3::ZERO, -light, up);
    for index in 0..4 {
        let ratio = (index + 1) as f32 / 4.;
        let split = 0.7 * near * (far / near).powf(ratio) + 0.3 * (near + (far - near) * ratio);
        splits[index] = split;
        let mut corners = [Vec3::ZERO; 8];
        for y in 0..2 {
            for x in 0..2 {
                let q = inverse * Vec4::new(x as f32 * 2. - 1., y as f32 * 2. - 1., 1., 1.);
                let ray = q.truncate() / q.w - camera;
                // Distances are radial, matching cascade selection in all shader consumers.
                let ray = ray.normalize_or(-Vec3::Z);
                let i = y * 2 + x;
                corners[i] = camera + ray * previous;
                corners[i + 4] = camera + ray * split;
            }
        }
        let center = corners.iter().copied().sum::<Vec3>() / 8.;
        let radius = (corners
            .iter()
            .map(|p| p.distance(center))
            .fold(0., f32::max)
            * 16.)
            .ceil()
            / 16.;
        let texel = radius * 2. / resolution.max(1) as f32;
        let mut light_center = orientation.transform_point3(center);
        light_center.x = (light_center.x / texel).round() * texel;
        light_center.y = (light_center.y / texel).round() * texel;
        // Padding admits offscreen casters above and behind the receiving slice.
        let depth_pad = 256.;
        let projection = glam::camera::rh::proj::directx::orthographic(
            light_center.x - radius,
            light_center.x + radius,
            light_center.y - radius,
            light_center.y + radius,
            -light_center.z - radius - depth_pad,
            -light_center.z + radius + depth_pad,
        );
        matrices[index] = projection * orientation;
        previous = split * 0.9;
    }
    (matrices, splits)
}

pub fn cinematic_begin_frame(
    view_proj: Mat4,
    camera: Vec3,
    clip_planes: (f32, f32),
    time: f32,
    delta: f32,
    reset: bool,
    uniforms: &DeferredUniforms,
) -> CinematicFrame {
    with_ctx(|c| {
        let Some(mut s) = c.cinematic.take() else {
            return CinematicFrame {
                view_proj,
                shadow_matrices: [Mat4::IDENTITY; 4],
            };
        };
        let reset = reset || camera.distance(s.previous_camera) > 8. || !s.history_valid;
        if reset {
            s.history_valid = false;
            s.frame = 0;
        }
        let jitter = Vec2::new(
            halton(s.frame % 16 + 1, 2) - 0.5,
            halton(s.frame % 16 + 1, 3) - 0.5,
        );
        let mut shift = Mat4::IDENTITY;
        shift.w_axis.x = jitter.x * 2. / s.width as f32;
        shift.w_axis.y = jitter.y * 2. / s.height as f32;
        let jittered = shift * view_proj;
        let sun = uniforms.sun_direction_illuminance;
        let moon = uniforms.moon_direction_illuminance;
        let active = if sun[3] >= moon[3] { sun } else { moon };
        let (shadows, splits) = cascade_matrices(
            view_proj,
            camera,
            Vec3::from_slice(&active),
            clip_planes.0,
            clip_planes.1,
            s.quality.shadow_resolution(),
        );
        let previous = s.effects.view_proj;
        s.effects.view_proj = jittered.to_cols_array();
        s.effects.previous_view_proj = previous;
        s.effects.shadows = shadows.map(|m| m.to_cols_array());
        s.effects.splits = splits;
        s.effects.time = [
            time,
            delta.clamp(0., 0.25),
            if s.history_valid { 1. } else { 0. },
            s.frame as f32,
        ];
        s.effects.active_light = active;
        let mut light = *uniforms;
        light.inv_view_proj = jittered.inverse().to_cols_array();
        let target = light.camera_pos_exposure[3];
        if reset || s.exposure <= 0. {
            s.exposure = target;
        } else {
            s.exposure = (s.exposure.ln()
                + (target.ln() - s.exposure.ln()) * (1. - (-delta.max(0.) * 1.8).exp()))
            .exp();
        }
        light.camera_pos_exposure[3] = s.exposure;
        s.effects.light = light;
        s.effects.volume_origin = [
            s.volume_origin[0] as f32,
            s.volume_origin[1] as f32,
            s.volume_origin[2] as f32,
            1.,
        ];
        s.group = frame_group(c, &s);
        s.previous_camera = camera;
        s.finished = false;
        s.frame = s.frame.wrapping_add(1);
        c.cinematic = Some(s);
        CinematicFrame {
            view_proj: jittered,
            shadow_matrices: shadows,
        }
    })
}

pub fn shadow_begin(cascade: usize) {
    with_ctx(|c| {
        let Some(s) = c.cinematic.as_mut() else {
            return;
        };
        assert!(cascade < 4);
        if s.restore_target.is_none() {
            s.restore_target = Some(c.passes.last().unwrap().target.clone());
        }
        let target = Target::Shadow {
            depth: s.shadow_layers[cascade].clone(),
        };
        let mut pass = PassRec::new(target);
        pass.clear_depth = true;
        c.passes.push(pass);
    });
}

pub fn shadow_end() {
    with_ctx(|c| {
        if let Some(target) = c.cinematic.as_mut().and_then(|s| s.restore_target.take()) {
            switch_target_inner(c, target);
        }
    });
}

pub fn water_depth_begin() {
    with_ctx(|c| {
        let Some(s) = c.cinematic.as_mut() else {
            return;
        };
        if s.restore_target.is_none() {
            s.restore_target = Some(c.passes.last().unwrap().target.clone());
        }
        let mut pass = PassRec::new(Target::Shadow {
            depth: s.water_depth_view.clone(),
        });
        pass.clear_depth = true;
        c.passes.push(pass);
    });
}

pub fn water_depth_end() {
    shadow_end();
}

pub fn cinematic_enabled() -> bool {
    with_ctx(|c| c.cinematic.is_some())
}

pub fn cinematic_capture_begin(prefix: Option<&str>) {
    assert!(prefix.is_none_or(|name| name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')));
    with_ctx(|c| {
        if let Some(s) = c.cinematic.as_mut() {
            s.capture_prefix = prefix.map(str::to_owned);
            s.captures.clear();
        }
    });
}

pub fn cinematic_capture_save() -> Result<(), String> {
    with_ctx(|c| {
        let captures = c
            .cinematic
            .as_mut()
            .map(|s| std::mem::take(&mut s.captures))
            .unwrap_or_default();
        for (name, texture) in captures {
            let path = crate::smoke::frame_capture_path(&name)?;
            save_texture_png(c, &texture, &path)?;
            println!("Rendered diagnostic {}", path.display());
        }
        Ok(())
    })
}

pub fn cinematic_upload_volume(origin: [i32; 3], offset: [u32; 3], extent: [u32; 3], data: &[u8]) {
    assert!((0..3).all(|i| extent[i] > 0 && offset[i] + extent[i] <= VOLUME_EXTENT[i]));
    assert_eq!(data.len(), extent.iter().product::<u32>() as usize * 4);
    with_ctx(|c| {
        let Some(s) = c.cinematic.as_mut() else {
            return;
        };
        if origin != s.volume_origin {
            // Unknown cells never emit outdoor shafts until their incremental upload arrives.
            let zero = vec![0u8; VOLUME_EXTENT.iter().product::<u32>() as usize * 4];
            c.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &s.volume,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &zero,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(VOLUME_EXTENT[0] * 4),
                    rows_per_image: Some(VOLUME_EXTENT[1]),
                },
                s.volume.size(),
            );
            s.volume_origin = origin;
        }
        c.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &s.volume,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: offset[0],
                    y: offset[1],
                    z: offset[2],
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(extent[0] * 4),
                rows_per_image: Some(extent[1]),
            },
            wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: extent[2],
            },
        );
    });
}

fn copy(c: &mut Ctx, source: &wgpu::Texture, destination: &wgpu::Texture) {
    let mut pass = PassRec::new(c.passes.last().unwrap().target.clone());
    pass.copies.push(TextureCopy {
        source: source.clone(),
        destination: destination.clone(),
    });
    c.passes.push(pass);
}

fn fullscreen(
    c: &mut Ctx,
    destination: &wgpu::Texture,
    pipeline: Arc<wgpu::RenderPipeline>,
    groups: Vec<wgpu::BindGroup>,
) {
    let mut pass = PassRec::new(Target::HdrResolve {
        color: view_of(destination),
    });
    pass.steps.push(Step::Fullscreen(FullscreenRec {
        pipeline,
        bind_groups: groups,
    }));
    c.passes.push(pass);
}

fn post_group(
    c: &Ctx,
    s: &Cinematic,
    scene: &wgpu::Texture,
    extra: &wgpu::Texture,
) -> wgpu::BindGroup {
    let d = c.deferred.as_ref().expect("deferred targets");
    let views = [
        view_of(scene),
        view_of(&d.depth),
        d.normal.clone(),
        view_of(&s.history),
        view_of(&s.history_depth),
        view_of(&s.history_normal),
        view_of(extra),
        d.mers.clone(),
    ];
    let entries: Vec<_> = views
        .iter()
        .enumerate()
        .map(|(i, v)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: wgpu::BindingResource::TextureView(v),
        })
        .collect();
    c.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("immutable post inputs"),
        layout: &s.post_layout,
        entries: &entries,
    })
}

fn post(
    c: &mut Ctx,
    s: &Cinematic,
    destination: &wgpu::Texture,
    pipeline: Arc<wgpu::RenderPipeline>,
    scene: &wgpu::Texture,
    extra: &wgpu::Texture,
) {
    let uniforms = c.deferred.as_ref().unwrap().uniform_group.clone();
    fullscreen(
        c,
        destination,
        pipeline,
        vec![uniforms, post_group(c, s, scene, extra), s.group.clone()],
    );
}

fn capture_stage(c: &mut Ctx, s: &mut Cinematic, name: &str, source: &wgpu::Texture) {
    let Some(prefix) = &s.capture_prefix else {
        return;
    };
    let texture = make_attachment(
        &c.device,
        "render diagnostic",
        s.width,
        s.height,
        OFFSCREEN_FORMAT,
    );
    let (pipeline, groups) = if name == "normals" {
        let module = c.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("normal validity diagnostic"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/normal_debug.wgsl").into(),
            ),
        });
        let layout = c
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("normal diagnostic"),
                bind_group_layouts: &[Some(&c.gbuffer_layout)],
                immediate_size: 0,
            });
        let pipeline = Arc::new(build_fullscreen_pipeline(
            &c.device,
            &layout,
            &module,
            &[OFFSCREEN_FORMAT],
            false,
        ));
        (
            pipeline,
            vec![c.deferred.as_ref().unwrap().gbuffer_group.clone()],
        )
    } else {
        let pipeline = Arc::new(build_fullscreen_pipeline(
            &c.device,
            &c.tonemap_layout,
            &c.tonemap_module,
            &[OFFSCREEN_FORMAT],
            false,
        ));
        let view = view_of(source);
        let group = c.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diagnostic HDR input"),
            layout: &c.hdr_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        (
            pipeline,
            vec![c.deferred.as_ref().unwrap().uniform_group.clone(), group],
        )
    };
    fullscreen(c, &texture, pipeline, groups);
    s.captures.push((format!("{prefix}-{name}"), texture));
}

pub(super) fn resolve(c: &mut Ctx, _uniforms: &DeferredUniforms) {
    let Some(mut s) = c.cinematic.take() else {
        return;
    };
    let light = s.effects.light;
    let layout = c.deferred_uniform_layout.clone();
    let uniform_group = snapshot_uniform(c, &layout, &light);
    c.deferred
        .as_mut()
        .expect("cinematic needs deferred geometry")
        .uniform_group = uniform_group;
    let d = c.deferred.as_ref().unwrap();
    let hdr = d.hdr_texture.clone();
    let depth = d.depth.clone();
    let geometry = d.gbuffer_group.clone();
    let uniform = d.uniform_group.clone();
    fullscreen(
        c,
        &s.ao,
        s.ao_pipeline.clone(),
        vec![uniform.clone(), geometry.clone()],
    );
    fullscreen(
        c,
        &hdr,
        s.lighting_pipeline.clone(),
        vec![uniform, geometry, s.group.clone()],
    );
    capture_stage(c, &mut s, "normals", &hdr);
    capture_stage(c, &mut s, "lit", &hdr);
    copy(c, &hdr, &s.opaque);
    copy(c, &depth, &s.opaque_depth);
    post(
        c,
        &s,
        &s.atmosphere,
        s.atmosphere_pipeline.clone(),
        &s.opaque,
        &s.opaque,
    );
    post(
        c,
        &s,
        &hdr,
        s.composite_pipeline.clone(),
        &s.opaque,
        &s.atmosphere,
    );
    capture_stage(c, &mut s, "atmosphere", &hdr);
    copy(c, &hdr, &s.opaque);
    switch_target_inner(
        c,
        Target::Hdr {
            color: view_of(&hdr),
            depth: view_of(&depth),
        },
    );
    c.cinematic = Some(s);
}

pub fn cinematic_begin_water() {
    with_ctx(|c| {
        let Some(s) = c.cinematic.as_ref() else {
            return;
        };
        let hdr = c.deferred.as_ref().unwrap().hdr_texture.clone();
        let opaque = s.opaque.clone();
        let (color, depth) = {
            let deferred = c.deferred.as_ref().unwrap();
            (deferred.hdr.clone(), view_of(&deferred.depth))
        };
        copy(c, &hdr, &opaque);
        switch_target_inner(c, Target::Hdr { color, depth });
    });
}

pub fn cinematic_finish_world() {
    with_ctx(|c| {
        let Some(mut s) = c.cinematic.take() else {
            return;
        };
        if s.finished {
            c.cinematic = Some(s);
            return;
        }
        let hdr = c.deferred.as_ref().unwrap().hdr_texture.clone();
        let depth = c.deferred.as_ref().unwrap().depth.clone();
        let normal = c.deferred.as_ref().unwrap().normal_texture.clone();
        capture_stage(c, &mut s, "forward", &hdr);
        // Color delta against the opaque snapshot is a reactive mask for water, particles and moving effects.
        post(
            c,
            &s,
            &s.scratch,
            s.temporal_pipeline.clone(),
            &hdr,
            &s.opaque,
        );
        let temporal = s.scratch.clone();
        capture_stage(c, &mut s, "temporal", &temporal);
        copy(c, &s.scratch, &s.history);
        copy(c, &depth, &s.history_depth);
        copy(c, &normal, &s.history_normal);
        post(
            c,
            &s,
            &s.bloom[0],
            s.bloom_extract.clone(),
            &s.scratch,
            &s.opaque,
        );
        for level in 1..s.bloom.len() {
            post(
                c,
                &s,
                &s.bloom[level],
                s.bloom_down.clone(),
                &s.bloom[level - 1],
                &s.opaque,
            );
        }
        let last = s.bloom.len() - 1;
        copy(c, &s.bloom[last], &s.bloom_scratch[last]);
        for level in (0..last).rev() {
            post(
                c,
                &s,
                &s.bloom_scratch[level],
                s.bloom_up.clone(),
                &s.bloom[level],
                &s.bloom_scratch[level + 1],
            );
        }
        post(
            c,
            &s,
            &hdr,
            s.bloom_composite.clone(),
            &s.scratch,
            &s.bloom_scratch[0],
        );
        capture_stage(c, &mut s, "bloom", &hdr);
        s.history_valid = true;
        s.finished = true;
        switch_target_inner(
            c,
            Target::Hdr {
                color: view_of(&hdr),
                depth: view_of(&depth),
            },
        );
        c.cinematic = Some(s);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effect_layout_is_separate_from_legacy_draw_abi() {
        assert_eq!(UNIFORM_SIZE, 240);
        assert_eq!(std::mem::offset_of!(Effects, light), 464);
        assert_eq!(
            std::mem::size_of::<Effects>(),
            464 + std::mem::size_of::<DeferredUniforms>()
        );
    }
    #[test]
    fn cascade_splits_are_monotonic_and_cover_far_distance() {
        let camera = Vec3::new(0., 100., 0.);
        let vp = glam::camera::rh::proj::directx::perspective(1.2, 1.6, 0.1, 512.)
            * glam::camera::rh::view::look_at_mat4(camera, camera - Vec3::Z, Vec3::Y);
        let (matrices, splits) = cascade_matrices(vp, camera, Vec3::Y, 0.1, 256., 4096);
        assert!(splits.windows(2).all(|s| s[1] > s[0]));
        assert!((splits[3] - 256.).abs() < 0.001);
        assert!(matrices.iter().all(|m| m.is_finite()));
    }
    #[test]
    fn halton_jitter_is_bounded_and_not_static() {
        let values: Vec<_> = (1..=16)
            .map(|i| Vec2::new(halton(i, 2), halton(i, 3)))
            .collect();
        assert!(
            values
                .iter()
                .all(|v| v.min_element() > 0. && v.max_element() < 1.)
        );
        assert!(values.windows(2).all(|v| v[0] != v[1]));
    }
    #[test]
    fn shaders_validate_without_adapter_or_window() {
        let sources = [
            (
                "common",
                include_str!("../../assets/shaders/cinematic_common.wgsl"),
                false,
            ),
            (
                "lighting",
                include_str!("../../assets/shaders/cinematic_lighting.wgsl"),
                true,
            ),
            ("AO", include_str!("../../assets/shaders/ao.wgsl"), false),
            (
                "atmosphere",
                include_str!("../../assets/shaders/atmosphere.wgsl"),
                true,
            ),
            (
                "composite",
                include_str!("../../assets/shaders/atmosphere_composite.wgsl"),
                true,
            ),
            (
                "water",
                include_str!("../../assets/shaders/cinematic_water.wgsl"),
                true,
            ),
            (
                "temporal",
                include_str!("../../assets/shaders/temporal.wgsl"),
                true,
            ),
            (
                "extract",
                include_str!("../../assets/shaders/bloom_extract.wgsl"),
                true,
            ),
            (
                "down",
                include_str!("../../assets/shaders/bloom_down.wgsl"),
                true,
            ),
            (
                "up",
                include_str!("../../assets/shaders/bloom_up.wgsl"),
                true,
            ),
            (
                "bloom composite",
                include_str!("../../assets/shaders/bloom_composite.wgsl"),
                true,
            ),
            (
                "shadow",
                include_str!("../../assets/shaders/shadow.wgsl"),
                false,
            ),
            (
                "water depth",
                include_str!("../../assets/shaders/water_depth.wgsl"),
                false,
            ),
            (
                "glass",
                include_str!("../../assets/shaders/cinematic_glass.wgsl"),
                true,
            ),
            (
                "geometry",
                include_str!("../../assets/shaders/gbuffer.wgsl"),
                false,
            ),
        ];
        for (name, source, common) in sources {
            let source = if common {
                compose(source)
            } else {
                source.to_owned()
            };
            let module = wgpu::naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }
}
