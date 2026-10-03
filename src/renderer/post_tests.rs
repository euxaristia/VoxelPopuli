use super::*;
use glam::{Mat4, Vec3};
use std::path::PathBuf;

const SIZE: u32 = 64;

struct Probe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    light_layout: wgpu::BindGroupLayout,
    post_layout: wgpu::BindGroupLayout,
    geometry_layout: wgpu::BindGroupLayout,
    effects_layout: wgpu::BindGroupLayout,
    light: wgpu::BindGroup,
    effects: wgpu::BindGroup,
    depth: wgpu::Texture,
    normal: wgpu::Texture,
    zero: wgpu::Texture,
}

fn shader(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/shaders");
    let source = std::fs::read_to_string(root.join(name)).unwrap();
    let common = std::fs::read_to_string(root.join("cinematic_common.wgsl")).unwrap();
    let post = if source.starts_with("// POST") {
        std::fs::read_to_string(root.join("post_common.wgsl")).unwrap()
    } else {
        String::new()
    };
    let source = format!("{common}\n{post}\n{source}");
    let module = wgpu::naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(&source)));
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    source
}

fn texture(
    device: &wgpu::Device,
    name: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(name),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn uniform(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    bytes: &[u8],
) -> wgpu::BindGroup {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("headless immutable uniform"),
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("headless immutable uniform"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

impl Probe {
    fn new(light: DeferredUniforms, history_valid: bool, previous: Mat4) -> Self {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = match std::env::var("WGPU_BACKEND").as_deref() {
            Ok("vulkan") => wgpu::Backends::VULKAN,
            Ok("dx12") | Err(_) => wgpu::Backends::DX12,
            Ok(value) => panic!("unsupported explicit probe backend: {value}"),
        };
        let instance = wgpu::Instance::new(descriptor);
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .expect("requested headless adapter required");
        println!(
            "HEADLESS adapter={:?}; surface=None; no game launched",
            adapter.get_info()
        );
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("headless post regression probe"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap();
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("headless lighting layout"),
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
        let post_entries: Vec<_> = (0..8)
            .map(|i| cinematic::texture_entry(i, wgpu::TextureViewDimension::D2, i == 1 || i == 4))
            .collect();
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("production post layout"),
            entries: &post_entries,
        });
        let geometry_entries: Vec<_> = (0..5)
            .map(|i| cinematic::texture_entry(i, wgpu::TextureViewDimension::D2, i == 4))
            .collect();
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("production gbuffer layout"),
            entries: &geometry_entries,
        });
        let effects_layout = cinematic::make_layout(&device);
        let light_group = uniform(&device, &queue, &light_layout, cast_slice(&[light]));
        let depth = texture(&device, "known geometry depth", SIZE, SIZE, DEPTH_FORMAT);
        let normal = texture(&device, "known geometry normal", SIZE, SIZE, HDR_FORMAT);
        let zero = texture(&device, "known zero color", SIZE, SIZE, HDR_FORMAT);
        let ao = texture(
            &device,
            "known contact AO",
            SIZE / 2,
            SIZE / 2,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let shadow = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("known unoccluded shadow layers"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 4,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let volume = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("known dry unlit volume"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 4,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            volume.as_image_copy(),
            &[0u8; 256],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16),
                rows_per_image: Some(4),
            },
            volume.size(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut fields = Vec::<f32>::new();
        fields.extend(Mat4::IDENTITY.to_cols_array());
        fields.extend(previous.to_cols_array());
        for _ in 0..4 {
            fields.extend(Mat4::IDENTITY.to_cols_array());
        }
        fields.extend([16., 48., 128., 256.]);
        fields.extend([SIZE as f32, SIZE as f32, 0.25, 24.]);
        fields.extend([0., 1. / 60., if history_valid { 1. } else { 0. }, 0.]);
        fields.extend([-2., -2., -2., 1.]);
        fields.extend(light.moon_direction_illuminance);
        assert_eq!(fields.len() * 4, 464);
        let mut bytes = cast_slice(&fields).to_vec();
        bytes.extend(cast_slice(&[light]));
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("known cinematic effects"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, &bytes);
        let views = [
            shadow.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            view_of(&volume),
            view_of(&zero),
            view_of(&depth),
            view_of(&ao),
            view_of(&depth),
        ];
        let effects = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("known cinematic inputs"),
            layout: &effects_layout,
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
                    resource: wgpu::BindingResource::Sampler(&sampler),
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
                    resource: wgpu::BindingResource::TextureView(&views[5]),
                },
            ],
        });
        let probe = Self {
            device,
            queue,
            light_layout,
            post_layout,
            geometry_layout,
            effects_layout,
            light: light_group,
            effects,
            depth,
            normal,
            zero,
        };
        probe.clear_depth(&probe.depth, 0.5);
        probe.clear_color(&probe.normal, [0., 0., 1., 1.]);
        probe.clear_color(&probe.zero, [0.; 4]);
        probe.clear_color(&ao, [1.; 4]);
        for layer in 0..4 {
            let view = shadow.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let mut encoder = probe.device.create_command_encoder(&Default::default());
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
            }
            probe.queue.submit([encoder.finish()]);
        }
        probe
    }

    fn clear_color(&self, target: &wgpu::Texture, color: [f64; 4]) {
        let view = view_of(target);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
    }

    fn clear_depth(&self, target: &wgpu::Texture, value: f32) {
        let view = view_of(target);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(value),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
    }

    fn input(&self, pixels: &[[f32; 4]]) -> wgpu::Texture {
        assert_eq!(pixels.len(), (SIZE * SIZE) as usize);
        let input = texture(
            &self.device,
            "synthetic HDR input",
            SIZE,
            SIZE,
            wgpu::TextureFormat::Rgba32Float,
        );
        self.queue.write_texture(
            input.as_image_copy(),
            cast_slice(pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 16),
                rows_per_image: Some(SIZE),
            },
            input.size(),
        );
        input
    }

    fn pipeline(&self, name: &str, geometry: bool) -> wgpu::RenderPipeline {
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(name),
                bind_group_layouts: &[
                    Some(&self.light_layout),
                    Some(if geometry {
                        &self.geometry_layout
                    } else {
                        &self.post_layout
                    }),
                    Some(&self.effects_layout),
                ],
                immediate_size: 0,
            });
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(shader(name).into()),
            });
        build_fullscreen_pipeline(&self.device, &layout, &module, &[HDR_FORMAT], false)
    }

    fn group(
        &self,
        layout: &wgpu::BindGroupLayout,
        textures: &[&wgpu::Texture],
    ) -> wgpu::BindGroup {
        let views: Vec<_> = textures.iter().map(|texture| view_of(texture)).collect();
        let entries: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(i, view)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("known probe inputs"),
            layout,
            entries: &entries,
        })
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        pipeline: &wgpu::RenderPipeline,
        inputs: &wgpu::BindGroup,
    ) {
        let view = view_of(target);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("headless production shader pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.light, &[]);
        pass.set_bind_group(1, inputs, &[]);
        pass.set_bind_group(2, &self.effects, &[]);
        pass.draw(0..3, 0..1);
    }

    fn post(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        pipeline: &wgpu::RenderPipeline,
        scene: &wgpu::Texture,
        extra: &wgpu::Texture,
        history: &wgpu::Texture,
    ) {
        let inputs = self.group(
            &self.post_layout,
            &[
                scene,
                &self.depth,
                &self.normal,
                history,
                &self.depth,
                &self.normal,
                extra,
                &self.zero,
            ],
        );
        self.draw(encoder, target, pipeline, &inputs);
    }

    fn bloom(&self, scene: &wgpu::Texture) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let extract = self.pipeline("bloom_extract.wgsl", false);
        let down = self.pipeline("bloom_down.wgsl", false);
        let up = self.pipeline("bloom_up.wgsl", false);
        let composite = self.pipeline("bloom_composite.wgsl", false);
        let pyramid: Vec<_> = (1..=5)
            .map(|level| {
                texture(
                    &self.device,
                    "bloom pyramid",
                    SIZE >> level,
                    SIZE >> level,
                    HDR_FORMAT,
                )
            })
            .collect();
        let scratch: Vec<_> = (1..=5)
            .map(|level| {
                texture(
                    &self.device,
                    "bloom upsample",
                    SIZE >> level,
                    SIZE >> level,
                    HDR_FORMAT,
                )
            })
            .collect();
        let output = texture(&self.device, "bloom composite", SIZE, SIZE, HDR_FORMAT);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.post(
            &mut encoder,
            &pyramid[0],
            &extract,
            scene,
            &self.zero,
            &self.zero,
        );
        for level in 1..5 {
            self.post(
                &mut encoder,
                &pyramid[level],
                &down,
                &pyramid[level - 1],
                &self.zero,
                &self.zero,
            );
        }
        encoder.copy_texture_to_texture(
            pyramid[4].as_image_copy(),
            scratch[4].as_image_copy(),
            pyramid[4].size(),
        );
        for level in (0..4).rev() {
            self.post(
                &mut encoder,
                &scratch[level],
                &up,
                &pyramid[level],
                &scratch[level + 1],
                &self.zero,
            );
        }
        self.post(
            &mut encoder,
            &output,
            &composite,
            scene,
            &scratch[0],
            &self.zero,
        );
        self.queue.submit([encoder.finish()]);
        (self.read(&scratch[0]), self.read(&output))
    }

    fn read(&self, target: &wgpu::Texture) -> Vec<[f32; 4]> {
        let stride = (target.width() * 8).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("numeric HDR readback"),
            size: (stride * target.height()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(target.height()),
                },
            },
            target.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap();
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        receiver.recv().unwrap().unwrap();
        let bytes = buffer.slice(..).get_mapped_range().unwrap();
        let mut result = Vec::new();
        for row in bytes.chunks_exact(stride as usize) {
            for pixel in row[..target.width() as usize * 8].as_chunks::<8>().0 {
                let color = std::array::from_fn(|i| {
                    half(u16::from_le_bytes([pixel[2 * i], pixel[2 * i + 1]]))
                });
                assert!(
                    color.iter().all(|v| v.is_finite()),
                    "non-finite GPU output: {color:?}"
                );
                result.push(color);
            }
        }
        drop(bytes);
        buffer.unmap();
        result
    }
}

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (bits >> 10) & 31;
    let mantissa = bits & 1023;
    match exponent {
        0 => sign * mantissa as f32 * 2f32.powi(-24),
        31 => {
            if mantissa == 0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1. + mantissa as f32 / 1024.) * 2f32.powi(exponent as i32 - 15),
    }
}

