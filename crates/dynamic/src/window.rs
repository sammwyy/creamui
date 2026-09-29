//! Window creation — the dlopen-backed equivalent of `creamui_render::run`/
//! `creamui_render::AppBuilder`.

use crate::arena::ClosureArena;
use crate::runtime::Runtime;
use crate::value::{BlurRegion, Color, RenderBackend, Size, WindowOptions};
use crate::widget::Widget;
use creamui_abi::{
    CBlurRegion, CWindowOptions, CWindowOptionsV2, CUI_BLUR_NONE, CUI_BLUR_RECT, CUI_BLUR_WINDOW,
    CUI_RENDER_BACKEND_CPU, CUI_RENDER_BACKEND_GPU,
};
use std::cell::RefCell;
use std::ffi::{c_void, CString};
use std::os::raw::c_int;
use std::rc::Rc;

/// Per-window handle passed to a `build_ui` closure: the loaded library
/// (for widget/theme/signal construction) plus this window's closure arena,
/// which keeps callback closures built this frame alive for exactly as
/// long as the ABI might still invoke them (see [`crate::arena`]).
#[derive(Clone)]
pub struct Context {
    pub(crate) rt: Rc<Runtime>,
    pub(crate) arena: Rc<ClosureArena>,
}

impl Context {
    fn new(rt: Rc<Runtime>) -> Self {
        Context {
            rt,
            arena: Rc::new(ClosureArena::new()),
        }
    }

    /// The loaded library handle, e.g. for creating a
    /// [`crate::SignalI32`]/[`crate::SignalF32`]/[`crate::SignalString`]
    /// without needing a separate `Rc<Runtime>` in scope.
    pub fn runtime(&self) -> &Rc<Runtime> {
        &self.rt
    }
}

/// A handle to a live window, for desktop-shell operations (resize, move,
/// always-on-top) issued from outside the render loop — e.g. a click
/// handler. Frees the underlying handle on drop; that's optional (it's also
/// cleaned up when the window closes) but lets an app stop holding it
/// earlier.
pub struct WindowHandle {
    rt: Rc<Runtime>,
    ptr: *mut c_void,
}

impl WindowHandle {
    fn from_raw(rt: Rc<Runtime>, ptr: *mut c_void) -> Self {
        WindowHandle { rt, ptr }
    }

    /// Requests a new logical-pixel window size.
    pub fn resize(&self, width: u32, height: u32) {
        unsafe { (self.rt.sym.window_resize)(self.ptr, width, height) };
    }

    /// Moves the window's top-left corner to a logical-pixel screen position.
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe { (self.rt.sym.window_set_position)(self.ptr, x, y) };
    }

    /// Pins (or unpins) the window above all others.
    pub fn set_always_on_top(&self, enabled: bool) {
        unsafe { (self.rt.sym.window_set_always_on_top)(self.ptr, enabled as c_int) };
    }

    /// Requests or clears compositor-side blur. Pass one of
    /// `creamui_abi::CUI_BLUR_*`; rectangle values are logical window-local
    /// coordinates and are ignored for other kinds.
    pub fn set_blur(&self, kind: c_int, x: f64, y: f64, width: f64, height: f64) {
        unsafe { (self.rt.sym.window_set_blur)(self.ptr, kind, x, y, width, height) };
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        unsafe { (self.rt.sym.window_handle_free)(self.ptr) };
    }
}

