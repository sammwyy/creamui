//! Window creation and the reactive render loop.
//!
//! [`run`] opens a single window; [`AppBuilder`] opens several, all sharing
//! one process and one winit event loop — e.g. a desktop-shell dock where
//! each icon/panel is its own window but spawning a process per icon would
//! multiply fixed per-process overhead (runtime, allocator, embedded font,
//! and — for the GPU backend — the graphics driver) for no benefit. Every
//! window keeps its own reactive state, so a signal change in one never
//! touches another's frame.
//!
//! On startup, and again on the next compositor frame after a
//! [`creamui_reactive::Signal`] read while building a window's UI changes,
//! that window's widget tree is rebuilt, its layout recomputed, it's repainted via
//! [`crate::painter::SkiaPainter`], and a redraw is requested; the actual
//! `RedrawRequested` handler only uploads the already-painted buffer to that
//! window's presenter (GPU or CPU — see [`crate::backend::RenderBackend`])
//! and presents it.

use crate::backend::RenderBackend;
#[cfg(not(target_arch = "wasm32"))]
use crate::cpu::CpuState;
use crate::devtools::{devtools_for_new_window, WindowDevtools};
#[cfg(not(target_arch = "wasm32"))]
use crate::gpu::GpuState;
use crate::painter::SkiaPainter;
#[cfg(target_arch = "wasm32")]
use crate::web::WebState;
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Modifiers, Point, Rect, Renderer, Scene, Size,
    WindowDragHandle,
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
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{
    CursorIcon as WinitCursorIcon, ResizeDirection, Window, WindowAttributes, WindowId, WindowLevel,
};

use winit::event_loop::EventLoopProxy;

/// How long the text-input caret stays in each visibility phase while
/// blinking (on, then off, then on again).
const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const RESIZE_SETTLE_DELAY: Duration = Duration::from_millis(100);

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

fn translate_cursor_icon(icon: CursorIcon) -> WinitCursorIcon {
    match icon {
        CursorIcon::Default => WinitCursorIcon::Default,
        CursorIcon::Text => WinitCursorIcon::Text,
        CursorIcon::Pointer => WinitCursorIcon::Pointer,
        CursorIcon::NotAllowed => WinitCursorIcon::NotAllowed,
        CursorIcon::ResizeHorizontal => WinitCursorIcon::EwResize,
        CursorIcon::ResizeVertical => WinitCursorIcon::NsResize,
        CursorIcon::ResizeNwse => WinitCursorIcon::NwseResize,
        CursorIcon::ResizeNesw => WinitCursorIcon::NeswResize,
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

/// Translates a winit logical key into CreamUI's backend-agnostic [`Key`].
/// Returns `None` for keys with no CreamUI meaning (modifiers, function
/// keys, etc.) — those are silently ignored rather than delivered.
fn translate_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Character(s) => s.chars().next().map(Key::Char),
        WinitKey::Named(NamedKey::Space) => Some(Key::Char(' ')),
        WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
        WinitKey::Named(NamedKey::Delete) => Some(Key::Delete),
        WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
        WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
        WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::Left),
        WinitKey::Named(NamedKey::ArrowRight) => Some(Key::Right),
        WinitKey::Named(NamedKey::ArrowUp) => Some(Key::Up),
        WinitKey::Named(NamedKey::ArrowDown) => Some(Key::Down),
        WinitKey::Named(NamedKey::Home) => Some(Key::Home),
        WinitKey::Named(NamedKey::End) => Some(Key::End),
        _ => None,
    }
}

/// Options for a window CreamUI opens, set once at startup. Transparent
/// windows need an alpha clear color and the GPU backend.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    /// How a close request from the window manager is handled.
    pub close_behavior: CloseBehavior,
    /// Which backend composites the CPU-rasterized frame to the window:
    /// GPU (`wgpu`, the default) or CPU-only (`softbuffer`). Can be
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
            resizable: true,
            decorations: true,
            transparent: false,
            close_behavior: CloseBehavior::Close,
            backend: RenderBackend::default(),
            theme: Theme::default(),
        }
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

struct FrameState {
    painter: SkiaPainter,
    renderer: Renderer,
    scene: Option<Scene>,
    devtools: Option<Box<dyn WindowDevtools>>,
}

/// Whichever backend is actually composing frames for a window, picked once
/// in `resumed` per [`WindowOptions::backend`] (as resolved by
/// [`RenderBackend::resolve`]).
enum Presenter {
    #[cfg(not(target_arch = "wasm32"))]
    Gpu(GpuState),
    #[cfg(not(target_arch = "wasm32"))]
    Cpu(CpuState),
    #[cfg(target_arch = "wasm32")]
    Web(WebState),
}

