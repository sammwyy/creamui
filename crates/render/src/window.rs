//! Window creation and the reactive render loop.
//!
//! [`run`] opens one window; [`AppBuilder`] opens several, all sharing one
//! platform event loop and, for GPU windows, one `wgpu` instance. Every
//! window keeps its own reactive state, so a signal change in one never
//! touches another's frame.
//!
//! Signal writes and input only invalidate a window. On the next
//! `RedrawRequested` the window rebuilds and lays out its widgets if needed,
//! records a display list, diffs it against the frame on screen and hands
//! only the changed regions to its presenter.

use crate::backend::RenderBackend;
#[cfg(not(target_arch = "wasm32"))]
use crate::cpu::SoftwareSurface;
use crate::devtools::{devtools_for_new_window, FrameReport, WindowDevtools};
use crate::display_list::{self, DisplayList};
#[cfg(not(target_arch = "wasm32"))]
use crate::gpu::GpuSurface;
use crate::raster::Rasterizer;
use crate::recorder::SceneRecorder;
#[cfg(target_arch = "wasm32")]
use crate::web::WebState;
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Modifiers, Point, Rect, Renderer, Scene, Size,
    WindowDragHandle,
};
use creamui_platform::{
    ActiveEventLoop, ApplicationHandler, BlurRegion, ControlFlow, CursorIcon as PlatformCursorIcon,
    DragIcon, EventLoop, EventLoopProxy, InputSerial, Key as PlatformKey, LogicalPosition,
    LogicalSize, Modifiers as PlatformModifiers, MouseButton, MouseScrollDelta, PlatformWindow,
    PopupOptions as PlatformPopupOptions, PopupPlacement, ResizeDirection,
    WindowAttributes as PlatformWindowAttributes, WindowEvent, WindowId, WindowLevel, WindowRole,
};
use creamui_reactive::{create_effect, Effect, Signal};
use creamui_theme::{Color, Theme, ThemeProvider};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// How long the text-input caret stays in each visibility phase while
/// blinking (on, then off, then on again).
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const ANIMATION_FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// What happens when the user asks the window manager to close a window.
///
/// [`CloseBehavior::Close`] is the normal desktop-window behavior. Use
/// [`CloseBehavior::Hide`] for a main window controlled by a system tray:
/// its UI and [`WindowHandle`] stay alive, and [`WindowHandle::show`] can
/// restore it without rebuilding the window. Native hiding is not available
/// on Wayland; portable tray applications should close and recreate their
/// window through [`AppHandle::append_window`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloseBehavior {
    #[default]
    Close,
    Hide,
}

/// Writes `value` to `signal` only if it differs, avoiding a needless
/// re-render when a window manager fires a resize event with no real change.
fn set_if_changed<T: Clone + PartialEq + 'static>(signal: &Signal<T>, value: T) {
    if signal.peek() != value {
        signal.set(value);
    }
}

fn translate_cursor_icon(icon: CursorIcon) -> PlatformCursorIcon {
    match icon {
        CursorIcon::Default => PlatformCursorIcon::Default,
        CursorIcon::Text => PlatformCursorIcon::Text,
        CursorIcon::Pointer => PlatformCursorIcon::Pointer,
        CursorIcon::NotAllowed => PlatformCursorIcon::NotAllowed,
        CursorIcon::ResizeHorizontal => PlatformCursorIcon::ResizeHorizontal,
        CursorIcon::ResizeVertical => PlatformCursorIcon::ResizeVertical,
        CursorIcon::ResizeNwse => PlatformCursorIcon::ResizeNwse,
        CursorIcon::ResizeNesw => PlatformCursorIcon::ResizeNesw,
    }
}

fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwse,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNesw,
    }
}

/// Translates a platform logical key into CreamUI's backend-agnostic [`Key`].
/// Returns `None` for keys with no CreamUI meaning (modifiers, function
/// keys, etc.) — those are silently ignored rather than delivered.
fn translate_key(key: &PlatformKey) -> Option<Key> {
    match key {
        PlatformKey::Character(s) => s.chars().next().map(Key::Char),
        PlatformKey::Space => Some(Key::Char(' ')),
        PlatformKey::Backspace => Some(Key::Backspace),
        PlatformKey::Delete => Some(Key::Delete),
        PlatformKey::Enter => Some(Key::Enter),
        PlatformKey::Tab => Some(Key::Tab),
        PlatformKey::Escape => Some(Key::Escape),
        PlatformKey::Left => Some(Key::Left),
        PlatformKey::Right => Some(Key::Right),
        PlatformKey::Up => Some(Key::Up),
        PlatformKey::Down => Some(Key::Down),
        PlatformKey::Home => Some(Key::Home),
        PlatformKey::End => Some(Key::End),
        _ => None,
    }
}

/// Options for a window CreamUI opens, set once at startup. Transparent
/// windows need a clear color with alpha.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Optional logical screen position used when creating the window.
    pub position: Option<(i32, i32)>,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    /// Compositor-side background blur, applied once at creation. Requires
    /// `transparent` and the `blur-kwin`/`blur-blair` platform feature
    /// matching the running compositor; `None` otherwise.
    pub blur: Option<BlurRegion>,
    /// Gives keyboard focus to the first focusable widget on the initial frame.
    pub focus_first: bool,
    pub role: WindowRole,
    /// How a close request from the window manager is handled.
    pub close_behavior: CloseBehavior,
    /// Which backend renders the window: GPU (`wgpu`, the default, falling
    /// back to CPU when no adapter is usable) or CPU-only. Can be
    /// force-overridden at launch with `CUI_OVERRIDE_RENDER_BACKEND=gpu|cpu`
    /// regardless of what's set here — see [`RenderBackend::resolve`].
    pub backend: RenderBackend,
    /// Made available to `use_theme()` while this window's `build_ui` runs.
    pub theme: Theme,
}

impl Default for WindowOptions {
    fn default() -> Self {
        WindowOptions {
            title: "CreamUI".to_string(),
            width: 800,
            height: 600,
            position: None,
            resizable: true,
            decorations: true,
            transparent: false,
            blur: None,
            focus_first: false,
            role: WindowRole::Normal,
            close_behavior: CloseBehavior::Close,
            backend: RenderBackend::default(),
            theme: Theme::default(),
        }
    }
}

impl WindowOptions {
    #[cfg(feature = "system-theme")]
    pub fn system_theme(mut self) -> Result<Self, creamui_theme_loader::ThemeLoadError> {
        let resolved = creamui_theme_loader::SystemThemeLoader::new().load()?;
        if let Some(font_family) = &resolved.font_family {
            creamui_fonts::use_system_font(font_family);
        }
        self.theme = resolved.theme;
        Ok(self)
    }

    /// Sets the initial logical screen position before the native window is
    /// created. This is preferable to moving the window after creation,
    /// especially on Wayland where compositors may ignore late moves.
    pub fn at_position(mut self, x: i32, y: i32) -> Self {
        self.position = Some((x, y));
        self
    }

    /// Requests compositor-side background blur behind `region`.
    pub fn blurred(mut self, region: BlurRegion) -> Self {
        self.blur = Some(region);
        self
    }

    pub fn top_panel(mut self) -> Self {
        self.role = WindowRole::TopPanel;
        self
    }

    pub fn bottom_panel(mut self) -> Self {
        self.role = WindowRole::BottomPanel;
        self
    }
}

/// Configuration for a popup anchored to a rectangle in its parent window.
/// The Wayland backend uses the input serial to create an `xdg_popup`.
#[derive(Clone)]
pub struct PopupOptions {
    parent: WindowHandle,
    anchor: Rect,
    input_serial: Option<InputSerial>,
    placement: PopupPlacement,
}

impl PopupOptions {
    pub fn new(parent: WindowHandle, anchor: Rect) -> Self {
        let input_serial = parent.last_input_serial.get();
        Self {
            parent,
            anchor,
            input_serial,
            placement: PopupPlacement::Below,
        }
    }

    pub fn with_input_serial(mut self, input_serial: InputSerial) -> Self {
        self.input_serial = Some(input_serial);
        self
    }

    pub fn above(mut self) -> Self {
        self.placement = PopupPlacement::Above;
        self
    }

    pub fn below(mut self) -> Self {
        self.placement = PopupPlacement::Below;
        self
    }

    pub fn placed(mut self, placement: PopupPlacement) -> Self {
        self.placement = placement;
        self
    }
}

impl std::fmt::Debug for PopupOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PopupOptions")
            .field("anchor", &self.anchor)
            .field("input_serial", &self.input_serial)
            .field("placement", &self.placement)
            .finish()
    }
}

/// Panic payload and source location captured by the panic hook installed
/// in [`install_panic_dispatch`], passed to an [`AppBuilder::on_panic`]
/// handler.
pub struct PanicDetails {
    pub message: String,
    pub location: Option<String>,
}

type PanicHandler = Rc<dyn Fn(&PanicDetails)>;

thread_local! {
    static PANIC_HANDLER: RefCell<Option<PanicHandler>> = const { RefCell::new(None) };
}

/// Wraps the process's current panic hook with a dispatcher that checks
/// `PANIC_HANDLER` first: if set, calls it with the panic's details instead
/// of running the previous hook; otherwise runs the previous hook
/// unchanged. Installed at most once per process via `Once`.
fn install_panic_dispatch() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let handled = PANIC_HANDLER.with(|cell| {
                let handler = cell.borrow().clone();
                match handler {
                    Some(handler) => {
                        handler(&PanicDetails {
                            message: panic_payload_message(info.payload()),
                            location: info.location().map(|l| l.to_string()),
                        });
                        true
                    }
                    None => false,
                }
            });
            if !handled {
                previous_hook(info);
            }
        }));
    });
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Runs `build(size)`, catching a panic when a handler is registered via
/// [`AppBuilder::on_panic`] and returning a blank widget in that case
/// instead of unwinding past the caller. Runs `build` directly (no
/// `catch_unwind` overhead) when no handler is registered.
fn build_ui_with_recovery(build: &Rc<dyn Fn(Size) -> BoxedWidget>, size: Size) -> BoxedWidget {
    let has_handler = PANIC_HANDLER.with(|cell| cell.borrow().is_some());
    if !has_handler {
        return build(size);
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(size))) {
        Ok(widget) => widget,
        Err(_) => Box::new(BlankWidget),
    }
}

fn with_theme_scope<R>(
    theme: &ThemeProvider,
    window_drag: &WindowDragHandle,
    f: impl FnOnce() -> R,
) -> R {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(theme.clone());
        creamui_reactive::provide_context(window_drag.clone());
        f()
    })
}

struct BlankWidget;
impl creamui_core::Widget for BlankWidget {
    fn style(&self) -> creamui_core::Style {
        creamui_core::layout::Style::default().into()
    }
    fn paint(&self, _painter: &mut dyn creamui_core::Painter, _rect: creamui_core::Rect) {}
}

/// Enables verbose logging when `CUI_DEBUG=1` is set in the environment,
/// without overriding an explicit `RUST_LOG`.
fn init_logging() {
    if std::env::var("CUI_DEBUG").as_deref() == Ok("1") && std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "creamui_render=debug,creamui_core=debug");
    }
    let _ = env_logger::try_init();
}