fn stats(label: &str, pixels: &[[f32; 4]]) -> (f32, f32, f32) {
    let min = pixels.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max = pixels
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let sum = pixels.iter().map(|p| p[0]).sum::<f32>();
    println!("{label}: red min={min:.9} max={max:.9} sum={sum:.9}");
    (min, max, sum)
}

fn night() -> DeferredUniforms {
    crate::vibrant::frame::build_uniforms(
        &crate::vibrant::VibrantPack::default(),
        &crate::vibrant::frame::FrameInput {
            day_fraction: 0.5,
            camera_pos: Vec3::new(0., 0., 2.),
            view_proj: Mat4::IDENTITY,
        },
    )
}

#[test]
#[ignore = "explicit native headless GPU probe; requests an adapter, never a surface"]
fn headless_bloom_dark_and_temporal_history_remain_bounded() {
    let mut light = night();
    light.camera_pos_exposure[3] = 1.;
    let probe = Probe::new(light, true, Mat4::from_translation(Vec3::new(0.1, 0., 0.)));
    let scene = probe.input(&vec![[0.04, 0.04, 0.04, 1.]; (SIZE * SIZE) as usize]);
    let poisoned = probe.input(&vec![[50000., 50000., 50000., 1.]; (SIZE * SIZE) as usize]);
    let temporal = probe.pipeline("temporal.wgsl", false);
    let resolved = texture(&probe.device, "temporal ping pong", SIZE, SIZE, HDR_FORMAT);
    let history = texture(
        &probe.device,
        "independent temporal history",
        SIZE,
        SIZE,
        HDR_FORMAT,
    );
    let mut encoder = probe.device.create_command_encoder(&Default::default());
    probe.post(
        &mut encoder,
        &resolved,
        &temporal,
        &scene,
        &scene,
        &poisoned,
    );
    encoder.copy_texture_to_texture(
        resolved.as_image_copy(),
        history.as_image_copy(),
        resolved.size(),
    );
    probe.queue.submit([encoder.finish()]);
    let first = probe.read(&resolved);
    stats("temporal shifted camera, poisoned history", &first);
    assert!(first.iter().all(|p| (p[0] - 0.04).abs() < 0.0001));
    for _ in 0..3 {
        let mut encoder = probe.device.create_command_encoder(&Default::default());
        probe.post(&mut encoder, &resolved, &temporal, &scene, &scene, &history);
        encoder.copy_texture_to_texture(
            resolved.as_image_copy(),
            history.as_image_copy(),
            resolved.size(),
        );
        probe.queue.submit([encoder.finish()]);
    }
    let (bloom, output) = probe.bloom(&resolved);
    assert_eq!(stats("dark full bloom chain", &bloom).1, 0.);
    stats("dark composite after four temporal frames", &output);
    assert!(output.iter().all(|p| (p[0] - 0.04).abs() < 0.0001));
}

