//! GPU rendering of a [`DisplayList`]: every primitive becomes one instance
//! of a single signed-distance-field pipeline, glyphs come from a shared
//! coverage atlas, and draw calls only split where the image texture
//! changes.

use crate::display_list::{Clip, DisplayList, Primitive};
use crate::text::GlyphBitmap;
use bytemuck::{Pod, Zeroable};
use creamui_platform::PlatformWindow;
use creamui_theme::Color;
use fontdue::layout::GlyphRasterConfig;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

const KIND_QUAD: f32 = 0.0;
const KIND_LINE: f32 = 1.0;
const KIND_GLYPH: f32 = 2.0;
const KIND_IMAGE: f32 = 3.0;
const KIND_GRADIENT_QUAD: f32 = 4.0;
const INITIAL_ATLAS_SIZE: u32 = 1024;
const IMAGE_CACHE_FRAMES: u64 = 300;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    bounds: [f32; 4],
    color: [f32; 4],
    border_color: [f32; 4],
    data: [f32; 4],
    clip: [f32; 4],
    rounded_clip: [f32; 4],
    params: [f32; 4],
    extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

fn premultiplied(color: Color) -> [f32; 4] {
    let a = color.a as f32 / 255.0;
    [
        color.r as f32 / 255.0 * a,
        color.g as f32 / 255.0 * a,
        color.b as f32 / 255.0 * a,
        a,
    ]
}

/// Creates the process-wide `wgpu` instance. Enumerating backends is the
/// slow part of GPU startup, so callers create it off the UI thread.
pub fn create_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    })
}

struct Shelf {
    y: u32,
    height: u32,
    x: u32,
}

struct GlyphAtlas {
    texture: wgpu::Texture,
    size: u32,
    shelves: Vec<Shelf>,
    entries: HashMap<GlyphRasterConfig, [u32; 4]>,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("creamui-glyph-atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        GlyphAtlas {
            texture,
            size,
            shelves: Vec::new(),
            entries: HashMap::new(),
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        let (w, h) = (width + 1, height + 1);
        if w > self.size || h > self.size {
            return None;
        }
        if let Some(shelf) = self
            .shelves
            .iter_mut()
            .filter(|s| s.height >= h && s.height <= h + h / 2 && s.x + w <= self.size)
            .min_by_key(|s| s.height)
        {
            let at = [shelf.x, shelf.y];
            shelf.x += w;
            return Some(at);
        }
        let y = self.shelves.last().map_or(0, |s| s.y + s.height);
        if y + h > self.size {
            return None;
        }
        self.shelves.push(Shelf { y, height: h, x: w });
        Some([0, y])
    }

    fn get_or_insert(&mut self, queue: &wgpu::Queue, glyph: &GlyphBitmap) -> Option<[u32; 4]> {
        if let Some(rect) = self.entries.get(&glyph.key) {
            return Some(*rect);
        }
        let [x, y] = self.allocate(glyph.width, glyph.height)?;
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &glyph.coverage,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(glyph.width),
                rows_per_image: Some(glyph.height),
            },
            wgpu::Extent3d {
                width: glyph.width,
                height: glyph.height,
                depth_or_array_layers: 1,
            },
        );
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.gpu_upload_bytes += glyph.coverage.len() as u64);
        let rect = [x, y, glyph.width, glyph.height];
        self.entries.insert(glyph.key, rect);
        Some(rect)
    }
}

struct ImageTexture {
    bind_group: wgpu::BindGroup,
    used: u64,
}

struct Batch {
    image: Option<u64>,
    instances: std::ops::Range<u32>,
}

/// Renders display lists with a `wgpu` device into any color target of
/// its format.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_group: wgpu::BindGroup,
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    empty_image: wgpu::BindGroup,
    atlas: GlyphAtlas,
    images: HashMap<u64, ImageTexture>,
    instances: Vec<Instance>,
    batches: Vec<Batch>,
    instance_buffer: wgpu::Buffer,
    frame: u64,
}

