use super::*;
use crate::raw::{DataTableController, RawDataTable};
use creamui_widgets::{ScrollController, TableColumn, VirtualListState};

pub struct DataTable {
    inner: RawDataTable,
}

impl Styled for DataTable {
    fn set_style(&mut self, style: creamui_core::Style) {
        let merged = self.inner.style().merged_over(style);
        self.inner.set_style(merged);
    }
}

impl DataTable {
    pub fn new(
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        controller: &DataTableController,
        scroll: ScrollController,
        state: VirtualListState,
        viewport_height: f32,
    ) -> Self {
        let theme = use_theme();
        let raw = RawDataTable::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(viewport_height.max(0.0) + 34.0),
                },
                ..Default::default()
            },
            scroll,
            columns,
            rows,
            controller,
            state,
            viewport_height,
        )
        .customize(|table| {
            table.header_height = 34.0;
            table.row_height = 32.0;
            table.header_background = Some(creamui_widgets::elevation::with_alpha(
                theme.text_primary,
                10,
            ));
            table.header_text_color = theme.text_secondary;
            table.cell_text_color = theme.text_primary;
            table.cell_padding = table.cell_padding.max(10.0);
            table.row_background = None;
            table.alt_row_background = Some(creamui_widgets::elevation::with_alpha(
                theme.text_primary,
                6,
            ));
            table.selected_row_background =
                Some(creamui_widgets::elevation::with_alpha(theme.accent, 56));
            table.divider_color = Some(theme.border);
        });
        Self { inner: raw }
    }

    pub fn dense(mut self) -> Self {
        self.inner = self.inner.customize(|table| {
            table.header_height = 28.0;
            table.row_height = 26.0;
            table.cell_padding = 4.0;
            table.style.layout.size.height = match table.style.layout.size.height {
                Dimension::Length(height) => Dimension::Length((height - 6.0).max(0.0)),
                dimension => dimension,
            };
        });
        self
    }
}

impl Widget for DataTable {
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
    fn themed_table_mounts_virtual_header_and_body() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = DataTableController::new(1);
            let mut table = DataTable::new(
                vec![TableColumn::new("Name", 100.0)],
                vec![vec!["Row".to_owned()]],
                &controller,
                ScrollController::new(0.0),
                VirtualListState::new(1, 32.0),
                120.0,
            );
            assert_eq!(table.children().len(), 3);
        });
    }
}
