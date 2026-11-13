use super::*;
use crate::raw::RawResizablePanels;

pub struct ResizablePanels {
    inner: RawResizablePanels,
}

impl Styled for ResizablePanels {
    fn set_style(&mut self, style: creamui_core::Style) {
        let merged = self.inner.style().merged_over(style);
        self.inner.set_style(merged);
    }
}

impl ResizablePanels {
    pub fn new(axis: SplitAxis, states: Vec<SplitState>) -> Self {
        let theme = use_theme();
        Self {
            inner: RawResizablePanels::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(150.0),
                    },
                    ..Default::default()
                },
                axis,
                states,
            )
            .divider(6.0, theme.border_strong),
        }
    }

    pub fn pane(mut self, content: BoxedWidget, min: f32, max: Option<f32>) -> Self {
        self.inner = self.inner.pane(content, min, max);
        self
    }
}

impl Widget for ResizablePanels {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_resizable_panels_mount_nested_splits() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let pane = || Box::new(RawView::new(Style::default())) as BoxedWidget;
            let mut panels = ResizablePanels::new(
                SplitAxis::Horizontal,
                vec![SplitState::new(0.4), SplitState::new(0.5)],
            )
            .pane(pane(), 40.0, None)
            .pane(pane(), 50.0, None)
            .pane(pane(), 60.0, None);
            assert_eq!(panels.children().len(), 1);
        });
    }
}