#[test]
#[ignore = "explicit native headless GPU regression; fails while extraction drops pixel phases"]
fn headless_bloom_preserves_highlight_energy_across_pixel_phase() {
    let mut light = night();
    light.camera_pos_exposure[3] = 1.;
    let probe = Probe::new(light, false, Mat4::IDENTITY);
    let mut sums = Vec::new();
    for x in [30, 31] {
        let mut pixels = vec![[0.04, 0.04, 0.04, 1.]; (SIZE * SIZE) as usize];
        pixels[(31 * SIZE + x) as usize] = [8., 8., 8., 1.];
        let input = probe.input(&pixels);
        let (bloom, output) = probe.bloom(&input);
        sums.push(
            stats(
                &format!("non-emissive HDR highlight x={x}, y=31 bloom"),
                &bloom,
            )
            .2,
        );
        stats(&format!("highlight x={x} composite"), &output);
    }
    assert!(
        sums.iter().all(|sum| *sum > 0.01),
        "one-pixel camera movement must not erase a highlight's bloom: {sums:?}"
    );
    assert!(
        (sums[0] - sums[1]).abs() / sums[0].max(sums[1]) < 0.1,
        "bloom energy must not depend strongly on pixel phase: {sums:?}"
    );
}

#[test]
#[ignore = "explicit native headless GPU regression for degenerate lighting inputs"]
fn headless_night_degenerate_normals_and_inactive_light_stay_bounded() {
    let mut light = night();
    light.camera_pos_exposure[0] = 1. / SIZE as f32;
    light.camera_pos_exposure[1] = -1. / SIZE as f32;
    light.sun_direction_illuminance = [0., 0., -1., 0.];
    light.moon_direction_illuminance = [0., 0., 1., 0.27];
    let probe = Probe::new(light, false, Mat4::IDENTITY);
    let pipeline = probe.pipeline("cinematic_lighting.wgsl", true);
    let albedo = texture(
        &probe.device,
        "non-emissive albedo",
        SIZE,
        SIZE,
        GBUFFER_ALBEDO_FORMAT,
    );
    let material = texture(
        &probe.device,
        "non-emissive matte material",
        SIZE,
        SIZE,
        GBUFFER_MERS_FORMAT,
    );
    let baked = texture(
        &probe.device,
        "no block light",
        SIZE,
        SIZE,
        GBUFFER_LIGHTING_FORMAT,
    );
    let output = texture(
        &probe.device,
        "degenerate lighting readback",
        SIZE,
        SIZE,
        HDR_FORMAT,
    );
    probe.clear_color(&albedo, [0.5, 0.5, 0.5, 1.]);
    probe.clear_color(&material, [0., 0., 1., 0.]);
    probe.clear_color(&baked, [1., 0., 1., 1.]);
    let mut bad_cases = Vec::new();
    for (name, normal) in [
        ("opposing inactive sun", [0., 0., 1., 1.]),
        ("zero normal", [0., 0., 0., 1.]),
        ("isolated zero normal", [0., 0., 1., 1.]),
        ("isolated NaN mapped normal", [0., 0., 1., 1.]),
    ] {
        let mut normals = vec![normal; (SIZE * SIZE) as usize];
        if name == "isolated zero normal" {
            normals[(31 * SIZE + 31) as usize] = [0., 0., 0., 1.];
        }
        if name == "isolated NaN mapped normal" {
            normals[(31 * SIZE + 31) as usize] = [f32::NAN, f32::NAN, f32::NAN, 1.];
        }
        let normal_input = probe.input(&normals);
        let group = probe.group(
            &probe.geometry_layout,
            &[&albedo, &normal_input, &material, &baked, &probe.depth],
        );
        let mut encoder = probe.device.create_command_encoder(&Default::default());
        probe.draw(&mut encoder, &output, &pipeline, &group);
        probe.queue.submit([encoder.finish()]);
        let pixels = probe.read(&output);
        let (_, maximum, _) = stats(name, &pixels);
        let center = pixels[(32 * SIZE + 32) as usize];
        let isolated = pixels[(31 * SIZE + 31) as usize];
        println!(
            "{name}: center={center:?} pixel31={isolated:?} exposed_peak={:.9}",
            maximum * light.camera_pos_exposure[3]
        );
        let (bloom, _) = probe.bloom(&output);
        stats(&format!("{name} bloom"), &bloom);
        if maximum * light.camera_pos_exposure[3] > 0.92 {
            bad_cases.push((name, maximum));
        }
    }
    assert!(
        bad_cases.is_empty(),
        "non-emissive matte inputs must not become bloom sources: {bad_cases:?}"
    );
}

