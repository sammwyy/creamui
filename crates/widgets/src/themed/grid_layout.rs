use super::*;
use crate::layout::StyleExt;
use crate::raw::{GridController, RawGridLayout};

pub struct GridLayout {
    inner: RawGridLayout,
}
impl_styled_inner!(GridLayout);

impl GridLayout {
    pub fn new(controller: &GridController, columns: usize) -> Self {
        Self {
            inner: RawGridLayout::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    ..Default::default()
                },
                controller,
                columns,
            ),
        }
    }

    pub fn item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        let theme = use_theme();
        let content = RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            ..Style::default().padding_all(theme.spacing_small)
        })
        .background(theme.surface_hover)
        .border(theme.border, 1.0)
        .corner_radius(theme.radius_small)
        .child(Box::new(RawText::new(
            label,
            theme.text_primary,
            theme.typography.body,
        )));
        self.inner = self.inner.item(id, Box::new(content));
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.inner = self.inner.row_height(height);
        self
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.inner = self.inner.locked(locked);
        self
    }
}

impl Widget for GridLayout {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn reports_layout(&self) -> bool {
        self.inner.reports_layout()
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.inner.on_layout(rect)
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::GridCell;

    #[test]
    fn themed_grid_mounts_controlled_cells() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = GridController::new(vec![GridCell::new("files", 0, 0)]);
            let mut grid = GridLayout::new(&controller, 2).item("files", "Files");
            assert_eq!(grid.children().len(), 1);
        });
    }
}
