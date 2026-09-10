//! Widgets for browsing structured data rather than picking one option:
//! [`ListView`] (arbitrary rows), [`TreeView`] (hierarchy), [`Table`]
//! (columns — a CSV viewer's shape). For a plain single-select list, see
//! [`crate::themed::ListBox`] in [`crate::themed::selection`] instead —
//! it's a selection control, not a data view.

use super::*;
use crate::layout::fill;
use crate::{ScrollController, TreeController};
use creamui_core::layout::{AlignItems, Dimension, JustifyContent};
use creamui_core::Key;

/// One node of the tree passed into [`TreeView::new`]. `id` must be unique
/// across the whole tree — it's the key [`TreeController`] tracks expanded
/// and selected state by.
pub struct TreeNode {
    pub id: u64,
    pub label: String,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            children: Vec::new(),
        }
    }

    pub fn child(mut self, node: TreeNode) -> Self {
        self.children.push(node);
        self
    }

    pub fn with_children(mut self, children: Vec<TreeNode>) -> Self {
        self.children = children;
        self
    }
}

struct FlatRow {
    id: u64,
    label: String,
    depth: u32,
    has_children: bool,
    expanded: bool,
    selected: bool,
}

fn flatten(
    nodes: &[TreeNode],
    depth: u32,
    controller: &TreeController,
    selected: Option<u64>,
    out: &mut Vec<FlatRow>,
) {
    for node in nodes {
        let expanded = controller.is_expanded(node.id);
        out.push(FlatRow {
            id: node.id,
            label: node.label.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded,
            selected: selected == Some(node.id),
        });
        if expanded {
            flatten(&node.children, depth + 1, controller, selected, out);
        }
    }
}

/// A scrollable, expandable/collapsible hierarchy — a file tree, a nested
/// category list. State (which nodes are expanded, which one is selected)
/// lives in a [`TreeController`] rather than the widget itself, the same
/// division every other controlled widget in this crate uses.
///
/// The visible rows are flattened out of `nodes` once, in
/// [`TreeView::new`] — not lazily in [`Widget::children`] — so that the
/// controller reads it depends on happen on the caller's own `build_ui`
/// call stack and actually subscribe to future changes; see
/// [`TreeController`]'s doc comment.
pub struct TreeView {
    theme: Theme,
    style: Style,
    scroll: ScrollController,
    controller: TreeController,
    rows: Vec<FlatRow>,
    row_height: f32,
    indent: f32,
}

impl TreeView {
    pub fn new(
        style: Style,
        scroll: ScrollController,
        controller: TreeController,
        nodes: &[TreeNode],
    ) -> Self {
        let theme = use_theme();
        let selected = controller.selected();
        let mut rows = Vec::new();
        flatten(nodes, 0, &controller, selected, &mut rows);
        Self {
            theme,
            style,
            scroll,
            controller,
            rows,
            row_height: 30.0,
            indent: 18.0,
        }
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(1.0);
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent.max(0.0);
        self
    }

    fn build_row(&self, row: &FlatRow) -> BoxedWidget {
        let row_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.row_height),
            },
            flex_shrink: 0.0,
            align_items: Some(AlignItems::Center),
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(
                    8.0 + row.depth as f32 * self.indent,
                ),
                right: creamui_core::layout::LengthPercentage::Length(8.0),
                top: creamui_core::layout::LengthPercentage::Length(0.0),
                bottom: creamui_core::layout::LengthPercentage::Length(0.0),
            },
            ..crate::layout::row(6.0)
        };
        let background = if row.selected {
            self.theme.accent
        } else {
            self.theme.surface_elevated
        };
        let foreground = if row.selected {
            self.theme.selection_text
        } else {
            self.theme.text_primary
        };

        let chevron_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(16.0),
                height: Dimension::Length(16.0),
            },
            flex_shrink: 0.0,
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..Default::default()
        };
        let chevron: BoxedWidget = if row.has_children {
            let controller = self.controller.clone();
            let id = row.id;
            let glyph = if row.expanded { "v" } else { ">" };
            Box::new(
                RawButton::new(chevron_style, move || controller.toggle(id)).child(Box::new(
                    RawText::new(glyph, self.theme.text_secondary, 11.0),
                )),
            )
        } else {
            Box::new(RawView::new(chevron_style))
        };

        let label_style = Style {
            flex_grow: 1.0,
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Percent(1.0),
            },
            align_items: Some(AlignItems::Center),
            ..Default::default()
        };
        let label = RawText::new(row.label.clone(), foreground, self.theme.typography.body)
            .text_align(TextAlign::Start)
            .layout(label_style);

        let controller = self.controller.clone();
        let id = row.id;
        let mut item =
            RawButton::new(row_style, move || controller.select(id)).background(background);
        item = item.hover_style(creamui_core::StateStyle::new().background(if row.selected {
            self.theme.accent_hover
        } else {
            self.theme.surface_hover
        }));
        Box::new(item.child(chevron).child(Box::new(label)))
    }
}