#[test]
#[ignore = "explicit native headless GPU probe of production nighttime lighting"]
fn headless_night_non_emissive_geometry_has_finite_radiance() {
    let light = night();
    println!(
        "night exposure={} sun_lux={} moon_lux={}",
        light.camera_pos_exposure[3],
        light.sun_direction_illuminance[3],
        light.moon_direction_illuminance[3]
    );
    let probe = Probe::new(light, false, Mat4::IDENTITY);
    let pipeline = probe.pipeline("cinematic_lighting.wgsl", true);
    let albedo = texture(
        &probe.device,
        "non-emissive albedo",
        SIZE,
        SIZE,
        GBUFFER_ALBEDO_FORMAT,
    );
    let material = texture(
        &probe.device,
        "zero emission dry material",
        SIZE,
        SIZE,
        GBUFFER_MERS_FORMAT,
    );
    let baked = texture(
        &probe.device,
        "zero block light",
        SIZE,
        SIZE,
        GBUFFER_LIGHTING_FORMAT,
    );
    let output = texture(
        &probe.device,
        "production nighttime lighting",
        SIZE,
        SIZE,
        HDR_FORMAT,
    );
    probe.clear_color(&albedo, [0.5, 0.5, 0.5, 1.]);
    probe.clear_color(&baked, [1., 0., 1., 1.]);
    let moon = Vec3::from_slice(&light.moon_direction_illuminance);
    let normal = (moon + Vec3::Z).normalize();
    probe.clear_color(
        &probe.normal,
        [normal.x as f64, normal.y as f64, normal.z as f64, 1.],
    );
    for (name, metal, rough) in [("stone", 0., 1.), ("glossy non-emissive metal", 1., 0.14)] {
        probe.clear_color(&material, [metal, 0., rough, 0.]);
        let group = probe.group(
            &probe.geometry_layout,
            &[&albedo, &probe.normal, &material, &baked, &probe.depth],
        );
        let mut encoder = probe.device.create_command_encoder(&Default::default());
        probe.draw(&mut encoder, &output, &pipeline, &group);
        probe.queue.submit([encoder.finish()]);
        let pixels = probe.read(&output);
        let (_, maximum, _) = stats(name, &pixels);
        println!(
            "{name}: exposed_peak={:.9} pixels_above_bloom_threshold={}",
            maximum * light.camera_pos_exposure[3],
            pixels
                .iter()
                .filter(|p| (p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722)
                    * light.camera_pos_exposure[3]
                    > 0.92)
                .count()
        );
        assert!(
            pixels
                .iter()
                .all(|p| p[..3].iter().all(|v| *v >= 0. && *v < 60000.))
        );
        if name == "stone" {
            assert!(maximum * light.camera_pos_exposure[3] < 0.92);
        }
        let (bloom, _) = probe.bloom(&output);
        stats(&format!("{name} full bloom chain"), &bloom);
    }
}

