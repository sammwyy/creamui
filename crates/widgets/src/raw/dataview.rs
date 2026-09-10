use super::*;
use crate::ScrollController;

fn fill_style() -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Percent(1.0),
            height: creamui_core::layout::Dimension::Percent(1.0),
        },
        ..Default::default()
    }
}

/// An unstyled, scrollable stack of arbitrary rows — the primitive
/// underneath an HTML `<ul>`/React `.map(row => …)` list: you build each
/// row widget yourself (however it needs to look) and hand the finished
/// list to [`RawListView::row`]/[`RawListView::rows`]; this only stacks
/// them, optionally draws a divider between consecutive rows, and scrolls
/// (with the same draggable thumb as [`crate::RawScrollView`], which this
/// wraps). For a single-select list of plain text, see
/// [`crate::themed::ListBox`] instead; for column-based data, see
/// [`RawTable`].
pub struct RawListView {
    pub style: Style,
    pub scroll: ScrollController,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub divider_color: Option<Color>,
    pub divider_width: f32,
    pub rows: Vec<BoxedWidget>,
}

impl RawListView {
    pub fn new(style: Style, scroll: ScrollController) -> Self {
        RawListView {
            style,
            scroll,
            background: None,
            corner_radius: 0.0,
            divider_color: None,
            divider_width: 1.0,
            rows: Vec::new(),
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn divider(mut self, color: Color, width: f32) -> Self {
        self.divider_color = Some(color);
        self.divider_width = width.max(0.0);
        self
    }

    pub fn row(mut self, widget: BoxedWidget) -> Self {
        self.rows.push(widget);
        self
    }

    pub fn rows(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.rows = widgets;
        self
    }
}

impl Widget for RawListView {
    fn style(&self) -> creamui_core::Style {
        Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..self.style.clone()
        }
        .into()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let rows = std::mem::take(&mut self.rows);
        let count = rows.len();
        let mut stacked: Vec<BoxedWidget> = Vec::with_capacity(count * 2);
        for (index, row) in rows.into_iter().enumerate() {
            stacked.push(row);
            if let Some(color) = self.divider_color {
                if index + 1 < count {
                    stacked.push(Box::new(
                        RawView::new(Style {
                            size: creamui_core::layout::Size {
                                width: creamui_core::layout::Dimension::Percent(1.0),
                                height: creamui_core::layout::Dimension::Length(self.divider_width),
                            },
                            flex_shrink: 0.0,
                            ..Default::default()
                        })
                        .background(color),
                    ));
                }
            }
        }
        let mut scroll_view = RawScrollView::controlled(fill_style(), self.scroll.clone())
            .corner_radius(self.corner_radius);
        if let Some(color) = self.background {
            scroll_view = scroll_view.background(color);
        }
        vec![Box::new(scroll_view.with_children(stacked))]
    }
}

/// A column definition for [`RawTable`]: a header label and a fixed pixel
/// width shared by the header cell and every row's cell in that column.
#[derive(Clone)]
pub struct TableColumn {
    pub label: String,
    pub width: f32,
}

impl TableColumn {
    pub fn new(label: impl Into<String>, width: f32) -> Self {
        Self {
            label: label.into(),
            width: width.max(1.0),
        }
    }
}

fn cell_style(width: f32, padding: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Length(width),
            height: creamui_core::layout::Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        align_items: Some(creamui_core::layout::AlignItems::Center),
        padding: creamui_core::layout::Rect {
            left: creamui_core::layout::LengthPercentage::Length(padding),
            right: creamui_core::layout::LengthPercentage::Length(padding),
            top: creamui_core::layout::LengthPercentage::Length(0.0),
            bottom: creamui_core::layout::LengthPercentage::Length(0.0),
        },
        ..Default::default()
    }
}