impl GpuRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-image"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-pipeline-layout"),
            bind_group_layouts: &[&globals_layout, &image_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creamui-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4,
                        3 => Float32x4,
                        4 => Float32x4,
                        5 => Float32x4,
                        6 => Float32x4,
                        7 => Float32x4,
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("creamui-image-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas = GlyphAtlas::new(&device, INITIAL_ATLAS_SIZE);
        let globals_group =
            Self::globals_group(&device, &globals_layout, &globals, &atlas, &sampler);
        let empty = Self::upload_image(&device, &queue, 1, 1, &[0; 4]);
        let empty_image = Self::image_group(&device, &image_layout, &empty);
        let instance_buffer = Self::instance_buffer(&device, 256);
        GpuRenderer {
            device,
            queue,
            format,
            pipeline,
            globals,
            globals_layout,
            globals_group,
            image_layout,
            sampler,
            empty_image,
            atlas,
            images: HashMap::new(),
            instances: Vec::new(),
            batches: Vec::new(),
            instance_buffer,
            frame: 0,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-instances"),
            size: capacity * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn globals_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        atlas: &GlyphAtlas,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let view = atlas
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-globals"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn upload_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> wgpu::Texture {
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.gpu_upload_bytes += pixels.len() as u64);
        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("creamui-image"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        )
    }

    fn image_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-image"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        })
    }

    fn grow_atlas(&mut self) {
        let max = self.device.limits().max_texture_dimension_2d;
        let size = (self.atlas.size * 2).min(max);
        log::debug!("creamui-render: glyph atlas reset at {size}px");
        self.atlas = GlyphAtlas::new(&self.device, size);
        self.globals_group = Self::globals_group(
            &self.device,
            &self.globals_layout,
            &self.globals,
            &self.atlas,
            &self.sampler,
        );
    }

    /// Builds and uploads this frame's instances. Returns `false` if the
    /// glyph atlas overflowed and the frame must be rebuilt.
    fn build(&mut self, list: &DisplayList) -> bool {
        self.instances.clear();
        self.batches.clear();
        let mut batch_image: Option<u64> = None;
        let mut batch_start = 0u32;
        for item in &list.items {
            let image = match &item.primitive {
                Primitive::Image(image) => Some(image.image.id()),
                _ => None,
            };
            if image != batch_image && self.instances.len() as u32 > batch_start {
                self.batches.push(Batch {
                    image: batch_image,
                    instances: batch_start..self.instances.len() as u32,
                });
                batch_start = self.instances.len() as u32;
            }
            batch_image = image;
            let clip = clip_params(&item.clip);
            match &item.primitive {
                Primitive::Quad(quad) => {
                    let (kind, color, border_color, data) = match quad.gradient {
                        Some(gradient) => (
                            KIND_GRADIENT_QUAD,
                            premultiplied(gradient.start_color),
                            premultiplied(gradient.end_color),
                            [
                                gradient.start[0],
                                gradient.start[1],
                                gradient.end[0],
                                gradient.end[1],
                            ],
                        ),
                        None => (
                            KIND_QUAD,
                            premultiplied(quad.background),
                            premultiplied(quad.border_color),
                            [0.0; 4],
                        ),
                    };
                    self.instances.push(Instance {
                        bounds: [
                            quad.bounds.x0,
                            quad.bounds.y0,
                            quad.bounds.x1,
                            quad.bounds.y1,
                        ],
                        color,
                        border_color,
                        data,
                        params: [kind, quad.radius, quad.border_width, clip.2],
                        clip: clip.0,
                        rounded_clip: clip.1,
                        ..Zeroable::zeroed()
                    });
                }
                Primitive::Line(line) => {
                    let b = item.primitive.bounds();
                    self.instances.push(Instance {
                        bounds: [b.x0, b.y0, b.x1, b.y1],
                        color: premultiplied(line.color),
                        data: [line.from[0], line.from[1], line.to[0], line.to[1]],
                        params: [KIND_LINE, 0.0, line.width, clip.2],
                        clip: clip.0,
                        rounded_clip: clip.1,
                        ..Zeroable::zeroed()
                    });
                }
                Primitive::Text(run) => {
                    let visible = item.visible_bounds();
                    for glyph in &run.layout.glyphs {
                        let (x, y) = ((run.x + glyph.x) as f32, (run.y + glyph.y) as f32);
                        let (w, h) = (glyph.bitmap.width as f32, glyph.bitmap.height as f32);
                        if x > visible.x1
                            || x + w * (1.0 + run.shear_factor()) < visible.x0
                            || y > visible.y1
                            || y + h < visible.y0
                        {
                            continue;
                        }
                        let Some(rect) = self.atlas.get_or_insert(&self.queue, &glyph.bitmap)
                        else {
                            return false;
                        };
                        self.instances.push(Instance {
                            bounds: [x, y, x + w, y + h],
                            color: premultiplied(run.glyph_color(glyph.byte_offset)),
                            data: rect.map(|v| v as f32),
                            params: [KIND_GLYPH, 0.0, 0.0, clip.2],
                            clip: clip.0,
                            rounded_clip: clip.1,
                            extra: [run.shear_factor(), 0.0, 0.0, 0.0],
                            ..Zeroable::zeroed()
                        });
                    }
                }
                Primitive::Image(prim) => {
                    let frame = self.frame;
                    let texture = self.images.entry(prim.image.id()).or_insert_with(|| {
                        let texture = Self::upload_image(
                            &self.device,
                            &self.queue,
                            prim.image.width(),
                            prim.image.height(),
                            prim.image.pixels(),
                        );
                        ImageTexture {
                            bind_group: Self::image_group(
                                &self.device,
                                &self.image_layout,
                                &texture,
                            ),
                            used: frame,
                        }
                    });
                    texture.used = frame;
                    let b = prim.bounds;
                    self.instances.push(Instance {
                        bounds: [b.x0, b.y0, b.x1, b.y1],
                        color: prim.tint.map_or([0.0; 4], premultiplied),
                        params: [KIND_IMAGE, 0.0, 0.0, clip.2],
                        clip: clip.0,
                        rounded_clip: clip.1,
                        ..Zeroable::zeroed()
                    });
                }
            }
        }
        if self.instances.len() as u32 > batch_start {
            self.batches.push(Batch {
                image: batch_image,
                instances: batch_start..self.instances.len() as u32,
            });
        }
        true
    }

    fn prepare(&mut self, list: &DisplayList) {
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("gpu_prepare").entered();
        self.frame += 1;
        if !self.build(list) {
            self.grow_atlas();
            if !self.build(list) {
                log::warn!("creamui-render: glyph atlas cannot fit this frame's text");
            }
        }
        let horizon = self.frame.saturating_sub(IMAGE_CACHE_FRAMES);
        self.images.retain(|_, texture| texture.used >= horizon);

        self.queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [list.width as f32, list.height as f32],
                _pad: [0.0; 2],
            }),
        );
        let needed = self.instances.len() as u64 * std::mem::size_of::<Instance>() as u64;
        if needed > self.instance_buffer.size() {
            let capacity = (self.instances.len() as u64).next_power_of_two();
            self.instance_buffer = Self::instance_buffer(&self.device, capacity);
        }
        if !self.instances.is_empty() {
            let bytes = bytemuck::cast_slice(&self.instances);
            #[cfg(feature = "perf-metrics")]
            creamui_core::metrics::record(|m| m.gpu_upload_bytes += bytes.len() as u64);
            self.queue.write_buffer(&self.instance_buffer, 0, bytes);
        }
    }

    /// Renders `list` into `target`, clearing it first.
    pub fn render(&mut self, list: &DisplayList, target: &wgpu::TextureView) {
        self.prepare(list);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("creamui-frame"),
            });
        {
            let [r, g, b, a] = premultiplied(list.clear);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("creamui-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: r as f64,
                            g: g as f64,
                            b: b as f64,
                            a: a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            for batch in &self.batches {
                let group = batch
                    .image
                    .and_then(|id| self.images.get(&id))
                    .map_or(&self.empty_image, |texture| &texture.bind_group);
                pass.set_bind_group(1, group, &[]);
                pass.draw(0..4, batch.instances.clone());
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| m.draw_calls += 1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
}

fn clip_params(clip: &Clip) -> ([f32; 4], [f32; 4], f32) {
    let b = clip.bounds;
    match clip.rounded {
        Some(r) => (
            [b.x0, b.y0, b.x1, b.y1],
            [r.bounds.x0, r.bounds.y0, r.bounds.x1, r.bounds.y1],
            r.radius.max(0.001),
        ),
        None => ([b.x0, b.y0, b.x1, b.y1], [0.0; 4], 0.0),
    }
}

async fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError> {
    adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("creamui-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        )
        .await
}

/// A window surface presented through [`GpuRenderer`].
pub struct GpuSurface {
    renderer: GpuRenderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    adapter_name: String,
}

impl GpuSurface {
    pub fn new(
        window: Arc<dyn PlatformWindow>,
        instance: &wgpu::Instance,
        transparent: bool,
    ) -> Result<Self, String> {
        let t0 = std::time::Instant::now();
        let size = window.inner_size();
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no compatible GPU adapter")?;
        let info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(request_device(&adapter)).map_err(|e| format!("device: {e}"))?;
        device.on_uncaptured_error(Box::new(|err| {
            log::error!("creamui-render: wgpu error: {err}");
        }));

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("surface reports no formats")?;
        let view_format = format.remove_srgb_suffix();
        let alpha_mode = if transparent {
            [
                wgpu::CompositeAlphaMode::PreMultiplied,
                wgpu::CompositeAlphaMode::Inherit,
                wgpu::CompositeAlphaMode::PostMultiplied,
            ]
            .into_iter()
            .find(|mode| caps.alpha_modes.contains(mode))
            .unwrap_or(caps.alpha_modes[0])
        } else {
            [wgpu::CompositeAlphaMode::Opaque]
                .into_iter()
                .find(|mode| caps.alpha_modes.contains(mode))
                .unwrap_or(caps.alpha_modes[0])
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: if view_format == format {
                vec![]
            } else {
                vec![view_format]
            },
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);
        let renderer = GpuRenderer::new(device, queue, view_format);
        log::debug!(
            "creamui-render: GPU surface ready on {} ({:?}) as {format:?}/{alpha_mode:?} in {:?}",
            info.name,
            info.backend,
            t0.elapsed()
        );
        Ok(GpuSurface {
            renderer,
            surface,
            config,
            view_format,
            adapter_name: format!("{} ({:?})", info.name, info.backend),
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Draws and presents `list`. Returns `false` when the surface was not
    /// ready and the frame must be retried.
    pub fn present(&mut self, list: &DisplayList) -> bool {
        if (list.width, list.height) != (self.config.width, self.config.height) {
            self.config.width = list.width;
            self.config.height = list.height;
            self.surface.configure(self.renderer.device(), &self.config);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(self.renderer.device(), &self.config);
                return false;
            }
            Err(err) => {
                log::warn!("creamui-render: failed to acquire surface texture: {err}");
                return false;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        self.renderer.render(list, &view);
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("present").entered();
        frame.present();
        true
    }
}

/// Offscreen rendering with pixel readback, for tests, benchmarks and
/// frame dumps without a window.
pub struct HeadlessGpu {
    renderer: GpuRenderer,
    adapter_name: String,
}

impl HeadlessGpu {
    /// Uses the first available adapter; `CUI_GPU_FALLBACK=1` forces a
    /// software adapter such as llvmpipe.
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: std::env::var("CUI_GPU_FALLBACK").as_deref() == Ok("1"),
        }))
        .ok_or("no GPU adapter")?;
        let info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(request_device(&adapter)).map_err(|e| format!("device: {e}"))?;
        Ok(HeadlessGpu {
            renderer: GpuRenderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
            adapter_name: format!("{} ({:?})", info.name, info.backend),
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn renderer(&self) -> &GpuRenderer {
        &self.renderer
    }

    /// Renders `list` and waits for the GPU to finish.
    pub fn render(&mut self, list: &DisplayList) -> wgpu::Texture {
        let texture = self
            .renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("creamui-headless"),
                size: wgpu::Extent3d {
                    width: list.width,
                    height: list.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render(list, &view);
        self.renderer.device.poll(wgpu::Maintain::Wait);
        texture
    }

    /// Renders `list` and reads back premultiplied RGBA8 pixels.
    pub fn render_to_pixels(&mut self, list: &DisplayList) -> Vec<u8> {
        let texture = self.render(list);
        let (width, height) = (list.width, list.height);
        let padded = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let device = &self.renderer.device;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("creamui-readback"),
            contents: &vec![0; (padded * height) as usize],
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("creamui-readback"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.renderer
            .queue
            .submit(std::iter::once(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            if let Err(err) = result {
                log::error!("creamui-render: readback failed: {err}");
            }
        });
        device.poll(wgpu::Maintain::Wait);
        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in mapped.chunks_exact(padded as usize) {
            pixels.extend_from_slice(&row[..(width * 4) as usize]);
        }
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::Damage;
    use crate::raster::Rasterizer;
    use crate::recorder::SceneRecorder;
    use creamui_core::{Painter, Point, Rect, RgbaImage, TextAlign};
    use creamui_theme::ColorScheme;

    fn headless() -> Option<HeadlessGpu> {
        match HeadlessGpu::new() {
            Ok(gpu) => Some(gpu),
            Err(err) => {
                eprintln!("skipping GPU test: {err}");
                None
            }
        }
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn scene() -> DisplayList {
        let mut r = SceneRecorder::new();
        r.begin(96, 64, 1.0, Color::rgb(20, 20, 24), ColorScheme::default());
        r.fill_rect(rect(4.0, 4.0, 40.0, 24.0), Color::rgb(200, 40, 40), 6.0);
        r.stroke_rect(
            rect(50.0, 4.0, 40.0, 24.0),
            Color::rgb(40, 200, 40),
            2.0,
            4.0,
        );
        r.stroke_line(
            Point { x: 4.0, y: 40.0 },
            Point { x: 90.0, y: 40.0 },
            Color::rgb(40, 40, 200),
            3.0,
        );
        r.push_clip_rounded(rect(4.0, 44.0, 40.0, 16.0), 8.0);
        r.fill_rect(rect(0.0, 40.0, 60.0, 30.0), Color::rgb(250, 250, 250), 0.0);
        r.pop_clip();
        r.fill_text(
            rect(50.0, 44.0, 44.0, 16.0),
            "Ab",
            Color::rgb(255, 255, 0),
            13.0,
            TextAlign::Start,
        );
        let image = RgbaImage::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        r.draw_image(rect(70.0, 30.0, 20.0, 4.0), &image, None);
        r.finish()
    }

    #[test]
    fn gpu_output_matches_the_cpu_rasterizer() {
        let Some(mut gpu) = headless() else { return };
        let list = scene();
        let gpu_pixels = gpu.render_to_pixels(&list);
        let mut cpu = Rasterizer::new(list.width, list.height);
        cpu.render(&list, &Damage::Full);
        let cpu_pixels = cpu.pixmap().data();
        let mut worst = 0u8;
        let mut differing = 0usize;
        for (g, c) in gpu_pixels.chunks_exact(4).zip(cpu_pixels.chunks_exact(4)) {
            let diff = g.iter().zip(c).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            worst = worst.max(diff);
            if diff > 48 {
                differing += 1;
            }
        }
        let total = gpu_pixels.len() / 4;
        assert!(
            differing * 50 < total,
            "{differing} of {total} pixels differ noticeably (worst {worst}) on {}",
            gpu.adapter_name()
        );
        assert_eq!(&gpu_pixels[..4], &[20, 20, 24, 255]);
        let center = (16 * 96 + 24) * 4;
        assert_eq!(&gpu_pixels[center..center + 4], &[200, 40, 40, 255]);
    }

    #[test]
    fn batches_only_split_on_images() {
        let Some(mut gpu) = headless() else { return };
        gpu.render(&scene());
        assert_eq!(gpu.renderer().batch_count(), 2);
        assert!(gpu.renderer().instance_count() >= 7);
    }

    #[test]
    fn atlas_overflow_grows_the_atlas() {
        let Some(mut gpu) = headless() else { return };
        let mut r = SceneRecorder::new();
        r.begin(512, 512, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        let text: String = ('!'..='~').collect();
        for size in [40.0, 60.0, 80.0, 100.0] {
            r.fill_text(
                rect(0.0, 0.0, 512.0, 512.0),
                &text,
                Color::rgb(255, 255, 255),
                size,
                TextAlign::Start,
            );
        }
        let list = r.finish();
        gpu.render(&list);
        assert!(gpu.renderer().atlas.size > INITIAL_ATLAS_SIZE);
    }

    #[test]
    fn shelf_allocation_packs_and_rejects() {
        let Some(gpu) = headless() else { return };
        let mut atlas = GlyphAtlas::new(gpu.renderer().device(), 33);
        assert_eq!(atlas.allocate(10, 10), Some([0, 0]));
        assert_eq!(atlas.allocate(10, 10), Some([11, 0]));
        assert_eq!(atlas.allocate(10, 20), Some([0, 11]));
        assert_eq!(atlas.allocate(40, 1), None);
        assert_eq!(atlas.allocate(10, 10), Some([22, 0]));
        assert_eq!(atlas.allocate(10, 10), None);
    }
}
