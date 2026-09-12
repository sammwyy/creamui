//! GPU presentation: uploads a CPU-rasterized RGBA buffer into a texture and
//! blits it to the window surface with a single textured fullscreen
//! triangle. This is deliberately the entire GPU pipeline for the MVP —
//! shape/text rasterization stays on the CPU (see [`crate::painter`]) while
//! the GPU only owns compositing and presentation.

use creamui_platform::PlatformWindow;
use std::sync::Arc;

const SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    tex_width: u32,
    tex_height: u32,
}

impl GpuState {
    /// Skip probing secondary backends (GL, DX11) — they're slower to
    /// enumerate (driver/ICD loading) and this MVP pipeline (a single
    /// textured blit) has no feature that needs them over the native
    /// primary backend (Vulkan/Metal/DX12).
    ///
    /// Takes ~100-200ms on Windows (Vulkan/DX12 loader + ICD enumeration).
    /// Callers should create this as early as possible — e.g. on a
    /// background thread started before the window even exists — since it
    /// has no dependency on the window and can overlap with other startup
    /// work instead of sitting on the critical path in [`GpuState::new`].
    pub fn create_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        })
    }

    pub fn new(
        window: Arc<dyn PlatformWindow>,
        instance: &wgpu::Instance,
        transparent: bool,
    ) -> Self {
        let t0 = std::time::Instant::now();
        let size = window.inner_size();
        log::debug!("creamui-render: instance ready: {:?}", t0.elapsed());
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create GPU surface for window");
        log::debug!("creamui-render: surface created: {:?}", t0.elapsed());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("failed to find a compatible GPU adapter");
        log::debug!("creamui-render: adapter requested: {:?}", t0.elapsed());
        log::debug!("creamui-render: using GPU adapter {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("creamui-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("failed to acquire GPU device");
        log::debug!("creamui-render: device requested: {:?}", t0.elapsed());

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let alpha_mode = if transparent {
            [
                wgpu::CompositeAlphaMode::PreMultiplied,
                wgpu::CompositeAlphaMode::PostMultiplied,
                wgpu::CompositeAlphaMode::Inherit,
            ]
            .into_iter()
            .find(|mode| surface_caps.alpha_modes.contains(mode))
            .unwrap_or(surface_caps.alpha_modes[0])
        } else {
            surface_caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-blit-bind-group-layout"),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("creamui-blit-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-blit-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-blit-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creamui-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (texture, bind_group) = create_texture_and_bind_group(
            &device,
            &bind_group_layout,
            &sampler,
            size.width.max(1),
            size.height.max(1),
        );
        log::debug!(
            "creamui-render: pipeline+texture created: {:?}",
            t0.elapsed()
        );

        GpuState {
            surface,
            device,
            queue,
            config,
            pipeline,
            sampler,
            bind_group_layout,
            texture,
            bind_group,
            tex_width: size.width.max(1),
            tex_height: size.height.max(1),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (texture, bind_group) = create_texture_and_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.sampler,
            width,
            height,
        );
        self.texture = texture;
        self.bind_group = bind_group;
        self.tex_width = width;
        self.tex_height = height;
    }

    /// Uploads `rgba` (straight RGBA8, `width * height * 4` bytes) and
    /// presents it to the window surface.
    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32) {
        if width != self.tex_width || height != self.tex_height {
            self.resize(width, height);
        }
        self.write_full(rgba, width, height);
        self.draw_and_present();
    }

    /// Like [`GpuState::present`], but only re-uploads the sub-regions of
    /// `rgba` covered by `dirty` (logical pixels, scaled to physical by
    /// `scale`) instead of the whole buffer — the bandwidth-sensitive half
    /// of presenting an unchanged window with one small animating widget.
    /// Falls back to a full upload if `rgba`'s size doesn't match the
    /// texture, since that implies a resize this caller didn't account for.
    pub fn present_partial(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        dirty: &[creamui_core::Rect],
        scale: f32,
    ) {
        if width != self.tex_width || height != self.tex_height {
            self.resize(width, height);
            self.write_full(rgba, width, height);
            self.draw_and_present();
            return;
        }
        for rect in dirty {
            let x0 = (rect.x * scale).floor().max(0.0) as u32;
            let y0 = (rect.y * scale).floor().max(0.0) as u32;
            let x1 = (((rect.x + rect.width) * scale).ceil().max(0.0) as u32).min(width);
            let y1 = (((rect.y + rect.height) * scale).ceil().max(0.0) as u32).min(height);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let (w, h) = (x1 - x0, y1 - y0);
            let row_bytes = w as usize * 4;
            let aligned_row_bytes =
                row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
            let mut pixels = vec![0; aligned_row_bytes * h as usize];
            for row in 0..h as usize {
                let source = ((y0 as usize + row) * width as usize + x0 as usize) * 4;
                let destination = row * aligned_row_bytes;
                pixels[destination..destination + row_bytes]
                    .copy_from_slice(&rgba[source..source + row_bytes]);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: x0, y: y0, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_row_bytes as u32),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.draw_and_present();
    }

    fn write_full(&mut self, rgba: &[u8], width: u32, height: u32) {
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
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
    }

    fn draw_and_present(&mut self) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(err) => {
                log::warn!("creamui-render: failed to acquire surface texture: {err}");
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("creamui-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("creamui-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

fn create_texture_and_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("creamui-frame-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("creamui-blit-bind-group"),
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
