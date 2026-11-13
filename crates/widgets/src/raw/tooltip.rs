use super::*;
use creamui_core::layout::{Dimension, LengthPercentageAuto, Position};
use creamui_reactive::Signal;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type TooltipDeadline = std::time::Instant;
#[cfg(target_arch = "wasm32")]
type TooltipDeadline = f64;

#[cfg(not(target_arch = "wasm32"))]
fn tooltip_deadline(delay: Duration) -> TooltipDeadline {
    std::time::Instant::now() + delay
}

#[cfg(target_arch = "wasm32")]
fn tooltip_deadline(delay: Duration) -> TooltipDeadline {
    js_sys::Date::now() + delay.as_secs_f64() * 1000.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipPosition {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone)]
pub struct TooltipController {
    visible: Signal<bool>,
    armed: Signal<bool>,
    hovered: Rc<Cell<bool>>,
    focused: Rc<Cell<bool>>,
    pending: Rc<Cell<Option<TooltipDeadline>>>,
    generation: Rc<Cell<u64>>,
    anchor: Rc<Cell<Rect>>,
    viewport: Rc<Cell<creamui_core::Size>>,
    delay: Duration,
}

impl TooltipController {
    pub fn new(delay: Duration) -> Self {
        Self {
            visible: Signal::new(false),
            armed: Signal::new(false),
            hovered: Rc::new(Cell::new(false)),
            focused: Rc::new(Cell::new(false)),
            pending: Rc::new(Cell::new(None)),
            generation: Rc::new(Cell::new(0)),
            anchor: Rc::new(Cell::new(Rect::default())),
            viewport: Rc::new(Cell::new(creamui_core::Size::default())),
            delay,
        }
    }

    pub fn visible(&self) -> bool {
        self.visible.get()
    }

    pub fn set_viewport(&self, viewport: creamui_core::Size) {
        self.viewport.set(viewport);
    }

    pub fn hover(&self, hovered: bool) {
        if self.hovered.replace(hovered) == hovered {
            return;
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        if hovered {
            self.pending.set(Some(tooltip_deadline(self.delay)));
            self.armed.set(true);
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsCast;
                let controller = self.clone();
                let generation = self.generation.get();
                let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
                    if controller.hovered.get() && controller.generation.get() == generation {
                        controller.visible.set(true);
                        controller.armed.set(false);
                    }
                });
                if let Some(window) = web_sys::window() {
                    if let Err(error) = window
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            callback.unchecked_ref(),
                            self.delay.as_millis().min(i32::MAX as u128) as i32,
                        )
                    {
                        log::warn!("creamui-widgets: cannot schedule tooltip: {error:?}");
                    }
                }
            }
        } else {
            self.pending.set(None);
            self.armed.set(false);
            if !self.focused.get() {
                self.visible.set(false);
            }
        }
    }

    pub fn focus(&self, focused: bool) {
        self.focused.set(focused);
        if focused {
            self.pending.set(None);
            self.armed.set(false);
            self.visible.set(true);
        } else if !self.hovered.get() {
            self.visible.set(false);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn tick(&self, painter: &mut dyn Painter) {
        if !self.armed.get() {
            return;
        }
        if let Some(deadline) = self.pending.get() {
            if std::time::Instant::now() >= deadline && self.hovered.get() {
                self.pending.set(None);
                self.armed.set(false);
                self.visible.set(true);
            } else {
                painter.animation_time();
            }
        }
    }
}

impl Default for TooltipController {
    fn default() -> Self {
        Self::new(Duration::from_millis(350))
    }
}

struct TooltipTrigger {
    inner: BoxedWidget,
    controller: TooltipController,
}

impl Widget for TooltipTrigger {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        self.inner.accepts_text_input()
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.controller.anchor.set(rect);
        self.inner.on_layout(rect)
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let on_key = self.inner.on_key();
        let controller = self.controller.clone();
        Some(Rc::new(move |input| {
            if input.key == Key::Escape {
                controller.hover(false);
                controller.focus(false);
            } else if let Some(callback) = &on_key {
                callback(input);
            }
        }))
    }

    fn on_focus_change(&self) -> Option<Rc<dyn Fn(bool)>> {
        let on_focus = self.inner.on_focus_change();
        let controller = self.controller.clone();
        Some(Rc::new(move |focused| {
            controller.focus(focused);
            if let Some(callback) = &on_focus {
                callback(focused);
            }
        }))
    }

    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        let on_hover = self.inner.on_hover();
        let controller = self.controller.clone();
        Some(Rc::new(move |hovered| {
            controller.hover(hovered);
            if let Some(callback) = &on_hover {
                callback(hovered);
            }
        }))
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.inner.on_click_at()
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag_start()
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag()
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_drag_end()
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        self.inner
            .paint_focused_overlay(painter, rect, caret_visible)
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        self.inner.measure()
    }

    fn measure_fingerprint(&self) -> Option<u64> {
        self.inner.measure_fingerprint()
    }
}

pub struct RawTooltip {
    pub style: creamui_core::Style,
    trigger: Option<BoxedWidget>,
    content: Option<BoxedWidget>,
    controller: TooltipController,
    position: TooltipPosition,
    popup_size: creamui_core::Size,
}

