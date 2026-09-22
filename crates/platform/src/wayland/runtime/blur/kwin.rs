use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::CompositorState,
    reexports::client::{globals::GlobalList, Connection, Dispatch, Proxy, QueueHandle},
};
use wayland_protocols_plasma::blur::client::{
    org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
};

use super::super::{BlurRequest, DispatchState, NativeWindow, WindowId};

pub(in super::super) struct KwinBlur {
    manager: OrgKdeKwinBlurManager,
    objects: HashMap<WindowId, OrgKdeKwinBlur>,
}

impl KwinBlur {
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
                        .or_insert_with(|| manager.create(&surface, qh, ()));
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
            blur.release();
        }
    }
}

impl Dispatch<OrgKdeKwinBlurManager, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinBlurManager,
        _: <OrgKdeKwinBlurManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<OrgKdeKwinBlur, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinBlur,
        _: <OrgKdeKwinBlur as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
