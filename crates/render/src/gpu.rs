//! GPU rendering of a [`DisplayList`]: every primitive becomes one instance
//! of a single signed-distance-field pipeline, glyphs come from a coverage
//! atlas, and draw calls only split where the image texture changes.
//! Pipelines, the atlas and image textures live in [`GpuShared`], reused by
//! every renderer on the same device.

use crate::display_list::{Clip, DisplayList, ImagePrimitive, Primitive};
use crate::text::{GlyphBitmap, GlyphKey};
use bytemuck::{Pod, Zeroable};
use creamui_platform::PlatformWindow;
use creamui_theme::Color;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use wgpu::util::DeviceExt;

const KIND_QUAD: u32 = 0;
const KIND_LINE: u32 = 1;
const KIND_GLYPH: u32 = 2;
const KIND_IMAGE: u32 = 3;
const KIND_GRADIENT_QUAD: u32 = 4;
const KIND_BITS: u32 = 4;
const CLIPS_PER_ROW: u32 = 256;
const TEXELS_PER_CLIP: u32 = 3;
const INITIAL_ATLAS_SIZE: u32 = 1024;
/// A full atlas is cleared and refilled at the same size, instead of grown,
/// only once it has lived this long; otherwise a frame whose own text nearly
/// fills it would re-upload every glyph each frame.
const ATLAS_RESET_COOLDOWN_FRAMES: u64 = 60;
const ATLAS_SHRINK_EVERY_FRAMES: u64 = 600;
const IMAGE_CACHE_FRAMES: u64 = 300;
const MIN_INSTANCE_CAPACITY: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    bounds: [f32; 4],
    data: [f32; 4],
    color: [u8; 4],
    border_color: [u8; 4],
    /// Corner radius, border or line width, and glyph shear.
    shape: [f32; 3],
    /// Primitive kind in the low `KIND_BITS`, clip table index above them.
    tag: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

fn rgba(color: Color) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
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

struct AtlasSlot {
    rect: [u32; 4],
    used: u64,
}

struct GlyphAtlas {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: u32,
    shelves: Vec<Shelf>,
    entries: HashMap<GlyphKey, AtlasSlot>,
    created: u64,
}