impl Widget for TreeView {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut scroll_view =
            RawScrollView::controlled(fill(Style::default()), self.scroll.clone())
                .background(self.theme.surface_elevated)
                .corner_radius(self.theme.input_radius);
        for row in &self.rows {
            scroll_view = scroll_view.child(self.build_row(row));
        }
        vec![Box::new(scroll_view)]
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let controller = self.controller.clone();
        let ids: Vec<u64> = self.rows.iter().map(|row| row.id).collect();
        let expandable: Vec<(u64, bool, bool)> = self
            .rows
            .iter()
            .map(|row| (row.id, row.has_children, row.expanded))
            .collect();
        Some(Rc::new(move |input| {
            if ids.is_empty() {
                return;
            }
            let current_index = controller
                .peek_selected()
                .and_then(|id| ids.iter().position(|row_id| *row_id == id));
            match input.key {
                Key::Down => {
                    let next = current_index.map_or(0, |i| (i + 1).min(ids.len() - 1));
                    controller.select(ids[next]);
                }
                Key::Up => {
                    let next = current_index.map_or(0, |i| i.saturating_sub(1));
                    controller.select(ids[next]);
                }
                Key::Right | Key::Left => {
                    let Some(id) = controller.peek_selected() else {
                        return;
                    };
                    let Some((_, has_children, expanded)) =
                        expandable.iter().find(|(row_id, _, _)| *row_id == id)
                    else {
                        return;
                    };
                    let want_expanded = input.key == Key::Right;
                    if *has_children && *expanded != want_expanded {
                        controller.set_expanded(id, want_expanded);
                    }
                }
                _ => {}
            }
        }))
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _caret_visible: bool) {
        painter.stroke_rect(
            Rect {
                x: rect.x - 2.0,
                y: rect.y - 2.0,
                width: rect.width + 4.0,
                height: rect.height + 4.0,
            },
            self.theme.accent,
            2.0,
            self.theme.input_radius + 2.0,
        );
    }
}

/// A themed [`RawListView`]: arbitrary rows, scrollable, with the theme's
/// surface, radius, and a hairline divider between rows applied by default.
pub struct ListView {
    inner: RawListView,
}
impl_styled_inner!(ListView);

impl ListView {
    pub fn new(style: Style, scroll: ScrollController) -> Self {
        let theme = use_theme();
        let inner = RawListView::new(style, scroll)
            .background(theme.surface_elevated)
            .corner_radius(theme.input_radius)
            .divider(theme.border, 1.0);
        ListView { inner }
    }

    pub fn row(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.row(widget);
        self
    }

    pub fn rows(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.rows(widgets);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawListView)) -> Self {
        customize(&mut self.inner);
        self
    }
}

impl Widget for ListView {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        Widget::children(&mut self.inner)
    }
}

/// A themed [`RawTable`]: header/row colors, alternating row shading, and a
/// tinted highlight for the selected row all read from the theme.
pub struct Table {
    inner: RawTable,
}
impl_styled_inner!(Table);

impl Table {
    pub fn new(style: Style, scroll: ScrollController, columns: Vec<TableColumn>) -> Self {
        let theme = use_theme();
        let selected_tint = Color::rgba(theme.accent.r, theme.accent.g, theme.accent.b, 60);
        let inner = RawTable::new(style, scroll, columns)
            .header_background(theme.surface_elevated)
            .header_text_color(theme.text_secondary)
            .cell_text_color(theme.text_primary)
            .row_background(theme.surface)
            .alt_row_background(theme.surface_elevated)
            .selected_row_background(selected_tint)
            .divider_color(theme.border);
        Table { inner }
    }

    pub fn row(mut self, cells: Vec<String>) -> Self {
        self.inner = self.inner.row(cells);
        self
    }

    pub fn rows(mut self, rows: Vec<Vec<String>>) -> Self {
        self.inner = self.inner.rows(rows);
        self
    }

    pub fn on_row_click(
        mut self,
        selected: Option<usize>,
        on_click: impl Fn(usize) + 'static,
    ) -> Self {
        self.inner = self.inner.on_row_click(selected, on_click);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawTable)) -> Self {
        customize(&mut self.inner);
        self
    }
}

impl Widget for Table {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        Widget::children(&mut self.inner)
    }
}
