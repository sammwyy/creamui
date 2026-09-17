//! `wl_shm` presentation with an `Argb8888` buffer, on the window's own
//! `wl_surface` and a private event queue of the host connection.

use super::{argb, span, BufferHistory};
use crate::display_list::Bounds;
use creamui_platform::PlatformWindow;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tiny_skia::Pixmap;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};

const MAX_BUFFERS: usize = 3;

struct State;

struct ShmBuffer {
    buffer: wl_buffer::WlBuffer,
    pool: wl_shm_pool::WlShmPool,
    fd: OwnedFd,
    map: *mut u8,
    map_len: usize,
    width: u32,
    height: u32,
    released: Arc<AtomicBool>,
    age: u8,
}

impl ShmBuffer {
    fn new(
        shm: &wl_shm::WlShm,
        width: u32,
        height: u32,
        qh: &QueueHandle<State>,
    ) -> Result<Self, String> {
        let map_len = (width as usize * height as usize * 4).next_power_of_two();
        // SAFETY: plain syscall with a NUL-terminated name.
        let raw = unsafe { libc::memfd_create(c"creamui-shm".as_ptr(), libc::MFD_CLOEXEC) };
        if raw < 0 {
            return Err(format!("memfd_create: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: `raw` is a freshly created descriptor owned by nobody else.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let map = map_fd(&fd, map_len)?;
        let released = Arc::new(AtomicBool::new(true));
        let pool = shm.create_pool(fd.as_fd(), map_len as i32, qh, ());
        let buffer = create_buffer(&pool, width, height, qh, &released);
        Ok(ShmBuffer {
            buffer,
            pool,
            fd,
            map,
            map_len,
            width,
            height,
            released,
            age: 0,
        })
    }

    fn resize(&mut self, width: u32, height: u32, qh: &QueueHandle<State>) -> Result<(), String> {
        if (self.width, self.height) == (width, height) {
            return Ok(());
        }
        self.buffer.destroy();
        let needed = width as usize * height as usize * 4;
        if needed > self.map_len {
            let map_len = needed.next_power_of_two();
            let map = map_fd(&self.fd, map_len)?;
            // SAFETY: `self.map` was mapped with `self.map_len` and is no
            // longer referenced by any slice.
            unsafe { libc::munmap(self.map.cast(), self.map_len) };
            self.pool.resize(map_len as i32);
            self.map = map;
            self.map_len = map_len;
        }
        self.buffer = create_buffer(&self.pool, width, height, qh, &self.released);
        self.width = width;
        self.height = height;
        self.age = 0;
        Ok(())
    }

    fn pixels(&mut self) -> &mut [u32] {
        let len = self.width as usize * self.height as usize;
        // SAFETY: the mapping is page-aligned, at least `len * 4` bytes, and
        // exclusively borrowed through `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.map.cast::<u32>(), len) }
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        // SAFETY: the mapping is owned by this buffer.
        unsafe { libc::munmap(self.map.cast(), self.map_len) };
    }
}

fn map_fd(fd: &OwnedFd, len: usize) -> Result<*mut u8, String> {
    use std::os::fd::AsRawFd;
    // SAFETY: `fd` is a valid memfd; the mapping length matches the size set
    // just before mapping.
    unsafe {
        if libc::ftruncate(fd.as_raw_fd(), len as libc::off_t) != 0 {
            return Err(format!("ftruncate: {}", std::io::Error::last_os_error()));
        }
        let map = libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        );
        if map == libc::MAP_FAILED {
            return Err(format!("mmap: {}", std::io::Error::last_os_error()));
        }
        Ok(map.cast())
    }
}

fn create_buffer(
    pool: &wl_shm_pool::WlShmPool,
    width: u32,
    height: u32,
    qh: &QueueHandle<State>,
    released: &Arc<AtomicBool>,
) -> wl_buffer::WlBuffer {
    pool.create_buffer(
        0,
        width as i32,
        height as i32,
        width as i32 * 4,
        wl_shm::Format::Argb8888,
        qh,
        released.clone(),
    )
}

pub struct ShmSurface {
    buffers: Vec<ShmBuffer>,
    surface: wl_surface::WlSurface,
    shm: wl_shm::WlShm,
    queue: EventQueue<State>,
    qh: QueueHandle<State>,
    history: BufferHistory,
    _conn: Connection,
    _window: Arc<dyn PlatformWindow>,
}

impl ShmSurface {
    pub fn new(window: Arc<dyn PlatformWindow>) -> Result<Self, String> {
        let RawDisplayHandle::Wayland(display) =
            window.display_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("not a Wayland display".into());
        };
        let RawWindowHandle::Wayland(handle) =
            window.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("not a Wayland window".into());
        };
        // SAFETY: the display pointer stays valid while `window` is alive,
        // and `_window` is dropped after every proxy below.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).map_err(|e| e.to_string())?;
        // SAFETY: the surface pointer belongs to `window`, which outlives the
        // proxy.
        let id = unsafe {
            ObjectId::from_ptr(
                wl_surface::WlSurface::interface(),
                handle.surface.as_ptr().cast(),
            )
        }
        .map_err(|e| e.to_string())?;
        let surface = wl_surface::WlSurface::from_id(&conn, id).map_err(|e| e.to_string())?;
        log::debug!("creamui-render: presenting through wl_shm Argb8888");
        Ok(ShmSurface {
            buffers: Vec::new(),
            surface,
            shm,
            queue,
            qh,
            history: BufferHistory::new(),
            _conn: conn,
            _window: window,
        })
    }

