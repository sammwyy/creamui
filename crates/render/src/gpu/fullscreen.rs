use creamui_theme::Color;
use std::rc::Rc;

pub(super) struct Pipelines {
    shader: wgpu::ShaderModule,
    clear_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, &'static str, Rc<wgpu::RenderPipeline>)>,
}

impl Pipelines {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creamui-fullscreen-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fullscreen.wgsl").into()),
        });
        let clear_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-clear-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creamui-blit-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        Self {
            shader,
            clear_layout,
            blit_layout,
            pipelines: Vec::new(),
        }
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        entry: &'static str,
        cache: Option<&wgpu::PipelineCache>,
    ) -> Rc<wgpu::RenderPipeline> {
        if let Some((_, _, pipeline)) = self
            .pipelines
            .iter()
            .find(|(f, e, _)| *f == format && *e == entry)
        {
            return pipeline.clone();
        }
        let layout = if entry == "clear" {
            &self.clear_layout
        } else {
            &self.blit_layout
        };
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creamui-fullscreen-layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });
        let pipeline = Rc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("creamui-fullscreen-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: "vs",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: entry,
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache,
            }),
        );
        log::debug!("creamui-render: created {entry} fullscreen pipeline for {format:?}");
        self.pipelines.push((format, entry, pipeline.clone()));
        pipeline
    }

    pub(super) fn clear(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        cache: Option<&wgpu::PipelineCache>,
    ) -> ClearPass {
        let pipeline = self.pipeline(device, format, "clear", cache);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creamui-clear-color"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-clear-group"),
            layout: &self.clear_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        ClearPass {
            pipeline,
            buffer,
            group,
            color: None,
        }
    }

    pub(super) fn blit(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        texture: &wgpu::TextureView,
        cache: Option<&wgpu::PipelineCache>,
    ) -> BlitPass {
        let pipeline = self.pipeline(device, format, "blit", cache);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creamui-blit-group"),
            layout: &self.blit_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture),
            }],
        });
        BlitPass { pipeline, group }
    }
}

pub(super) struct ClearPass {
    pipeline: Rc<wgpu::RenderPipeline>,
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    color: Option<Color>,
}

impl ClearPass {
    pub(super) fn set_color(&mut self, queue: &wgpu::Queue, color: Color) {
        if self.color == Some(color) {
            return;
        }
        self.color = Some(color);
        let alpha = color.a as f32 / 255.0;
        let rgba = [
            color.r as f32 / 255.0 * alpha,
            color.g as f32 / 255.0 * alpha,
            color.b as f32 / 255.0 * alpha,
            alpha,
        ];
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&rgba));
    }

    pub(super) fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub(super) struct BlitPass {
    pipeline: Rc<wgpu::RenderPipeline>,
    group: wgpu::BindGroup,
}

impl BlitPass {
    pub(super) fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}