#[test]
#[ignore = "explicit headless GPU regression for production gbuffer normal generation"]
fn headless_gbuffer_neutral_and_degenerate_atlas_derivatives_keep_valid_normals() {
    let probe = Probe::new(night(), false, Mat4::IDENTITY);
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/shaders/gbuffer.wgsl"),
    )
    .unwrap();
    let module = probe
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("production geometry normal regression"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let formats = [
        GBUFFER_ALBEDO_FORMAT,
        GBUFFER_NORMAL_FORMAT,
        GBUFFER_MERS_FORMAT,
        GBUFFER_LIGHTING_FORMAT,
    ];
    let targets: Vec<_> = formats
        .iter()
        .map(|format| {
            Some(wgpu::ColorTargetState {
                format: *format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        })
        .collect();
    let attributes =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x4];
    let pipeline = probe
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("production geometry producer regression"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 48,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
    let mut fields = [0f32; UNIFORM_SIZE / 4];
    fields[..16].copy_from_slice(&Mat4::IDENTITY.to_cols_array());
    fields[16..32].copy_from_slice(&Mat4::IDENTITY.to_cols_array());
    fields[32..36].fill(1.);
    let uniforms = uniform(
        &probe.device,
        &probe.queue,
        &pipeline.get_bind_group_layout(0),
        cast_slice(&fields),
    );
    let atlas = texture(
        &probe.device,
        "one texel albedo atlas",
        1,
        1,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let mers = texture(
        &probe.device,
        "one texel non-emissive material",
        1,
        1,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let mapped = texture(
        &probe.device,
        "one texel normal atlas",
        1,
        1,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    probe.clear_color(&atlas, [0.5, 0.5, 0.5, 1.]);
    probe.clear_color(&mers, [0., 0., 1., 0.]);
    let views = [view_of(&atlas), view_of(&mers), view_of(&mapped)];
    let sampler = probe.device.create_sampler(&Default::default());
    let inputs = probe.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("production geometry material inputs"),
        layout: &pipeline.get_bind_group_layout(1),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&views[2]),
            },
        ],
    });
    let outputs: Vec<_> = formats
        .iter()
        .map(|format| {
            texture(
                &probe.device,
                "production gbuffer output",
                SIZE,
                SIZE,
                *format,
            )
        })
        .collect();
    let output_views: Vec<_> = outputs.iter().map(view_of).collect();
    let vertices = probe.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("geometry derivative fixture"),
        size: 3 * 48,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let positions = [[-1., -1., 0.5], [3., -1., 0.5], [-1., 3., 0.5]];
    let neutral = [128. / 255., 128. / 255., 1., 1.];
    let tilted = [191. / 255., 128. / 255., 238. / 255., 1.];
    let diagonal = [[0.2, 0.2], [0.6, 0.4], [0.4, 0.6]];
    let collapsed = [[0.2, 0.2]; 3];
    let rank_one = [[0.2, 0.2], [0.4, 0.4], [0.6, 0.6]];
    for (name, uvs, geometric, normal_texel, expected) in [
        (
            "diagonal neutral atlas",
            diagonal,
            [0., 0., 1.],
            neutral,
            [0., 0., 1.],
        ),
        (
            "collapsed neutral atlas",
            collapsed,
            [0., 0., 1.],
            neutral,
            [0., 0., 1.],
        ),
        (
            "collapsed tilted atlas",
            collapsed,
            [0., 0., 1.],
            tilted,
            [0., 0., 1.],
        ),
        (
            "rank-one tilted atlas",
            rank_one,
            [0., 0., 1.],
            tilted,
            [0., 0., 1.],
        ),
        (
            "zero geometric normal",
            diagonal,
            [0., 0., 0.],
            neutral,
            [0., 1., 0.],
        ),
    ] {
        probe.clear_color(&mapped, normal_texel);
        let mut data = Vec::new();
        for (position, uv) in positions.iter().zip(uvs) {
            data.extend(position);
            data.extend(uv);
            data.extend(geometric);
            data.extend([1f32, 0., 1., 1.]);
        }
        probe.queue.write_buffer(&vertices, 0, cast_slice(&data));
        let attachments: Vec<_> = output_views
            .iter()
            .map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();
        let mut encoder = probe.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(name),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &uniforms, &[]);
            pass.set_bind_group(1, &inputs, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..3, 0..1);
        }
        probe.queue.submit([encoder.finish()]);
        let normals = probe.read(&outputs[1]);
        let maximum_error = normals
            .iter()
            .map(|pixel| {
                (0..3)
                    .map(|i| (pixel[i] - expected[i]).abs())
                    .fold(0., f32::max)
            })
            .fold(0., f32::max);
        println!(
            "gbuffer {name}: pixels={} maximum_normal_error={maximum_error:.9}",
            normals.len()
        );
        assert!(
            maximum_error < 0.002,
            "{name} must preserve its geometric normal or documented fallback"
        );
    }
}