    fn free_buffer(&mut self, width: u32, height: u32) -> Result<usize, String> {
        if let Err(err) = self.queue.dispatch_pending(&mut State) {
            log::warn!("creamui-render: wl_shm dispatch failed: {err}");
        }
        loop {
            let free = self
                .buffers
                .iter()
                .enumerate()
                .filter(|(_, b)| b.released.load(Ordering::SeqCst))
                .min_by_key(|(_, b)| if b.age == 0 { u8::MAX } else { b.age })
                .map(|(i, _)| i);
            if let Some(index) = free {
                self.buffers[index].resize(width, height, &self.qh)?;
                return Ok(index);
            }
            if self.buffers.len() < MAX_BUFFERS {
                self.buffers
                    .push(ShmBuffer::new(&self.shm, width, height, &self.qh)?);
                return Ok(self.buffers.len() - 1);
            }
            self.queue
                .blocking_dispatch(&mut State)
                .map_err(|e| e.to_string())?;
        }
    }

    pub fn present(&mut self, frame: &Pixmap, regions: &[Bounds]) {
        let (width, height) = (frame.width(), frame.height());
        let index = match self.free_buffer(width, height) {
            Ok(index) => index,
            Err(err) => {
                log::error!("creamui-render: no wl_shm buffer available: {err}");
                return;
            }
        };
        let age = self.buffers[index].age;
        let full = [Bounds::new(0.0, 0.0, width as f32, height as f32)];
        let copy = self
            .history
            .regions_for_age(age, regions)
            .unwrap_or_else(|| full.to_vec());
        let data = frame.data();
        let pixels = self.buffers[index].pixels();
        for region in &copy {
            let (x0, y0, x1, y1) = span(region, width, height);
            for y in y0..y1 {
                let row = y * width as usize;
                for (dst, src) in pixels[row + x0..row + x1]
                    .iter_mut()
                    .zip(data[(row + x0) * 4..(row + x1) * 4].chunks_exact(4))
                {
                    *dst = argb(src);
                }
            }
        }

        for (i, buffer) in self.buffers.iter_mut().enumerate() {
            if i == index {
                buffer.age = 1;
            } else if buffer.age != 0 {
                buffer.age = buffer.age.saturating_add(1);
            }
        }
        let buffer = &self.buffers[index];
        buffer.released.store(false, Ordering::SeqCst);
        self.surface.attach(Some(&buffer.buffer), 0, 0);
        if self.surface.version() >= 4 {
            for region in regions {
                let (x0, y0, x1, y1) = span(region, width, height);
                self.surface.damage_buffer(
                    x0 as i32,
                    y0 as i32,
                    (x1 - x0) as i32,
                    (y1 - y0) as i32,
                );
            }
        } else {
            self.surface.damage(0, 0, i32::MAX, i32::MAX);
        }
        self.surface.commit();
        if let Err(err) = self.queue.flush() {
            log::warn!("creamui-render: wl_shm flush failed: {err}");
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut State,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(
        _: &mut State,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(
        _: &mut State,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<AtomicBool>> for State {
    fn event(
        _: &mut State,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        released: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_buffer::Event::Release = event {
            released.store(true, Ordering::SeqCst);
        }
    }
}