/// Which backend turns a window's display lists into pixels, picked once
/// per window per [`WindowOptions::backend`].
enum Presenter {
    #[cfg(not(target_arch = "wasm32"))]
    Gpu(GpuSurface),
    #[cfg(not(target_arch = "wasm32"))]
    Software(SoftwareSurface),
    #[cfg(target_arch = "wasm32")]
    Web(WebState),
}

impl Presenter {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(
        window: &Arc<dyn PlatformWindow>,
        backend: RenderBackend,
        transparent: bool,
        gpu_instance: Option<&wgpu::Instance>,
        gpu_context: &mut Option<crate::gpu::GpuContext>,
    ) -> Result<Self, String> {
        if let (RenderBackend::Gpu, Some(instance)) = (backend, gpu_instance) {
            match GpuSurface::new(window.clone(), instance, transparent, gpu_context) {
                Ok(surface) => return Ok(Presenter::Gpu(surface)),
                Err(err) => log::warn!(
                    "creamui-render: GPU backend unavailable ({err}), falling back to CPU"
                ),
            }
        }
        SoftwareSurface::new(window.clone()).map(Presenter::Software)
    }

    #[cfg(target_arch = "wasm32")]
    fn new(window: &Arc<dyn PlatformWindow>) -> Self {
        Presenter::Web(WebState::new(window.clone()))
    }

    fn name(&self) -> &'static str {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Gpu(_) => "gpu",
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Software(_) => "cpu",
            #[cfg(target_arch = "wasm32")]
            Presenter::Web(_) => "web",
        }
    }

    fn adapter(&self) -> Option<Rc<str>> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Gpu(surface) => Some(surface.adapter_name().into()),
            _ => None,
        }
    }

    fn needs_raster(&self) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Gpu(_) => false,
            _ => true,
        }
    }
}

struct FrameState {
    recorder: SceneRecorder,
    renderer: Renderer,
    scene: Option<Scene>,
    /// The newest recorded frame, not yet presented.
    pending: Option<DisplayList>,
    /// The frame currently on screen.
    presented: Option<DisplayList>,
    raster: Option<Rasterizer>,
    devtools: Option<Box<dyn WindowDevtools>>,
    report: FrameReport,
    last_report: FrameReport,
    adapter: Option<Rc<str>>,
}

/// Coalesces invalidations into at most one build, layout, record and
/// present per compositor frame.
struct Pipeline {
    frame: RefCell<FrameState>,
    viewport: Signal<Size>,
    scale_factor: Signal<f64>,
    window: SharedWindow,
    build_ui: Rc<dyn Fn(Size) -> BoxedWidget>,
    theme: ThemeProvider,
    window_drag: WindowDragHandle,
    clear_color: Color,
    focused: Cell<Option<usize>>,
    caret_visible: Cell<bool>,
    pending_root: RefCell<Option<BoxedWidget>>,
    needs_layout: Cell<bool>,
    needs_paint: Cell<bool>,
    dump_frame_path: Option<String>,
}

impl Pipeline {
    fn with_scope<R>(&self, f: impl FnOnce() -> R) -> R {
        with_theme_scope(&self.theme, &self.window_drag, f)
    }

    fn request_redraw(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.request_redraw();
        }
    }

    /// Rebuilds the widget tree from `build_ui`. Runs inside the window's
    /// reactive effect so it stays subscribed to whatever it reads; layout
    /// and paint wait for the next frame.
    fn build(&self) {
        let started = Instant::now();
        let root = self.with_scope(|| {
            #[cfg(feature = "perf-metrics")]
            let _span = tracing::info_span!("ui_build").entered();
            build_ui_with_recovery(&self.build_ui, self.viewport.peek())
        });
        *self.pending_root.borrow_mut() = Some(root);
        if let Ok(mut frame) = self.frame.try_borrow_mut() {
            frame.report.build += started.elapsed();
        }
        self.invalidate_layout();
    }

    fn invalidate_layout(&self) {
        self.needs_layout.set(true);
        self.invalidate_paint();
    }

    fn invalidate_paint(&self) {
        if !self.needs_paint.replace(true) {
            self.request_redraw();
        }
    }

    fn physical_size(&self) -> (Size, u32, u32, f64) {
        let logical = self.viewport.peek();
        let scale = self.scale_factor.peek();
        let width = (logical.width as f64 * scale).round().max(1.0) as u32;
        let height = (logical.height as f64 * scale).round().max(1.0) as u32;
        (logical, width, height, scale)
    }

    /// Performs whatever layout and recording is outstanding. Returns whether
    /// a new display list was recorded.
    fn update(&self) -> bool {
        if !self.needs_paint.get() {
            return false;
        }
        self.with_scope(|| {
            if self.needs_layout.replace(false) {
                let root = self.pending_root.borrow_mut().take();
                let root = root.unwrap_or_else(|| {
                    let started = Instant::now();
                    let root = build_ui_with_recovery(&self.build_ui, self.viewport.peek());
                    self.frame.borrow_mut().report.build += started.elapsed();
                    root
                });
                let mut frame = self.frame.borrow_mut();
                let started = Instant::now();
                frame.renderer.update(root, self.viewport.peek());
                frame.report.layout += started.elapsed();
                frame.report.rebuilt = true;
            }
            self.needs_paint.set(false);
            self.record();
        });
        true
    }

    fn record(&self) {
        let (logical, width, height, scale) = self.physical_size();
        let colors = self.theme.get().colors;
        let started = Instant::now();
        let mut frame = self.frame.borrow_mut();
        let FrameState {
            recorder,
            renderer,
            scene,
            pending,
            devtools,
            report,
            ..
        } = &mut *frame;
        recorder.begin(width, height, scale as f32, self.clear_color, colors);
        if let Some(next) = renderer.paint(recorder, self.focused.get(), self.caret_visible.get()) {
            *scene = Some(next);
        }
        if let Some(devtools) = devtools.as_ref() {
            devtools.paint_overlay(recorder, logical);
        }
        let list = recorder.finish();
        report.display_items = list.items.len();
        if let Some(stale) = pending.replace(list) {
            recorder.recycle(stale);
        }
        report.record += started.elapsed();
    }

    /// Presents the pending display list, touching only what changed since
    /// the last presented one.
    fn present(&self, presenter: Option<&mut Presenter>) {
        let mut frame = self.frame.borrow_mut();
        let Some(list) = frame.pending.take() else {
            return;
        };
        let damage = display_list::damage(frame.presented.as_ref(), &list);
        if damage.is_none() {
            frame.report = FrameReport::default();
            frame.recorder.recycle(list);
            return;
        }
        let viewport = list.viewport();
        let needs_raster =
            presenter.as_ref().is_none_or(|p| p.needs_raster()) || self.dump_frame_path.is_some();
        let started = Instant::now();
        let damage = if needs_raster {
            frame
                .raster
                .get_or_insert_with(|| Rasterizer::new(list.width, list.height))
                .render(&list, &damage)
        } else {
            damage
        };
        let regions = damage.regions(viewport);
        frame.report.raster += started.elapsed();

        let started = Instant::now();
        if let Some(presenter) = presenter {
            if let Some(window) = self.window.borrow().as_ref() {
                window.pre_present_notify();
            }
            frame.report.backend = presenter.name();
            let presented = match presenter {
                #[cfg(not(target_arch = "wasm32"))]
                Presenter::Gpu(surface) => surface.present(&list),
                #[cfg(not(target_arch = "wasm32"))]
                Presenter::Software(surface) => {
                    let raster = frame
                        .raster
                        .as_ref()
                        .expect("software frames are rasterized");
                    surface.present(raster.pixmap(), &regions);
                    true
                }
                #[cfg(target_arch = "wasm32")]
                Presenter::Web(web) => {
                    let raster = frame.raster.as_ref().expect("web frames are rasterized");
                    web.present(raster.pixmap(), &regions);
                    true
                }
            };
            if !presented {
                frame.pending = Some(list);
                drop(frame);
                self.request_redraw();
                return;
            }
        }
        frame.report.present += started.elapsed();

        if let Some(path) = &self.dump_frame_path {
            if let Some(raster) = frame.raster.as_ref() {
                if let Err(err) = raster.pixmap().save_png(path) {
                    log::warn!("creamui-render: failed to write CUI_DUMP_FRAME to {path}: {err}");
                }
            }
        }

        let FrameState {
            recorder,
            presented,
            devtools,
            report,
            last_report,
            adapter,
            ..
        } = &mut *frame;
        report.adapter = adapter.clone();
        report.damaged_regions = regions.len();
        report.damaged_pixels = damage.area(viewport) as u64;
        report.frame_pixels = (viewport.width() * viewport.height()) as u64;
        report.cached_text_layouts = recorder.text().cached_layouts();
        report.cached_glyphs = recorder.text().cached_glyphs();
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| {
            m.damaged_rect_count += regions.len() as u64;
            m.damaged_pixel_area += report.damaged_pixels;
        });
        report.metrics = creamui_core::metrics::frame_metrics();
        creamui_core::metrics::reset_frame_metrics();
        log::trace!(
            "creamui-render: frame {} in {:?} ({} items, {} px damaged)",
            report.backend,
            report.total(),
            report.display_items,
            report.damaged_pixels
        );
        if let Some(devtools) = devtools.as_mut() {
            devtools.frame_presented(report);
        }
        *last_report = std::mem::take(report);
        if let Some(old) = presented.replace(list) {
            recorder.recycle(old);
        }
    }

    fn animated(&self) -> bool {
        self.frame.borrow().recorder.animated()
    }

    fn devtools_refresh(&self) -> Option<Duration> {
        self.frame
            .borrow()
            .devtools
            .as_ref()
            .and_then(|devtools| devtools.refresh_interval())
    }
}

type SharedWindow = Rc<RefCell<Option<Arc<dyn PlatformWindow>>>>;

/// Handle for the application event loop.
///
/// It is deliberately cheap to clone and is passed to tray actions and can
/// also be retrieved from [`WindowHandle::app`]. It is tied to CreamUI's UI
/// thread (like signals and widgets), so use it from window callbacks, tray
/// callbacks, or [`AppBuilder::on_started`] rather than sending it to a
/// worker thread.
#[derive(Clone)]
pub struct AppHandle {
    commands: Rc<RefCell<AppCommands>>,
}

struct AppCommands {
    windows: Vec<PendingWindow>,
    exit_requested: bool,
    proxy: EventLoopProxy<AppEvent>,
    next_job_id: u64,
    background_jobs: HashMap<u64, Box<dyn FnOnce(Box<dyn Any + Send>)>>,
}

