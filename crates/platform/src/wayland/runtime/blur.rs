use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::CompositorState,
    reexports::client::{globals::GlobalList, protocol::wl_region::WlRegion, QueueHandle},
};

use super::{BlurRequest, DispatchState, NativeWindow, WindowId};
use crate::BlurRegion;

#[cfg(feature = "blur-blair")]
mod blair;
#[cfg(feature = "blur-kwin")]
mod kwin;

pub(super) enum BlurBackend {
    #[cfg(feature = "blur-kwin")]
    Kwin(kwin::KwinBlur),
    #[cfg(feature = "blur-blair")]
    Blair(blair::BlairBlur),
}

impl BlurBackend {
    #[allow(unused_variables)]
    pub(super) fn bind(globals: &GlobalList, qh: &QueueHandle<DispatchState>) -> Option<Self> {
        #[cfg(feature = "blur-kwin")]
        if let Some(backend) = kwin::KwinBlur::bind(globals, qh) {
            return Some(Self::Kwin(backend));
        }
        #[cfg(feature = "blur-blair")]
        if let Some(backend) = blair::BlairBlur::bind(globals, qh) {
            return Some(Self::Blair(backend));
        }
        None
    }

    #[allow(unused_variables)]
    pub(super) fn apply(
        &mut self,
        compositor: &CompositorState,
        windows: &HashMap<WindowId, NativeWindow>,
        requests: &[BlurRequest],
        qh: &QueueHandle<DispatchState>,
    ) {
        match self {
            #[cfg(feature = "blur-kwin")]
            Self::Kwin(backend) => backend.apply(compositor, windows, requests, qh),
            #[cfg(feature = "blur-blair")]
            Self::Blair(backend) => backend.apply(compositor, windows, requests, qh),
            #[cfg(not(any(feature = "blur-kwin", feature = "blur-blair")))]
            _ => {}
        }
    }

    #[allow(unused_variables)]
    pub(super) fn forget(&mut self, id: WindowId) {
        match self {
            #[cfg(feature = "blur-kwin")]
            Self::Kwin(backend) => backend.forget(id),
            #[cfg(feature = "blur-blair")]
            Self::Blair(backend) => backend.forget(id),
            #[cfg(not(any(feature = "blur-kwin", feature = "blur-blair")))]
            _ => {}
        }
    }
}

#[cfg_attr(
    not(any(feature = "blur-kwin", feature = "blur-blair")),
    allow(dead_code)
)]
fn wl_region_for(
    region: BlurRegion,
    compositor: &CompositorState,
    qh: &QueueHandle<DispatchState>,
) -> Option<WlRegion> {
    match region {
        BlurRegion::Window => None,
        BlurRegion::Rect {
            x,
            y,
            width,
            height,
        } => {
            let wl_region = compositor.wl_compositor().create_region(qh, ());
            wl_region.add(
                x as i32,
                y as i32,
                width.ceil() as i32,
                height.ceil() as i32,
            );
            Some(wl_region)
        }
    }
}