fn row_container_style(height: f32) -> Style {
    Style {
        display: creamui_core::layout::Display::Flex,
        flex_direction: creamui_core::layout::FlexDirection::Row,
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Auto,
            height: creamui_core::layout::Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// An unstyled, column-aligned data grid over plain string cells — a CSV
/// viewer's shape: fixed-width columns with a header row, and a scrollable
/// body of rows underneath it (the header itself never scrolls). Rows are
/// optionally clickable via [`RawTable::on_row_click`] for row selection.
///
/// Every row is laid out and painted every frame regardless of whether it's
/// currently visible — there is no virtualization, so this is meant for
/// hundreds, not hundreds of thousands, of rows.
pub struct RawTable {
    pub style: Style,
    pub scroll: ScrollController,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<Vec<String>>,
    pub header_height: f32,
    pub row_height: f32,
    pub cell_padding: f32,
    pub header_background: Option<Color>,
    pub header_text_color: Color,
    pub cell_text_color: Color,
    pub row_background: Option<Color>,
    pub alt_row_background: Option<Color>,
    pub selected_row_background: Option<Color>,
    pub divider_color: Option<Color>,
    pub selected_row: Option<usize>,
    pub on_row_click: Option<Rc<dyn Fn(usize)>>,
}

impl RawTable {
    pub fn new(style: Style, scroll: ScrollController, columns: Vec<TableColumn>) -> Self {
        RawTable {
            style,
            scroll,
            columns,
            rows: Vec::new(),
            header_height: 32.0,
            row_height: 28.0,
            cell_padding: 8.0,
            header_background: None,
            header_text_color: Color::rgb(0, 0, 0),
            cell_text_color: Color::rgb(0, 0, 0),
            row_background: None,
            alt_row_background: None,
            selected_row_background: None,
            divider_color: None,
            selected_row: None,
            on_row_click: None,
        }
    }

    pub fn row(mut self, cells: Vec<String>) -> Self {
        self.rows.push(cells);
        self
    }

    pub fn rows(mut self, rows: Vec<Vec<String>>) -> Self {
        self.rows = rows;
        self
    }

    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height.max(1.0);
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(1.0);
        self
    }

    pub fn cell_padding(mut self, padding: f32) -> Self {
        self.cell_padding = padding.max(0.0);
        self
    }

    pub fn header_background(mut self, color: Color) -> Self {
        self.header_background = Some(color);
        self
    }

    pub fn header_text_color(mut self, color: Color) -> Self {
        self.header_text_color = color;
        self
    }

    pub fn cell_text_color(mut self, color: Color) -> Self {
        self.cell_text_color = color;
        self
    }

    pub fn row_background(mut self, color: Color) -> Self {
        self.row_background = Some(color);
        self
    }

    pub fn alt_row_background(mut self, color: Color) -> Self {
        self.alt_row_background = Some(color);
        self
    }

    pub fn selected_row_background(mut self, color: Color) -> Self {
        self.selected_row_background = Some(color);
        self
    }

    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    /// Makes rows clickable: `selected` (if any) highlights that row with
    /// [`RawTable::selected_row_background`], and clicking any row calls
    /// `on_click` with its index.
    pub fn on_row_click(
        mut self,
        selected: Option<usize>,
        on_click: impl Fn(usize) + 'static,
    ) -> Self {
        self.selected_row = selected;
        self.on_row_click = Some(Rc::new(on_click));
        self
    }

    fn build_header(&self) -> BoxedWidget {
        let mut header = RawView::new(row_container_style(self.header_height));
        if let Some(color) = self.header_background {
            header = header.background(color);
        }
        for column in &self.columns {
            header = header.child(Box::new(
                RawText::new(column.label.clone(), self.header_text_color, 13.0)
                    .align(TextAlign::Start)
                    .layout_style(cell_style(column.width, self.cell_padding)),
            ));
        }
        Box::new(header)
    }

    fn build_row(&self, index: usize, cells: &[String]) -> BoxedWidget {
        let selected = self.selected_row == Some(index);
        let background = if selected {
            self.selected_row_background.or(self.row_background)
        } else if index % 2 == 1 {
            self.alt_row_background.or(self.row_background)
        } else {
            self.row_background
        };
        let style = row_container_style(self.row_height);
        let mut cell_widgets = Vec::with_capacity(self.columns.len());
        for (column, text) in self.columns.iter().zip(cells.iter()) {
            cell_widgets.push(Box::new(
                RawText::new(text.clone(), self.cell_text_color, 13.0)
                    .align(TextAlign::Start)
                    .layout_style(cell_style(column.width, self.cell_padding)),
            ) as BoxedWidget);
        }
        if let Some(on_click) = &self.on_row_click {
            let on_click = on_click.clone();
            let mut row =
                RawButton::new(style, move || on_click(index)).with_children(cell_widgets);
            if let Some(color) = background {
                row = row.background(color);
            }
            Box::new(row)
        } else {
            let mut row = RawView::new(style).with_children(cell_widgets);
            if let Some(color) = background {
                row = row.background(color);
            }
            Box::new(row)
        }
    }
}

impl Widget for RawTable {
    fn style(&self) -> creamui_core::Style {
        Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..self.style.clone()
        }
        .into()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children: Vec<BoxedWidget> = vec![self.build_header()];
        if let Some(color) = self.divider_color {
            children.push(Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Percent(1.0),
                        height: creamui_core::layout::Dimension::Length(1.0),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .background(color),
            ));
        }
        let rows = std::mem::take(&mut self.rows);
        let body_rows: Vec<BoxedWidget> = rows
            .iter()
            .enumerate()
            .map(|(index, cells)| self.build_row(index, cells))
            .collect();
        self.rows = rows;
        let body = RawScrollView::controlled(
            Style {
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Percent(1.0),
                    height: creamui_core::layout::Dimension::Percent(1.0),
                },
                ..Default::default()
            },
            self.scroll.clone(),
        )
        .with_children(body_rows);
        children.push(Box::new(body));
        children
    }
}