impl AppHandle {
    /// Queues a new top-level window. If the event loop is already running,
    /// it is created at the end of the current event turn; otherwise it is
    /// created as soon as [`AppBuilder::run`] starts.
    pub fn append_window(
        &self,
        options: WindowOptions,
        clear_color: Color,
        on_window_ready: impl FnOnce(WindowHandle) + 'static,
        build_ui: impl Fn(Size) -> BoxedWidget + 'static,
    ) {
        self.commands.borrow_mut().windows.push(PendingWindow {
            options,
            popup: None,
            clear_color,
            on_window_ready: Box::new(on_window_ready),
            build_ui: Box::new(build_ui),
        });
    }

    /// Queues a popup relative to a rectangle in its parent window. Its
    /// requested size remains subject to platform negotiation.
    pub fn append_popup(
        &self,
        options: WindowOptions,
        popup: PopupOptions,
        clear_color: Color,
        on_window_ready: impl FnOnce(WindowHandle) + 'static,
        build_ui: impl Fn(Size) -> BoxedWidget + 'static,
    ) {
        self.commands.borrow_mut().windows.push(PendingWindow {
            options,
            popup: Some(popup),
            clear_color,
            on_window_ready: Box::new(on_window_ready),
            build_ui: Box::new(build_ui),
        });
    }

    /// Ends the application event loop and drops every remaining window and
    /// system tray icon.
    pub fn exit(&self) {
        self.commands.borrow_mut().exit_requested = true;
    }

    /// Runs `work` on a new background thread; once it finishes, `on_done`
    /// runs on the UI thread with the result, safe to touch `Signal`s from.
    /// `work` must be `Send` (it crosses the thread boundary); `on_done`
    /// does not, since it only ever runs here.
    pub fn spawn_background<T, F, D>(&self, work: F, on_done: D)
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        D: FnOnce(T) + 'static,
    {
        let (job_id, proxy) = {
            let mut commands = self.commands.borrow_mut();
            let job_id = commands.next_job_id;
            commands.next_job_id += 1;
            commands.background_jobs.insert(
                job_id,
                Box::new(move |value: Box<dyn Any + Send>| {
                    let value = *value
                        .downcast::<T>()
                        .expect("creamui-render: background job result type mismatch");
                    on_done(value);
                }),
            );
            (job_id, commands.proxy.clone())
        };
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result: Box<dyn Any + Send> = Box::new(work());
            let _ = proxy.send_event(AppEvent::BackgroundJob(job_id, result));
        });
        // No threads on wasm32: run inline and resolve immediately so
        // `spawn_background` behaves like a (blocking) no-op there rather
        // than silently dropping the job.
        #[cfg(target_arch = "wasm32")]
        {
            let result: Box<dyn Any + Send> = Box::new(work());
            let _ = proxy.send_event(AppEvent::BackgroundJob(job_id, result));
        }
    }
}

/// Looks up and runs the `on_done` continuation registered by
/// [`AppHandle::spawn_background`] for `job_id`, if it hasn't already been
/// resolved (e.g. by a duplicate/stale event). Factored out of
/// [`AppHandler::user_event`] so it's callable without a real
/// `ActiveEventLoop`.
fn resolve_background_job(
    commands: &Rc<RefCell<AppCommands>>,
    job_id: u64,
    value: Box<dyn Any + Send>,
) {
    let callback = commands.borrow_mut().background_jobs.remove(&job_id);
    if let Some(callback) = callback {
        callback(value);
    }
}

#[cfg(all(feature = "tray", target_os = "linux"))]
pub use creamui_tray::TrayIcon;

/// Builder for a native Linux system-tray icon and its context-menu actions.
///
/// Available with the `tray` feature. It uses freedesktop's
/// StatusNotifierItem protocol over D-Bus — it does not depend on GTK. Each
/// menu action receives an [`AppHandle`], so it can update a [`Signal`] and/or
/// append windows without plumbing a separate channel.
#[cfg(all(feature = "tray", target_os = "linux"))]
pub struct TrayBuilder {
    icon: TrayIcon,
    tooltip: Option<String>,
    items: Vec<TrayMenuItem>,
}

#[cfg(all(feature = "tray", target_os = "linux"))]
struct TrayMenuItem {
    id: String,
    label: String,
    enabled: bool,
    action: Rc<dyn Fn(&AppHandle)>,
}

#[cfg(all(feature = "tray", target_os = "linux"))]
impl TrayBuilder {
    /// Starts a system tray definition with a raw RGBA [`TrayIcon`].
    pub fn new(icon: TrayIcon) -> Self {
        Self {
            icon,
            tooltip: None,
            items: Vec::new(),
        }
    }

    /// Adds an accessible title for tray implementations that display one.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Adds an enabled context-menu item. `id` only needs to be unique
    /// within this tray.
    pub fn item(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        action: impl Fn(&AppHandle) + 'static,
    ) -> Self {
        self.items.push(TrayMenuItem {
            id: id.into(),
            label: label.into(),
            enabled: true,
            action: Rc::new(action),
        });
        self
    }

    /// Adds a disabled, non-interactive context-menu item.
    pub fn disabled_item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(TrayMenuItem {
            id: id.into(),
            label: label.into(),
            enabled: false,
            action: Rc::new(|_| {}),
        });
        self
    }

    /// Adds the conventional action that quits the entire application.
    pub fn quit_item(self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.item(id, label, |app| app.exit())
    }

    fn install(
        self,
        tray_index: usize,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Result<InstalledTray, String> {
        let mut actions = HashMap::new();
        let mut native =
            creamui_tray::TrayBuilder::new(self.icon).with_id(format!("creamui-tray-{tray_index}"));
        if let Some(tooltip) = self.tooltip {
            native = native.title(tooltip);
        }
        for item in self.items {
            // Prefix the public id so several CreamUI trays may safely use
            // natural ids such as "show" and "quit".
            let id = format!("creamui-tray-{tray_index}-{}", item.id);
            native = if item.enabled {
                native.item(id.clone(), item.label)
            } else {
                native.disabled_item(id.clone(), item.label)
            };
            actions.insert(id, item.action);
        }
        let native = native
            .build(move |event| {
                let _ = proxy.send_event(AppEvent::TrayMenu(event.id));
            })
            .map_err(|error| error.to_string())?;
        Ok(InstalledTray {
            _native: native,
            actions,
        })
    }
}

#[cfg(all(feature = "tray", target_os = "linux"))]
struct InstalledTray {
    _native: creamui_tray::Tray,
    actions: HashMap<String, Rc<dyn Fn(&AppHandle)>>,
}

/// A handle to a live window, for desktop-shell operations (resize, move,
/// always-on-top) issued from outside the render loop — e.g. a click
/// handler. Cheap to clone; every clone shares the same underlying window.
///
/// Handed to a window's `on_window_ready` callback once that window has
/// actually been created (platform windows don't exist until the event loop
/// resumes, so this can't be available any earlier). All methods are no-ops
/// if called after the window has closed.
#[derive(Clone)]
pub struct WindowHandle {
    window: SharedWindow,
    theme: ThemeProvider,
    close_requested: Rc<Cell<bool>>,
    focus_lost_handler: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    last_input_serial: Rc<Cell<Option<InputSerial>>>,
    app: AppHandle,
}

impl WindowHandle {
    /// Requests a new logical-pixel window size. The actual resize (and any
    /// resulting `Resized` event) happens asynchronously, same as a user
    /// dragging the window border.
    pub fn resize(&self, width: u32, height: u32) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.request_inner_size(LogicalSize::new(width as f64, height as f64));
        }
    }

    /// Moves the window's top-left corner to a logical-pixel screen position.
    pub fn set_position(&self, x: i32, y: i32) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_outer_position(LogicalPosition::new(x as f64, y as f64));
        }
    }

    /// Returns the window's top-left position in logical screen pixels.
    pub fn position(&self) -> Option<(i32, i32)> {
        let window = self.window.borrow();
        let window = window.as_ref()?;
        let position = window.outer_position()?;
        let scale = window.scale_factor();
        Some((
            (position.x as f64 / scale) as i32,
            (position.y as f64 / scale) as i32,
        ))
    }

    /// Returns the current monitor's logical size in pixels.
    pub fn monitor_size(&self) -> Option<(i32, i32)> {
        let window = self.window.borrow();
        let window = window.as_ref()?;
        let size = window.monitor_size()?;
        let scale = window.scale_factor();
        Some((
            (size.width as f64 / scale) as i32,
            (size.height as f64 / scale) as i32,
        ))
    }

    /// Pins (or unpins) the window above all others — the standard
    /// desktop-shell/widget-overlay behavior.
    pub fn set_always_on_top(&self, enabled: bool) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_window_level(if enabled {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
        }
    }

    /// Requests (or clears, with `None`) compositor-side background blur
    /// behind the window. A no-op without a matching `blur-*` platform
    /// feature or compositor support.
    pub fn set_blur_region(&self, region: Option<BlurRegion>) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_blur_region(region);
        }
    }

    /// Requests that this window close.
    pub fn close(&self) {
        self.close_requested.set(true);
        if let Some(window) = self.window.borrow().as_ref() {
            window.request_redraw();
        }
    }

    /// Starts a real drag-and-drop grab, tracked/rendered by the
    /// compositor across every surface on the output. Uses the input
    /// serial of this window's most recent pointer-button-press, per the
    /// platform's requirement that a drag must originate from one.
    pub fn start_drag(&self, mime_types: &[String], icon: Option<DragIcon>) -> Result<(), String> {
        let serial = self
            .last_input_serial
            .get()
            .ok_or_else(|| "no pointer press to start a drag from".to_owned())?;
        let window = self.window.borrow();
        let window = window
            .as_ref()
            .ok_or_else(|| "window not yet created".to_owned())?;
        window.start_drag(serial, mime_types, icon)
    }

    /// Registers a callback invoked when the native window loses focus.
    /// Registering a new callback replaces the previous one.
    pub fn on_focus_lost(&self, handler: impl Fn() + 'static) {
        *self.focus_lost_handler.borrow_mut() = Some(Rc::new(handler));
    }

    /// Hides the window while retaining its UI state and native resources.
    /// Use [`show`](Self::show) to make it visible again. Wayland does not
    /// support changing a window's visibility; use `close` plus
    /// [`AppHandle::append_window`] for portable background apps.
    pub fn hide(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_visible(false);
        }
    }

    /// Shows a previously hidden window and requests keyboard focus where
    /// the platform allows it.
    pub fn show(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_visible(true);
            window.set_minimized(false);
            window.focus();
            window.request_redraw();
        }
    }

    /// Returns whether the native window still exists. A handle remains safe
    /// to retain after `close()`, but it no longer refers to an open window.
    pub fn is_open(&self) -> bool {
        self.window.borrow().is_some()
    }

    /// Returns the owning application, for app-level work such as opening
    /// another window or exiting from a window callback.
    pub fn app(&self) -> AppHandle {
        self.app.clone()
    }

    /// Minimizes or restores the window.
    pub fn set_minimized(&self, minimized: bool) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_minimized(minimized);
        }
    }

    /// Minimizes the window.
    pub fn minimize(&self) {
        self.set_minimized(true);
    }

    /// Maximizes or restores the window.
    pub fn set_maximized(&self, maximized: bool) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_maximized(maximized);
        }
    }

    /// Maximizes the window.
    pub fn maximize(&self) {
        self.set_maximized(true);
    }

    /// Starts the platform's native window drag gesture.
    pub fn drag_window(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            let _ = window.drag_window();
        }
    }

    /// Reads the window's current theme.
    pub fn theme(&self) -> Theme {
        self.theme.get()
    }

    /// Replaces the window's theme. `use_theme()` reflects it on the next
    /// rebuild, which this schedules immediately.
    pub fn set_theme(&self, theme: Theme) {
        self.theme.set(theme);
    }
}

