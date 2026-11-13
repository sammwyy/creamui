use super::*;
use creamui_core::layout::FlexDirection;
use creamui_reactive::Signal;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type ToastDeadline = std::time::Instant;
#[cfg(target_arch = "wasm32")]
type ToastDeadline = f64;

#[cfg(not(target_arch = "wasm32"))]
fn toast_now() -> ToastDeadline {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn toast_now() -> ToastDeadline {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn toast_deadline(duration: Duration) -> Option<ToastDeadline> {
    toast_now().checked_add(duration)
}

#[cfg(target_arch = "wasm32")]
fn toast_deadline(duration: Duration) -> Option<ToastDeadline> {
    Some(toast_now() + duration.as_secs_f64() * 1000.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastStatus {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
pub struct ToastEntry {
    pub id: u64,
    pub status: ToastStatus,
    pub title: String,
    pub message: String,
    pub action: Option<(String, Rc<dyn Fn()>)>,
    expires: Option<ToastDeadline>,
}

impl ToastEntry {
    pub fn new(
        status: ToastStatus,
        title: impl Into<String>,
        message: impl Into<String>,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            id: 0,
            status,
            title: title.into(),
            message: message.into(),
            action: None,
            expires: duration.and_then(toast_deadline),
        }
    }

    pub fn action(mut self, label: impl Into<String>, callback: impl Fn() + 'static) -> Self {
        self.action = Some((label.into(), Rc::new(callback)));
        self
    }
}

#[derive(Clone)]
pub struct ToastController {
    entries: Signal<Vec<ToastEntry>>,
    next_id: Rc<Cell<u64>>,
}

impl ToastController {
    pub fn new() -> Self {
        Self {
            entries: Signal::new(Vec::new()),
            next_id: Rc::new(Cell::new(0)),
        }
    }

    pub fn push(&self, mut entry: ToastEntry) -> u64 {
        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        entry.id = id;
        #[cfg(target_arch = "wasm32")]
        if let Some(expires) = entry.expires {
            use wasm_bindgen::JsCast;
            let controller = self.clone();
            let timeout = (expires - toast_now()).max(0.0).min(i32::MAX as f64) as i32;
            let callback =
                wasm_bindgen::closure::Closure::once_into_js(move || controller.dismiss(id));
            if let Some(window) = web_sys::window() {
                if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref(),
                    timeout,
                ) {
                    log::warn!("creamui-widgets: cannot schedule toast dismissal: {error:?}");
                }
            }
        }
        self.entries.update(move |entries| {
            if entries.len() >= 6 {
                entries.remove(0);
            }
            entries.push(entry);
        });
        id
    }

    pub fn dismiss(&self, id: u64) {
        if self.entries.peek().iter().any(|entry| entry.id == id) {
            self.entries
                .update(|entries| entries.retain(|entry| entry.id != id));
        }
    }

    pub fn items(&self) -> Vec<ToastEntry> {
        self.entries.get()
    }

    pub fn expire(&self) -> bool {
        let now = toast_now();
        if !self
            .entries
            .peek()
            .iter()
            .any(|entry| entry.expires.is_some_and(|expires| expires <= now))
        {
            return false;
        }
        self.entries.update(|entries| {
            entries.retain(|entry| entry.expires.is_none_or(|expires| expires > now))
        });
        true
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn has_timed_items(&self) -> bool {
        self.entries
            .peek()
            .iter()
            .any(|entry| entry.expires.is_some())
    }
}

impl Default for ToastController {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RawToast {
    pub style: creamui_core::Style,
    body: Option<BoxedWidget>,
    action: Option<(BoxedWidget, Rc<dyn Fn()>)>,
    dismiss: Option<(BoxedWidget, Rc<dyn Fn()>)>,
}

impl RawToast {
    pub fn new(style: impl Into<creamui_core::Style>, body: BoxedWidget) -> Self {
        Self {
            style: style.into(),
            body: Some(body),
            action: None,
            dismiss: None,
        }
    }

    pub fn action(mut self, label: BoxedWidget, callback: impl Fn() + 'static) -> Self {
        self.action = Some((label, Rc::new(callback)));
        self
    }

    pub fn dismiss(mut self, label: BoxedWidget, callback: impl Fn() + 'static) -> Self {
        self.dismiss = Some((label, Rc::new(callback)));
        self
    }
}

impl Widget for RawToast {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        children.extend(self.body.take());
        for (label, callback) in self.action.take().into_iter().chain(self.dismiss.take()) {
            children.push(Box::new(
                RawButton::new(Style::default(), move || callback()).child(label),
            ));
        }
        children
    }
}

type ToastRenderer = dyn Fn(ToastEntry, Rc<dyn Fn()>) -> BoxedWidget;

pub struct RawToastHost {
    pub style: creamui_core::Style,
    controller: ToastController,
    render: Rc<ToastRenderer>,
}

impl RawToastHost {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &ToastController,
        render: impl Fn(ToastEntry, Rc<dyn Fn()>) -> BoxedWidget + 'static,
    ) -> Self {
        let _ = controller.items();
        Self {
            style: style.into(),
            controller: controller.clone(),
            render: Rc::new(render),
        }
    }
}

impl Widget for RawToastHost {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, painter: &mut dyn Painter, _: Rect) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.controller.has_timed_items() {
            painter.animation_time();
            self.controller.expire();
        }
        #[cfg(target_arch = "wasm32")]
        let _ = painter;
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.controller
            .items()
            .into_iter()
            .map(|entry| {
                let id = entry.id;
                let controller = self.controller.clone();
                (self.render)(entry, Rc::new(move || controller.dismiss(id)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_limits_entries_and_supports_dismiss_and_expiry() {
        let controller = ToastController::new();
        for index in 0..7 {
            controller.push(ToastEntry::new(
                ToastStatus::Info,
                format!("Toast {index}"),
                "",
                None,
            ));
        }
        let items = controller.items();
        assert_eq!(items.len(), 6);
        assert_eq!(items[0].title, "Toast 1");
        controller.dismiss(items[0].id);
        assert_eq!(controller.items().len(), 5);
        controller.push(ToastEntry::new(
            ToastStatus::Success,
            "Transient",
            "",
            Some(Duration::ZERO),
        ));
        assert!(controller.expire());
        assert_eq!(controller.items().len(), 5);
    }

    #[test]
    fn host_and_toast_keep_independent_actions() {
        let controller = ToastController::new();
        let activated = Rc::new(Cell::new(false));
        let target = activated.clone();
        controller.push(
            ToastEntry::new(ToastStatus::Warning, "Needs review", "", None)
                .action("Review", move || target.set(true)),
        );
        let mut host = RawToastHost::new(Style::default(), &controller, |entry, dismiss| {
            let mut toast = RawToast::new(
                Style::default(),
                Box::new(RawText::new(entry.title, Color::rgb(0, 0, 0), 14.0)),
            )
            .dismiss(
                Box::new(RawText::new("x", Color::rgb(0, 0, 0), 14.0)),
                move || dismiss(),
            );
            if let Some((label, action)) = entry.action {
                toast = toast.action(
                    Box::new(RawText::new(label, Color::rgb(0, 0, 0), 14.0)),
                    move || action(),
                );
            }
            Box::new(toast)
        });
        let mut items = host.children();
        let buttons = items[0].children();
        buttons[1].on_click().unwrap()();
        assert!(activated.get());
        assert_eq!(controller.items().len(), 1);
        buttons[2].on_click().unwrap()();
        assert!(controller.items().is_empty());
    }
}
