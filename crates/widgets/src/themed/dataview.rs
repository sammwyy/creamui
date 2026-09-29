//! Widgets for browsing structured data rather than picking one option:
//! [`ListView`] (arbitrary rows), [`TreeView`] (hierarchy), [`Table`]
//! (columns — a CSV viewer's shape). For a plain single-select list, see
//! [`crate::themed::ListBox`] in [`crate::themed::selection`] instead —
//! it's a selection control, not a data view.

use super::*;
use crate::layout::fill;
use crate::{RawVirtualList, ScrollController, TreeController, VirtualListState};
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
#[derive(Clone)]
pub struct TreeView {
    theme: Theme,
    style: Style,
    scroll: ScrollController,
    controller: TreeController,
    rows: Rc<Vec<FlatRow>>,
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
            rows: Rc::new(rows),
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
        for row in self.rows.iter() {
            scroll_view = scroll_view.child(self.build_row(row));
        }
        vec![Box::new(scroll_view)]
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let controller = self.controller.clone();
        let rows = self.rows.clone();
        Some(Rc::new(move |input| {
            if rows.is_empty() {
                return;
            }
            let current_index = controller
                .peek_selected()
                .and_then(|id| rows.iter().position(|row| row.id == id));
            match input.key {
                Key::Down => {
                    let next = current_index.map_or(0, |i| (i + 1).min(rows.len() - 1));
                    controller.select(rows[next].id);
                }
                Key::Up => {
                    let next = current_index.map_or(0, |i| i.saturating_sub(1));
                    controller.select(rows[next].id);
                }
                Key::Right | Key::Left => {
                    let Some(id) = controller.peek_selected() else {
                        return;
                    };
                    let Some(row) = rows.iter().find(|row| row.id == id) else {
                        return;
                    };
                    let want_expanded = input.key == Key::Right;
                    if row.has_children && row.expanded != want_expanded {
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

/// A tree that flattens expanded nodes but mounts only rows near the viewport.
/// Each mounted row keeps its node ID as its widget key. Keep `state` across
/// rebuilds and initialize it with the configured row height.
pub struct VirtualTreeView {
    tree: TreeView,
    state: VirtualListState,
    viewport_height: f32,
    overscan: usize,
}

impl VirtualTreeView {
    pub fn new(
        style: Style,
        scroll: ScrollController,
        controller: TreeController,
        nodes: &[TreeNode],
        state: VirtualListState,
        viewport_height: f32,
    ) -> Self {
        let tree = TreeView::new(style, scroll, controller, nodes);
        if state.len() != tree.rows.len() {
            state.set_item_count(tree.rows.len(), tree.row_height);
        }
        Self {
            tree,
            state,
            viewport_height: viewport_height.max(0.0),
            overscan: 4,
        }
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.tree.row_height = height.max(1.0);
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.tree.indent = indent.max(0.0);
        self
    }

    pub fn overscan(mut self, rows: usize) -> Self {
        self.overscan = rows;
        self
    }
}

impl Widget for VirtualTreeView {
    fn style(&self) -> creamui_core::Style {
        self.tree.style()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let theme = self.tree.theme;
        let rows = self.tree.rows.clone();
        let tree = self.tree.clone();
        let list = RawVirtualList::new(
            fill(Style::default()),
            self.state.clone(),
            self.tree.scroll.clone(),
            self.viewport_height,
            move |index| tree.build_row(&tree.rows[index]),
        )
        .keyed_by(move |index| rows[index].id.into())
        .overscan(self.overscan)
        .background(theme.surface_elevated)
        .corner_radius(theme.input_radius);
        vec![Box::new(list)]
    }

    fn focusable(&self) -> bool {
        self.tree.focusable()
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.tree.on_key()
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        self.tree
            .paint_focused_overlay(painter, rect, caret_visible);
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

fn apply_table_theme(table: &mut RawTable) {
    let theme = use_theme();
    table.header_background = Some(theme.surface_elevated);
    table.header_text_color = theme.text_secondary;
    table.cell_text_color = theme.text_primary;
    table.row_background = Some(theme.surface);
    table.alt_row_background = Some(theme.surface_elevated);
    table.selected_row_background = Some(Color::rgba(
        theme.accent.r,
        theme.accent.g,
        theme.accent.b,
        60,
    ));
    table.divider_color = Some(theme.border);
}

impl Table {
    pub fn new(style: Style, scroll: ScrollController, columns: Vec<TableColumn>) -> Self {
        let mut inner = RawTable::new(style, scroll, columns);
        apply_table_theme(&mut inner);
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

/// A themed table that creates only rows inside the scrolling viewport.
pub struct VirtualTable {
    inner: RawVirtualTable,
}
impl_styled_inner!(VirtualTable);

impl VirtualTable {
    pub fn new(
        style: Style,
        scroll: ScrollController,
        columns: Vec<TableColumn>,
        state: VirtualListState,
        viewport_height: f32,
        row: impl Fn(usize) -> Vec<String> + 'static,
    ) -> Self {
        let inner = RawVirtualTable::new(style, scroll, columns, state, viewport_height, row)
            .customize(apply_table_theme);
        Self { inner }
    }

    pub fn overscan(mut self, rows: usize) -> Self {
        self.inner = self.inner.overscan(rows);
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
        self.inner = self.inner.customize(customize);
        self
    }
}

impl Widget for VirtualTable {
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
    use creamui_core::{visible_range, HeightIndex, WidgetKey};

    fn mounted_keys(mut widget: BoxedWidget, keys: &mut Vec<WidgetKey>) {
        if let Some(key) = widget.key() {
            keys.push(key);
        }
        for child in widget.children() {
            mounted_keys(child, keys);
        }
    }

    #[test]
    fn virtual_tree_mounts_visible_rows_with_stable_node_keys() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = TreeController::new();
            controller.set_expanded(1, true);
            let scroll = ScrollController::new(0.0);
            let state = VirtualListState::new(0, 30.0);
            let nodes = [
                TreeNode::new(1, "Parent").child(TreeNode::new(4, "Child")),
                TreeNode::new(2, "Second"),
                TreeNode::new(3, "Third"),
            ];
            let mut tree = VirtualTreeView::new(
                Style::default(),
                scroll.clone(),
                controller.clone(),
                &nodes,
                state.clone(),
                60.0,
            )
            .overscan(0);
            assert_eq!(state.len(), 4);
            let mut keys = Vec::new();
            mounted_keys(tree.children().pop().unwrap(), &mut keys);
            assert_eq!(keys, vec![1u64.into(), 4u64.into(), 2u64.into()]);

            let reordered = [
                TreeNode::new(2, "Second"),
                TreeNode::new(1, "Parent").child(TreeNode::new(4, "Child")),
                TreeNode::new(3, "Third"),
            ];
            let mut tree = VirtualTreeView::new(
                Style::default(),
                scroll,
                controller,
                &reordered,
                state,
                60.0,
            )
            .overscan(0);
            let mut keys = Vec::new();
            mounted_keys(tree.children().pop().unwrap(), &mut keys);
            assert_eq!(keys, vec![2u64.into(), 1u64.into(), 4u64.into()]);
        });
    }

    #[test]
    fn virtual_tree_mounts_a_small_slice_of_a_large_tree() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let nodes: Vec<_> = (0..10_000)
                .map(|id| TreeNode::new(id, format!("Node {id}")))
                .collect();
            let state = VirtualListState::new(0, 30.0);
            let mut tree = VirtualTreeView::new(
                Style::default(),
                ScrollController::new(150_000.0),
                TreeController::new(),
                &nodes,
                state.clone(),
                90.0,
            )
            .overscan(2);
            assert_eq!(state.len(), nodes.len());

            let mut keys = Vec::new();
            mounted_keys(tree.children().pop().unwrap(), &mut keys);
            let expected: Vec<_> =
                visible_range(&HeightIndex::uniform(nodes.len(), 30.0), 150_000.0, 90.0, 2)
                    .map(|index| (index as u64).into())
                    .collect();
            assert_eq!(keys, expected);
            assert!(keys.len() < 20);
        });
    }
}
