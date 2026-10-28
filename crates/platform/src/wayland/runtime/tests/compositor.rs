use super::{Event, EventLoopProxy};
use std::{
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use wayland_client::Connection;
use wayland_protocols::xdg::shell::server::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_server::{
    protocol::{wl_compositor, wl_region, wl_shm, wl_shm_pool, wl_surface},
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New,
};

pub(super) struct Compositor {
    stop: Arc<AtomicBool>,
    actions: Arc<Mutex<Vec<&'static str>>>,
    thread: Option<JoinHandle<()>>,
}

impl Compositor {
    pub(super) fn start(proxy: EventLoopProxy<Event>) -> (Self, Connection) {
        let (server, client) = UnixStream::pair().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let actions = Arc::new(Mutex::new(Vec::new()));
        let thread = thread::spawn({
            let stop = stop.clone();
            let actions = actions.clone();
            move || {
                let mut display = Display::<State>::new().unwrap();
                let mut handle = display.handle();
                handle.create_global::<State, wl_compositor::WlCompositor, _>(6, ());
                handle.create_global::<State, wl_shm::WlShm, _>(1, ());
                handle.create_global::<State, xdg_wm_base::XdgWmBase, _>(6, ());
                handle.insert_client(server, Arc::new(())).unwrap();
                let mut state = State {
                    windows: Vec::new(),
                    actions,
                    proxy,
                    serial: 0,
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
                thread: Some(thread),
            },
            Connection::from_socket(client).unwrap(),
        )
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
    configured: bool,
}

struct State {
    windows: Vec<Window>,
    actions: Arc<Mutex<Vec<&'static str>>>,
    proxy: EventLoopProxy<Event>,
    serial: u32,
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
        window
            .toplevel
            .as_ref()
            .unwrap()
            .configure(640, 400, states);
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
        if let xdg_wm_base::Request::GetXdgSurface { id, surface } = request {
            state.windows.push(Window {
                surface,
                xdg: data.init(id, ()),
                toplevel: None,
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
        if let xdg_surface::Request::GetToplevel { id } = request {
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