impl RawTooltip {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &TooltipController,
        trigger: BoxedWidget,
        content: BoxedWidget,
    ) -> Self {
        let _ = controller.visible();
        let _ = controller.armed.get();
        Self {
            style: style.into(),
            trigger: Some(trigger),
            content: Some(content),
            controller: controller.clone(),
            position: TooltipPosition::Top,
            popup_size: creamui_core::Size {
                width: 200.0,
                height: 44.0,
            },
        }
    }

    pub fn position(mut self, position: TooltipPosition) -> Self {
        self.position = position;
        self
    }

    pub fn content(mut self, content: BoxedWidget) -> Self {
        self.content = Some(content);
        self
    }

    pub fn popup_size(mut self, width: f32, height: f32) -> Self {
        self.popup_size = creamui_core::Size {
            width: width.max(1.0),
            height: height.max(1.0),
        };
        self
    }

    fn popup_offset(&self) -> Point {
        let anchor = self.controller.anchor.get();
        let viewport = self.controller.viewport.get();
        let width = self.popup_size.width;
        let height = self.popup_size.height;
        let gap = 6.0;
        let position = match self.position {
            TooltipPosition::Top if anchor.y < height + gap => TooltipPosition::Bottom,
            TooltipPosition::Bottom
                if anchor.y + anchor.height + height + gap > viewport.height
                    && viewport.height > 0.0 =>
            {
                TooltipPosition::Top
            }
            TooltipPosition::Left if anchor.x < width + gap => TooltipPosition::Right,
            TooltipPosition::Right
                if anchor.x + anchor.width + width + gap > viewport.width
                    && viewport.width > 0.0 =>
            {
                TooltipPosition::Left
            }
            other => other,
        };
        let (x, y) = match position {
            TooltipPosition::Top => (anchor.x, anchor.y - height - gap),
            TooltipPosition::Bottom => (anchor.x, anchor.y + anchor.height + gap),
            TooltipPosition::Left => (
                anchor.x - width - gap,
                anchor.y + (anchor.height - height) / 2.0,
            ),
            TooltipPosition::Right => (
                anchor.x + anchor.width + gap,
                anchor.y + (anchor.height - height) / 2.0,
            ),
        };
        let x = if viewport.width > 0.0 {
            x.clamp(4.0, (viewport.width - width - 4.0).max(4.0))
        } else {
            x
        };
        let y = if viewport.height > 0.0 {
            y.clamp(4.0, (viewport.height - height - 4.0).max(4.0))
        } else {
            y
        };
        Point {
            x: x - anchor.x,
            y: y - anchor.y,
        }
    }
}

impl Widget for RawTooltip {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, _: Rect) {
        #[cfg(not(target_arch = "wasm32"))]
        self.controller.tick(painter);
        #[cfg(target_arch = "wasm32")]
        let _ = painter;
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        if let Some(trigger) = self.trigger.take() {
            children.push(Box::new(TooltipTrigger {
                inner: trigger,
                controller: self.controller.clone(),
            }) as BoxedWidget);
        }
        if self.controller.visible() {
            if let Some(content) = self.content.take() {
                let offset = self.popup_offset();
                children.push(Box::new(
                    RawView::new(Style {
                        position: Position::Absolute,
                        inset: creamui_core::layout::Rect {
                            left: LengthPercentageAuto::Length(offset.x),
                            top: LengthPercentageAuto::Length(offset.y),
                            right: LengthPercentageAuto::Auto,
                            bottom: LengthPercentageAuto::Auto,
                        },
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(self.popup_size.width),
                            height: Dimension::Auto,
                        },
                        ..Default::default()
                    })
                    .child(content),
                ));
            }
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> BoxedWidget {
        Box::new(RawText::new("Help", Color::rgb(0, 0, 0), 14.0))
    }

    #[test]
    fn tooltip_keeps_trigger_actions_and_focus_visibility() {
        let controller = TooltipController::new(Duration::from_secs(10));
        let clicked = Rc::new(Cell::new(false));
        let target = clicked.clone();
        let mut tooltip = RawTooltip::new(
            Style::default(),
            &controller,
            Box::new(RawButton::new(Style::default(), move || target.set(true)).child(slot())),
            slot(),
        );
        let children = tooltip.children();
        assert_eq!(children.len(), 1);
        children[0].on_click().unwrap()();
        assert!(clicked.get());
        children[0].on_focus_change().unwrap()(true);
        assert!(controller.visible());
    }

    #[test]
    fn tooltip_clamps_and_flips_at_viewport_edges() {
        let controller = TooltipController::default();
        controller.set_viewport(creamui_core::Size {
            width: 320.0,
            height: 240.0,
        });
        controller.anchor.set(Rect {
            x: 270.0,
            y: 4.0,
            width: 30.0,
            height: 20.0,
        });
        let tooltip =
            RawTooltip::new(Style::default(), &controller, slot(), slot()).popup_size(150.0, 40.0);
        let offset = tooltip.popup_offset();
        assert_eq!(offset.y, 26.0);
        assert!(offset.x < 0.0);
    }
}
