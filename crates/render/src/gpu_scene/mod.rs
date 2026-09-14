//! An instanced-quad GPU pipeline that rasterizes retained paint
//! primitives ([`creamui_core::runtime::PaintFragment`]) directly, instead
//! of compositing a CPU-rasterized framebuffer (see [`crate::gpu`]).
//!
//! Solid quads and borders (with rounded corners) are supported; text,
//! images, clips, and transforms are not yet consumed here — see
//! REFACTOR.md 14.8's migration order.

mod quad;

pub use quad::{quad_instances_for_fragment, GpuPrimitiveId, QuadInstance, QuadStore};

use creamui_core::runtime::{PaintFragment, RuntimeNodeId};
use creamui_platform::PlatformWindow;
use std::collections::HashMap;
use std::sync::Arc;

const SHADER_SRC: &str = include_str!("quad.wgsl");
const MIN_INSTANCE_CAPACITY: u32 = 64;

pub struct GpuSceneState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    screen_size_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    decode_srgb: bool,
    store: QuadStore,
    node_quads: HashMap<RuntimeNodeId, Vec<GpuPrimitiveId>>,
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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-quad-bind-group-layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-quad-bind-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_size_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-quad-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-quad-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_attributes = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32,
            5 => Float32,
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creamui-quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-quad-instances"),
            size: (MIN_INSTANCE_CAPACITY as u64) * std::mem::size_of::<QuadInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        GpuSceneState {
            surface,
            device,
            queue,
            config,
            pipeline,
            screen_size_buffer,
            bind_group,
            instance_buffer,
            instance_capacity: MIN_INSTANCE_CAPACITY,
            decode_srgb,
            store: QuadStore::new(),
            node_quads: HashMap::new(),
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
    /// current paint fragment, reusing existing [`GpuPrimitiveId`] slots
    /// where the instance count didn't change so most updates only dirty
    /// a handful of buffer slots instead of the whole scene.
    pub fn sync_node(&mut self, node: RuntimeNodeId, fragment: &PaintFragment) {
        let new_instances = quad::quad_instances_for_fragment(fragment, self.decode_srgb);
        let old_ids = self.node_quads.remove(&node).unwrap_or_default();

        let mut ids = Vec::with_capacity(new_instances.len());
        for (i, instance) in new_instances.into_iter().enumerate() {
            match old_ids.get(i) {
                Some(&id) => {
                    self.store.update(id, instance);
                    ids.push(id);
                }
                None => ids.push(self.store.insert(instance)),
            }
        }
        for &stale in &old_ids[ids.len()..] {
            self.store.remove(stale);
        }

        self.node_quads.insert(node, ids);
    }

    pub fn remove_node(&mut self, node: RuntimeNodeId) {
        if let Some(ids) = self.node_quads.remove(&node) {
            for id in ids {
                self.store.remove(id);
            }
        }
    }

    pub fn render(&mut self) {
        self.sync_instance_buffer();

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
                label: Some("creamui-quad-pass"),
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
            let count = self.store.instances().len() as u32;
            if count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..count);
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| m.draw_calls += 1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    fn sync_instance_buffer(&mut self) {
        let needed = self.store.instances().len() as u32;

        if needed > self.instance_capacity {
            let capacity = needed.next_power_of_two().max(MIN_INSTANCE_CAPACITY);
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("creamui-quad-instances"),
                size: (capacity as u64) * std::mem::size_of::<QuadInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = capacity;
            if needed > 0 {
                let instances = self.store.instances();
                self.queue
                    .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
                #[cfg(feature = "perf-metrics")]
                creamui_core::metrics::record(|m| {
                    m.gpu_upload_bytes += std::mem::size_of_val(instances) as u64
                });
            }
            self.store.take_dirty_range();
            return;
        }

        let Some((min, max)) = self.store.take_dirty_range() else {
            return;
        };
        let (start, end) = (min as usize, max as usize + 1);
        let instances = self.store.instances();
        let dirty_slice = &instances[start..end];
        self.queue.write_buffer(
            &self.instance_buffer,
            (start * std::mem::size_of::<QuadInstance>()) as u64,
            bytemuck::cast_slice(dirty_slice),
        );
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| {
            m.gpu_upload_bytes += std::mem::size_of_val(dirty_slice) as u64
        });
    }
}
