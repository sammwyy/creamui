use super::{Event, EventLoopProxy};
use std::{
    os::{fd::AsFd, unix::net::UnixStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use wayland_client::Connection;
use wayland_protocols::xdg::shell::server::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_server::{
    protocol::{
        wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer, wl_data_source,
        wl_region, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
};

pub(super) struct Compositor {
    stop: Arc<AtomicBool>,
    actions: Arc<Mutex<Vec<&'static str>>>,
    minimums: Arc<Mutex<Vec<(i32, i32)>>>,
    app_ids: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
    popups: Arc<Mutex<Vec<PopupRecord>>>,
    popup_closes: Arc<Mutex<Vec<u32>>>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Positioner {
    pub size: (i32, i32),
    pub anchor: (i32, i32, i32, i32),
    pub adjustments: u32,
}
#[derive(Clone, Debug)]
pub(super) struct PopupRecord {
    pub surface: u32,
    pub popup: u32,
    pub parent: u32,
    pub positioner: Positioner,
    pub serial: Option<u32>,
}

impl Compositor {
    pub(super) fn start(proxy: EventLoopProxy<Event>) -> (Self, Connection) {
        let (server, client) = UnixStream::pair().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let actions = Arc::new(Mutex::new(Vec::new()));
        let minimums = Arc::new(Mutex::new(Vec::new()));
        let app_ids = Arc::new(Mutex::new(Vec::new()));
        let popups = Arc::new(Mutex::new(Vec::new()));
        let popup_closes = Arc::new(Mutex::new(Vec::new()));
        let thread = thread::spawn({
            let stop = stop.clone();
            let actions = actions.clone();
            let minimums = minimums.clone();
            let app_ids = app_ids.clone();
            let popups = popups.clone();
            let popup_closes = popup_closes.clone();
            move || {
                let mut display = Display::<State>::new().unwrap();
                let mut handle = display.handle();
                handle.create_global::<State, wl_compositor::WlCompositor, _>(6, ());
                handle.create_global::<State, wl_shm::WlShm, _>(1, ());
                handle.create_global::<State, xdg_wm_base::XdgWmBase, _>(6, ());
                handle.create_global::<State, wl_seat::WlSeat, _>(7, ());
                handle
                    .create_global::<State, wl_data_device_manager::WlDataDeviceManager, _>(3, ());
                handle.insert_client(server, Arc::new(())).unwrap();
                let mut state = State {
                    device: None,
                    source: None,
                    windows: Vec::new(),
                    minimums,
                    app_ids,
                    actions,
                    proxy,
                    serial: 0,
                    popups,
                    popup_closes,
                };
                while !stop.load(Ordering::Acquire) {
                    display.dispatch_clients(&mut state).unwrap();
                    display.flush_clients().unwrap();
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });
        (
            Self {
                stop,
                actions,
                minimums,
                app_ids,
                thread: Some(thread),
                popups,
                popup_closes,
            },
            Connection::from_socket(client).unwrap(),
        )
    }

    pub(super) fn popups(&self) -> Vec<PopupRecord> {
        self.popups.lock().unwrap().clone()
    }
    pub(super) fn popup_closes(&self) -> Vec<u32> {
        self.popup_closes.lock().unwrap().clone()
    }

    pub(super) fn app_ids(&self) -> Vec<String> {
        self.app_ids.lock().unwrap().clone()
    }

    pub(super) fn minimums(&self) -> Vec<(i32, i32)> {
        self.minimums.lock().unwrap().clone()
    }

    pub(super) fn actions(&self) -> Vec<&'static str> {
        self.actions.lock().unwrap().clone()
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
    }
}

struct Window {
    surface: wl_surface::WlSurface,
    xdg: xdg_surface::XdgSurface,
    toplevel: Option<xdg_toplevel::XdgToplevel>,
    popup: Option<xdg_popup::XdgPopup>,
    popup_size: (i32, i32),
    configured: bool,
}

struct State {
    device: Option<wl_data_device::WlDataDevice>,
    source: Option<wl_data_source::WlDataSource>,
    windows: Vec<Window>,
    actions: Arc<Mutex<Vec<&'static str>>>,
    minimums: Arc<Mutex<Vec<(i32, i32)>>>,
    app_ids: Arc<Mutex<Vec<String>>>,
    proxy: EventLoopProxy<Event>,
    serial: u32,
    popups: Arc<Mutex<Vec<PopupRecord>>>,
    popup_closes: Arc<Mutex<Vec<u32>>>,
}

impl State {
    fn configure(&mut self, index: usize, maximized: bool) {
        let window = &mut self.windows[index];
        let states = if maximized {
            (xdg_toplevel::State::Maximized as u32)
                .to_ne_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        if let Some(toplevel) = &window.toplevel {
            toplevel.configure(640, 400, states);
        }
        if let Some(popup) = &window.popup {
            popup.configure(
                10,
                20,
                window.popup_size.0.min(300),
                window.popup_size.1.min(180),
            );
        }
        self.serial += 1;
        window.xdg.configure(self.serial);
        window.configured = true;
    }
}

macro_rules! global {
    ($interface:ty) => {
        impl GlobalDispatch<$interface, ()> for State {
            fn bind(
                _: &mut Self,
                _: &DisplayHandle,
                _: &Client,
                resource: New<$interface>,
                _: &(),
                data: &mut DataInit<'_, Self>,
            ) {
                data.init(resource, ());
            }
        }
    };
}

global!(wl_data_device_manager::WlDataDeviceManager);
global!(wl_compositor::WlCompositor);
global!(wl_shm::WlShm);
global!(xdg_wm_base::XdgWmBase);

impl Dispatch<wl_compositor::WlCompositor, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                data.init(id, ());
            }
            wl_compositor::Request::CreateRegion { id } => {
                data.init(id, ());
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm::WlShm,
        request: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        data: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, .. } = request {
            data.init(id, ());
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        surface: &wl_surface::WlSurface,
        request: wl_surface::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wl_surface::Request::Commit = request {
            if let Some(index) = state
                .windows
                .iter()
                .position(|window| window.surface == *surface && !window.configured)
            {
                state.configure(index, false);
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &xdg_wm_base::XdgWmBase,
        request: xdg_wm_base::Request,
        _: &(),
        _: &DisplayHandle,
        data: &mut DataInit<'_, Self>,
    ) {
        if let xdg_wm_base::Request::CreatePositioner { id } = request {
            data.init(id, Mutex::new(Positioner::default()));
        } else if let xdg_wm_base::Request::GetXdgSurface { id, surface } = request {
            state.windows.push(Window {
                surface,
                xdg: data.init(id, ()),
                toplevel: None,
                popup: None,
                popup_size: (1, 1),
                configured: false,
            });
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        surface: &xdg_surface::XdgSurface,
        request: xdg_surface::Request,
        _: &(),
        _: &DisplayHandle,
        data: &mut DataInit<'_, Self>,
    ) {
        if let xdg_surface::Request::GetPopup {
            id,
            parent,
            positioner,
        } = request
        {
            let positioner = positioner
                .data::<Mutex<Positioner>>()
                .unwrap()
                .lock()
                .unwrap()
                .clone();
            let popup = data.init(id, ());
            state.popups.lock().unwrap().push(PopupRecord {
                surface: surface.id().protocol_id(),
                popup: popup.id().protocol_id(),
                parent: parent.unwrap().id().protocol_id(),
                positioner: positioner.clone(),
                serial: None,
            });
            let window = state
                .windows
                .iter_mut()
                .find(|window| window.xdg == *surface)
                .unwrap();
            window.popup = Some(popup);
            window.popup_size = positioner.size;
        } else if let xdg_surface::Request::GetToplevel { id } = request {
            state
                .windows
                .iter_mut()
                .find(|window| window.xdg == *surface)
                .unwrap()
                .toplevel = Some(data.init(id, ()));
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        toplevel: &xdg_toplevel::XdgToplevel,
        request: xdg_toplevel::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let maximized = match request {
            xdg_toplevel::Request::SetMinSize { width, height } => {
                state.minimums.lock().unwrap().push((width, height));
                None
            }
            xdg_toplevel::Request::SetAppId { app_id } => {
                state.app_ids.lock().unwrap().push(app_id);
                None
            }
            xdg_toplevel::Request::SetMaximized => Some(true),
            xdg_toplevel::Request::UnsetMaximized => Some(false),
            xdg_toplevel::Request::SetMinimized => {
                state.actions.lock().unwrap().push("minimize");
                state.proxy.send_event(Event::Minimized).unwrap();
                None
            }
            xdg_toplevel::Request::Destroy => {
                state.actions.lock().unwrap().push("close");
                state.proxy.send_event(Event::Closed).unwrap();
                None
            }
            _ => None,
        };
        if let Some(maximized) = maximized {
            state
                .actions
                .lock()
                .unwrap()
                .push(if maximized { "maximize" } else { "restore" });
            let index = state
                .windows
                .iter()
                .position(|window| window.toplevel.as_ref() == Some(toplevel))
                .unwrap();
            state.configure(index, maximized);
        }
    }
}

impl Dispatch<wl_region::WlRegion, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_region::WlRegion,
        _: wl_region::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm_pool::WlShmPool,
        request: wl_shm_pool::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        assert!(matches!(request, wl_shm_pool::Request::Destroy));
    }
}

impl GlobalDispatch<wl_seat::WlSeat, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        seat: New<wl_seat::WlSeat>,
        _: &(),
        data: &mut DataInit<'_, Self>,
    ) {
        let seat = data.init(seat, ());
        seat.capabilities(wl_seat::Capability::empty());
        seat.name("test-seat".into());
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_seat::WlSeat,
        request: wl_seat::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        assert!(matches!(request, wl_seat::Request::Release));
    }
}
impl Dispatch<xdg_positioner::XdgPositioner, Mutex<Positioner>> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &xdg_positioner::XdgPositioner,
        request: xdg_positioner::Request,
        positioner: &Mutex<Positioner>,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let mut positioner = positioner.lock().unwrap();
        match request {
            xdg_positioner::Request::SetSize { width, height } => positioner.size = (width, height),
            xdg_positioner::Request::SetAnchorRect {
                x,
                y,
                width,
                height,
            } => positioner.anchor = (x, y, width, height),
            xdg_positioner::Request::SetConstraintAdjustment {
                constraint_adjustment,
            } => {
                positioner.adjustments = match constraint_adjustment {
                    wayland_server::WEnum::Value(flags) => flags.bits(),
                    wayland_server::WEnum::Unknown(flags) => flags,
                };
            }
            _ => {}
        }
    }
}
impl Dispatch<xdg_popup::XdgPopup, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        popup: &xdg_popup::XdgPopup,
        request: xdg_popup::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_popup::Request::Reposition { positioner, token } => {
                let geometry = positioner
                    .data::<Mutex<Positioner>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .clone();
                let window = state
                    .windows
                    .iter_mut()
                    .find(|window| window.popup.as_ref() == Some(popup))
                    .unwrap();
                window.popup_size = geometry.size;
                popup.repositioned(token);
                popup.configure(0, 0, geometry.size.0, geometry.size.1);
                window.xdg.configure(10);
                state
                    .popups
                    .lock()
                    .unwrap()
                    .iter_mut()
                    .find(|record| record.popup == popup.id().protocol_id())
                    .unwrap()
                    .positioner = geometry;
            }
            xdg_popup::Request::Grab { serial, .. } => {
                state
                    .popups
                    .lock()
                    .unwrap()
                    .iter_mut()
                    .find(|record| record.popup == popup.id().protocol_id())
                    .unwrap()
                    .serial = Some(serial);
            }
            xdg_popup::Request::Destroy => {
                state
                    .popup_closes
                    .lock()
                    .unwrap()
                    .push(popup.id().protocol_id());
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_device_manager::WlDataDeviceManager, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wl_data_device_manager::WlDataDeviceManager,
        request: wl_data_device_manager::Request,
        _: &(),
        _: &DisplayHandle,
        data: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_device_manager::Request::CreateDataSource { id } => {
                data.init(id, ());
            }
            wl_data_device_manager::Request::GetDataDevice { id, .. } => {
                state.device = Some(data.init(id, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wl_data_source::WlDataSource,
        request: wl_data_source::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_source::Request::Offer { mime_type } => {
                assert_eq!(mime_type, "text/uri-list");
            }
            wl_data_source::Request::SetActions { dnd_actions } => {
                assert_eq!(
                    dnd_actions,
                    wayland_server::WEnum::Value(wl_data_device_manager::DndAction::Copy)
                );
                state.actions.lock().unwrap().push("drag-copy");
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for State {
    fn request(
        state: &mut Self,
        client: &Client,
        device: &wl_data_device::WlDataDevice,
        request: wl_data_device::Request,
        _: &(),
        handle: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wl_data_device::Request::StartDrag {
            source: Some(source),
            serial,
            ..
        } = request
        {
            assert_eq!(serial, 42);
            state.source = Some(source);
            let offer = client
                .create_resource::<wl_data_offer::WlDataOffer, (), Self>(handle, 3, ())
                .unwrap();
            device.data_offer(&offer);
            offer.offer("text/uri-list".into());
            offer.source_actions(wl_data_device_manager::DndAction::Copy);
            device.enter(
                43,
                &state.windows.last().unwrap().surface,
                24.0,
                36.0,
                Some(&offer),
            );
            device.motion(1, 48.0, 60.0);
            device.drop();
        }
    }
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wl_data_offer::WlDataOffer,
        request: wl_data_offer::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_offer::Request::Receive { mime_type, fd } => {
                state.source.as_ref().unwrap().send(mime_type, fd.as_fd());
            }
            wl_data_offer::Request::Finish => {
                state.actions.lock().unwrap().push("drag-finished");
                state.source.take().unwrap().dnd_finished();
            }
            _ => {}
        }
    }
}
