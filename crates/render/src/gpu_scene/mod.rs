//! An instanced-quad GPU pipeline that rasterizes retained paint
//! primitives ([`creamui_core::runtime::PaintFragment`]) directly, instead
//! of compositing a CPU-rasterized framebuffer (see [`crate::gpu`]).
//!
//! Solid quads and borders (with rounded corners) are supported, as is
//! text through a cached-shaping, GPU-atlas glyph pipeline (REFACTOR.md
//! Phase 9). Retained per-node transform, opacity, and clip (REFACTOR.md
//! Phase 10) are applied per-instance; images are not yet consumed here.

mod quad;
mod text;

pub use quad::{quad_instances_for_fragment, GpuPrimitiveId, QuadInstance, QuadStore};
pub use text::{
    AtlasRect, GlyphAtlas, GlyphInstance, GlyphPrimitiveId, GlyphStore, ShapeCache, ShapedRun,
};

use creamui_core::runtime::{PaintFragment, RuntimeNodeId, TextPrimitive, Transform2D};
use creamui_core::Rect;
use creamui_platform::PlatformWindow;
use std::collections::HashMap;
use std::sync::Arc;

const QUAD_SHADER_SRC: &str = include_str!("quad.wgsl");
const GLYPH_SHADER_SRC: &str = include_str!("glyph.wgsl");
const MIN_INSTANCE_CAPACITY: u32 = 64;
const GLYPH_ATLAS_SIZE: u32 = 1024;

fn upload_instances<T: bytemuck::Pod>(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    offset_elems: usize,
    elems: &[T],
) {
    queue.write_buffer(
        buffer,
        (offset_elems * std::mem::size_of::<T>()) as u64,
        bytemuck::cast_slice(elems),
    );
}

fn grow_instance_buffer(
    device: &wgpu::Device,
    label: &str,
    capacity: u32,
    element_size: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (capacity as u64) * element_size as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub struct GpuSceneState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    screen_bind_group: wgpu::BindGroup,
    screen_size_buffer: wgpu::Buffer,
    decode_srgb: bool,

    quad_pipeline: wgpu::RenderPipeline,
    quad_instance_buffer: wgpu::Buffer,
    quad_instance_capacity: u32,
    quad_store: QuadStore,
    node_quads: HashMap<RuntimeNodeId, Vec<GpuPrimitiveId>>,

    glyph_pipeline: wgpu::RenderPipeline,
    glyph_instance_buffer: wgpu::Buffer,
    glyph_instance_capacity: u32,
    glyph_store: GlyphStore,
    node_glyphs: HashMap<RuntimeNodeId, Vec<GlyphPrimitiveId>>,
    glyph_atlas: GlyphAtlas,
    glyph_atlas_texture: wgpu::Texture,
    glyph_atlas_bind_group: wgpu::BindGroup,
    shape_cache: ShapeCache,

    node_transforms: HashMap<RuntimeNodeId, Transform2D>,
    node_opacities: HashMap<RuntimeNodeId, f32>,
    node_clips: HashMap<RuntimeNodeId, Option<Rect>>,
}