fn c_window_options(options: &WindowOptions, title: &CString) -> CWindowOptionsV2 {
    let blur = match options.blur {
        None => CBlurRegion {
            kind: CUI_BLUR_NONE,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        Some(BlurRegion::Window) => CBlurRegion {
            kind: CUI_BLUR_WINDOW,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        Some(BlurRegion::Rect {
            x,
            y,
            width,
            height,
        }) => CBlurRegion {
            kind: CUI_BLUR_RECT,
            x,
            y,
            width,
            height,
        },
    };
    CWindowOptionsV2 {
        base: CWindowOptions {
            title: title.as_ptr(),
            width: options.width,
            height: options.height,
            resizable: options.resizable as c_int,
            decorations: options.decorations as c_int,
            transparent: options.transparent as c_int,
            backend: match options.backend {
                RenderBackend::Gpu => CUI_RENDER_BACKEND_GPU,
                RenderBackend::Cpu => CUI_RENDER_BACKEND_CPU,
            },
        },
        blur,
    }
}

/// One queued window's build closures plus its [`Context`] — boxed once and
/// kept alive in [`AppBuilder::windows`] until [`AppBuilder::run`] blocks
/// and returns, since its address is handed across the ABI as `userdata`
/// for that entire span.
struct WindowUserData {
    ctx: Context,
    build_ui: Box<dyn Fn(&Context, Size) -> Widget>,
    on_window_ready: RefCell<Option<Box<dyn FnOnce(WindowHandle)>>>,
}

extern "C" fn build_trampoline(width: f32, height: f32, userdata: *mut c_void) -> *mut c_void {
    let data = unsafe { &*(userdata as *const WindowUserData) };
    data.ctx.arena.begin_frame();
    let widget = (data.build_ui)(&data.ctx, Size { width, height });
    let ptr = widget.into_raw();
    data.ctx.arena.end_frame();
    ptr
}

extern "C" fn window_ready_trampoline(handle_ptr: *mut c_void, userdata: *mut c_void) {
    let data = unsafe { &*(userdata as *const WindowUserData) };
    let handle = WindowHandle::from_raw(data.ctx.rt.clone(), handle_ptr);
    if let Some(on_window_ready) = data.on_window_ready.borrow_mut().take() {
        on_window_ready(handle);
    }
}

/// Opens a window and runs the reactive render loop until it is closed —
/// the dlopen-backed equivalent of `creamui_render::run`.
///
/// `build_ui` is called once up front and again whenever a signal read
/// while building the tree changes; it must construct a fresh widget tree
/// covering `size` each time. `background` is the color the window is
/// wiped to before that tree is painted. `on_window_ready` is called once,
/// as soon as the window exists.
///
/// To open several windows sharing one process and event loop (e.g. a
/// desktop-shell dock), use [`AppBuilder`] instead.
pub fn run(
    rt: &Rc<Runtime>,
    options: WindowOptions,
    background: Color,
    on_window_ready: impl FnOnce(WindowHandle) + 'static,
    build_ui: impl Fn(&Context, Size) -> Widget + 'static,
) {
    AppBuilder::new(rt)
        .window(options, background, on_window_ready, build_ui)
        .run();
}

/// Builds and runs one or more CreamUI windows sharing a single process and
/// event loop — the dlopen-backed equivalent of `creamui_render::AppBuilder`.
pub struct AppBuilder {
    rt: Rc<Runtime>,
    ptr: *mut c_void,
    windows: Vec<Box<WindowUserData>>,
}

impl AppBuilder {
    pub fn new(rt: &Rc<Runtime>) -> Self {
        let ptr = unsafe { (rt.sym.app_builder_new)() };
        AppBuilder {
            rt: rt.clone(),
            ptr,
            windows: Vec::new(),
        }
    }

    /// Queues a window to be opened when [`AppBuilder::run`] starts the
    /// shared event loop. See [`run`] for what each argument does.
    pub fn window(
        mut self,
        options: WindowOptions,
        background: Color,
        on_window_ready: impl FnOnce(WindowHandle) + 'static,
        build_ui: impl Fn(&Context, Size) -> Widget + 'static,
    ) -> Self {
        let ctx = Context::new(self.rt.clone());
        let data = Box::new(WindowUserData {
            ctx,
            build_ui: Box::new(build_ui),
            on_window_ready: RefCell::new(Some(Box::new(on_window_ready))),
        });
        let userdata_ptr = data.as_ref() as *const WindowUserData as *mut c_void;

        let title = CString::new(options.title.as_str()).unwrap_or_default();
        let c_options = c_window_options(&options, &title);
        unsafe {
            (self.rt.sym.app_builder_add_window_v2)(
                self.ptr,
                c_options,
                background,
                build_trampoline,
                Some(window_ready_trampoline),
                userdata_ptr,
            );
        }
        self.windows.push(data);
        self
    }

    /// Opens every queued window and runs one shared event loop until all
    /// of them have closed.
    pub fn run(self) {
        unsafe { (self.rt.sym.app_builder_run)(self.ptr) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_options_encode_initial_blur() {
        let title = CString::new("Blurred").unwrap();
        let options = WindowOptions {
            transparent: true,
            blur: Some(BlurRegion::Rect {
                x: 3.0,
                y: 4.0,
                width: 80.0,
                height: 40.0,
            }),
            ..WindowOptions::default()
        };
        let encoded = c_window_options(&options, &title);
        assert_eq!(encoded.base.title, title.as_ptr());
        assert_eq!(encoded.base.transparent, 1);
        assert_eq!(
            encoded.blur,
            CBlurRegion {
                kind: CUI_BLUR_RECT,
                x: 3.0,
                y: 4.0,
                width: 80.0,
                height: 40.0,
            }
        );
    }
}
