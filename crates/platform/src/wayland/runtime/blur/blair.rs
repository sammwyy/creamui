use std::collections::HashMap;

use blair_blur_protocol::client::{
    blair_blur_manager_v1::BlairBlurManagerV1, blair_blur_v1::BlairBlurV1,
};
use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList, Connection, Dispatch, Proxy, QueueHandle,
};

use super::super::{BlurRequest, DispatchState, NativeWindow, WindowId};

pub(in super::super) struct BlairBlur {
    manager: BlairBlurManagerV1,
    objects: HashMap<WindowId, BlairBlurV1>,
}

impl BlairBlur {
    pub(in super::super) fn bind(
        globals: &GlobalList,
        qh: &QueueHandle<DispatchState>,
    ) -> Option<Self> {
        let manager = globals.bind(qh, 1..=1, ()).ok()?;
        Some(Self {
            manager,
            objects: HashMap::new(),
        })
    }

    pub(in super::super) fn apply(
        &mut self,
        compositor: &CompositorState,
        windows: &HashMap<WindowId, NativeWindow>,
        requests: &[BlurRequest],
        qh: &QueueHandle<DispatchState>,
    ) {
        let manager = self.manager.clone();
        for request in requests {
            let Some(surface) = windows
                .get(&request.window_id)
                .map(|window| window.surface().clone())
            else {
                continue;
            };
            match request.region {
                None => {
                    manager.unset(&surface);
                    self.forget(request.window_id);
                }
                Some(region) => {
                    let blur = self
                        .objects
                        .entry(request.window_id)
                        .or_insert_with(|| manager.get_blur(&surface, qh, ()));
                    let wl_region = super::wl_region_for(region, compositor, qh);
                    blur.set_region(wl_region.as_ref());
                    blur.commit();
                    if let Some(wl_region) = wl_region {
                        wl_region.destroy();
                    }
                }
            }
        }
    }

    pub(in super::super) fn forget(&mut self, id: WindowId) {
        if let Some(blur) = self.objects.remove(&id) {
            blur.destroy();
        }
    }
}

impl Dispatch<BlairBlurManagerV1, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &BlairBlurManagerV1,
        _: <BlairBlurManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<BlairBlurV1, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &BlairBlurV1,
        _: <BlairBlurV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