impl GpuSceneState {
    /// See [`crate::gpu::GpuState::create_instance`] — same rationale for
    /// probing only the primary GPU backend.
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
        let size = window.inner_size();
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create GPU surface for window");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("failed to find a compatible GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("creamui-scene-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("failed to acquire GPU device");

        let surface_caps = surface.get_capabilities(&adapter);
        let (surface_format, decode_srgb) = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .map(|f| (f, false))
            .unwrap_or_else(|| (surface_caps.formats[0], surface_caps.formats[0].is_srgb()));

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

        let screen_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("creamui-screen-bind-group-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let screen_size_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-screen-size"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &screen_size_buffer,
            0,
            bytemuck::cast_slice(&[
                size.width.max(1) as f32,
                size.height.max(1) as f32,
                0.0,
                0.0,
            ]),
        );

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-screen-bind-group"),
            layout: &screen_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_size_buffer.as_entire_binding(),
            }],
        });

        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-quad-shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER_SRC.into()),
        });

        let quad_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-quad-pipeline-layout"),
            bind_group_layouts: &[&screen_bind_group_layout],
            push_constant_ranges: &[],
        });

        let quad_attributes = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32,
            5 => Float32,
            6 => Float32x2,
            7 => Float32x2,
        ];
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creamui-quad-pipeline"),
            layout: Some(&quad_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &quad_attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let quad_instance_buffer = grow_instance_buffer(
            &device,
            "creamui-quad-instances",
            MIN_INSTANCE_CAPACITY,
            std::mem::size_of::<QuadInstance>(),
        );

        let glyph_atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("creamui-glyph-atlas-bind-group-layout"),
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

        let glyph_atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("creamui-glyph-atlas"),
            size: wgpu::Extent3d {
                width: GLYPH_ATLAS_SIZE,
                height: GLYPH_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let glyph_atlas_view =
            glyph_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let glyph_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("creamui-glyph-atlas-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let glyph_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-glyph-atlas-bind-group"),
            layout: &glyph_atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&glyph_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_atlas_sampler),
                },
            ],
        });

        let glyph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-glyph-shader"),
            source: wgpu::ShaderSource::Wgsl(GLYPH_SHADER_SRC.into()),
        });

        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("creamui-glyph-pipeline-layout"),
                bind_group_layouts: &[&screen_bind_group_layout, &glyph_atlas_bind_group_layout],
                push_constant_ranges: &[],
            });

        let glyph_attributes = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32x4,
            5 => Float32x2,
            6 => Float32x2,
        ];
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creamui-glyph-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &glyph_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &glyph_attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &glyph_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let glyph_instance_buffer = grow_instance_buffer(
            &device,
            "creamui-glyph-instances",
            MIN_INSTANCE_CAPACITY,
            std::mem::size_of::<GlyphInstance>(),
        );

        GpuSceneState {
            surface,
            device,
            queue,
            config,
            screen_bind_group,
            screen_size_buffer,
            decode_srgb,

            quad_pipeline,
            quad_instance_buffer,
            quad_instance_capacity: MIN_INSTANCE_CAPACITY,
            quad_store: QuadStore::new(),
            node_quads: HashMap::new(),

            glyph_pipeline,
            glyph_instance_buffer,
            glyph_instance_capacity: MIN_INSTANCE_CAPACITY,
            glyph_store: GlyphStore::new(),
            node_glyphs: HashMap::new(),
            glyph_atlas: GlyphAtlas::new(GLYPH_ATLAS_SIZE),
            glyph_atlas_texture,
            glyph_atlas_bind_group,
            shape_cache: ShapeCache::new(),

            node_transforms: HashMap::new(),
            node_opacities: HashMap::new(),
            node_clips: HashMap::new(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.queue.write_buffer(
            &self.screen_size_buffer,
            0,
            bytemuck::cast_slice(&[width as f32, height as f32, 0.0, 0.0]),
        );
    }

    /// Replaces `node`'s quad instances with the ones derived from its
    /// current paint fragment, `transform`, `opacity`, and `clip`, reusing
    /// existing [`GpuPrimitiveId`] slots where the instance count didn't
    /// change so most updates only dirty a handful of buffer slots instead
    /// of the whole scene. A single-property change should go through
    /// [`GpuSceneState::sync_transform`]/[`GpuSceneState::sync_opacity`]/
    /// [`GpuSceneState::sync_clip`] instead, which skip re-deriving from
    /// `fragment` entirely.
    pub fn sync_node(
        &mut self,
        node: RuntimeNodeId,
        fragment: &PaintFragment,
        transform: Transform2D,
        opacity: f32,
        clip: Option<Rect>,
    ) {
        let (clip_min, clip_max) = quad::clip_bounds(clip);
        let new_instances: Vec<QuadInstance> =
            quad::quad_instances_for_fragment(fragment, self.decode_srgb)
                .into_iter()
                .map(|instance| {
                    instance
                        .translated(transform.x, transform.y)
                        .scaled_alpha(opacity)
                        .clipped(clip_min, clip_max)
                })
                .collect();
        let old_ids = self.node_quads.remove(&node).unwrap_or_default();

        let mut ids = Vec::with_capacity(new_instances.len());
        for (i, instance) in new_instances.into_iter().enumerate() {
            match old_ids.get(i) {
                Some(&id) => {
                    self.quad_store.update(id, instance);
                    ids.push(id);
                }
                None => ids.push(self.quad_store.insert(instance)),
            }
        }
        for &stale in &old_ids[ids.len()..] {
            self.quad_store.remove(stale);
        }

        self.node_quads.insert(node, ids);
        self.node_transforms.insert(node, transform);
        self.node_opacities.insert(node, opacity);
        self.node_clips.insert(node, clip);
    }

    /// Shapes (cache-hit on unchanged text/style) `primitive`'s text, makes
    /// sure every glyph it needs is rasterized into the atlas, and replaces
    /// `node`'s glyph instances the same incremental way
    /// [`GpuSceneState::sync_node`] does for quads. Glyphs that don't fit
    /// the atlas are silently dropped rather than growing/rebaking it — see
    /// `TODO.md`.
    pub fn sync_text_node(
        &mut self,
        node: RuntimeNodeId,
        primitive: &TextPrimitive,
        transform: Transform2D,
        opacity: f32,
        clip: Option<Rect>,
    ) {
        let (clip_min, clip_max) = quad::clip_bounds(clip);
        let family = primitive.family.as_deref();
        let bold = primitive.bold;
        let shaped = self.shape_cache.shape(
            &primitive.text,
            primitive.font_size,
            primitive.rect.width.max(1.0),
            family,
            bold,
        );

        let weight = if bold {
            creamui_fonts::FontWeight::Bold
        } else {
            creamui_fonts::FontWeight::Regular
        };
        let face = creamui_fonts::resolve(family.unwrap_or(creamui_fonts::DEFAULT_FAMILY), weight);

        let align_x = match primitive.align {
            creamui_core::TextAlign::Start => 0.0,
            creamui_core::TextAlign::Center => {
                ((primitive.rect.width - shaped.width) / 2.0).max(0.0)
            }
            creamui_core::TextAlign::End => (primitive.rect.width - shaped.width).max(0.0),
        };
        let align_y = ((primitive.rect.height - shaped.height) / 2.0).max(0.0);
        let [r, g, b, a] = quad::quad_color(primitive.color, self.decode_srgb);
        let color = [r, g, b, a * opacity];

        let mut new_instances = Vec::with_capacity(shaped.glyphs.len());
        for glyph in &shaped.glyphs {
            let rect = match self.glyph_atlas.rect_for(glyph.raster_key) {
                Some(rect) => rect,
                None => {
                    let (metrics, bitmap) = face.rasterize_config(glyph.raster_key);
                    if metrics.width == 0 || metrics.height == 0 {
                        continue;
                    }
                    let Some(rect) = self.glyph_atlas.place(
                        glyph.raster_key,
                        metrics.width as u32,
                        metrics.height as u32,
                    ) else {
                        continue;
                    };
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &self.glyph_atlas_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: rect.x,
                                y: rect.y,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &bitmap,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(rect.width),
                            rows_per_image: Some(rect.height),
                        },
                        wgpu::Extent3d {
                            width: rect.width,
                            height: rect.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    #[cfg(feature = "perf-metrics")]
                    creamui_core::metrics::record(|m| m.gpu_upload_bytes += bitmap.len() as u64);
                    rect
                }
            };

            let position = [
                primitive.rect.x + align_x + glyph.x + transform.x,
                primitive.rect.y + align_y + glyph.y + transform.y,
            ];
            let size = [rect.width as f32, rect.height as f32];
            new_instances.push(
                GlyphInstance::new(position, size, rect, self.glyph_atlas.size(), color)
                    .clipped(clip_min, clip_max),
            );
        }

        let old_ids = self.node_glyphs.remove(&node).unwrap_or_default();
        let mut ids = Vec::with_capacity(new_instances.len());
        for (i, instance) in new_instances.into_iter().enumerate() {
            match old_ids.get(i) {
                Some(&id) => {
                    self.glyph_store.update(id, instance);
                    ids.push(id);
                }
                None => ids.push(self.glyph_store.insert(instance)),
            }
        }
        for &stale in &old_ids[ids.len()..] {
            self.glyph_store.remove(stale);
        }
        self.node_glyphs.insert(node, ids);
        self.node_transforms.insert(node, transform);
        self.node_opacities.insert(node, opacity);
        self.node_clips.insert(node, clip);
    }

    /// Repositions `node`'s already-retained quad and glyph instances by
    /// the delta between `transform` and whatever was last applied,
    /// without touching [`ShapeCache`], the glyph atlas, or re-deriving
    /// anything from a [`PaintFragment`]/[`TextPrimitive`] — the
    /// property-only update path (REFACTOR.md Phase 10).
    pub fn sync_transform(&mut self, node: RuntimeNodeId, transform: Transform2D) {
        let previous = self.node_transforms.get(&node).copied().unwrap_or_default();
        if previous == transform {
            return;
        }
        let (dx, dy) = (transform.x - previous.x, transform.y - previous.y);

        if let Some(ids) = self.node_quads.get(&node) {
            for &id in ids {
                let current = self.quad_store.get(id);
                self.quad_store.update(id, current.translated(dx, dy));
            }
        }
        if let Some(ids) = self.node_glyphs.get(&node) {
            for &id in ids {
                let current = self.glyph_store.get(id);
                self.glyph_store.update(id, current.translated(dx, dy));
            }
        }
        self.node_transforms.insert(node, transform);
    }

    /// Rescales `node`'s already-retained quad and glyph alpha by the ratio
    /// between `opacity` and whatever was last applied, the same
    /// property-only update path [`GpuSceneState::sync_transform`] uses for
    /// position. Since the ratio is relative to the previously applied
    /// opacity, a node last synced at `0.0` can't recover a nonzero alpha
    /// this way — that case needs a full [`GpuSceneState::sync_node`]/
    /// [`GpuSceneState::sync_text_node`] call instead (see `TODO.md`).
    pub fn sync_opacity(&mut self, node: RuntimeNodeId, opacity: f32) {
        let previous = self.node_opacities.get(&node).copied().unwrap_or(1.0);
        if previous == opacity {
            return;
        }
        let ratio = if previous != 0.0 {
            opacity / previous
        } else {
            0.0
        };

        if let Some(ids) = self.node_quads.get(&node) {
            for &id in ids {
                let current = self.quad_store.get(id);
                self.quad_store.update(id, current.scaled_alpha(ratio));
            }
        }
        if let Some(ids) = self.node_glyphs.get(&node) {
            for &id in ids {
                let current = self.glyph_store.get(id);
                self.glyph_store.update(id, current.scaled_alpha(ratio));
            }
        }
        self.node_opacities.insert(node, opacity);
    }

    /// Overwrites `node`'s already-retained quad and glyph clip bounds with
    /// `clip`, the same property-only update path [`GpuSceneState::sync_transform`]
    /// uses for position. Unlike [`GpuSceneState::sync_opacity`], a clip
    /// bound is absolute rather than cumulative, so this needs no ratio and
    /// no previously-applied state to recover.
    pub fn sync_clip(&mut self, node: RuntimeNodeId, clip: Option<Rect>) {
        if self.node_clips.get(&node).copied().flatten() == clip {
            return;
        }
        let (clip_min, clip_max) = quad::clip_bounds(clip);

        if let Some(ids) = self.node_quads.get(&node) {
            for &id in ids {
                let current = self.quad_store.get(id);
                self.quad_store
                    .update(id, current.clipped(clip_min, clip_max));
            }
        }
        if let Some(ids) = self.node_glyphs.get(&node) {
            for &id in ids {
                let current = self.glyph_store.get(id);
                self.glyph_store
                    .update(id, current.clipped(clip_min, clip_max));
            }
        }
        self.node_clips.insert(node, clip);
    }

    pub fn remove_node(&mut self, node: RuntimeNodeId) {
        if let Some(ids) = self.node_quads.remove(&node) {
            for id in ids {
                self.quad_store.remove(id);
            }
        }
        if let Some(ids) = self.node_glyphs.remove(&node) {
            for id in ids {
                self.glyph_store.remove(id);
            }
        }
        self.node_transforms.remove(&node);
        self.node_opacities.remove(&node);
        self.node_clips.remove(&node);
    }

    pub fn render(&mut self) {
        self.sync_quad_buffer();
        self.sync_glyph_buffer();

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
                label: Some("creamui-scene-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("creamui-scene-pass"),
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

            let quad_count = self.quad_store.instances().len() as u32;
            if quad_count > 0 {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &self.screen_bind_group, &[]);
                pass.set_vertex_buffer(0, self.quad_instance_buffer.slice(..));
                pass.draw(0..6, 0..quad_count);
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| m.draw_calls += 1);
            }

            let glyph_count = self.glyph_store.instances().len() as u32;
            if glyph_count > 0 {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.screen_bind_group, &[]);
                pass.set_bind_group(1, &self.glyph_atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, self.glyph_instance_buffer.slice(..));
                pass.draw(0..6, 0..glyph_count);
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| m.draw_calls += 1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    fn sync_quad_buffer(&mut self) {
        let needed = self.quad_store.instances().len() as u32;

        if needed > self.quad_instance_capacity {
            let capacity = needed.next_power_of_two().max(MIN_INSTANCE_CAPACITY);
            self.quad_instance_buffer = grow_instance_buffer(
                &self.device,
                "creamui-quad-instances",
                capacity,
                std::mem::size_of::<QuadInstance>(),
            );
            self.quad_instance_capacity = capacity;
            if needed > 0 {
                let instances = self.quad_store.instances();
                upload_instances(&self.queue, &self.quad_instance_buffer, 0, instances);
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| {
                    m.gpu_upload_bytes += std::mem::size_of_val(instances) as u64
                });
            }
            self.quad_store.take_dirty_range();
            return;
        }

        let Some((min, max)) = self.quad_store.take_dirty_range() else {
            return;
        };
        let (start, end) = (min as usize, max as usize + 1);
        let dirty_slice = &self.quad_store.instances()[start..end];
        upload_instances(&self.queue, &self.quad_instance_buffer, start, dirty_slice);
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| {
            m.gpu_upload_bytes += std::mem::size_of_val(dirty_slice) as u64
        });
    }

    fn sync_glyph_buffer(&mut self) {
        let needed = self.glyph_store.instances().len() as u32;

        if needed > self.glyph_instance_capacity {
            let capacity = needed.next_power_of_two().max(MIN_INSTANCE_CAPACITY);
            self.glyph_instance_buffer = grow_instance_buffer(
                &self.device,
                "creamui-glyph-instances",
                capacity,
                std::mem::size_of::<GlyphInstance>(),
            );
            self.glyph_instance_capacity = capacity;
            if needed > 0 {
                let instances = self.glyph_store.instances();
                upload_instances(&self.queue, &self.glyph_instance_buffer, 0, instances);
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| {
                    m.gpu_upload_bytes += std::mem::size_of_val(instances) as u64
                });
            }
            self.glyph_store.take_dirty_range();
            return;
        }

        let Some((min, max)) = self.glyph_store.take_dirty_range() else {
            return;
        };
        let (start, end) = (min as usize, max as usize + 1);
        let dirty_slice = &self.glyph_store.instances()[start..end];
        upload_instances(&self.queue, &self.glyph_instance_buffer, start, dirty_slice);
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| {
            m.gpu_upload_bytes += std::mem::size_of_val(dirty_slice) as u64
        });
    }
}