/// One window's worth of setup, queued via [`AppBuilder::window`] and opened
/// once [`AppBuilder::run`] starts the shared event loop.
struct WindowSpec {
    options: WindowOptions,
    popup: Option<PopupOptions>,
    on_window_ready: Box<dyn FnOnce(WindowHandle)>,
    pipeline: Rc<Pipeline>,
    _effect: Effect,
    close_requested: Rc<Cell<bool>>,
    focus_lost_handler: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    last_input_serial: Rc<Cell<Option<InputSerial>>>,
}

/// Builds and runs one or more CreamUI windows sharing a single process and
/// event loop.
///
/// Each window keeps entirely separate reactive/paint state — a signal
/// change in one window's UI only ever rebuilds and repaints that window.
/// The main cost this amortizes across windows is the *fixed* per-process
/// overhead a naive one-process-per-window design would otherwise multiply:
/// the Rust runtime, the embedded font and its glyph atlas, and — for any
/// window using [`RenderBackend::Gpu`] — a single shared `wgpu::Instance`
/// (GPU driver init is normally the single biggest contributor to a
/// CreamUI process's memory footprint; see [`RenderBackend::Cpu`] to avoid
/// it altogether).
///
/// ```no_run
/// # use creamui_render::{AppBuilder, WindowOptions};
/// # use creamui_theme::Color;
/// # use creamui_core::{BoxedWidget, Size};
/// # fn build(_: Size) -> BoxedWidget { unimplemented!() }
/// AppBuilder::new()
///     .window(WindowOptions::default(), Color::rgb(0, 0, 0), |_handle| {}, build)
///     .window(WindowOptions::default(), Color::rgb(0, 0, 0), |_handle| {}, build)
///     .run();
/// ```
pub struct AppBuilder {
    specs: Vec<PendingWindow>,
    system_theme: Option<Theme>,
    on_panic: Option<PanicHandler>,
    on_started: Option<Box<dyn FnOnce(AppHandle)>>,
    exit_when_last_window_closes: bool,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    trays: Vec<TrayBuilder>,
}

/// Everything [`AppBuilder::window`] needs to defer construction to
/// [`AppBuilder::run`], where all windows' backends are resolved together
/// (so a single shared `wgpu::Instance` can be started once, up front, if
/// any of them need it).
struct PendingWindow {
    options: WindowOptions,
    popup: Option<PopupOptions>,
    clear_color: Color,
    on_window_ready: Box<dyn FnOnce(WindowHandle)>,
    build_ui: Box<dyn Fn(Size) -> BoxedWidget>,
}

impl AppBuilder {
    pub fn new() -> Self {
        AppBuilder {
            specs: Vec::new(),
            system_theme: None,
            on_panic: None,
            on_started: None,
            // Preserve the original AppBuilder/run behavior for regular
            // applications. Background applications opt into persistence.
            exit_when_last_window_closes: true,
            #[cfg(all(feature = "tray", target_os = "linux"))]
            trays: Vec::new(),
        }
    }

    /// Keeps the event loop alive after every window has closed. Pair this
    /// with [`AppHandle::exit`] (usually from a tray's Quit item) for a
    /// background application.
    pub fn keep_running(mut self) -> Self {
        self.exit_when_last_window_closes = false;
        self
    }

    #[cfg(feature = "system-theme")]
    pub fn system_theme(mut self) -> Result<Self, creamui_theme_loader::ThemeLoadError> {
        let resolved = creamui_theme_loader::SystemThemeLoader::new().load()?;
        if let Some(font_family) = &resolved.font_family {
            creamui_fonts::use_system_font(font_family);
        }
        let theme = resolved.theme;
        for spec in &mut self.specs {
            spec.options.theme = theme;
        }
        self.system_theme = Some(theme);
        Ok(self)
    }

    /// Runs `handler` once the native event loop is ready. It receives an
    /// [`AppHandle`] and may append the first window, which makes a truly
    /// windowless startup possible.
    pub fn on_started(mut self, handler: impl FnOnce(AppHandle) + 'static) -> Self {
        self.on_started = Some(Box::new(handler));
        self
    }

    /// Registers a native system tray. This also makes sense with no queued
    /// windows when combined with [`keep_running`](Self::keep_running).
    #[cfg(all(feature = "tray", target_os = "linux"))]
    pub fn tray(mut self, tray: TrayBuilder) -> Self {
        self.trays.push(tray);
        self
    }

    /// Registers a handler for panics raised inside `build_ui`. Without one,
    /// a panic crashes the app as usual. With one, the panic is caught, the
    /// handler runs instead of the default output, the frame is replaced
    /// with a blank one, and the app keeps running.
    pub fn on_panic(mut self, handler: impl Fn(&PanicDetails) + 'static) -> Self {
        self.on_panic = Some(Rc::new(handler));
        self
    }

    /// Queues a window to be opened when [`run`](AppBuilder::run) starts the
    /// shared event loop. See [`crate::run`] for what each argument does.
    pub fn window(
        mut self,
        mut options: WindowOptions,
        clear_color: Color,
        on_window_ready: impl FnOnce(WindowHandle) + 'static,
        build_ui: impl Fn(Size) -> BoxedWidget + 'static,
    ) -> Self {
        if let Some(theme) = self.system_theme {
            options.theme = theme;
        }
        self.specs.push(PendingWindow {
            options,
            popup: None,
            clear_color,
            on_window_ready: Box::new(on_window_ready),
            build_ui: Box::new(build_ui),
        });
        self
    }