impl Presenter {
    fn present(&mut self, rgba: &[u8], width: u32, height: u32) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Gpu(gpu) => gpu.present(rgba, width, height),
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Cpu(cpu) => cpu.present(rgba, width, height),
            #[cfg(target_arch = "wasm32")]
            Presenter::Web(web) => web.present(rgba, width, height),
        }
    }

    /// Like [`Presenter::present`], but only re-uploads `dirty` (window-space
    /// logical rects, scaled to physical pixels by `scale`) to the GPU
    /// texture rather than the whole buffer. Backends with no partial-upload
    /// path fall back to a full [`Presenter::present`].
    fn present_partial(&mut self, rgba: &[u8], width: u32, height: u32, dirty: &[Rect], scale: f32) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Gpu(gpu) => gpu.present_partial(rgba, width, height, dirty, scale),
            #[cfg(not(target_arch = "wasm32"))]
            Presenter::Cpu(cpu) => cpu.present(rgba, width, height),
            #[cfg(target_arch = "wasm32")]
            Presenter::Web(web) => web.present(rgba, width, height),
        }
    }
}

type SharedWindow = Rc<RefCell<Option<Arc<Window>>>>;

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
/// actually been created (winit windows don't exist until the event loop
/// resumes, so this can't be available any earlier). All methods are no-ops
/// if called after the window has closed.
#[derive(Clone)]
pub struct WindowHandle {
    window: SharedWindow,
    theme: ThemeProvider,
    close_requested: Rc<Cell<bool>>,
    app: AppHandle,
}

impl WindowHandle {
    /// Requests a new logical-pixel window size. The actual resize (and any
    /// resulting `Resized` event) happens asynchronously, same as a user
    /// dragging the window border.
    pub fn resize(&self, width: u32, height: u32) {
        if let Some(window) = self.window.borrow().as_ref() {
            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
        }
    }

    /// Moves the window's top-left corner to a logical-pixel screen position.
    pub fn set_position(&self, x: i32, y: i32) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        }
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

    /// Requests that this window close.
    pub fn close(&self) {
        self.close_requested.set(true);
        if let Some(window) = self.window.borrow().as_ref() {
            window.request_redraw();
        }
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
            window.focus_window();
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
    on_window_ready: Box<dyn FnOnce(WindowHandle)>,
    repaint: Rc<dyn Fn()>,
    repaint_scene: Rc<dyn Fn()>,
    repaint_light: Rc<dyn Fn()>,
    repaint_animated: Rc<dyn Fn()>,
    render: Rc<dyn Fn()>,
    dirty: Rc<Cell<bool>>,
    scene_dirty: Rc<Cell<bool>>,
    animated_damage: Rc<RefCell<Vec<Rect>>>,
    _effect: Effect,
    viewport: Signal<Size>,
    scale_factor: Signal<f64>,
    frame: Rc<RefCell<FrameState>>,
    shared_window: SharedWindow,
    close_requested: Rc<Cell<bool>>,
    theme_provider: ThemeProvider,
    focused: Rc<Cell<Option<usize>>>,
    caret_visible: Rc<Cell<bool>>,
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
    clear_color: Color,
    on_window_ready: Box<dyn FnOnce(WindowHandle)>,
    build_ui: Box<dyn Fn(Size) -> BoxedWidget>,
}

