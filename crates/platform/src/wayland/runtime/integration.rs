//! Optional compositor integrations. Blair is one backend; the public
//! CreamUI request/event API intentionally does not expose this module.

use std::collections::HashMap;

use smithay_client_toolkit::reexports::client::{globals::GlobalList, QueueHandle};

use super::{DispatchState, IntegrationRequest, NativeWindow, WindowId};

#[cfg(feature = "window-integration-blair")]
mod blair {
    use std::sync::Mutex;

    use blair_window_integration_protocol::client::{
        blair_window_integration_manager_v1::BlairWindowIntegrationManagerV1,
        blair_window_integration_v1::{BlairWindowIntegrationV1, Event, Mode},
    };
    use smithay_client_toolkit::reexports::client::{
        globals::GlobalList, Connection, Dispatch, Proxy, QueueHandle, WEnum,
    };

    use super::super::{DispatchState, IntegrationRequest, NativeWindow, WindowId};
    use crate::{
        CompositorControls, CompositorIntegrationMode, CompositorIntegrationRequest, WindowEvent,
    };

    pub(super) struct BlairIntegration {
        manager: BlairWindowIntegrationManagerV1,
        objects: std::collections::HashMap<WindowId, BlairWindowIntegrationV1>,
    }

    #[derive(Debug)]
    pub(super) struct ObjectData {
        window_id: WindowId,
        mode: Mutex<CompositorIntegrationMode>,
    }

    impl BlairIntegration {
        pub(super) fn bind(globals: &GlobalList, qh: &QueueHandle<DispatchState>) -> Option<Self> {
            Some(Self {
                manager: globals.bind(qh, 1..=1, ()).ok()?,
                objects: Default::default(),
            })
        }

        pub(super) fn apply(
            &mut self,
            windows: &std::collections::HashMap<WindowId, NativeWindow>,
            requests: &[IntegrationRequest],
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
                let object = self.objects.entry(request.window_id).or_insert_with(|| {
                    manager.get_integration(
                        &surface,
                        qh,
                        ObjectData {
                            window_id: request.window_id,
                            mode: Mutex::new(CompositorIntegrationMode::None),
                        },
                    )
                });
                object.set_hybrid(matches!(
                    request.request,
                    Some(CompositorIntegrationRequest::Hybrid)
                ) as i32);
            }
        }

        pub(super) fn forget(&mut self, id: WindowId) {
            if let Some(object) = self.objects.remove(&id) {
                object.destroy();
            }
        }
    }

    impl Dispatch<BlairWindowIntegrationManagerV1, ()> for DispatchState {
        fn event(
            _: &mut Self,
            _: &BlairWindowIntegrationManagerV1,
            _: <BlairWindowIntegrationManagerV1 as Proxy>::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<BlairWindowIntegrationV1, ObjectData> for DispatchState {
        fn event(
            state: &mut Self,
            _: &BlairWindowIntegrationV1,
            event: Event,
            data: &ObjectData,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                Event::Mode { mode } => {
                    let mode = match mode {
                        WEnum::Value(Mode::Hybrid) => CompositorIntegrationMode::Hybrid,
                        _ => CompositorIntegrationMode::None,
                    };
                    *data.mode.lock().expect("integration mode lock poisoned") = mode;
                    publish(state, data.window_id, mode, CompositorControls::default());
                }
                Event::Controls {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let mode = *data.mode.lock().expect("integration mode lock poisoned");
                    publish(
                        state,
                        data.window_id,
                        mode,
                        CompositorControls {
                            x,
                            y,
                            width,
                            height,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    fn publish(
        state: &mut DispatchState,
        window_id: crate::WindowId,
        mode: CompositorIntegrationMode,
        controls: CompositorControls,
    ) {
        let mut runtime = state.runtime.borrow_mut();
        if let Some(native) = runtime.windows.get(&window_id) {
            let decorations = native.handle().configure_integration(mode, controls);
            runtime
                .events
                .push((window_id, WindowEvent::DecorationsChanged(decorations)));
        }
        runtime.events.push((
            window_id,
            WindowEvent::CompositorIntegration { mode, controls },
        ));
    }
}

pub(super) struct IntegrationBackend {
    #[cfg(feature = "window-integration-blair")]
    blair: blair::BlairIntegration,
}

impl IntegrationBackend {
    pub(super) fn bind(globals: &GlobalList, qh: &QueueHandle<DispatchState>) -> Option<Self> {
        #[cfg(feature = "window-integration-blair")]
        {
            return blair::BlairIntegration::bind(globals, qh).map(|blair| Self { blair });
        }
        #[cfg(not(feature = "window-integration-blair"))]
        {
            let _ = (globals, qh);
            None
        }
    }

    pub(super) fn apply(
        &mut self,
        windows: &HashMap<WindowId, NativeWindow>,
        requests: &[IntegrationRequest],
        qh: &QueueHandle<DispatchState>,
    ) {
        #[cfg(feature = "window-integration-blair")]
        self.blair.apply(windows, requests, qh);
        #[cfg(not(feature = "window-integration-blair"))]
        {
            let _ = (windows, requests, qh);
        }
    }

    pub(super) fn forget(&mut self, id: WindowId) {
        #[cfg(feature = "window-integration-blair")]
        self.blair.forget(id);
        #[cfg(not(feature = "window-integration-blair"))]
        {
            let _ = id;
        }
    }
}
