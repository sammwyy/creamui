use super::*;
use crate::ScrollController;
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentageAuto, Position};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetEdge {
    Left,
    Right,
    Bottom,
}

struct SheetPanel {
    style: creamui_core::Style,
    content: Option<BoxedWidget>,
    header: Option<BoxedWidget>,
    scroll: ScrollController,
    dismiss: Rc<dyn Fn()>,
}

impl Widget for SheetPanel {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn blocks_pointer(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(Rc::new(|| {}))
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let dismiss = self.dismiss.clone();
        Some(Rc::new(move |input| {
            if input.key == Key::Escape {
                dismiss();
            }
        }))
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children: Vec<BoxedWidget> = self.header.take().into_iter().collect();
        if let Some(content) = self.content.take() {
            children.push(Box::new(
                RawScrollView::controlled(
                    Style {
                        flex_grow: 1.0,
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Length(0.0),
                        },
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        ..Default::default()
                    },
                    self.scroll.clone(),
                )
                .child(content),
            ));
        }
        children
    }
}

pub struct RawSheet {
    pub style: creamui_core::Style,
    panel_style: creamui_core::Style,
    edge: SheetEdge,
    extent: f32,
    modal: bool,
    scrim: Color,
    content: Option<BoxedWidget>,
    header: Option<BoxedWidget>,
    scroll: ScrollController,
    dismiss: Rc<dyn Fn()>,
}

impl RawSheet {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        scroll: ScrollController,
        edge: SheetEdge,
        content: BoxedWidget,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            panel_style: Style::default().into(),
            edge,
            extent: 320.0,
            modal: true,
            scrim: Color::rgba(0, 0, 0, 110),
            content: Some(content),
            header: None,
            scroll,
            dismiss: Rc::new(on_dismiss),
        }
    }

    pub fn header(mut self, header: BoxedWidget) -> Self {
        self.header = Some(header);
        self
    }

    pub fn panel_style(mut self, style: impl Into<creamui_core::Style>) -> Self {
        self.panel_style = style.into();
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = extent.max(1.0);
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn scrim(mut self, scrim: Color) -> Self {
        self.scrim = scrim;
        self
    }
}

impl Widget for RawSheet {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.position = Position::Absolute;
        style.layout.inset = creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Length(0.0),
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Length(0.0),
        };
        style.layout.size = creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        };
        style
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if self.modal {
            painter.fill_rect(rect, self.scrim, 0.0);
        }
    }

    fn is_modal(&self) -> bool {
        self.modal
    }

    fn blocks_pointer(&self) -> bool {
        self.modal
    }

    fn on_modal_dismiss(&self) -> Option<Rc<dyn Fn()>> {
        self.modal.then(|| self.dismiss.clone())
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.modal.then(|| self.dismiss.clone())
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut style = self.panel_style.clone();
        style.layout.position = Position::Absolute;
        style.layout.flex_direction = FlexDirection::Column;
        style.layout.inset = creamui_core::layout::Rect {
            left: if self.edge == SheetEdge::Right {
                LengthPercentageAuto::Auto
            } else {
                LengthPercentageAuto::Length(0.0)
            },
            right: if self.edge == SheetEdge::Left {
                LengthPercentageAuto::Auto
            } else {
                LengthPercentageAuto::Length(0.0)
            },
            top: if self.edge == SheetEdge::Bottom {
                LengthPercentageAuto::Auto
            } else {
                LengthPercentageAuto::Length(0.0)
            },
            bottom: LengthPercentageAuto::Length(0.0),
        };
        style.layout.size = if self.edge == SheetEdge::Bottom {
            creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.extent),
            }
        } else {
            creamui_core::layout::Size {
                width: Dimension::Length(self.extent),
                height: Dimension::Percent(1.0),
            }
        };
        style.layout.max_size = creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        };
        vec![Box::new(SheetPanel {
            style,
            content: self.content.take(),
            header: self.header.take(),
            scroll: self.scroll.clone(),
            dismiss: self.dismiss.clone(),
        })]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Renderer, Size};

    struct TestPainter;

    impl Painter for TestPainter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }

    #[test]
    fn sheet_restricts_modal_focus_and_routes_escape() {
        let dismissed = Rc::new(Cell::new(false));
        let target = dismissed.clone();
        let sheet = RawSheet::new(
            Style::default(),
            ScrollController::new(0.0),
            SheetEdge::Right,
            Box::new(RawView::new(Style::default())),
            move || target.set(true),
        )
        .extent(180.0);
        let mut painter = TestPainter;
        let scene = Renderer::new().render(
            Box::new(sheet),
            Size {
                width: 360.0,
                height: 240.0,
            },
            &mut painter,
        );
        assert!(scene.is_modal());
        assert!(scene.modal_focus().is_some());
        scene.modal_dismiss().unwrap()();
        assert!(dismissed.get());
    }
}
