use super::{fullscreen, GpuRenderer};
use crate::display_list::{Damage, DisplayList};

pub(super) struct RetainedFrame {
    pub(super) texture: wgpu::Texture,
    view: wgpu::TextureView,
    blit: Option<fullscreen::BlitPass>,
}

pub(super) fn update_retained<'a>(
    state: &'a mut Option<RetainedFrame>,
    renderer: &mut GpuRenderer,
    list: &DisplayList,
    damage: &Damage,
    format: wgpu::TextureFormat,
) -> &'a mut RetainedFrame {
    let resized = state.as_ref().is_none_or(|frame| {
        frame.texture.width() != list.width
            || frame.texture.height() != list.height
            || frame.texture.format() != format
    });
    if resized {
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("creamui-retained-frame"),
            size: wgpu::Extent3d {
                width: list.width,
                height: list.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: if format == renderer.format {
                &[]
            } else {
                std::slice::from_ref(&renderer.format)
            },
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(renderer.format),
            ..Default::default()
        });
        *state = Some(RetainedFrame {
            texture,
            view,
            blit: None,
        });
        log::debug!(
            "creamui-render: allocated retained GPU frame {}x{} as {format:?}",
            list.width,
            list.height
        );
    }
    let frame = state.as_mut().expect("retained frame is allocated");
    renderer.render_damage(
        list,
        &frame.view,
        if resized { &Damage::Full } else { damage },
    );
    frame
}

pub(super) fn present_retained(
    renderer: &GpuRenderer,
    frame: &mut RetainedFrame,
    target: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    copy: bool,
) {
    let mut encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("creamui-present"),
        });
    if copy {
        encoder.copy_texture_to_texture(
            frame.texture.as_image_copy(),
            target.as_image_copy(),
            wgpu::Extent3d {
                width: frame.texture.width(),
                height: frame.texture.height(),
                depth_or_array_layers: 1,
            },
        );
    } else {
        if frame.blit.is_none() {
            let mut shared = renderer.shared.borrow_mut();
            let shared = &mut *shared;
            let cache = shared.pipeline_cache.as_ref().map(|file| &file.cache);
            let pipelines = shared
                .fullscreen
                .get_or_insert_with(|| fullscreen::Pipelines::new(&renderer.device));
            frame.blit =
                Some(pipelines.blit(&renderer.device, renderer.format, &frame.view, cache));
            if let Some(file) = &mut shared.pipeline_cache {
                file.save();
            }
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("creamui-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        frame
            .blit
            .as_ref()
            .expect("retained frame has a blit pass")
            .draw(&mut pass);
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.draw_calls += 1);
    }
    renderer.queue.submit(std::iter::once(encoder.finish()));
}