impl AppBuilder {
    pub fn new() -> Self {
        AppBuilder {
            specs: Vec::new(),
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
        options: WindowOptions,
        clear_color: Color,
        on_window_ready: impl FnOnce(WindowHandle) + 'static,
        build_ui: impl Fn(Size) -> BoxedWidget + 'static,
    ) -> Self {
        self.specs.push(PendingWindow {
            options,
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

/// Per-window state for a window that has actually been created (its winit
/// [`Window`] exists and its presenter is ready). Lives in [`AppHandler`],
/// keyed by [`WindowId`], from the moment `resumed` creates it until
/// `CloseRequested` removes it.
struct WindowState {
    viewport: Signal<Size>,
    scale_factor: Signal<f64>,
    frame: Rc<RefCell<FrameState>>,
    window: SharedWindow,
    close_requested: Rc<Cell<bool>>,
    close_behavior: CloseBehavior,
    frameless_resizable: bool,
    presenter: Option<Presenter>,
    pointer_pos: Point,
    modifiers: ModifiersState,
    /// Index into the current `Scene`'s focusables, if any widget has
    /// keyboard focus. Only stable while the widget tree's shape doesn't
    /// change — see `Scene`'s doc comment. Shared with `repaint` (below) so
    /// each repaint knows which widget, if any, to paint a focus overlay
    /// (e.g. a text input's caret) onto.
    focused: Rc<Cell<Option<usize>>>,
    /// The text-input caret's current blink phase, shared with `repaint`
    /// the same way as `focused`.
    caret_visible: Rc<Cell<bool>>,
    /// When the caret should next toggle visibility (see `about_to_wait`).
    next_blink: Instant,
    next_animation: Instant,
    next_resize_render: Instant,
    resize_pending: bool,
    pending_viewport: Option<Size>,
    /// The system cursor icon last set on the window, so `CursorMoved`
    /// only calls into the backend when it actually changes.
    current_cursor: CursorIcon,
    /// Callback for the widget currently under the pointer. Keeping the
    /// callback rather than a scene index makes it safe across re-renders.
    hovered: Option<(creamui_core::Rect, Rc<dyn Fn(bool)>)>,
    /// Index into the current `Scene`'s draggables while the left mouse
    /// button is held down over one, `None` otherwise.
    dragging: Option<usize>,
    /// Invalidates this window. Signal changes and caret/focus updates are
    /// coalesced until the next `RedrawRequested` frame.
    repaint: Rc<dyn Fn()>,
    repaint_scene: Rc<dyn Fn()>,
    /// Paint-only refresh (no rebuild, no layout) for hover/press/focus-only
    /// changes — cheap enough to call from every `CursorMoved`.
    repaint_light: Rc<dyn Fn()>,
    /// Repaints only the widgets currently promoted to their own layer (see
    /// `Painter::push_layer`), driven by `about_to_wait`'s animation tick.
    /// Populates `animated_damage` instead of touching `dirty`/`scene_dirty`,
    /// since it never rebuilds, relayouts, or invalidates the `Scene`.
    repaint_animated: Rc<dyn Fn()>,
    /// Executes the deferred build/layout/paint pass. Signal writes only
    /// schedule this; `RedrawRequested` performs it once per compositor
    /// frame.
    render: Rc<dyn Fn()>,
    dirty: Rc<Cell<bool>>,
    scene_dirty: Rc<Cell<bool>>,
    /// Window-space rects painted by the last `repaint_animated`, consumed
    /// by `RedrawRequested` for a partial GPU texture upload.
    animated_damage: Rc<RefCell<Vec<Rect>>>,
    _effect: Effect,
    t_run: Instant,
    first_present_logged: bool,
}

impl WindowState {
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

    fn viewport_from_window(&self) -> Size {
        let Some(window) = self.window.borrow().as_ref().cloned() else {
            return self.viewport.peek();
        };
        let physical = window.inner_size();
        let scale = self.scale_factor.peek();
        Size {
            width: (physical.width as f64 / scale) as f32,
            height: (physical.height as f64 / scale) as f32,
        }
    }

    fn schedule_resize_render(&mut self) {
        self.next_resize_render = Instant::now() + RESIZE_SETTLE_DELAY;
        self.resize_pending = true;
    }

    fn queue_viewport(&mut self, viewport: Size) {
        if self
            .pending_viewport
            .unwrap_or_else(|| self.viewport.peek())
            != viewport
        {
            self.pending_viewport = Some(viewport);
            self.schedule_resize_render();
        }
    }

    fn flush_pending_viewport(&mut self) {
        if let Some(viewport) = self.pending_viewport.take() {
            self.viewport.set(viewport);
            (self.repaint)();
        }
    }

    fn handle_key_input(&mut self, key: KeyInput) {
        if key.key == Key::Tab {
            let next = self
                .frame
                .borrow()
                .scene
                .as_ref()
                .and_then(|scene| scene.next_focus(self.focused.get(), key.modifiers.shift));
            self.focused.set(next);
            (self.repaint_light)();
            return;
        }
        let Some(index) = self.focused.get() else {
            return;
        };

        let handler = self
            .frame
            .borrow()
            .scene
            .as_ref()
            .and_then(|scene| scene.on_key_at(index).cloned());
        if let Some(handler) = handler {
            self.caret_visible.set(true);
            self.next_blink = Instant::now() + CARET_BLINK_INTERVAL;
            handler(key);
            (self.render)();
        }
    }

    fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(new_size) => {
                if new_size.width == 0 || new_size.height == 0 {
                    return;
                }
                let scale = self.scale_factor.peek();
                let viewport = Size {
                    width: (new_size.width as f64 / scale) as f32,
                    height: (new_size.height as f64 / scale) as f32,
                };
                self.queue_viewport(viewport);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log::debug!("creamui-render: scale factor changed to {scale_factor}");
                set_if_changed(&self.scale_factor, scale_factor);
                let viewport = self.viewport_from_window();
                self.queue_viewport(viewport);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale_factor.peek();
                self.pointer_pos = Point {
                    x: (position.x / scale) as f32,
                    y: (position.y / scale) as f32,
                };
                self.frame.borrow_mut().painter.pointer = Some(self.pointer_pos);
                (self.repaint_light)();

                let hovered_cursor =
                    self.resize_direction()
                        .map(resize_cursor)
                        .unwrap_or_else(|| {
                            let frame = self.frame.borrow();
                            frame
                                .scene
                                .as_ref()
                                .and_then(|scene| scene.cursor_hit_test(self.pointer_pos))
                                .unwrap_or(CursorIcon::Default)
                        });
                if hovered_cursor != self.current_cursor {
                    self.current_cursor = hovered_cursor;
                    if let Some(window) = self.window.borrow().as_ref() {
                        window.set_cursor(translate_cursor_icon(hovered_cursor));
                    }
                }

                let next_hover = self
                    .frame
                    .borrow()
                    .scene
                    .as_ref()
                    .and_then(|scene| scene.hover_hit_test(self.pointer_pos));
                // Widget descriptions are recreated on each reactive frame,
                // so callback `Rc`s are not stable. The visible rect is: it
                // prevents a stationary pointer from producing leave/enter
                // churn after an unrelated redraw.
                let unchanged = matches!(
                    (&self.hovered, &next_hover),
                    (Some((current_rect, _)), Some((next_rect, _))) if current_rect == next_rect
                );
                if !unchanged {
                    if let Some((_, current)) = self.hovered.take() {
                        current(false);
                    }
                    if let Some((rect, next)) = next_hover {
                        next(true);
                        self.hovered = Some((rect, next));
                    }
                }

                if let Some(index) = self.dragging {
                    let frame = self.frame.borrow();
                    if let Some(scene) = frame.scene.as_ref() {
                        if let Some((rect, handler)) = scene.draggable_at(index) {
                            let local = Point {
                                x: self.pointer_pos.x - rect.x,
                                y: self.pointer_pos.y - rect.y,
                            };
                            let handler = handler.clone();
                            drop(frame);
                            handler(local, rect);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.frame.borrow_mut().painter.press_origin = Some(self.pointer_pos);
                (self.repaint_light)();
                if let Some(direction) = self.resize_direction() {
                    if let Some(window) = self.window.borrow().as_ref() {
                        let _ = window.drag_resize_window(direction);
                    }
                    return;
                }
                let frame = self.frame.borrow();
                let Some(scene) = frame.scene.as_ref() else {
                    return;
                };

                let click_handler = scene.hit_test(self.pointer_pos).cloned();
                let new_focus = scene.focus_hit_test(self.pointer_pos);
                let focus_changed = new_focus != self.focused.get();
                self.focused.set(new_focus);
                let drag_start = scene.drag_hit_test(self.pointer_pos).and_then(|index| {
                    scene
                        .draggable_at(index)
                        .map(|(rect, handler)| (index, rect, handler.clone()))
                });
                let drag_anchor = scene.drag_start_at(self.pointer_pos);
                drop(frame);

                if focus_changed {
                    // Reset the blink phase so the caret appears solid the
                    // instant a text input gains focus, rather than
                    // possibly landing mid-blink.
                    self.caret_visible.set(true);
                    self.next_blink = Instant::now() + CARET_BLINK_INTERVAL;
                    (self.repaint_light)();
                }

                if let Some((index, rect, handler)) = drag_start {
                    self.dragging = Some(index);
                    let local = Point {
                        x: self.pointer_pos.x - rect.x,
                        y: self.pointer_pos.y - rect.y,
                    };
                    handler(local, rect);
                }
                if let Some((rect, handler)) = drag_anchor {
                    handler(
                        Point {
                            x: self.pointer_pos.x - rect.x,
                            y: self.pointer_pos.y - rect.y,
                        },
                        rect,
                    );
                }
                if let Some(handler) = click_handler {
                    log::debug!("creamui-render: click hit at {:?}", self.pointer_pos);
                    handler();
                    (self.render)();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = None;
                self.frame.borrow_mut().painter.press_origin = None;
                (self.repaint_light)();
            }
            WindowEvent::CursorLeft { .. } => {
                self.frame.borrow_mut().painter.pointer = None;
                if let Some((_, callback)) = self.hovered.take() {
                    callback(false);
                }
                (self.repaint_light)();
            }
            WindowEvent::Focused(false) => {
                self.frame.borrow_mut().painter.press_origin = None;
                self.dragging = None;
                (self.repaint_light)();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.scale_factor.peek();
                // Convention: positive `delta_y` reveals content further
                // down (increases a scroll view's offset), matching
                // "natural" wheel-down scrolling.
                let delta_y: f32 = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => -(pos.y / scale) as f32,
                };

                let frame = self.frame.borrow();
                let Some(scene) = frame.scene.as_ref() else {
                    return;
                };
                let handler = scene.scroll_hit_test(self.pointer_pos).map(|index| {
                    (
                        scene.on_scroll_at(index).cloned(),
                        scene.scroll_is_local_at(index),
                    )
                });
                drop(frame);

                if let Some((Some(handler), local)) = handler {
                    handler(delta_y);
                    if local {
                        (self.repaint_light)();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if event.logical_key == WinitKey::Named(NamedKey::F3) {
                    let toggled = {
                        let mut frame = self.frame.borrow_mut();
                        frame
                            .devtools
                            .as_mut()
                            .is_some_and(|devtools| devtools.toggle())
                    };
                    if toggled {
                        (self.repaint_light)();
                    }
                    return;
                }
                let Some(key) = translate_key(&event.logical_key) else {
                    return;
                };
                self.handle_key_input(KeyInput {
                    key,
                    modifiers: Modifiers {
                        ctrl: self.modifiers.control_key(),
                        shift: self.modifiers.shift_key(),
                    },
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::RedrawRequested => {
                let mut full_repaint = true;
                if self.dirty.get() {
                    self.flush_pending_viewport();
                    (self.render)();
                    // `render` already repaints the scene, so a pending
                    // `scene_dirty` from earlier in the same event is moot.
                    self.scene_dirty.set(false);
                } else if self.scene_dirty.replace(false) {
                    (self.repaint_scene)();
                } else {
                    full_repaint = false;
                }
                let damage = std::mem::take(&mut *self.animated_damage.borrow_mut());
                let frame = self.frame.borrow();
                let pixmap = &frame.painter.pixmap;
                if let Some(presenter) = self.presenter.as_mut() {
                    if full_repaint || damage.is_empty() {
                        presenter.present(pixmap.data(), pixmap.width(), pixmap.height());
                    } else {
                        presenter.present_partial(
                            pixmap.data(),
                            pixmap.width(),
                            pixmap.height(),
                            &damage,
                            self.scale_factor.peek() as f32,
                        );
                    }
                }
                if !self.first_present_logged {
                    self.first_present_logged = true;
                    log::debug!(
                        "creamui-render: first present done: {:?}",
                        self.t_run.elapsed()
                    );
                }
            }
            _ => {}
        }
    }
}

/// The shared [`ApplicationHandler`] driving every window opened by
/// [`AppBuilder`] (and, for a single window, [`run`]) from one event loop.
struct AppHandler {
    /// Drained the first time `resumed` runs: creates each window's winit
    /// `Window` and presenter, then moves it into `windows`. `resumed` can
    /// in principle be called again later (e.g. mobile lifecycle), at which
    /// point this is already empty and a no-op.
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
    /// Shared by every window using [`RenderBackend::Gpu`] — one
    /// `wgpu::Instance` regardless of how many GPU windows are open, since
    /// its ~100-200ms Windows loader/ICD cost and driver memory footprint
    /// are the whole reason multi-window-in-one-process is worth doing.
    /// `None` if no queued window resolved to the GPU backend.
    #[cfg(not(target_arch = "wasm32"))]
    gpu_instance: Option<Rc<wgpu::Instance>>,
}

impl AppHandler {
    fn create_window(&mut self, event_loop: &ActiveEventLoop, spec: WindowSpec) {
        let t0 = Instant::now();
        let attrs = WindowAttributes::default()
            .with_title(spec.options.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                spec.options.width,
                spec.options.height,
            ))
            .with_resizable(spec.options.resizable)
            .with_decorations(spec.options.decorations)
            .with_transparent(spec.options.transparent);
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_append(true)
        };

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );
        // Show the window the instant it exists rather than waiting for
        // GPU init (adapter/device/pipeline — several hundred ms on
        // Windows) to finish. That init cost doesn't go away, but the
        // window appearing immediately is what "the app feels slow to
        // launch" is actually about; the OS-default surface briefly
        // shown underneath gets replaced by the real first frame a
        // moment later.
        window.set_visible(true);
        log::debug!(
            "creamui-render: window created and shown: {:?}",
            t0.elapsed()
        );
        log::debug!(
            "creamui-render: window created ({}x{} logical, scale factor {})",
            spec.options.width,
            spec.options.height,
            window.scale_factor()
        );

        spec.scale_factor.set(window.scale_factor());
        {
            let physical = window.inner_size();
            let scale = spec.scale_factor.peek();
            spec.viewport.set(Size {
                width: (physical.width as f64 / scale) as f32,
                height: (physical.height as f64 / scale) as f32,
            });
        }
        (spec.repaint)();

        #[cfg(not(target_arch = "wasm32"))]
        let mut presenter = match spec.options.backend {
            RenderBackend::Gpu => {
                if self.gpu_instance.is_none() {
                    self.gpu_instance = Some(Rc::new(GpuState::create_instance()));
                }
                let instance = self
                        .gpu_instance
                        .as_ref()
                        .expect("a window resolved to RenderBackend::Gpu but no shared wgpu::Instance was created");
                Presenter::Gpu(GpuState::new(
                    window.clone(),
                    instance,
                    spec.options.transparent,
                ))
            }
            RenderBackend::Cpu => Presenter::Cpu(CpuState::new(window.clone())),
        };
        #[cfg(target_arch = "wasm32")]
        let mut presenter = Presenter::Web(WebState::new(window.clone()));
        log::debug!(
            "creamui-render: {:?} presenter ready: {:?}",
            spec.options.backend,
            t0.elapsed()
        );

        // Present the already-painted first frame (built by the initial
        // `create_effect` run in `run_windows`, before this window
        // existed).
        {
            let frame = spec.frame.borrow();
            let pixmap = &frame.painter.pixmap;
            presenter.present(pixmap.data(), pixmap.width(), pixmap.height());
        }
        log::debug!("creamui-render: first frame presented: {:?}", t0.elapsed());

        *spec.shared_window.borrow_mut() = Some(window.clone());
        (spec.on_window_ready)(WindowHandle {
            window: spec.shared_window.clone(),
            theme: spec.theme_provider.clone(),
            close_requested: spec.close_requested.clone(),
            app: self.app.clone(),
        });

        let window_id = window.id();
        let frameless_resizable = !spec.options.decorations && spec.options.resizable;
        self.windows.insert(
            window_id,
            WindowState {
                viewport: spec.viewport,
                scale_factor: spec.scale_factor,
                frame: spec.frame,
                window: spec.shared_window.clone(),
                close_requested: spec.close_requested,
                close_behavior: spec.options.close_behavior,
                frameless_resizable,
                presenter: Some(presenter),
                pointer_pos: Point::default(),
                modifiers: ModifiersState::default(),
                focused: spec.focused,
                caret_visible: spec.caret_visible,
                next_blink: Instant::now() + CARET_BLINK_INTERVAL,
                next_animation: Instant::now(),
                next_resize_render: Instant::now(),
                resize_pending: false,
                pending_viewport: None,
                current_cursor: CursorIcon::Default,
                hovered: None,
                dragging: None,
                repaint: spec.repaint,
                repaint_scene: spec.repaint_scene,
                repaint_light: spec.repaint_light,
                repaint_animated: spec.repaint_animated,
                render: spec.render,
                dirty: spec.dirty,
                scene_dirty: spec.scene_dirty,
                animated_damage: spec.animated_damage,
                _effect: spec._effect,
                t_run: t0,
                first_present_logged: false,
            },
        );
    }
}

enum AppEvent {
    #[cfg(all(feature = "tray", target_os = "linux"))]
    TrayMenu(String),
    BackgroundJob(u64, Box<dyn Any + Send>),
}

impl ApplicationHandler<AppEvent> for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
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
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            if self
                .windows
                .get(&window_id)
                .is_some_and(|state| state.close_behavior == CloseBehavior::Hide)
            {
                if let Some(window) = self
                    .windows
                    .get(&window_id)
                    .and_then(|state| state.window.borrow().as_ref().cloned())
                {
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drain_app_commands(event_loop);
        if self.commands.borrow().exit_requested {
            event_loop.exit();
            return;
        }
        let close_requests: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, state)| state.close_requested.get().then_some(*id))
            .collect();
        for window_id in close_requests {
            self.close_window(event_loop, window_id);
        }
        if self.windows.is_empty() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let now = Instant::now();
        let mut next_wake: Option<Instant> = None;
        for state in self.windows.values_mut() {
            if state.resize_pending {
                if now >= state.next_resize_render {
                    state.resize_pending = false;
                    state.dirty.set(true);
                    if let Some(window) = state.window.borrow().as_ref() {
                        window.request_redraw();
                    }
                } else {
                    next_wake = Some(next_wake.map_or(state.next_resize_render, |t| {
                        t.min(state.next_resize_render)
                    }));
                }
            }
            if state.frame.borrow().painter.animated {
                if now >= state.next_animation {
                    state.next_animation = now + Duration::from_millis(32);
                    (state.repaint_animated)();
                }
                next_wake =
                    Some(next_wake.map_or(state.next_animation, |t| t.min(state.next_animation)));
            }
            if state.focused.get().is_none() {
                continue;
            }
            if now >= state.next_blink {
                state.caret_visible.set(!state.caret_visible.get());
                state.next_blink = now + CARET_BLINK_INTERVAL;
                (state.repaint)();
            }
            next_wake = Some(next_wake.map_or(state.next_blink, |t| t.min(state.next_blink)));
        }
        event_loop.set_control_flow(match next_wake {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}

impl AppHandler {
    fn drain_app_commands(&mut self, event_loop: &ActiveEventLoop) {
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

    fn close_window(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        log::debug!("creamui-render: close requested for window {window_id:?}");
        if let Some(state) = self.windows.remove(&window_id) {
            state.window.borrow_mut().take();
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
        // The web demo presents directly to its canvas; it has no desktop
        // GPU/softbuffer choice, so keep the public option harmless here.
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
    let gpu_instance_handle = any_gpu.then(|| std::thread::spawn(GpuState::create_instance));

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
    };
    #[cfg(not(target_arch = "wasm32"))]
    let mut handler = handler;
    #[cfg(not(target_arch = "wasm32"))]
    event_loop
        .run_app(&mut handler)
        .expect("event loop exited with an error");
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(handler);
    }
}

/// Builds one window's pre-creation state (signals, frame buffer, reactive
/// effect) — everything that doesn't depend on the winit `Window` actually
/// existing yet. `resumed` finishes the job once the event loop starts.
fn build_window_spec(
    index: usize,
    spec: PendingWindow,
    dump_frame_path: Option<&str>,
    multiple_windows: bool,
) -> WindowSpec {
    let PendingWindow {
        options,
        clear_color,
        on_window_ready,
        build_ui,
    } = spec;

    let viewport = Signal::new(Size {
        width: options.width as f32,
        height: options.height as f32,
    });
    let scale_factor = Signal::new(1.0f64);
    let frame = Rc::new(RefCell::new(FrameState {
        painter: SkiaPainter::new(options.width, options.height),
        renderer: Renderer::new(),
        scene: None,
        devtools: devtools_for_new_window(),
    }));
    let shared_window: SharedWindow = Rc::new(RefCell::new(None));
    let window_drag = WindowDragHandle::new({
        let window = shared_window.clone();
        move || {
            if let Some(window) = window.borrow().as_ref() {
                let _ = window.drag_window();
            }
        }
    });
    let close_requested = Rc::new(Cell::new(false));
    let theme_provider = ThemeProvider::new(options.theme);
    let build_ui: Rc<dyn Fn(Size) -> BoxedWidget> = Rc::from(build_ui);
    let focused: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let caret_visible: Rc<Cell<bool>> = Rc::new(Cell::new(true));

    // With multiple windows sharing one `CUI_DUMP_FRAME` path, suffix each
    // window's dump with its index rather than having every window's
    // repaint clobber the same file.
    let dump_frame_path: Option<String> = dump_frame_path.map(|path| {
        if multiple_windows {
            format!("{path}.{index}")
        } else {
            path.to_string()
        }
    });

    let dirty = Rc::new(Cell::new(false));
    // Tree built eagerly by `repaint`, consumed by the next `render`.
    let pending_root: Rc<RefCell<Option<BoxedWidget>>> = Rc::new(RefCell::new(None));

    // The expensive half of a frame. It is deliberately separate from
    // `repaint` below: pointer input may invalidate a UI dozens of times
    // before the compositor is ready for its next frame.
    let render: Rc<dyn Fn()> = Rc::new({
        let viewport = viewport.clone();
        let scale_factor = scale_factor.clone();
        let frame = frame.clone();
        let window = shared_window.clone();
        let build_ui = build_ui.clone();
        let focused = focused.clone();
        let caret_visible = caret_visible.clone();
        let dirty = dirty.clone();
        let pending_root = pending_root.clone();
        let theme_provider = theme_provider.clone();
        let window_drag = window_drag.clone();
        move || {
            with_theme_scope(&theme_provider, &window_drag, || {
                dirty.set(false);
                // Widgets are laid out in logical pixels; the painter (and the
                // presenter it feeds) is sized in physical pixels so HiDPI
                // displays stay crisp — see `SkiaPainter`'s doc comment.
                let logical_size = viewport.peek();
                let scale = scale_factor.peek();
                // `repaint` usually already built this; fall back for
                // non-signal-driven redraws (animation ticks, caret blink).
                let root = pending_root
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| build_ui_with_recovery(&build_ui, logical_size));

                let mut frame = frame.borrow_mut();
                let FrameState {
                    painter, renderer, ..
                } = &mut *frame;
                let physical_width = (logical_size.width as f64 * scale).round() as u32;
                let physical_height = (logical_size.height as f64 * scale).round() as u32;
                painter.set_scale(scale as f32);
                painter.set_color_scheme(theme_provider.get().colors);
                painter.resize(physical_width, physical_height);
                painter.clear(clear_color);
                let paint_started = Instant::now();
                let scene = renderer.render_focused(
                    root,
                    logical_size,
                    painter,
                    focused.get(),
                    caret_visible.get(),
                );
                let paint_duration = paint_started.elapsed();
                frame.scene = Some(scene);
                let FrameState {
                    painter, devtools, ..
                } = &mut *frame;
                if let Some(devtools) = devtools.as_mut() {
                    devtools.after_paint(painter, logical_size, paint_duration);
                }

                // Debug aid: dump each painted frame to a PNG on disk, e.g. for
                // headless verification where no on-screen compositor is available.
                if let Some(path) = &dump_frame_path {
                    if let Err(err) = frame.painter.pixmap.save_png(path) {
                        log::warn!(
                            "creamui-render: failed to write CUI_DUMP_FRAME to {path}: {err}"
                        );
                    }
                }
                drop(frame);

                if let Some(window) = window.borrow().as_ref() {
                    window.request_redraw();
                }
            })
        }
    });

    let repaint_scene: Rc<dyn Fn()> = Rc::new({
        let viewport = viewport.clone();
        let scale_factor = scale_factor.clone();
        let frame = frame.clone();
        let window = shared_window.clone();
        let focused = focused.clone();
        let caret_visible = caret_visible.clone();
        let theme_provider = theme_provider.clone();
        let window_drag = window_drag.clone();
        move || {
            with_theme_scope(&theme_provider, &window_drag, || {
                let logical_size = viewport.peek();
                let scale = scale_factor.peek();
                let mut frame = frame.borrow_mut();
                let physical_width = (logical_size.width as f64 * scale).round() as u32;
                let physical_height = (logical_size.height as f64 * scale).round() as u32;
                frame.painter.set_scale(scale as f32);
                frame.painter.set_color_scheme(theme_provider.get().colors);
                frame.painter.resize(physical_width, physical_height);
                frame.painter.clear(clear_color);
                let FrameState {
                    painter, renderer, ..
                } = &mut *frame;
                if let Some(scene) =
                    renderer.repaint_focused(painter, focused.get(), caret_visible.get())
                {
                    frame.scene = Some(scene);
                }
                let FrameState {
                    painter, devtools, ..
                } = &mut *frame;
                if let Some(devtools) = devtools.as_ref() {
                    devtools.repaint_overlay(painter, logical_size);
                }
                drop(frame);
                if let Some(window) = window.borrow().as_ref() {
                    window.request_redraw();
                }
            })
        }
    });

    let animated_damage: Rc<RefCell<Vec<Rect>>> = Rc::new(RefCell::new(Vec::new()));

    // Driven by `about_to_wait`'s animation tick: repaints only whatever
    // widgets are currently promoted to their own layer, with no rebuild,
    // no layout, and no full-window clear. Doesn't touch `frame.scene` —
    // a pure animation tick changes no interactive geometry, so the last
    // full render's `Scene` stays valid.
    let repaint_animated: Rc<dyn Fn()> = Rc::new({
        let frame = frame.clone();
        let window = shared_window.clone();
        let focused = focused.clone();
        let caret_visible = caret_visible.clone();
        let theme_provider = theme_provider.clone();
        let window_drag = window_drag.clone();
        let animated_damage = animated_damage.clone();
        move || {
            with_theme_scope(&theme_provider, &window_drag, || {
                let mut frame = frame.borrow_mut();
                let FrameState {
                    painter, renderer, ..
                } = &mut *frame;
                let damage = renderer.repaint_animated(painter, focused.get(), caret_visible.get());
                drop(frame);
                if damage.is_empty() {
                    return;
                }
                animated_damage.borrow_mut().extend(damage);
                if let Some(window) = window.borrow().as_ref() {
                    window.request_redraw();
                }
            })
        }
    });

    // `create_effect` wraps this, so it must call `build_ui` itself, right
    // here, to stay subscribed to whatever `Signal`s the active branch
    // reads — a closure that only flips `dirty` for `render` to build later
    // reads no `Signal` and de-subscribes the effect from everything after
    // its first run. Layout/paint stay deferred through `dirty`.
    let repaint: Rc<dyn Fn()> = Rc::new({
        let viewport = viewport.clone();
        let build_ui = build_ui.clone();
        let pending_root = pending_root.clone();
        let render = render.clone();
        let window = shared_window.clone();
        let dirty = dirty.clone();
        let theme_provider = theme_provider.clone();
        let window_drag = window_drag.clone();
        move || {
            with_theme_scope(&theme_provider, &window_drag, || {
                let logical_size = viewport.peek();
                *pending_root.borrow_mut() = Some(build_ui_with_recovery(&build_ui, logical_size));
                // The first reactive run happens before winit has created the
                // window, so render immediately to provide its initial frame.
                // Afterwards merely mark dirty and let RedrawRequested coalesce
                // all input updates into one layout/paint pass.
                if let Some(window) = window.borrow().as_ref() {
                    if !dirty.replace(true) {
                        window.request_redraw();
                    }
                } else {
                    render();
                }
            })
        }
    });

    let scene_dirty = Rc::new(Cell::new(false));

    // Paint-only counterpart to `repaint`: no rebuild, no layout.
    let repaint_light: Rc<dyn Fn()> = Rc::new({
        let repaint_scene = repaint_scene.clone();
        let window = shared_window.clone();
        let dirty = dirty.clone();
        let scene_dirty = scene_dirty.clone();
        move || {
            if let Some(window) = window.borrow().as_ref() {
                if !dirty.get() && !scene_dirty.replace(true) {
                    window.request_redraw();
                }
            } else {
                repaint_scene();
            }
        }
    });

    let effect_repaint = repaint.clone();
    let effect = create_effect(move || effect_repaint());

    WindowSpec {
        options,
        on_window_ready,
        repaint,
        repaint_scene,
        repaint_light,
        repaint_animated,
        render,
        dirty,
        scene_dirty,
        animated_damage,
        _effect: effect,
        viewport,
        scale_factor,
        frame,
        shared_window,
        close_requested,
        theme_provider,
        focused,
        caret_visible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use winit::event::DeviceId;

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
                    clear_color: Color::rgba(0, 0, 0, 255),
                    on_window_ready: Box::new(|_| {}),
                    build_ui: Box::new(build_ui),
                },
                None,
                false,
            );
            WindowEventHarness {
                state: WindowState {
                    viewport: spec.viewport,
                    scale_factor: spec.scale_factor,
                    frame: spec.frame,
                    window: spec.shared_window,
                    close_requested: spec.close_requested,
                    close_behavior: CloseBehavior::Close,
                    frameless_resizable: false,
                    presenter: None,
                    pointer_pos: Point::default(),
                    modifiers: ModifiersState::default(),
                    focused: spec.focused,
                    caret_visible: spec.caret_visible,
                    next_blink: Instant::now() + CARET_BLINK_INTERVAL,
                    next_animation: Instant::now(),
                    next_resize_render: Instant::now(),
                    resize_pending: false,
                    pending_viewport: None,
                    current_cursor: CursorIcon::Default,
                    hovered: None,
                    dragging: None,
                    repaint: spec.repaint,
                    repaint_scene: spec.repaint_scene,
                    repaint_light: spec.repaint_light,
                    repaint_animated: spec.repaint_animated,
                    render: spec.render,
                    dirty: spec.dirty,
                    scene_dirty: spec.scene_dirty,
                    animated_damage: spec.animated_damage,
                    _effect: spec._effect,
                    t_run: Instant::now(),
                    first_present_logged: false,
                },
            }
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
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(20.0, 20.0),
        });
        harness.send(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        });
        harness.key(KeyInput {
            key: Key::Char('x'),
            modifiers: Modifiers::default(),
        });

        assert_eq!(clicks.get(), 1);
        assert_eq!(keys.get(), "x");
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
    fn frame_scope_is_available_to_lazy_children() {
        let _harness = WindowEventHarness::new(|_| Box::new(ThemeInChildren));
    }

    // `EventLoop::build()` refuses to run outside the main thread, which
    // `cargo test` never is — the X11 `any_thread` escape hatch is the only
    // way to get a real `EventLoopProxy` here, and winit allows only one
    // `EventLoop` per process ever, so the (leaked) loop behind it is
    // shared across every test that needs a proxy. This only proves the
    // registration/dispatch logic below; the hop through a real running
    // `ActiveEventLoop`'s `user_event` is pre-existing winit machinery
    // already exercised by the tray feature, not re-tested here.
    #[cfg(target_os = "linux")]
    fn test_proxy() -> EventLoopProxy<AppEvent> {
        use winit::platform::x11::EventLoopBuilderExtX11;
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