    /// Opens every queued window and runs one shared event loop. By default
    /// it exits once the last window closes; [`keep_running`](Self::keep_running)
    /// makes the application's lifetime explicit instead.
    pub fn run(self) {
        run_windows(
            self.specs,
            self.on_panic,
            self.on_started,
            self.exit_when_last_window_closes,
            #[cfg(all(feature = "tray", target_os = "linux"))]
            self.trays,
        );
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-window state for a window whose platform window and presenter exist.
/// Lives in [`AppHandler`], keyed by [`WindowId`], until the window closes.
struct WindowState {
    pipeline: Rc<Pipeline>,
    close_requested: Rc<Cell<bool>>,
    focus_lost_handler: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    last_input_serial: Rc<Cell<Option<InputSerial>>>,
    close_behavior: CloseBehavior,
    frameless_resizable: bool,
    presenter: Option<Presenter>,
    pointer_pos: Point,
    modifiers: PlatformModifiers,
    next_blink: Instant,
    next_animation: Instant,
    next_devtools_refresh: Instant,
    pending_viewport: Option<Size>,
    /// The system cursor icon last set on the window, so `CursorMoved`
    /// only calls into the backend when it actually changes.
    current_cursor: CursorIcon,
    /// Callback for the widget currently under the pointer. Keeping the
    /// callback rather than a scene index makes it safe across re-renders.
    hovered: Option<(Rect, Rc<dyn Fn(bool)>)>,
    /// Index into the current `Scene`'s draggables while the left mouse
    /// button is held down over one.
    dragging: Option<usize>,
    drag_end: Option<Rc<dyn Fn()>>,
    drag_click: Option<DragClick>,
    /// Latest drag-handler call since the last frame; only the position
    /// current at the next `RedrawRequested` is dispatched.
    pending_drag: Option<(Point, Rect, Rc<dyn Fn(Point, Rect)>)>,
    _effect: Effect,
    t_run: Instant,
    first_present_logged: bool,
}

struct DragClick {
    origin: Point,
    on_click: Option<Rc<dyn Fn()>>,
    on_click_at: Option<Rc<dyn Fn(Point)>>,
    moved: bool,
}

impl WindowState {
    fn scene<R>(&self, f: impl FnOnce(&Scene) -> Option<R>) -> Option<R> {
        self.pipeline.frame.borrow().scene.as_ref().and_then(f)
    }

    fn resize_direction(&self) -> Option<ResizeDirection> {
        if !self.frameless_resizable {
            return None;
        }
        let size = self.viewport_from_window();
        let edge = 8.0;
        let left = self.pointer_pos.x <= edge;
        let right = self.pointer_pos.x >= size.width - edge;
        let top = self.pointer_pos.y <= edge;
        let bottom = self.pointer_pos.y >= size.height - edge;
        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    /// Recomputes `pending_drag` for the widget at `self.dragging` from the
    /// current `pointer_pos`, requesting a redraw if it's still draggable.
    fn refresh_pending_drag(&mut self) {
        let Some(index) = self.dragging else { return };
        let pointer = self.pointer_pos;
        let pending = self.scene(|scene| {
            scene.draggable_at(index).map(|(rect, handler)| {
                let local = Point {
                    x: pointer.x - rect.x,
                    y: pointer.y - rect.y,
                };
                (local, rect, handler.clone())
            })
        });
        if pending.is_some() {
            self.pending_drag = pending;
            self.pipeline.request_redraw();
        }
    }

    fn viewport_from_window(&self) -> Size {
        let Some(window) = self.pipeline.window.borrow().as_ref().cloned() else {
            return self.pipeline.viewport.peek();
        };
        let physical = window.inner_size();
        let scale = self.pipeline.scale_factor.peek();
        Size {
            width: (physical.width as f64 / scale) as f32,
            height: (physical.height as f64 / scale) as f32,
        }
    }

    fn queue_viewport(&mut self, viewport: Size) {
        if self
            .pending_viewport
            .unwrap_or_else(|| self.pipeline.viewport.peek())
            != viewport
        {
            self.pending_viewport = Some(viewport);
            self.pipeline.invalidate_layout();
        }
    }

    fn flush_pending_viewport(&mut self) {
        if let Some(viewport) = self.pending_viewport.take() {
            if self.pipeline.viewport.peek() != viewport {
                self.pipeline.viewport.set(viewport);
                self.pipeline.build();
            }
        }
    }

    fn restart_caret(&mut self) {
        self.pipeline.caret_visible.set(true);
        self.next_blink = Instant::now() + CARET_BLINK_INTERVAL;
    }

    fn set_pointer(&mut self, pointer: Option<Point>) {
        self.pipeline.frame.borrow_mut().recorder.pointer = pointer;
    }

    fn set_press_origin(&mut self, origin: Option<Point>) {
        self.pipeline.frame.borrow_mut().recorder.press_origin = origin;
    }

    fn handle_key_input(&mut self, key: KeyInput) {
        if key.key == Key::Tab {
            let current = self.pipeline.focused.get();
            let next = self.scene(|scene| scene.next_focus(current, key.modifiers.shift));
            self.pipeline.focused.set(next);
            self.restart_caret();
            self.pipeline.invalidate_paint();
            return;
        }
        let Some(index) = self.pipeline.focused.get() else {
            return;
        };
        let handler = self.scene(|scene| scene.on_key_at(index).cloned());
        if let Some(handler) = handler {
            self.restart_caret();
            handler(key);
            self.pipeline.invalidate_layout();
        }
    }

    fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(new_size) => {
                if new_size.width == 0 || new_size.height == 0 {
                    return;
                }
                let scale = self.pipeline.scale_factor.peek();
                self.queue_viewport(Size {
                    width: (new_size.width as f64 / scale) as f32,
                    height: (new_size.height as f64 / scale) as f32,
                });
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log::debug!("creamui-render: scale factor changed to {scale_factor}");
                set_if_changed(&self.pipeline.scale_factor, scale_factor);
                let viewport = self.viewport_from_window();
                self.queue_viewport(viewport);
                self.pipeline.invalidate_layout();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.pipeline.scale_factor.peek();
                self.pointer_pos = Point {
                    x: (position.x / scale) as f32,
                    y: (position.y / scale) as f32,
                };
                self.set_pointer(Some(self.pointer_pos));
                if let Some(drag_click) = self.drag_click.as_mut() {
                    let dx = self.pointer_pos.x - drag_click.origin.x;
                    let dy = self.pointer_pos.y - drag_click.origin.y;
                    drag_click.moved |= dx.hypot(dy) >= 4.0;
                }
                let pointer = self.pointer_pos;
                let hovered_cursor = self
                    .resize_direction()
                    .map(resize_cursor)
                    .or_else(|| self.scene(|scene| scene.cursor_hit_test(pointer)))
                    .unwrap_or(CursorIcon::Default);
                if hovered_cursor != self.current_cursor {
                    self.current_cursor = hovered_cursor;
                    if let Some(window) = self.pipeline.window.borrow().as_ref() {
                        window.set_cursor(translate_cursor_icon(hovered_cursor));
                    }
                }

                // Widgets are rebuilt on every reactive frame, so callback
                // `Rc`s are not stable identities; the hovered rect is.
                let next_hover = self.scene(|scene| scene.hover_hit_test(pointer));
                let unchanged = matches!(
                    (&self.hovered, &next_hover),
                    (Some((current, _)), Some((next, _))) if current == next
                );
                if !unchanged {
                    if let Some((_, previous)) = self.hovered.take() {
                        previous(false);
                    }
                    if let Some((rect, next)) = next_hover {
                        next(true);
                        self.hovered = Some((rect, next));
                    }
                }

                if self.dragging.is_some() {
                    self.refresh_pending_drag();
                } else if !unchanged {
                    self.pipeline.invalidate_paint();
                }
            }
            // A real Wayland drag-and-drop suppresses normal pointer motion
            // for the duration of the grab; these drive the same callbacks.
            WindowEvent::DragEntered { position } | WindowEvent::DragMoved { position } => {
                let scale = self.pipeline.scale_factor.peek();
                self.pointer_pos = Point {
                    x: (position.x / scale) as f32,
                    y: (position.y / scale) as f32,
                };
                if self.dragging.is_some() {
                    self.refresh_pending_drag();
                }
            }
            WindowEvent::DragLeft | WindowEvent::DragDropped => {
                self.dragging = None;
                self.pending_drag = None;
                if let Some(handler) = self.drag_end.take() {
                    handler();
                }
                self.pipeline.invalidate_paint();
            }
            WindowEvent::MouseInput {
                pressed: true,
                button: MouseButton::Left,
                serial,
            } => {
                self.last_input_serial.set(serial);
                self.set_press_origin(Some(self.pointer_pos));
                self.pipeline.invalidate_paint();
                if let Some(direction) = self.resize_direction() {
                    if let Some(window) = self.pipeline.window.borrow().as_ref() {
                        let _ = window.drag_resize_window(direction);
                    }
                    return;
                }
                let pointer = self.pointer_pos;
                let Some((click, click_at, new_focus, drag_start, drag_anchor)) =
                    self.scene(|scene| {
                        let drag_start = scene.drag_hit_test(pointer).and_then(|index| {
                            scene.draggable_at(index).map(|(rect, handler)| {
                                (index, rect, handler.clone(), scene.drag_end_at(index))
                            })
                        });
                        Some((
                            scene.hit_test(pointer).cloned(),
                            scene.hit_test_at(pointer).cloned(),
                            scene.focus_hit_test(pointer),
                            drag_start,
                            scene.drag_start_at(pointer),
                        ))
                    })
                else {
                    return;
                };

                let has_drag = drag_start.is_some();
                if has_drag && (click.is_some() || click_at.is_some()) {
                    self.drag_click = Some(DragClick {
                        origin: pointer,
                        on_click: click.clone(),
                        on_click_at: click_at.clone(),
                        moved: false,
                    });
                }
                if new_focus != self.pipeline.focused.replace(new_focus) {
                    self.restart_caret();
                }
                if let Some((rect, handler)) = drag_anchor {
                    handler(
                        Point {
                            x: pointer.x - rect.x,
                            y: pointer.y - rect.y,
                        },
                        rect,
                    );
                }
                if let Some((index, rect, handler, drag_end)) = drag_start {
                    self.dragging = Some(index);
                    self.drag_end = drag_end;
                    handler(
                        Point {
                            x: pointer.x - rect.x,
                            y: pointer.y - rect.y,
                        },
                        rect,
                    );
                    self.pipeline.invalidate_layout();
                    return;
                }
                if let Some(handler) = click_at {
                    log::debug!("creamui-render: click at {pointer:?}");
                    handler(pointer);
                    self.pipeline.invalidate_layout();
                } else if let Some(handler) = click {
                    log::debug!("creamui-render: click at {pointer:?}");
                    handler();
                    self.pipeline.invalidate_layout();
                }
            }
            WindowEvent::MouseInput {
                pressed: false,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = None;
                self.pending_drag = None;
                self.set_press_origin(None);
                if let Some(handler) = self.drag_end.take() {
                    handler();
                }
                if let Some(drag_click) = self.drag_click.take() {
                    if !drag_click.moved {
                        if let Some(handler) = drag_click.on_click_at {
                            handler(self.pointer_pos);
                        } else if let Some(handler) = drag_click.on_click {
                            handler();
                        }
                        self.pipeline.invalidate_layout();
                    }
                }
                self.pipeline.invalidate_paint();
            }
            WindowEvent::CursorLeft => {
                self.set_pointer(None);
                if let Some((_, callback)) = self.hovered.take() {
                    callback(false);
                }
                self.pipeline.invalidate_paint();
            }
            WindowEvent::Focused(false) => {
                self.set_press_origin(None);
                self.dragging = None;
                self.pending_drag = None;
                self.pipeline.invalidate_paint();
                if let Some(handler) = self.focus_lost_handler.borrow().as_ref().cloned() {
                    handler();
                }
            }
            WindowEvent::PopupDone => {
                self.close_requested.set(true);
                if let Some(handler) = self.focus_lost_handler.borrow().as_ref().cloned() {
                    handler();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.pipeline.scale_factor.peek();
                // Positive `delta_y` reveals content further down.
                let delta_y: f32 = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                    MouseScrollDelta::PixelDelta(pos) => -(pos.y / scale) as f32,
                };
                let pointer = self.pointer_pos;
                let handler = self.scene(|scene| {
                    let index = scene.scroll_hit_test(pointer)?;
                    Some((
                        scene.on_scroll_at(index)?.clone(),
                        scene.scroll_is_local_at(index),
                    ))
                });
                if let Some((handler, local)) = handler {
                    handler(delta_y);
                    if local {
                        self.pipeline.invalidate_paint();
                    }
                }
            }
            WindowEvent::KeyboardInput(event) if !event.synthetic => {
                if !event.pressed {
                    return;
                }
                if event.key == PlatformKey::F3 {
                    let toggled = self
                        .pipeline
                        .frame
                        .borrow_mut()
                        .devtools
                        .as_mut()
                        .is_some_and(|devtools| devtools.toggle());
                    if toggled {
                        self.next_devtools_refresh = Instant::now();
                        self.pipeline.invalidate_paint();
                    }
                    return;
                }
                let Some(key) = translate_key(&event.key) else {
                    return;
                };
                self.handle_key_input(KeyInput {
                    key,
                    modifiers: Modifiers {
                        ctrl: self.modifiers.ctrl,
                        shift: self.modifiers.shift,
                        alt: self.modifiers.alt,
                        logo: self.modifiers.logo,
                    },
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn redraw(&mut self) {
        if let Some((local, rect, handler)) = self.pending_drag.take() {
            handler(local, rect);
            // Drag handlers may write plain `Cell`s rather than signals.
            self.pipeline.invalidate_layout();
        }
        self.flush_pending_viewport();
        self.pipeline.update();
        let ready = self
            .pipeline
            .window
            .borrow()
            .as_ref()
            .is_none_or(|window| window.is_ready());
        if !ready {
            return;
        }
        self.pipeline.present(self.presenter.as_mut());
        if !self.first_present_logged {
            self.first_present_logged = true;
            log::debug!(
                "creamui-render: first present done: {:?}",
                self.t_run.elapsed()
            );
        }
    }

    /// Fires time-driven repaints that are due and returns when the next one
    /// is.
    fn tick(&mut self, now: Instant) -> Option<Instant> {
        let mut next_wake: Option<Instant> = None;
        let mut wake_at = |at: Instant| {
            next_wake = Some(next_wake.map_or(at, |t: Instant| t.min(at)));
        };
        if self.pipeline.animated() {
            if now >= self.next_animation {
                self.next_animation = now + ANIMATION_FRAME_INTERVAL;
                self.pipeline.invalidate_paint();
            }
            wake_at(self.next_animation);
        }
        if let Some(interval) = self.pipeline.devtools_refresh() {
            if now >= self.next_devtools_refresh {
                self.next_devtools_refresh = now + interval;
                self.pipeline.invalidate_paint();
            }
            wake_at(self.next_devtools_refresh);
        }
        if self.pipeline.focused.get().is_some() {
            if now >= self.next_blink {
                let caret = &self.pipeline.caret_visible;
                caret.set(!caret.get());
                self.next_blink = now + CARET_BLINK_INTERVAL;
                self.pipeline.invalidate_paint();
            }
            wake_at(self.next_blink);
        }
        next_wake
    }
}

/// The shared [`ApplicationHandler`] driving every window opened by
/// [`AppBuilder`] (and, for a single window, [`run`]) from one event loop.
struct AppHandler {
    /// Drained the first time `resumed` runs.
    pending: Vec<WindowSpec>,
    commands: Rc<RefCell<AppCommands>>,
    app: AppHandle,
    on_started: Option<Box<dyn FnOnce(AppHandle)>>,
    started: bool,
    exit_when_last_window_closes: bool,
    dump_frame_path: Option<String>,
    next_window_index: usize,
    windows: HashMap<WindowId, WindowState>,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    pending_trays: Vec<TrayBuilder>,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    trays: Vec<InstalledTray>,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    proxy: EventLoopProxy<AppEvent>,
    /// One `wgpu::Instance` shared by every GPU window; `None` if no queued
    /// window resolved to the GPU backend.
    #[cfg(not(target_arch = "wasm32"))]
    gpu_instance: Option<Rc<wgpu::Instance>>,
    /// Adapter/device/queue negotiated by the first GPU window and reused
    /// by every later one; `None` until that first window is created.
    #[cfg(not(target_arch = "wasm32"))]
    gpu_context: Option<crate::gpu::GpuContext>,
}

impl AppHandler {
    fn create_window(&mut self, event_loop: &ActiveEventLoop<'_>, spec: WindowSpec) {
        let t0 = Instant::now();
        let attrs = PlatformWindowAttributes {
            title: spec.options.title.clone(),
            size: LogicalSize::new(spec.options.width as f64, spec.options.height as f64),
            position: spec
                .options
                .position
                .map(|(x, y)| LogicalPosition::new(x as f64, y as f64)),
            resizable: spec.options.resizable,
            decorations: spec.options.decorations,
            transparent: spec.options.transparent,
            role: spec.options.role,
        };

        let window = match spec.popup.as_ref() {
            Some(popup) => {
                let parent = popup
                    .parent
                    .window
                    .borrow()
                    .as_ref()
                    .map(|window| window.id())
                    .expect("creamui-render: the popup parent window no longer exists");
                event_loop
                    .create_popup(
                        attrs,
                        PlatformPopupOptions {
                            parent,
                            anchor_x: popup.anchor.x,
                            anchor_y: popup.anchor.y,
                            anchor_width: popup.anchor.width,
                            anchor_height: popup.anchor.height,
                            input_serial: popup.input_serial,
                            placement: popup.placement,
                        },
                    )
                    .expect("failed to create popup")
            }
            None => event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        };
        if let Some(region) = spec.options.blur {
            window.set_blur_region(Some(region));
        }
        // Show the window immediately; the presenter's first frame replaces
        // the platform's placeholder surface moments later.
        window.set_visible(true);
        log::debug!(
            "creamui-render: window created ({}x{} logical, scale factor {}) in {:?}",
            spec.options.width,
            spec.options.height,
            window.scale_factor(),
            t0.elapsed()
        );

        let pipeline = spec.pipeline;
        pipeline.scale_factor.set(window.scale_factor());
        {
            let physical = window.inner_size();
            let scale = window.scale_factor();
            let viewport = Size {
                width: (physical.width as f64 / scale) as f32,
                height: (physical.height as f64 / scale) as f32,
            };
            if pipeline.viewport.peek() != viewport {
                pipeline.viewport.set(viewport);
                pipeline.build();
            }
        }
        *pipeline.window.borrow_mut() = Some(window.clone());

        #[cfg(not(target_arch = "wasm32"))]
        let presenter = {
            if spec.options.backend == RenderBackend::Gpu && self.gpu_instance.is_none() {
                self.gpu_instance = Some(Rc::new(crate::gpu::create_instance()));
            }
            match Presenter::new(
                &window,
                spec.options.backend,
                spec.options.transparent,
                self.gpu_instance.as_deref(),
                &mut self.gpu_context,
            ) {
                Ok(presenter) => presenter,
                Err(err) => panic!("creamui-render: no usable presenter for the window: {err}"),
            }
        };
        #[cfg(target_arch = "wasm32")]
        let presenter = Presenter::new(&window);
        pipeline.frame.borrow_mut().adapter = presenter.adapter();
        log::debug!(
            "creamui-render: {} presenter ready: {:?}",
            presenter.name(),
            t0.elapsed()
        );

        (spec.on_window_ready)(WindowHandle {
            window: pipeline.window.clone(),
            theme: pipeline.theme.clone(),
            close_requested: spec.close_requested.clone(),
            focus_lost_handler: spec.focus_lost_handler.clone(),
            last_input_serial: spec.last_input_serial.clone(),
            app: self.app.clone(),
        });

        let now = Instant::now();
        let mut state = WindowState {
            pipeline,
            close_requested: spec.close_requested,
            focus_lost_handler: spec.focus_lost_handler,
            last_input_serial: spec.last_input_serial,
            close_behavior: spec.options.close_behavior,
            frameless_resizable: !spec.options.decorations && spec.options.resizable,
            presenter: Some(presenter),
            pointer_pos: Point::default(),
            modifiers: PlatformModifiers::default(),
            next_blink: now + CARET_BLINK_INTERVAL,
            next_animation: now,
            next_devtools_refresh: now,
            pending_viewport: None,
            current_cursor: CursorIcon::Default,
            hovered: None,
            dragging: None,
            drag_end: None,
            drag_click: None,
            pending_drag: None,
            _effect: spec._effect,
            t_run: t0,
            first_present_logged: false,
        };
        if window.is_ready() {
            state.redraw();
        }
        self.windows.insert(window.id(), state);
    }
}

enum AppEvent {
    #[cfg(all(feature = "tray", target_os = "linux"))]
    TrayMenu(String),
    BackgroundJob(u64, Box<dyn Any + Send>),
}

impl ApplicationHandler<AppEvent> for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop<'_>) {
        #[cfg(all(feature = "tray", target_os = "linux"))]
        if self.trays.is_empty() {
            self.trays = std::mem::take(&mut self.pending_trays)
                .into_iter()
                .enumerate()
                .map(|(index, tray)| {
                    tray.install(index, self.proxy.clone())
                        .expect("creamui-render: failed to create system tray icon")
                })
                .collect();
        }
        if !self.started {
            self.started = true;
            if let Some(on_started) = self.on_started.take() {
                on_started(self.app.clone());
            }
        }
        let pending = std::mem::take(&mut self.pending);
        for spec in pending {
            self.create_window(event_loop, spec);
        }
        self.drain_app_commands(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop<'_>,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            let hide = self
                .windows
                .get(&window_id)
                .filter(|state| state.close_behavior == CloseBehavior::Hide);
            if let Some(state) = hide {
                if let Some(window) = state.pipeline.window.borrow().as_ref() {
                    window.set_visible(false);
                }
                return;
            }
            self.close_window(event_loop, window_id);
            return;
        }

        if let Some(state) = self.windows.get_mut(&window_id) {
            state.handle_window_event(event);
        }
        if self
            .windows
            .get(&window_id)
            .is_some_and(|state| state.close_requested.get())
        {
            self.close_window(event_loop, window_id);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop<'_>, event: AppEvent) {
        match event {
            #[cfg(all(feature = "tray", target_os = "linux"))]
            AppEvent::TrayMenu(id) => {
                for tray in &self.trays {
                    if let Some(action) = tray.actions.get(&id) {
                        action(&self.app);
                        break;
                    }
                }
            }
            AppEvent::BackgroundJob(job_id, value) => {
                resolve_background_job(&self.commands, job_id, value);
            }
        }
        self.drain_app_commands(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop<'_>) {
        // Closes before creates: a replacement popup must not be requested
        // while the one it's replacing is still alive server-side.
        let close_requests: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, state)| state.close_requested.get().then_some(*id))
            .collect();
        for window_id in close_requests {
            self.close_window(event_loop, window_id);
        }

        self.drain_app_commands(event_loop);
        if self.commands.borrow().exit_requested {
            event_loop.exit();
            return;
        }

        let now = Instant::now();
        let next_wake = self
            .windows
            .values_mut()
            .filter_map(|state| state.tick(now))
            .min();
        event_loop.set_control_flow(match next_wake {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}

impl AppHandler {
    fn drain_app_commands(&mut self, event_loop: &ActiveEventLoop<'_>) {
        let (windows, exit_requested) = {
            let mut commands = self.commands.borrow_mut();
            (
                std::mem::take(&mut commands.windows),
                commands.exit_requested,
            )
        };
        if exit_requested {
            event_loop.exit();
            return;
        }

        for mut pending in windows {
            #[cfg(not(target_arch = "wasm32"))]
            {
                pending.options.backend = RenderBackend::resolve(pending.options.backend);
            }
            #[cfg(target_arch = "wasm32")]
            {
                pending.options.backend = RenderBackend::Gpu;
            }
            let spec = build_window_spec(
                self.next_window_index,
                pending,
                self.dump_frame_path.as_deref(),
                true,
            );
            self.next_window_index += 1;
            self.create_window(event_loop, spec);
        }
    }

    fn close_window(&mut self, event_loop: &ActiveEventLoop<'_>, window_id: WindowId) {
        log::debug!("creamui-render: close requested for window {window_id:?}");
        if let Some(mut state) = self.windows.remove(&window_id) {
            state.presenter = None;
            if let Some(window) = state.pipeline.window.borrow_mut().take() {
                window.close();
            }
        }
        if self.windows.is_empty() && self.exit_when_last_window_closes {
            event_loop.exit();
        }
    }
}

/// Opens a window and runs the reactive render loop until it is closed.
///
/// `build_ui` is called once up front and again whenever a signal it reads
/// changes; it must construct a fresh widget tree covering `viewport` each
/// time (widgets are cheap, immutable descriptions — see
/// `creamui_core::Widget`). `clear_color` is the color the window is wiped
/// to before `build_ui`'s tree is painted. `on_window_ready` is called once,
/// as soon as the window exists, with a [`WindowHandle`] for issuing
/// window-level operations (resize, move, always-on-top) later — e.g. from
/// a click handler.
///
/// To open several windows sharing one process and event loop (e.g. a
/// desktop-shell dock), use [`AppBuilder`] instead.
pub fn run(
    options: WindowOptions,
    clear_color: Color,
    on_window_ready: impl FnOnce(WindowHandle) + 'static,
    build_ui: impl Fn(Size) -> BoxedWidget + 'static,
) {
    AppBuilder::new()
        .window(options, clear_color, on_window_ready, build_ui)
        .run();
}

fn run_windows(
    specs: Vec<PendingWindow>,
    on_panic: Option<PanicHandler>,
    on_started: Option<Box<dyn FnOnce(AppHandle)>>,
    exit_when_last_window_closes: bool,
    #[cfg(all(feature = "tray", target_os = "linux"))] trays: Vec<TrayBuilder>,
) {
    #[cfg(all(feature = "tray", target_os = "linux"))]
    let has_tray = !trays.is_empty();
    #[cfg(not(all(feature = "tray", target_os = "linux")))]
    let has_tray = false;
    assert!(
        !specs.is_empty() || on_started.is_some() || !exit_when_last_window_closes || has_tray,
        "creamui-render: AppBuilder::run() needs a window, on_started handler, tray, or keep_running()"
    );

    if let Some(handler) = on_panic {
        install_panic_dispatch();
        PANIC_HANDLER.with(|cell| *cell.borrow_mut() = Some(handler));
    }

    init_logging();
    let t_run = Instant::now();
    log::debug!("creamui-render: run() start with {} window(s)", specs.len());

    // Each window's requested backend can be force-overridden at launch via
    // `CUI_OVERRIDE_RENDER_BACKEND` — resolve up front, once per window, so
    // every later decision (whether to pay GPU init cost at all, which
    // presenter `resumed` builds for that window) uses the same value.
    let mut specs = specs;
    #[cfg(not(target_arch = "wasm32"))]
    for spec in &mut specs {
        spec.options.backend = RenderBackend::resolve(spec.options.backend);
    }
    #[cfg(target_arch = "wasm32")]
    for spec in &mut specs {
        // The web demo always rasterizes into its canvas; there is no
        // backend choice to make.
        spec.options.backend = RenderBackend::Gpu;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let any_gpu = specs
        .iter()
        .any(|s| matches!(s.options.backend, RenderBackend::Gpu));

    // `wgpu::Instance::new` doesn't depend on any window and costs
    // ~100-200ms on Windows (Vulkan/DX12 loader + ICD enumeration) — kick it
    // off now so it overlaps with the initial UI builds below instead of
    // sitting on `resumed`'s critical path. One instance is shared by every
    // GPU-backend window; skipped entirely if none of them need it.
    #[cfg(not(target_arch = "wasm32"))]
    let gpu_instance_handle = any_gpu.then(|| std::thread::spawn(crate::gpu::create_instance));

    let dump_frame_path = std::env::var("CUI_DUMP_FRAME").ok();
    let multiple_windows = specs.len() > 1;
    let initial_window_count = specs.len();

    let pending: Vec<WindowSpec> = specs
        .into_iter()
        .enumerate()
        .map(|(index, spec)| {
            build_window_spec(index, spec, dump_frame_path.as_deref(), multiple_windows)
        })
        .collect();

    log::debug!(
        "creamui-render: before EventLoop::new: {:?}",
        t_run.elapsed()
    );
    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    log::debug!("creamui-render: event loop created: {:?}", t_run.elapsed());
    event_loop.set_control_flow(ControlFlow::Wait);

    #[cfg(not(target_arch = "wasm32"))]
    let gpu_instance = gpu_instance_handle.map(|handle| {
        let instance = handle
            .join()
            .expect("gpu instance creation thread panicked");
        log::debug!("creamui-render: gpu instance ready: {:?}", t_run.elapsed());
        Rc::new(instance)
    });

    let commands = Rc::new(RefCell::new(AppCommands {
        windows: Vec::new(),
        exit_requested: false,
        proxy: event_loop.create_proxy(),
        next_job_id: 0,
        background_jobs: HashMap::new(),
    }));
    let app = AppHandle {
        commands: commands.clone(),
    };
    let handler = AppHandler {
        pending,
        commands,
        app,
        on_started,
        started: false,
        exit_when_last_window_closes,
        dump_frame_path,
        // Dynamic windows continue after the initially queued specs.
        next_window_index: initial_window_count,
        windows: HashMap::new(),
        #[cfg(all(feature = "tray", target_os = "linux"))]
        pending_trays: trays,
        #[cfg(all(feature = "tray", target_os = "linux"))]
        trays: Vec::new(),
        #[cfg(all(feature = "tray", target_os = "linux"))]
        proxy: event_loop.create_proxy(),
        #[cfg(not(target_arch = "wasm32"))]
        gpu_instance,
        #[cfg(not(target_arch = "wasm32"))]
        gpu_context: None,
    };
    #[cfg(not(target_arch = "wasm32"))]
    let mut handler = handler;
    #[cfg(not(target_arch = "wasm32"))]
    event_loop
        .run_app(&mut handler)
        .expect("event loop exited with an error");
    #[cfg(target_arch = "wasm32")]
    {
        event_loop.spawn_app(handler);
    }
}

/// Builds one window's pre-creation state (signals, pipeline, reactive
/// effect) — everything that doesn't need the platform window yet.
fn build_window_spec(
    index: usize,
    spec: PendingWindow,
    dump_frame_path: Option<&str>,
    multiple_windows: bool,
) -> WindowSpec {
    let PendingWindow {
        options,
        popup,
        clear_color,
        on_window_ready,
        build_ui,
    } = spec;

    let window: SharedWindow = Rc::new(RefCell::new(None));
    let window_drag = WindowDragHandle::new({
        let window = window.clone();
        move || {
            if let Some(window) = window.borrow().as_ref() {
                let _ = window.drag_window();
            }
        }
    });
    // Several windows sharing one `CUI_DUMP_FRAME` path each get a suffix.
    let dump_frame_path = dump_frame_path.map(|path| {
        if multiple_windows {
            format!("{path}.{index}")
        } else {
            path.to_string()
        }
    });
    let pipeline = Rc::new(Pipeline {
        frame: RefCell::new(FrameState {
            recorder: SceneRecorder::new(),
            renderer: Renderer::new(),
            scene: None,
            pending: None,
            presented: None,
            raster: None,
            devtools: devtools_for_new_window(),
            report: FrameReport::default(),
            last_report: FrameReport::default(),
            adapter: None,
        }),
        viewport: Signal::new(Size {
            width: options.width as f32,
            height: options.height as f32,
        }),
        scale_factor: Signal::new(1.0),
        window,
        build_ui: Rc::from(build_ui),
        theme: ThemeProvider::new(options.theme),
        window_drag,
        clear_color,
        focused: Cell::new(options.focus_first.then_some(0)),
        caret_visible: Cell::new(true),
        pending_root: RefCell::new(None),
        needs_layout: Cell::new(true),
        needs_paint: Cell::new(true),
        dump_frame_path,
    });
    let effect = create_effect({
        let pipeline = pipeline.clone();
        move || pipeline.build()
    });

    WindowSpec {
        options,
        popup,
        on_window_ready,
        pipeline,
        _effect: effect,
        close_requested: Rc::new(Cell::new(false)),
        focus_lost_handler: Rc::new(RefCell::new(None)),
        last_input_serial: Rc::new(Cell::new(None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct WindowEventHarness {
        state: WindowState,
    }

    impl WindowEventHarness {
        fn new(build_ui: impl Fn(Size) -> BoxedWidget + 'static) -> Self {
            let spec = build_window_spec(
                0,
                PendingWindow {
                    options: WindowOptions {
                        width: 100,
                        height: 100,
                        ..WindowOptions::default()
                    },
                    popup: None,
                    clear_color: Color::rgba(0, 0, 0, 255),
                    on_window_ready: Box::new(|_| {}),
                    build_ui: Box::new(build_ui),
                },
                None,
                false,
            );
            let now = Instant::now();
            let mut state = WindowState {
                pipeline: spec.pipeline,
                close_requested: spec.close_requested,
                focus_lost_handler: spec.focus_lost_handler,
                last_input_serial: spec.last_input_serial,
                close_behavior: CloseBehavior::Close,
                frameless_resizable: false,
                presenter: None,
                pointer_pos: Point::default(),
                modifiers: PlatformModifiers::default(),
                next_blink: now + CARET_BLINK_INTERVAL,
                next_animation: now,
                next_devtools_refresh: now,
                pending_viewport: None,
                current_cursor: CursorIcon::Default,
                hovered: None,
                dragging: None,
                drag_end: None,
                drag_click: None,
                pending_drag: None,
                _effect: spec._effect,
                t_run: now,
                first_present_logged: false,
            };
            state.redraw();
            WindowEventHarness { state }
        }

        fn send(&mut self, event: WindowEvent) {
            self.state.handle_window_event(event);
        }

        fn key(&mut self, input: KeyInput) {
            self.state.handle_key_input(input);
        }
    }

    struct InteractiveWidget {
        clicks: Signal<usize>,
        keys: Signal<String>,
    }

    struct ThemeInChildren;

    impl creamui_core::Widget for ThemeInChildren {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style::default().into()
        }

        fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {}

        fn children(&mut self) -> Vec<BoxedWidget> {
            let _ = creamui_theme::use_theme();
            Vec::new()
        }
    }

    impl creamui_core::Widget for InteractiveWidget {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(100.0),
                    height: creamui_core::layout::Dimension::Length(100.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {}

        fn focusable(&self) -> bool {
            true
        }

        fn on_click(&self) -> Option<Rc<dyn Fn()>> {
            let clicks = self.clicks.clone();
            Some(Rc::new(move || clicks.update(|count| *count += 1)))
        }

        fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
            let keys = self.keys.clone();
            Some(Rc::new(move |input| {
                if let Key::Char(character) = input.key {
                    keys.update(|text| text.push(character));
                }
            }))
        }
    }

    fn window_size() -> Size {
        Size {
            width: 100.0,
            height: 100.0,
        }
    }

    #[test]
    fn build_ui_with_recovery_passes_through_without_a_registered_handler() {
        PANIC_HANDLER.with(|cell| *cell.borrow_mut() = None);
        let build: Rc<dyn Fn(Size) -> BoxedWidget> = Rc::new(|_size| Box::new(BlankWidget));
        let _widget = build_ui_with_recovery(&build, window_size());
    }

    #[test]
    fn build_ui_with_recovery_catches_a_panic_and_calls_the_handler() {
        install_panic_dispatch();
        let called = Rc::new(Cell::new(false));
        let handler_called = called.clone();
        PANIC_HANDLER.with(|cell| {
            *cell.borrow_mut() = Some(Rc::new(move |details: &PanicDetails| {
                handler_called.set(true);
                assert_eq!(details.message, "boom");
            }));
        });

        let build: Rc<dyn Fn(Size) -> BoxedWidget> = Rc::new(|_size| panic!("boom"));
        let _widget = build_ui_with_recovery(&build, window_size());

        assert!(called.get(), "on_panic handler must run for a caught panic");
        PANIC_HANDLER.with(|cell| *cell.borrow_mut() = None);
    }

    #[test]
    fn app_builder_accepts_a_one_shot_window_ready_callback() {
        let message = String::from("ready");
        let _app = AppBuilder::new().window(
            WindowOptions::default(),
            Color::rgba(0, 0, 0, 255),
            move |_| drop(message),
            |_| Box::new(BlankWidget),
        );
    }

    #[test]
    fn headless_harness_delivers_clicks_and_keys() {
        let clicks = Signal::new(0);
        let keys = Signal::new(String::new());
        let mut harness = WindowEventHarness::new({
            let clicks = clicks.clone();
            let keys = keys.clone();
            move |_| {
                Box::new(InteractiveWidget {
                    clicks: clicks.clone(),
                    keys: keys.clone(),
                })
            }
        });

        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 20.0, y: 20.0 },
        });
        harness.send(WindowEvent::MouseInput {
            pressed: true,
            button: MouseButton::Left,
            serial: None,
        });
        harness.key(KeyInput {
            key: Key::Char('x'),
            modifiers: Modifiers::default(),
        });

        assert_eq!(clicks.get(), 1);
        assert_eq!(keys.get(), "x");
    }

    struct DraggableWidget {
        calls: Rc<RefCell<Vec<Point>>>,
        end_calls: Rc<Cell<u32>>,
    }

    impl creamui_core::Widget for DraggableWidget {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(100.0),
                    height: creamui_core::layout::Dimension::Length(100.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {}

        fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
            let calls = self.calls.clone();
            Some(Rc::new(move |local, _rect| calls.borrow_mut().push(local)))
        }

        fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
            let end_calls = self.end_calls.clone();
            Some(Rc::new(move || end_calls.set(end_calls.get() + 1)))
        }
    }

    #[test]
    fn cursor_moved_during_a_drag_coalesces_to_the_latest_position_per_redraw() {
        let calls: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
        let end_calls = Rc::new(Cell::new(0));
        let mut harness = WindowEventHarness::new({
            let calls = calls.clone();
            let end_calls = end_calls.clone();
            move |_| {
                Box::new(DraggableWidget {
                    calls: calls.clone(),
                    end_calls: end_calls.clone(),
                })
            }
        });

        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 10.0, y: 10.0 },
        });
        harness.send(WindowEvent::MouseInput {
            pressed: true,
            button: MouseButton::Left,
            serial: None,
        });
        assert_eq!(
            calls.borrow().len(),
            1,
            "the initial press dispatches immediately"
        );

        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 20.0, y: 20.0 },
        });
        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 30.0, y: 30.0 },
        });
        assert_eq!(
            calls.borrow().len(),
            1,
            "raw motion events during a drag must not each dispatch the handler"
        );

        harness.send(WindowEvent::RedrawRequested);
        assert_eq!(
            calls.borrow().len(),
            2,
            "a redraw flushes exactly one call, for the latest queued position"
        );
        assert_eq!(calls.borrow()[1], Point { x: 30.0, y: 30.0 });
    }

    #[test]
    fn real_wayland_drag_events_drive_the_same_on_drag_callbacks() {
        let calls: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
        let end_calls = Rc::new(Cell::new(0));
        let mut harness = WindowEventHarness::new({
            let calls = calls.clone();
            let end_calls = end_calls.clone();
            move |_| {
                Box::new(DraggableWidget {
                    calls: calls.clone(),
                    end_calls: end_calls.clone(),
                })
            }
        });

        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 10.0, y: 10.0 },
        });
        harness.send(WindowEvent::MouseInput {
            pressed: true,
            button: MouseButton::Left,
            serial: None,
        });
        assert_eq!(
            calls.borrow().len(),
            1,
            "the initial press dispatches immediately"
        );

        // Once a real drag starts, the compositor stops sending CursorMoved
        // and sends these instead.
        harness.send(WindowEvent::DragMoved {
            position: creamui_platform::PhysicalPosition { x: 40.0, y: 40.0 },
        });
        harness.send(WindowEvent::RedrawRequested);
        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(calls.borrow()[1], Point { x: 40.0, y: 40.0 });

        harness.send(WindowEvent::DragDropped);
        assert_eq!(
            end_calls.get(),
            1,
            "a drop must end the drag like releasing the mouse does"
        );
    }

    struct HoverCountingWidget {
        paint_calls: Rc<Cell<usize>>,
    }

    impl creamui_core::Widget for HoverCountingWidget {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(100.0),
                    height: creamui_core::layout::Dimension::Length(100.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {
            self.paint_calls.set(self.paint_calls.get() + 1);
        }

        fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
            Some(Rc::new(|_| {}))
        }
    }

    #[test]
    fn cursor_moved_within_the_same_hover_region_does_not_repaint() {
        let paint_calls = Rc::new(Cell::new(0));
        let mut harness = WindowEventHarness::new({
            let paint_calls = paint_calls.clone();
            move |_| {
                Box::new(HoverCountingWidget {
                    paint_calls: paint_calls.clone(),
                })
            }
        });

        let baseline = paint_calls.get();
        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 10.0, y: 10.0 },
        });
        harness.send(WindowEvent::RedrawRequested);
        let after_enter = paint_calls.get();
        assert!(
            after_enter > baseline,
            "entering a hover region must still repaint"
        );

        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 20.0, y: 20.0 },
        });
        harness.send(WindowEvent::RedrawRequested);
        assert_eq!(
            paint_calls.get(),
            after_enter,
            "moving within the same hover region must not trigger another repaint"
        );
    }

    struct SmallHoverWidget {
        paint_calls: Rc<Cell<usize>>,
    }

    impl creamui_core::Widget for SmallHoverWidget {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(20.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, painter: &mut dyn creamui_core::Painter, rect: creamui_core::Rect) {
            self.paint_calls.set(self.paint_calls.get() + 1);
            let color = if painter.hovered(rect) {
                Color::rgb(255, 0, 0)
            } else {
                Color::rgb(0, 0, 255)
            };
            painter.fill_rect(rect, color, 0.0);
        }

        fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
            Some(Rc::new(|_| {}))
        }
    }

    struct SmallHoverRoot {
        paint_calls: Rc<Cell<usize>>,
    }

    impl creamui_core::Widget for SmallHoverRoot {
        fn style(&self) -> creamui_core::Style {
            creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(100.0),
                    height: creamui_core::layout::Dimension::Length(100.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {}

        fn children(&mut self) -> Vec<BoxedWidget> {
            vec![Box::new(SmallHoverWidget {
                paint_calls: self.paint_calls.clone(),
            })]
        }
    }

    #[test]
    fn many_invalidations_coalesce_into_one_frame() {
        let paint_calls = Rc::new(Cell::new(0));
        let mut harness = WindowEventHarness::new({
            let paint_calls = paint_calls.clone();
            move |_| {
                Box::new(SmallHoverRoot {
                    paint_calls: paint_calls.clone(),
                })
            }
        });
        let baseline = paint_calls.get();
        for _ in 0..10 {
            harness.state.pipeline.invalidate_layout();
            harness.state.pipeline.invalidate_paint();
        }
        assert_eq!(
            paint_calls.get(),
            baseline,
            "invalidation alone never paints"
        );
        harness.send(WindowEvent::RedrawRequested);
        harness.send(WindowEvent::RedrawRequested);
        assert_eq!(paint_calls.get(), baseline + 1);
    }

    #[test]
    fn hovering_a_small_region_presents_only_its_pixels() {
        let mut harness = WindowEventHarness::new(|_| {
            Box::new(SmallHoverRoot {
                paint_calls: Rc::new(Cell::new(0)),
            })
        });
        harness.send(WindowEvent::CursorMoved {
            position: creamui_platform::PhysicalPosition { x: 10.0, y: 10.0 },
        });
        harness.send(WindowEvent::RedrawRequested);
        let frame = harness.state.pipeline.frame.borrow();
        let report = &frame.last_report;
        assert!(report.damaged_pixels > 0, "the hovered widget must repaint");
        assert!(
            report.damaged_pixels < report.frame_pixels,
            "a hover change must not repaint the whole window: {report:?}"
        );
    }

    #[test]
    fn resizing_updates_layout_on_the_next_frame() {
        let sizes = Rc::new(RefCell::new(Vec::new()));
        let mut harness = WindowEventHarness::new({
            let sizes = sizes.clone();
            move |size| {
                sizes.borrow_mut().push(size);
                Box::new(BlankWidget)
            }
        });
        for width in [120, 140, 160] {
            harness.send(WindowEvent::Resized(creamui_platform::PhysicalSize {
                width,
                height: 90,
            }));
        }
        let builds = sizes.borrow().len();
        harness.send(WindowEvent::RedrawRequested);
        assert_eq!(sizes.borrow().len(), builds + 1, "one rebuild per frame");
        assert_eq!(sizes.borrow().last().unwrap().width, 160.0);
        let frame = harness.state.pipeline.frame.borrow();
        assert_eq!(frame.presented.as_ref().unwrap().width, 160);
    }

    #[test]
    fn frameless_windows_expose_resize_edges() {
        let mut harness = WindowEventHarness::new(|_| Box::new(BlankWidget));
        harness.state.frameless_resizable = true;
        harness.state.pointer_pos = Point { x: 2.0, y: 2.0 };
        assert_eq!(
            harness.state.resize_direction(),
            Some(ResizeDirection::NorthWest)
        );
        harness.state.pointer_pos = Point { x: 50.0, y: 50.0 };
        assert_eq!(harness.state.resize_direction(), None);
    }

    #[test]
    fn popup_done_requests_close() {
        let mut harness = WindowEventHarness::new(|_| Box::new(BlankWidget));
        harness.send(WindowEvent::PopupDone);
        assert!(harness.state.close_requested.get());
    }

    #[test]
    fn frame_scope_is_available_to_lazy_children() {
        let _harness = WindowEventHarness::new(|_| Box::new(ThemeInChildren));
    }

    // `EventLoop::build()` refuses to run outside the main thread, which
    // `cargo test` never is — the X11 `any_thread` escape hatch is the only
    // way to get a real `EventLoopProxy` here, and the adapter allows only one
    // `EventLoop` per process, so the (leaked) loop behind it is
    // shared across every test that needs a proxy. This only proves the
    // registration/dispatch logic below; the hop through a real running
    // `ActiveEventLoop`'s `user_event` is pre-existing adapter machinery
    // already exercised by the tray feature, not re-tested here.
    #[cfg(target_os = "linux")]
    fn test_proxy() -> EventLoopProxy<AppEvent> {
        static PROXY: std::sync::OnceLock<EventLoopProxy<AppEvent>> = std::sync::OnceLock::new();
        PROXY
            .get_or_init(|| {
                let mut builder = EventLoop::<AppEvent>::with_user_event();
                builder.with_any_thread(true);
                let event_loop = builder
                    .build()
                    .expect("failed to build a headless event loop");
                let proxy = event_loop.create_proxy();
                std::mem::forget(event_loop);
                proxy
            })
            .clone()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn spawn_background_runs_work_off_thread_and_resolve_background_job_delivers_it() {
        let commands = Rc::new(RefCell::new(AppCommands {
            windows: Vec::new(),
            exit_requested: false,
            proxy: test_proxy(),
            next_job_id: 0,
            background_jobs: HashMap::new(),
        }));
        let handle = AppHandle {
            commands: commands.clone(),
        };

        let result: Rc<Cell<Option<u32>>> = Rc::new(Cell::new(None));
        let result_for_done = result.clone();
        let (worked_tx, worked_rx) = std::sync::mpsc::channel::<std::thread::ThreadId>();
        handle.spawn_background(
            move || {
                let _ = worked_tx.send(std::thread::current().id());
                42u32
            },
            move |value| result_for_done.set(Some(value)),
        );

        let worker_thread = worked_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("work did not run on a background thread in time");
        assert_ne!(
            worker_thread,
            std::thread::current().id(),
            "work must run off the calling thread"
        );

        assert_eq!(commands.borrow().background_jobs.len(), 1);
        assert!(result.get().is_none(), "on_done must not have run yet");

        resolve_background_job(&commands, 0, Box::new(42u32));

        assert_eq!(result.get(), Some(42));
        assert!(commands.borrow().background_jobs.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolve_background_job_is_a_no_op_for_an_unknown_or_already_resolved_id() {
        let commands = Rc::new(RefCell::new(AppCommands {
            windows: Vec::new(),
            exit_requested: false,
            proxy: test_proxy(),
            next_job_id: 0,
            background_jobs: HashMap::new(),
        }));
        resolve_background_job(&commands, 99, Box::new(0u32));
    }
}