impl GlyphAtlas {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        size: u32,
        frame: u64,
    ) -> Self {
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-atlas"),
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
        GlyphAtlas {
            texture,
            bind_group,
            size,
            shelves: Vec::new(),
            entries: HashMap::new(),
            created: frame,
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

    fn get_or_insert(
        &mut self,
        queue: &wgpu::Queue,
        glyph: &GlyphBitmap,
        frame: u64,
    ) -> Option<[u32; 4]> {
        if let Some(slot) = self.entries.get_mut(&glyph.key) {
            slot.used = frame;
            return Some(slot.rect);
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
        self.entries
            .insert(glyph.key, AtlasSlot { rect, used: frame });
        Some(rect)
    }

    fn has_stale_entries(&self, frame: u64) -> bool {
        self.entries.values().any(|slot| slot.used < frame)
    }

    /// Texels (padding included) of the glyphs used since `since`.
    fn live_area(&self, since: u64) -> u64 {
        self.entries
            .values()
            .filter(|slot| slot.used >= since)
            .map(|slot| (slot.rect[2] as u64 + 1) * (slot.rect[3] as u64 + 1))
            .sum()
    }
}

struct ImageTexture {
    bind_group: wgpu::BindGroup,
    /// How many times the source was halved before upload.
    level: u32,
    used: u64,
    /// Renderers whose latest frame draws this texture.
    holders: u32,
}

/// How many times a `width` x `height` image can be halved while staying at
/// least as large as `shown`, and at most `max` texels on a side.
fn downscale_level(width: u32, height: u32, shown: [f32; 2], max: u32) -> u32 {
    let mut level = 0;
    while level < 31 {
        let (w, h) = (width >> level, height >> level);
        let (half_w, half_h) = (w >> 1, h >> 1);
        let too_large = w > max || h > max;
        let half_fits =
            half_w > 0 && half_h > 0 && half_w as f32 >= shown[0] && half_h as f32 >= shown[1];
        if !too_large && !half_fits {
            break;
        }
        level += 1;
    }
    level
}

/// Box-filters premultiplied RGBA8 pixels down to half size.
fn halve(width: u32, height: u32, pixels: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let (last_x, last_y) = (width as usize - 1, height as usize - 1);
    let stride = width as usize * 4;
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h as usize {
        let rows = [
            (2 * y).min(last_y) * stride,
            (2 * y + 1).min(last_y) * stride,
        ];
        for x in 0..w as usize {
            let columns = [(2 * x).min(last_x) * 4, (2 * x + 1).min(last_x) * 4];
            for channel in 0..4 {
                let sum: u32 = rows
                    .iter()
                    .flat_map(|row| columns.iter().map(move |column| row + column + channel))
                    .map(|index| pixels[index] as u32)
                    .sum();
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    (w, h, out)
}

/// Device resources shared by every [`GpuRenderer`] on one device: the
/// shader and pipelines, the glyph atlas and uploaded images.
pub struct GpuShared {
    device: Rc<wgpu::Device>,
    queue: Rc<wgpu::Queue>,
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    globals_layout: wgpu::BindGroupLayout,
    atlas_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, Rc<wgpu::RenderPipeline>)>,
    sampler: wgpu::Sampler,
    empty_image: wgpu::BindGroup,
    atlas: GlyphAtlas,
    images: HashMap<u64, ImageTexture>,
    frame: u64,
}

impl GpuShared {
    pub fn new(device: Rc<wgpu::Device>, queue: Rc<wgpu::Queue>) -> Rc<RefCell<Self>> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let unfilterable = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
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
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: unfilterable,
                    count: None,
                },
            ],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: unfilterable,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-pipeline-layout"),
            bind_group_layouts: &[&globals_layout, &atlas_layout, &image_layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("creamui-image-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas = GlyphAtlas::new(&device, &atlas_layout, &sampler, INITIAL_ATLAS_SIZE, 0);
        let empty = upload_image(&device, &queue, 1, 1, &[0; 4]);
        let empty_image = image_group(&device, &image_layout, &empty);
        Rc::new(RefCell::new(GpuShared {
            device,
            queue,
            shader,
            pipeline_layout,
            globals_layout,
            atlas_layout,
            image_layout,
            pipelines: Vec::new(),
            sampler,
            empty_image,
            atlas,
            images: HashMap::new(),
            frame: 0,
        }))
    }

    fn pipeline(&mut self, format: wgpu::TextureFormat) -> Rc<wgpu::RenderPipeline> {
        if let Some((_, pipeline)) = self.pipelines.iter().find(|(f, _)| *f == format) {
            return pipeline.clone();
        }
        let pipeline = Rc::new(self.device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("creamui-pipeline"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: "vs",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x4,
                            1 => Float32x4,
                            2 => Unorm8x4,
                            3 => Unorm8x4,
                            4 => Float32x3,
                            5 => Uint32,
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
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
            },
        ));
        self.pipelines.push((format, pipeline.clone()));
        pipeline
    }

    fn begin_frame(&mut self) -> u64 {
        self.frame += 1;
        let frame = self.frame;
        if frame.is_multiple_of(ATLAS_SHRINK_EVERY_FRAMES) && self.atlas.size > INITIAL_ATLAS_SIZE {
            let half = self.atlas.size / 2;
            let live = self
                .atlas
                .live_area(frame.saturating_sub(ATLAS_SHRINK_EVERY_FRAMES));
            if live * 4 <= (half as u64).pow(2) {
                self.recreate_atlas(half);
            }
        }
        frame
    }

    fn recreate_atlas(&mut self, size: u32) {
        log::debug!(
            "creamui-render: glyph atlas {}px -> {size}px",
            self.atlas.size
        );
        self.atlas = GlyphAtlas::new(
            &self.device,
            &self.atlas_layout,
            &self.sampler,
            size,
            self.frame,
        );
    }

    /// Frees atlas space after this frame's glyphs overflowed it: clears it
    /// when older glyphs can go, grows it otherwise. Returns `false` when
    /// neither is possible.
    fn make_atlas_room(&mut self) -> bool {
        let frame = self.frame;
        if frame - self.atlas.created >= ATLAS_RESET_COOLDOWN_FRAMES
            && self.atlas.has_stale_entries(frame)
        {
            self.recreate_atlas(self.atlas.size);
            return true;
        }
        let max = self.device.limits().max_texture_dimension_2d;
        if self.atlas.size >= max {
            return false;
        }
        self.recreate_atlas((self.atlas.size * 2).min(max));
        true
    }

    /// Makes sure `prim`'s image has a texture at least as large as it is
    /// drawn, uploading a downscaled copy when the source is larger.
    fn prepare_image(&mut self, prim: &ImagePrimitive, frame: u64) {
        let image = &prim.image;
        let level = downscale_level(
            image.width(),
            image.height(),
            [prim.bounds.width(), prim.bounds.height()],
            self.device.limits().max_texture_dimension_2d,
        );
        let holders = match self.images.get_mut(&image.id()) {
            Some(texture) if texture.level <= level => {
                texture.used = frame;
                return;
            }
            Some(texture) => texture.holders,
            None => 0,
        };
        let pixels = image.pixels();
        let (mut width, mut height) = (image.width(), image.height());
        let mut scaled: Option<Vec<u8>> = None;
        for _ in 0..level {
            let (w, h, out) = halve(width, height, scaled.as_deref().unwrap_or(&pixels));
            (width, height, scaled) = (w, h, Some(out));
        }
        let texture = upload_image(
            &self.device,
            &self.queue,
            width,
            height,
            scaled.as_deref().unwrap_or(&pixels),
        );
        drop(pixels);
        let discarded = image.discard_pixels();
        log::debug!(
            "creamui-render: uploaded image {} ({}x{}) at {width}x{height}{}",
            image.id(),
            image.width(),
            image.height(),
            if discarded {
                ", CPU pixels discarded"
            } else {
                ""
            }
        );
        self.images.insert(
            image.id(),
            ImageTexture {
                bind_group: image_group(&self.device, &self.image_layout, &texture),
                level,
                used: frame,
                holders,
            },
        );
    }

    fn evict_images(&mut self) {
        let horizon = self.frame.saturating_sub(IMAGE_CACHE_FRAMES);
        self.images
            .retain(|_, texture| texture.holders > 0 || texture.used >= horizon);
    }

    fn release_images(&mut self, ids: &[u64]) {
        for id in ids {
            if let Some(texture) = self.images.get_mut(id) {
                texture.holders = texture.holders.saturating_sub(1);
            }
        }
    }
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

struct Batch {
    image: Option<u64>,
    instances: std::ops::Range<u32>,
}

/// Renders display lists into any color target of its format, holding only
/// per-target state; everything else comes from its [`GpuShared`].
pub struct GpuRenderer {
    shared: Rc<RefCell<GpuShared>>,
    device: Rc<wgpu::Device>,
    queue: Rc<wgpu::Queue>,
    format: wgpu::TextureFormat,
    pipeline: Rc<wgpu::RenderPipeline>,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    clip_texture: wgpu::Texture,
    clip_rows: u32,
    clip_texels: Vec<[f32; 4]>,
    clip_ids: HashMap<[u32; 9], u32>,
    last_clip: Option<(Clip, u32)>,
    instances: Vec<Instance>,
    batches: Vec<Batch>,
    instance_buffer: wgpu::Buffer,
    frame_images: Vec<u64>,
    held_images: Vec<u64>,
}

impl GpuRenderer {
    pub fn new(shared: Rc<RefCell<GpuShared>>, format: wgpu::TextureFormat) -> Self {
        let mut resources = shared.borrow_mut();
        let pipeline = resources.pipeline(format);
        let (device, queue) = (resources.device.clone(), resources.queue.clone());
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clip_texture = clip_texture(&device, 1);
        let globals_group =
            globals_group(&device, &resources.globals_layout, &globals, &clip_texture);
        drop(resources);
        let instance_buffer = instance_buffer(&device, MIN_INSTANCE_CAPACITY);
        GpuRenderer {
            shared,
            device,
            queue,
            format,
            pipeline,
            globals,
            globals_group,
            clip_texture,
            clip_rows: 1,
            clip_texels: Vec::new(),
            clip_ids: HashMap::new(),
            last_clip: None,
            instances: Vec::new(),
            batches: Vec::new(),
            instance_buffer,
            frame_images: Vec::new(),
            held_images: Vec::new(),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    fn clip_index(&mut self, clip: &Clip) -> u32 {
        match self.last_clip {
            Some((last, index)) if last == *clip => return index,
            _ => {}
        }
        let b = clip.bounds;
        let (rounded, radius) = match clip.rounded {
            Some(r) => (
                [r.bounds.x0, r.bounds.y0, r.bounds.x1, r.bounds.y1],
                r.radius.max(0.001),
            ),
            None => ([0.0; 4], 0.0),
        };
        let texels = [[b.x0, b.y0, b.x1, b.y1], rounded, [radius, 0.0, 0.0, 0.0]];
        let mut key = [0u32; 9];
        for (slot, value) in key.iter_mut().zip(texels.iter().flatten()) {
            *slot = value.to_bits();
        }
        let next = self.clip_ids.len() as u32;
        let index = *self.clip_ids.entry(key).or_insert_with(|| {
            self.clip_texels.extend(texels);
            next
        });
        self.last_clip = Some((*clip, index));
        index
    }

    /// Builds this frame's instances. Returns `false` if the glyph atlas
    /// overflowed and the frame must be rebuilt.
    fn build(&mut self, list: &DisplayList, shared: &mut GpuShared, frame: u64) -> bool {
        self.instances.clear();
        self.batches.clear();
        self.clip_texels.clear();
        self.clip_ids.clear();
        self.last_clip = None;
        self.frame_images.clear();
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
            let clip = self.clip_index(&item.clip) << KIND_BITS;
            match &item.primitive {
                Primitive::Quad(quad) => {
                    let (kind, color, border_color, data) = match quad.gradient {
                        Some(gradient) => (
                            KIND_GRADIENT_QUAD,
                            gradient.start_color,
                            gradient.end_color,
                            [
                                gradient.start[0],
                                gradient.start[1],
                                gradient.end[0],
                                gradient.end[1],
                            ],
                        ),
                        None => (KIND_QUAD, quad.background, quad.border_color, [0.0; 4]),
                    };
                    self.instances.push(Instance {
                        bounds: [
                            quad.bounds.x0,
                            quad.bounds.y0,
                            quad.bounds.x1,
                            quad.bounds.y1,
                        ],
                        data,
                        color: rgba(color),
                        border_color: rgba(border_color),
                        shape: [quad.radius, quad.border_width, 0.0],
                        tag: kind | clip,
                    });
                }
                Primitive::Line(line) => {
                    let b = item.primitive.bounds();
                    self.instances.push(Instance {
                        bounds: [b.x0, b.y0, b.x1, b.y1],
                        data: [line.from[0], line.from[1], line.to[0], line.to[1]],
                        color: rgba(line.color),
                        shape: [0.0, line.width, 0.0],
                        tag: KIND_LINE | clip,
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
                        let Some(rect) =
                            shared
                                .atlas
                                .get_or_insert(&shared.queue, &glyph.bitmap, frame)
                        else {
                            return false;
                        };
                        self.instances.push(Instance {
                            bounds: [x, y, x + w, y + h],
                            data: rect.map(|v| v as f32),
                            color: rgba(run.glyph_color(glyph.byte_offset)),
                            shape: [0.0, 0.0, run.shear_factor()],
                            tag: KIND_GLYPH | clip,
                            ..Zeroable::zeroed()
                        });
                    }
                }
                Primitive::Image(prim) => {
                    shared.prepare_image(prim, frame);
                    self.frame_images.push(prim.image.id());
                    let b = prim.bounds;
                    self.instances.push(Instance {
                        bounds: [b.x0, b.y0, b.x1, b.y1],
                        color: prim.tint.map_or([0; 4], rgba),
                        tag: KIND_IMAGE | clip,
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

    fn hold_frame_images(&mut self, shared: &mut GpuShared) {
        self.frame_images.sort_unstable();
        self.frame_images.dedup();
        for id in &self.frame_images {
            if let Some(texture) = shared.images.get_mut(id) {
                texture.holders += 1;
            }
        }
        shared.release_images(&self.held_images);
        std::mem::swap(&mut self.held_images, &mut self.frame_images);
    }

    fn upload_clips(&mut self, shared: &GpuShared) {
        let rows = (self.clip_ids.len() as u32).div_ceil(CLIPS_PER_ROW).max(1);
        if rows > self.clip_rows {
            self.clip_rows = rows.next_power_of_two();
            self.clip_texture = clip_texture(&self.device, self.clip_rows);
            self.globals_group = globals_group(
                &self.device,
                &shared.globals_layout,
                &self.globals,
                &self.clip_texture,
            );
        }
        let row_texels = (CLIPS_PER_ROW * TEXELS_PER_CLIP) as usize;
        self.clip_texels
            .resize(rows as usize * row_texels, [0.0; 4]);
        self.queue.write_texture(
            self.clip_texture.as_image_copy(),
            bytemuck::cast_slice(&self.clip_texels),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(row_texels as u32 * 16),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: row_texels as u32,
                height: rows,
                depth_or_array_layers: 1,
            },
        );
    }

    fn prepare(&mut self, list: &DisplayList, shared: &mut GpuShared) {
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("gpu_prepare").entered();
        let frame = shared.begin_frame();
        while !self.build(list, shared, frame) {
            if !shared.make_atlas_room() {
                log::warn!("creamui-render: glyph atlas cannot fit this frame's text");
                break;
            }
        }
        self.hold_frame_images(shared);
        shared.evict_images();
        self.upload_clips(shared);

        self.queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [list.width as f32, list.height as f32],
                _pad: [0.0; 2],
            }),
        );
        let needed = (self.instances.len() as u64)
            .next_power_of_two()
            .max(MIN_INSTANCE_CAPACITY);
        let capacity = self.instance_buffer.size() / std::mem::size_of::<Instance>() as u64;
        if needed > capacity || capacity > needed * 4 {
            self.instance_buffer = instance_buffer(&self.device, needed);
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
        let shared = self.shared.clone();
        let mut shared = shared.borrow_mut();
        self.prepare(list, &mut shared);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("creamui-frame"),
            });
        {
            let clear = list.clear;
            let a = clear.a as f64 / 255.0;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("creamui-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear.r as f64 / 255.0 * a,
                            g: clear.g as f64 / 255.0 * a,
                            b: clear.b as f64 / 255.0 * a,
                            a,
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
            pass.set_bind_group(1, &shared.atlas.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            for batch in &self.batches {
                let group = batch
                    .image
                    .and_then(|id| shared.images.get(&id))
                    .map_or(&shared.empty_image, |texture| &texture.bind_group);
                pass.set_bind_group(2, group, &[]);
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

impl Drop for GpuRenderer {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.shared.try_borrow_mut() {
            shared.release_images(&self.held_images);
        }
    }
}

fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("creamui-instances"),
        size: capacity * std::mem::size_of::<Instance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Clip rects looked up by index from the vertex shader, so each instance
/// carries an index instead of two rects and a radius.
fn clip_texture(device: &wgpu::Device, rows: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("creamui-clips"),
        size: wgpu::Extent3d {
            width: CLIPS_PER_ROW * TEXELS_PER_CLIP,
            height: rows,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn globals_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    clips: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = clips.create_view(&wgpu::TextureViewDescriptor::default());
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
        ],
    })
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

/// The adapter/device/queue negotiated for the first GPU window in a
/// process, reused by every later one so only the first window pays for
/// `request_adapter`/`request_device`.
pub struct GpuContext {
    adapter: wgpu::Adapter,
    device: Rc<wgpu::Device>,
    shared: Rc<RefCell<GpuShared>>,
    adapter_name: String,
}

impl GpuContext {
    fn request(
        instance: &wgpu::Instance,
        compatible_surface: &wgpu::Surface<'static>,
    ) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(compatible_surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no compatible GPU adapter")?;
        let info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(request_device(&adapter)).map_err(|e| format!("device: {e}"))?;
        device.on_uncaptured_error(Box::new(|err| {
            log::error!("creamui-render: wgpu error: {err}");
        }));
        let (device, queue) = (Rc::new(device), Rc::new(queue));
        Ok(GpuContext {
            adapter,
            shared: GpuShared::new(device.clone(), queue),
            device,
            adapter_name: format!("{} ({:?})", info.name, info.backend),
        })
    }
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
    /// `gpu_context` is filled in on the first call and reused on every
    /// later one, so only the first GPU window in a process requests its
    /// own adapter and device.
    pub fn new(
        window: Arc<dyn PlatformWindow>,
        instance: &wgpu::Instance,
        transparent: bool,
        gpu_context: &mut Option<GpuContext>,
    ) -> Result<Self, String> {
        let t0 = std::time::Instant::now();
        let size = window.inner_size();
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("surface: {e}"))?;
        if gpu_context.is_none() {
            *gpu_context = Some(GpuContext::request(instance, &surface)?);
        }
        let context = gpu_context.as_ref().expect("just initialized above");
        let (adapter, device) = (&context.adapter, &context.device);

        let caps = surface.get_capabilities(adapter);
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
        surface.configure(device, &config);
        let renderer = GpuRenderer::new(context.shared.clone(), view_format);
        log::debug!(
            "creamui-render: GPU surface ready on {} as {format:?}/{alpha_mode:?} in {:?}",
            context.adapter_name,
            t0.elapsed()
        );
        Ok(GpuSurface {
            renderer,
            surface,
            config,
            view_format,
            adapter_name: context.adapter_name.clone(),
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
        let shared = GpuShared::new(Rc::new(device), Rc::new(queue));
        Ok(HeadlessGpu {
            renderer: GpuRenderer::new(shared, wgpu::TextureFormat::Rgba8Unorm),
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
    fn atlas_overflow_within_one_frame_grows_the_atlas() {
        let Some(mut gpu) = headless() else { return };
        let mut r = SceneRecorder::new();
        r.begin(2048, 2048, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        let text: String = ('!'..='~').collect();
        for size in [100.0, 200.0] {
            r.fill_text(
                rect(0.0, 0.0, 2048.0, 2048.0),
                &text,
                Color::rgb(255, 255, 255),
                size,
                TextAlign::Start,
            );
        }
        let list = r.finish();
        gpu.render(&list);
        assert!(gpu.renderer().shared.borrow().atlas.size > INITIAL_ATLAS_SIZE);
    }

    fn bitmap(glyph: u16, size: u32) -> GlyphBitmap {
        GlyphBitmap {
            key: GlyphKey {
                face: 0,
                glyph,
                px: 0,
            },
            width: size,
            height: size,
            left: 0,
            top: 0,
            coverage: vec![255; (size * size) as usize].into_boxed_slice(),
        }
    }

    #[test]
    fn a_full_atlas_is_reset_when_it_holds_stale_glyphs_and_grown_otherwise() {
        let Some(gpu) = headless() else { return };
        let mut shared = gpu.renderer().shared.borrow_mut();
        let queue = shared.queue.clone();
        shared
            .atlas
            .get_or_insert(&queue, &bitmap(1, 16), 0)
            .unwrap();

        shared.frame = ATLAS_RESET_COOLDOWN_FRAMES;
        let frame = shared.frame;
        shared
            .atlas
            .get_or_insert(&queue, &bitmap(2, 16), frame)
            .unwrap();
        assert!(shared.make_atlas_room());
        assert_eq!(shared.atlas.size, INITIAL_ATLAS_SIZE);
        assert!(shared.atlas.entries.is_empty());

        shared
            .atlas
            .get_or_insert(&queue, &bitmap(3, 16), frame)
            .unwrap();
        assert!(shared.make_atlas_room());
        assert_eq!(shared.atlas.size, INITIAL_ATLAS_SIZE * 2);
    }

    #[test]
    fn a_mostly_unused_atlas_shrinks() {
        let Some(gpu) = headless() else { return };
        let mut shared = gpu.renderer().shared.borrow_mut();
        shared.recreate_atlas(INITIAL_ATLAS_SIZE * 4);
        shared.frame = ATLAS_SHRINK_EVERY_FRAMES - 1;
        shared.begin_frame();
        assert_eq!(shared.atlas.size, INITIAL_ATLAS_SIZE * 2);
    }

    #[test]
    fn shelf_allocation_packs_and_rejects() {
        let Some(gpu) = headless() else { return };
        let shared = gpu.renderer().shared.borrow();
        let mut atlas =
            GlyphAtlas::new(&shared.device, &shared.atlas_layout, &shared.sampler, 33, 0);
        assert_eq!(atlas.allocate(10, 10), Some([0, 0]));
        assert_eq!(atlas.allocate(10, 10), Some([11, 0]));
        assert_eq!(atlas.allocate(10, 20), Some([0, 11]));
        assert_eq!(atlas.allocate(40, 1), None);
        assert_eq!(atlas.allocate(10, 10), Some([22, 0]));
        assert_eq!(atlas.allocate(10, 10), None);
    }

    #[test]
    fn instances_stay_compact() {
        assert_eq!(std::mem::size_of::<Instance>(), 56);
    }

    #[test]
    fn downscale_level_keeps_at_least_the_shown_size() {
        assert_eq!(downscale_level(5120, 2880, [240.0, 135.0], 16384), 4);
        assert_eq!(downscale_level(64, 64, [64.0, 64.0], 16384), 0);
        assert_eq!(downscale_level(64, 64, [200.0, 10.0], 16384), 0);
        assert_eq!(downscale_level(8192, 16, [8192.0, 16.0], 2048), 2);
    }

    #[test]
    fn halving_averages_two_by_two_blocks() {
        let pixels = [
            [0, 0, 0, 0],
            [100, 100, 100, 100],
            [50, 50, 50, 50],
            [200, 200, 200, 200],
            [20, 20, 20, 20],
            [40, 40, 40, 40],
        ]
        .concat();
        let (w, h, out) = halve(3, 2, &pixels);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![80, 80, 80, 80]);
        let (w, h, out) = halve(1, 2, &[[10; 4], [30; 4]].concat());
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![20, 20, 20, 20]);
    }

    fn image_scene(image: &RgbaImage, width: f32) -> DisplayList {
        let mut r = SceneRecorder::new();
        r.begin(64, 64, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        r.draw_image(rect(0.0, 0.0, width, width), image, None);
        r.finish()
    }

    #[test]
    fn large_images_upload_downscaled_and_drop_reloadable_pixels() {
        let Some(mut gpu) = headless() else { return };
        let reloads = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let image = RgbaImage::reloadable(64, 64, vec![255; 64 * 64 * 4], {
            let reloads = reloads.clone();
            move || {
                reloads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![255; 64 * 64 * 4]
            }
        })
        .unwrap();

        gpu.render(&image_scene(&image, 8.0));
        assert_eq!(gpu.renderer().shared.borrow().images[&image.id()].level, 3);
        assert!(!image.discard_pixels(), "the upload already discarded them");

        gpu.render(&image_scene(&image, 32.0));
        assert_eq!(gpu.renderer().shared.borrow().images[&image.id()].level, 1);
        assert_eq!(reloads.load(std::sync::atomic::Ordering::Relaxed), 1);

        gpu.render(&image_scene(&image, 16.0));
        assert_eq!(gpu.renderer().shared.borrow().images[&image.id()].level, 1);
        assert_eq!(reloads.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn renderers_on_one_device_share_glyphs_and_images() {
        let Some(mut gpu) = headless() else { return };
        let shared = gpu.renderer().shared.clone();
        let mut second = GpuRenderer::new(shared.clone(), wgpu::TextureFormat::Rgba8Unorm);
        let list = scene();
        let target = gpu.render(&list);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let glyphs = shared.borrow().atlas.entries.len();
        second.render(&list, &view);
        assert_eq!(shared.borrow().atlas.entries.len(), glyphs);
        assert_eq!(shared.borrow().images.len(), 1);
        assert_eq!(shared.borrow().pipelines.len(), 1);
        let id = *shared.borrow().images.keys().next().unwrap();
        assert_eq!(shared.borrow().images[&id].holders, 2);
        drop(second);
        assert_eq!(shared.borrow().images[&id].holders, 1);
    }

    #[test]
    fn the_instance_buffer_shrinks_after_a_large_frame() {
        let Some(mut gpu) = headless() else { return };
        let mut r = SceneRecorder::new();
        r.begin(64, 64, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        for i in 0..5000 {
            r.fill_rect(
                rect((i % 64) as f32, 0.0, 1.0, 1.0),
                Color::rgb(1, 2, 3),
                0.0,
            );
        }
        gpu.render(&r.finish());
        let large = gpu.renderer().instance_buffer.size();
        gpu.render(&scene());
        assert!(gpu.renderer().instance_buffer.size() < large);
    }
}
