use super::{layout_container_methods, shrinkable, StyleExt};
use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Painter, Point, Rect, Widget, WindowDragHandle};
use std::rc::Rc;

pub struct CUIWindowDragArea {
    inner: crate::raw::RawView,
    drag: Option<WindowDragHandle>,
}

impl CUIWindowDragArea {
    pub fn new() -> Self {
        Self::with_style(Style::default())
    }

    pub fn with_style(style: Style) -> Self {
        Self {
            inner: crate::raw::RawView::new(shrinkable(style)),
            drag: creamui_reactive::try_use_context(),
        }
    }

    layout_container_methods!();
}

impl Default for CUIWindowDragArea {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for CUIWindowDragArea {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let drag = self.drag.clone()?;
        Some(Rc::new(move |_, _| drag.start_drag()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn starts_the_drag_provided_by_the_window() {
        let started = Rc::new(Cell::new(false));
        let started_by_handle = started.clone();
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(WindowDragHandle::new(move || {
                started_by_handle.set(true);
            }));
            let area = CUIWindowDragArea::new();
            area.on_drag_start().unwrap()(Point::default(), Rect::default());
        });
        assert!(started.get());
    }
}
