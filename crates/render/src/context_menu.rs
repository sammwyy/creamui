use crate::{
    platform::PopupPlacement, BlurRegion, PopupOptions, RenderBackend, WindowHandle, WindowOptions,
};
use creamui_core::{BoxedWidget, Key, Rect, Size};
use creamui_reactive::{batch, create_effect, untrack, Effect, Signal};
use creamui_theme::{Color, Theme};
use creamui_widgets::{ContextMenu, ContextMenuController, ContextMenuItem, ContextMenuPane};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub struct ContextMenuHost {
    inner: Rc<HostState>,
}
struct HostState {
    controller: ContextMenuController,
    native: Signal<bool>,
    session: RefCell<Option<Session>>,
    next_token: Cell<u64>,
    effect: RefCell<Option<Effect>>,
}
struct Session {
    token: u64,
    root: PopupOptions,
    theme: Theme,
    panes: Vec<PaneSlot>,
}
struct PaneSlot {
    token: u64,
    path: Vec<usize>,
    anchor: Option<Rect>,
    window: Option<WindowHandle>,
}
impl Session {
    fn truncate(&mut self, count: usize) {
        for pane in self.panes.drain(count..).rev() {
            if let Some(window) = pane.window {
                window.close();
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.truncate(0);
    }
}
struct PopupRequest {
    session: u64,
    token: u64,
    path: Vec<usize>,
    popup: PopupOptions,
    options: WindowOptions,
}
impl Default for ContextMenuHost {
    fn default() -> Self {
        let inner = Rc::new(HostState {
            controller: ContextMenuController::new(),
            native: Signal::new(false),
            session: RefCell::new(None),
            next_token: Cell::new(0),
            effect: RefCell::new(None),
        });
        let weak = Rc::downgrade(&inner);
        let effect = create_effect(move || {
            if let Some(inner) = weak.upgrade() {
                let panes = inner.controller.visible_panes();
                let native = inner.native.get();
                untrack(|| inner.reconcile(panes, native));
            }
        });
        *inner.effect.borrow_mut() = Some(effect);
        Self { inner }
    }
}
impl ContextMenuHost {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn controller(&self) -> ContextMenuController {
        self.inner.controller.clone()
    }
    pub fn open(&self, popup: PopupOptions, items: Vec<ContextMenuItem>) {
        self.close();
        if items.is_empty() {
            return;
        }
        let native = popup.supports_native_popup();
        let anchor = creamui_core::Point {
            x: popup.anchor.x,
            y: popup.anchor.y,
        };
        let token = self.inner.token();
        let theme = popup.parent.theme();
        *self.inner.session.borrow_mut() = Some(Session {
            token,
            root: popup,
            theme,
            panes: Vec::new(),
        });
        batch(|| {
            self.inner.native.set(native);
            self.inner.controller.open(anchor, items);
        });
    }
    pub fn close(&self) {
        self.inner.controller.close();
    }
    pub fn is_open(&self) -> bool {
        self.inner.controller.is_open()
    }
    pub fn uses_native_popup(&self) -> bool {
        self.is_open() && self.inner.native.get()
    }
    pub fn parent_focus_lost(&self) {
        if !self.uses_native_popup() {
            self.close();
        }
    }
    pub fn handle_key(&self, key: &Key) -> bool {
        self.inner.controller.handle_key(key)
    }
    pub fn overlay(&self, viewport: Size) -> Option<BoxedWidget> {
        (self.is_open() && !self.inner.native.get())
            .then(|| Box::new(ContextMenu::new(&self.inner.controller, viewport)) as BoxedWidget)
    }
}
impl HostState {
    fn token(&self) -> u64 {
        let token = self.next_token.get().wrapping_add(1);
        self.next_token.set(token);
        token
    }
    fn current(&self, session: u64, token: u64) -> bool {
        self.native.peek()
            && self.controller.is_open()
            && self.session.borrow().as_ref().is_some_and(|current| {
                current.token == session && current.panes.iter().any(|pane| pane.token == token)
            })
    }
    fn parent_closed(&self) -> bool {
        self.session
            .borrow()
            .as_ref()
            .is_none_or(|session| session.root.parent.is_closed())
    }
    fn synchronize(self: &Rc<Self>) {
        untrack(|| self.reconcile(self.controller.visible_panes(), self.native.peek()));
    }
    fn reconcile(self: &Rc<Self>, desired: Vec<ContextMenuPane>, native: bool) {
        if desired.is_empty() {
            let session = self.session.borrow_mut().take();
            drop(session);
            return;
        }
        if !native {
            if let Some(session) = self.session.borrow_mut().as_mut() {
                session.truncate(0);
            }
            return;
        }
        let request = {
            let mut slot = self.session.borrow_mut();
            let Some(session) = slot.as_mut() else {
                return;
            };
            let shared = session
                .panes
                .iter()
                .zip(&desired)
                .take_while(|(current, wanted)| {
                    current.path == wanted.path && current.anchor == wanted.anchor
                })
                .count();
            session.truncate(shared);
            let Some(pane) = desired.get(shared) else {
                return;
            };
            let popup = if shared == 0 {
                session.root.clone()
            } else {
                let Some(parent) = session.panes[shared - 1]
                    .window
                    .as_ref()
                    .filter(|parent| parent.is_ready())
                    .cloned()
                else {
                    return;
                };
                let size = parent.logical_size();
                let Some(anchor) = pane.anchor.and_then(|rect| {
                    rect.intersect(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: size.width,
                        height: size.height,
                    })
                }) else {
                    return;
                };
                PopupOptions::new(parent, anchor)
                    .placed(PopupPlacement::RightTop)
                    .with_input_serial(session.root.input_serial.expect("native menu input serial"))
            };
            let Some(size) = ContextMenu::pane_size(&self.controller, &pane.path, &session.theme)
            else {
                return;
            };
            let token = self.token();
            session.panes.push(PaneSlot {
                token,
                path: pane.path.clone(),
                anchor: pane.anchor,
                window: None,
            });
            PopupRequest {
                session: session.token,
                token,
                path: pane.path.clone(),
                popup,
                options: WindowOptions {
                    title: "Context menu".into(),
                    width: size.width.ceil() as u32,
                    height: size.height.min(640.0 * session.theme.text_scale).ceil() as u32,
                    decorations: false,
                    resizable: false,
                    transparent: true,
                    blur: Some(BlurRegion::Content),
                    focus_first: true,
                    backend: RenderBackend::Cpu,
                    theme: session.theme,
                    ..Default::default()
                },
            }
        };
        self.queue(request);
    }
    fn queue(self: &Rc<Self>, request: PopupRequest) {
        let PopupRequest {
            session,
            token,
            path,
            popup,
            options,
        } = request;
        let current = Rc::downgrade(self);
        let failed = Rc::downgrade(self);
        let ready = Rc::downgrade(self);
        let controller = self.controller.clone();
        let build_path = path.clone();
        let app = popup.parent.app();
        let popup = popup.resize_to_fit(true).while_current(move || {
            let Some(host) = current
                .upgrade()
                .filter(|host| host.current(session, token))
            else {
                return false;
            };
            if host.parent_closed() {
                host.controller.close();
                return false;
            }
            true
        });
        let popup = popup.on_error(move |error| {
            if let Some(host) = failed.upgrade().filter(|host| host.current(session, token)) {
                log::debug!("creamui-render: context menu popup failed: {error}");
                if host.parent_closed() {
                    host.controller.close();
                } else {
                    host.native.set(false);
                }
            }
        });
        app.append_popup(
            options,
            popup,
            Color::rgba(0, 0, 0, 0),
            move |window| {
                let Some(host) = ready.upgrade().filter(|host| host.current(session, token)) else {
                    window.close();
                    return;
                };
                let keys = host.controller.clone();
                window.on_keyboard_input(move |event, _| {
                    if !keys.is_open() {
                        return false;
                    }
                    if event.pressed {
                        if let Some(key) = crate::window::translate_key(&event.key) {
                            keys.handle_key(&key);
                        }
                    }
                    true
                });
                let closed = Rc::downgrade(&host);
                window.on_close_requested(move || {
                    if let Some(host) = closed.upgrade().filter(|host| host.current(session, token))
                    {
                        if path.is_empty() {
                            host.controller.close();
                        } else {
                            host.controller.close_submenu(&path);
                        }
                    }
                    true
                });
                {
                    let mut slot = host.session.borrow_mut();
                    if let Some(pane) = slot.as_mut().and_then(|session| {
                        session.panes.iter_mut().find(|pane| pane.token == token)
                    }) {
                        pane.window = Some(window);
                    }
                }
                host.synchronize();
            },
            move |size| {
                Box::new(ContextMenu::pane(&controller, build_path.clone(), size)) as BoxedWidget
            },
        );
    }
}
