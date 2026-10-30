use super::*;
use crate::{layout::column, ScrollController};
use creamui_core::layout::{Dimension, LengthPercentage, LengthPercentageAuto, Position};
use creamui_core::{Key, Size};
use creamui_reactive::Signal;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: String,
    pub enabled: bool,
    pub children: Vec<Self>,
    action: Option<Rc<dyn Fn()>>,
    separator: bool,
}

impl ContextMenuItem {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: String::new(),
            enabled: true,
            children: Vec::new(),
            action: Some(Rc::new(action)),
            separator: false,
        }
    }
    pub fn submenu(id: impl Into<String>, label: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            children,
            action: None,
            ..Self::new(id, label, || {})
        }
    }
    pub fn separator() -> Self {
        Self {
            separator: true,
            enabled: false,
            ..Self::new("", "", || {})
        }
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }
    fn selectable(&self) -> bool {
        self.enabled && !self.separator && (self.action.is_some() || !self.children.is_empty())
    }
}

#[derive(Clone)]
struct MenuState {
    anchor: Point,
    items: Rc<Vec<ContextMenuItem>>,
    selected: Vec<usize>,
    expanded: Vec<usize>,
    scrolls: Vec<ScrollController>,
}

#[derive(Clone)]
pub struct ContextMenuController {
    state: Signal<Option<MenuState>>,
    layouts: Rc<RefCell<HashMap<Vec<usize>, Rect>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenuPane {
    pub path: Vec<usize>,
    pub anchor: Option<Rect>,
}
impl Default for ContextMenuController {
    fn default() -> Self {
        Self {
            state: Signal::new(None),
            layouts: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}
impl ContextMenuController {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn open(&self, anchor: Point, items: Vec<ContextMenuItem>) {
        self.layouts.borrow_mut().clear();
        self.state.set(Some(MenuState {
            anchor,
            items: Rc::new(items),
            selected: Vec::new(),
            expanded: Vec::new(),
            scrolls: vec![ScrollController::new(0.0)],
        }));
    }
    pub fn update_item(&self, item: ContextMenuItem) -> bool {
        let Some(mut state) = self.state.peek() else {
            return false;
        };
        let Some(path) = item_path(&state.items, &item.id).filter(|_| !item.id.is_empty()) else {
            return false;
        };
        let mut items = state.items.as_ref().clone();
        let mut current = &mut items[path[0]];
        for index in &path[1..] {
            current = &mut current.children[*index];
        }
        if current.separator != item.separator {
            return false;
        }
        let selectable = item.selectable();
        *current = item;
        if state.expanded.starts_with(&path) {
            state.expanded.truncate(path.len() - 1);
            state.scrolls.truncate(state.expanded.len() + 1);
        }
        if state.selected.starts_with(&path) {
            state.selected = if selectable {
                path.clone()
            } else {
                path[..path.len() - 1].to_vec()
            };
        }
        self.layouts
            .borrow_mut()
            .retain(|key, _| key == &path || !key.starts_with(&path));
        state.items = Rc::new(items);
        self.state.set(Some(state));
        true
    }

    pub fn close(&self) {
        if self.is_open() {
            self.layouts.borrow_mut().clear();
            self.state.set(None);
        }
    }
    pub fn is_open(&self) -> bool {
        self.state.get().is_some()
    }
    pub fn visible_panes(&self) -> Vec<ContextMenuPane> {
        let Some(state) = self.state.get() else {
            return Vec::new();
        };
        let layouts = self.layouts.borrow();
        (0..=state.expanded.len())
            .map(|depth| {
                let path = state.expanded[..depth].to_vec();
                ContextMenuPane {
                    anchor: layouts.get(&path).copied(),
                    path,
                }
            })
            .collect()
    }
    pub fn pane_height(&self, path: &[usize], row_height: f32) -> Option<f32> {
        let state = self.state.peek()?;
        let items = children_at(&state.items, path);
        if items.is_empty() {
            return None;
        }
        Some(
            items
                .iter()
                .map(|item| if item.separator { 9.0 } else { row_height })
                .sum::<f32>()
                + 10.0,
        )
    }
    pub fn close_submenu(&self, path: &[usize]) {
        let Some(state) = self.state.peek() else {
            return;
        };
        if !path.is_empty() && state.expanded.starts_with(path) {
            self.select(path.to_vec(), false);
        }
    }
    fn collapse_after(&self, path: &[usize]) {
        let Some(mut state) = self.state.peek() else {
            return;
        };
        if state.expanded.len() <= path.len() || !state.expanded.starts_with(path) {
            return;
        }
        state.expanded.truncate(path.len());
        state.selected.truncate(path.len());
        state.scrolls.truncate(path.len() + 1);
        self.state.set(Some(state));
    }

    fn select(&self, path: Vec<usize>, expand: bool) {
        let Some(mut state) = self.state.peek() else {
            return;
        };
        let Some(item) = item_at(&state.items, &path) else {
            return;
        };
        if !item.selectable() {
            return;
        }
        let branch = !item.children.is_empty();
        state.selected = path.clone();
        state.expanded = if expand && branch {
            path
        } else {
            path[..path.len() - 1].to_vec()
        };
        state.scrolls.truncate(state.expanded.len() + 1);
        while state.scrolls.len() <= state.expanded.len() {
            state.scrolls.push(ScrollController::new(0.0));
        }
        self.state.set(Some(state));
    }
    fn activate(&self, path: Vec<usize>) {
        let Some(state) = self.state.peek() else {
            return;
        };
        let Some(item) = item_at(&state.items, &path).filter(|item| item.selectable()) else {
            return;
        };
        if !item.children.is_empty() {
            self.select(path, true);
            return;
        }
        let action = item.action.clone();
        self.close();
        if let Some(action) = action {
            action();
        }
    }
    /// Accepts native keyboard routing without requiring a themed render context.
    pub fn handle_key(&self, key: &Key) -> bool {
        let Some(state) = self.state.peek() else {
            return false;
        };
        match key {
            Key::Escape | Key::Tab => self.close(),
            Key::Left => {
                if !state.expanded.is_empty() {
                    self.select(state.expanded.clone(), false);
                }
            }
            Key::Enter | Key::Char(' ') => {
                if !state.selected.is_empty() {
                    self.activate(state.selected.clone());
                }
            }
            Key::Right => {
                if let Some(item) = item_at(&state.items, &state.selected) {
                    if !item.children.is_empty() {
                        let mut path = state.selected.clone();
                        if let Some(index) =
                            item.children.iter().position(ContextMenuItem::selectable)
                        {
                            path.push(index);
                            self.select(path, false);
                        }
                    }
                }
            }
            Key::Down | Key::Up | Key::Home | Key::End => {
                let parent = state.expanded.clone();
                let items = children_at(&state.items, &parent);
                let available: Vec<_> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| item.selectable())
                    .map(|(i, _)| i)
                    .collect();
                if available.is_empty() {
                    return true;
                }
                let current = if state.selected.len() == parent.len() + 1 {
                    available
                        .iter()
                        .position(|i| Some(i) == state.selected.last())
                } else {
                    None
                };
                let index = match key {
                    Key::Home => 0,
                    Key::End => available.len() - 1,
                    Key::Up => current.map_or(available.len() - 1, |i| {
                        (i + available.len() - 1) % available.len()
                    }),
                    _ => current.map_or(0, |i| (i + 1) % available.len()),
                };
                let mut path = parent;
                path.push(available[index]);
                self.select(path, false);
            }
            _ => {}
        }
        true
    }
}
fn item_path(items: &[ContextMenuItem], id: &str) -> Option<Vec<usize>> {
    for (index, item) in items.iter().enumerate() {
        if item.id == id {
            return Some(vec![index]);
        }
        if let Some(mut path) = item_path(&item.children, id) {
            path.insert(0, index);
            return Some(path);
        }
    }
    None
}

fn children_at<'a>(items: &'a [ContextMenuItem], path: &[usize]) -> &'a [ContextMenuItem] {
    if path.is_empty() {
        items
    } else {
        item_at(items, path).map_or(&[], |item| &item.children)
    }
}
fn item_at<'a>(mut items: &'a [ContextMenuItem], path: &[usize]) -> Option<&'a ContextMenuItem> {
    let mut result = None;
    for index in path {
        let item = items.get(*index)?;
        result = Some(item);
        items = &item.children;
    }
    result
}

#[derive(Clone, Copy)]
pub struct ContextMenuStyle {
    pub background: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub active: Color,
    pub radius: f32,
    pub item_radius: f32,
    pub row_height: f32,
    pub font_size: f32,
    pub width: f32,
    pub shadow: Option<creamui_core::BoxShadow>,
}

pub struct RawContextMenu {
    controller: ContextMenuController,
    viewport: Size,
    tokens: ContextMenuStyle,
    pane_path: Option<Vec<usize>>,
}
impl RawContextMenu {
    pub fn new(
        controller: &ContextMenuController,
        viewport: Size,
        tokens: ContextMenuStyle,
    ) -> Self {
        Self {
            controller: controller.clone(),
            viewport,
            tokens,
            pane_path: None,
        }
    }
    pub fn tokens(&self) -> ContextMenuStyle {
        self.tokens
    }
    pub fn pane(
        controller: &ContextMenuController,
        path: Vec<usize>,
        viewport: Size,
        tokens: ContextMenuStyle,
    ) -> Self {
        Self {
            pane_path: Some(path),
            ..Self::new(controller, viewport, tokens)
        }
    }
}
fn placed(rect: Rect) -> Style {
    Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(rect.x),
            top: LengthPercentageAuto::Length(rect.y),
            right: LengthPercentageAuto::Auto,
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Length(rect.width),
            height: Dimension::Length(rect.height),
        },
        ..Default::default()
    }
}
fn row_height(item: &ContextMenuItem, tokens: ContextMenuStyle) -> f32 {
    if item.separator {
        9.0
    } else {
        tokens.row_height
    }
}
fn pane_rect(anchor: Point, parent: Option<Rect>, height: f32, viewport: Size, width: f32) -> Rect {
    let width = width.min((viewport.width - 8.0).max(0.0));
    let height = height.min((viewport.height - 8.0).max(0.0));
    let x = parent.map_or(anchor.x, |parent| {
        if parent.x + parent.width + width <= viewport.width - 4.0 {
            parent.x + parent.width
        } else {
            parent.x - width
        }
    });
    Rect {
        x: x.clamp(4.0, (viewport.width - width - 4.0).max(4.0)),
        y: anchor
            .y
            .clamp(4.0, (viewport.height - height - 4.0).max(4.0)),
        width,
        height,
    }
}
impl Widget for RawContextMenu {
    fn style(&self) -> creamui_core::Style {
        placed(Rect {
            x: 0.0,
            y: 0.0,
            width: self.viewport.width,
            height: self.viewport.height,
        })
        .into()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn is_modal(&self) -> bool {
        self.controller.is_open()
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let c = self.controller.clone();
        Some(Rc::new(move || c.close()))
    }
    fn on_context_menu(&self) -> Option<Rc<dyn Fn(Point)>> {
        let c = self.controller.clone();
        Some(Rc::new(move |_| c.close()))
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let c = self.controller.clone();
        Some(Rc::new(move |input| {
            c.handle_key(&input.key);
        }))
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let Some(state) = self.controller.state.get() else {
            return Vec::new();
        };
        let mut panes = Vec::new();
        let mut anchor = state.anchor;
        let mut parent = None;
        let paths: Vec<_> = self.pane_path.clone().map_or_else(
            || {
                (0..=state.expanded.len())
                    .map(|depth| state.expanded[..depth].to_vec())
                    .collect()
            },
            |path| vec![path],
        );
        for path in paths {
            let depth = path.len();
            if depth > state.expanded.len() || !state.expanded.starts_with(&path) {
                continue;
            }
            let prefix = &path[..];
            let items = children_at(&state.items, prefix);
            if items.is_empty() {
                break;
            }
            let height = items
                .iter()
                .map(|item| row_height(item, self.tokens))
                .sum::<f32>()
                + 10.0;
            let rect = if self.pane_path.is_some() {
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: self.viewport.width,
                    height: self.viewport.height,
                }
            } else {
                pane_rect(anchor, parent, height, self.viewport, self.tokens.width)
            };
            let mut rows = Vec::new();
            for (index, item) in items.iter().enumerate() {
                let mut path = prefix.to_vec();
                path.push(index);
                rows.push(Box::new(MenuRow {
                    item: item.clone(),
                    active: state.selected == path || state.expanded.starts_with(&path),
                    path,
                    controller: self.controller.clone(),
                    tokens: self.tokens,
                }) as BoxedWidget);
            }
            let scroll = state.scrolls[depth].clone();
            scroll.report_max_offset((height - rect.height).max(0.0));
            if state.selected.len() == depth + 1 && state.selected.starts_with(prefix) {
                let index = state.selected[depth];
                let top = items[..index]
                    .iter()
                    .map(|item| row_height(item, self.tokens))
                    .sum::<f32>();
                let bottom = top + row_height(&items[index], self.tokens);
                let visible_height = (rect.height - 10.0).max(0.0);
                if top < scroll.offset() {
                    scroll.set(top);
                } else if bottom > scroll.offset() + visible_height {
                    scroll.set((bottom - visible_height).max(0.0));
                }
            }
            if let Some(index) = state.expanded.get(depth) {
                anchor.y = rect.y
                    + 5.0
                    + items[..*index]
                        .iter()
                        .map(|item| row_height(item, self.tokens))
                        .sum::<f32>()
                    - scroll.offset();
            }
            let inner = RawView::new(column(0.0)).with_children(rows);
            let mut style = creamui_core::Style::from(placed(rect))
                .background(self.tokens.background)
                .border(self.tokens.border, 1.0)
                .corner_radius(self.tokens.radius);
            style.paint.box_shadow = self.tokens.shadow;
            let mut scroll_view = RawScrollView::controlled(
                Style {
                    padding: creamui_core::layout::Rect {
                        left: LengthPercentage::Length(4.0),
                        right: LengthPercentage::Length(4.0),
                        top: LengthPercentage::Length(4.0),
                        bottom: LengthPercentage::Length(4.0),
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Percent(1.0),
                    },
                    ..column(0.0)
                },
                scroll.clone(),
            )
            .scrollbar(height > rect.height + 0.5)
            .rebuild_on_scroll(true)
            .child(Box::new(inner));
            let controller = self.controller.clone();
            let prefix = path.clone();
            scroll_view.on_scroll_bounded = Some(Rc::new(move |delta, maximum| {
                let previous = scroll.offset();
                scroll.scroll_by(delta, maximum);
                if previous != scroll.offset() {
                    controller.collapse_after(&prefix);
                }
            }));
            let pane = RawView::new(style).child(Box::new(scroll_view));
            panes.push(Box::new(pane) as BoxedWidget);
            parent = Some(rect);
        }
        panes
    }
}
struct MenuRow {
    item: ContextMenuItem,
    path: Vec<usize>,
    active: bool,
    controller: ContextMenuController,
    tokens: ContextMenuStyle,
}
impl Widget for MenuRow {
    fn reports_layout(&self) -> bool {
        !self.item.children.is_empty()
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.controller
            .layouts
            .borrow_mut()
            .insert(self.path.clone(), rect);
        false
    }
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(row_height(&self.item, self.tokens)),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let t = self.tokens;
        if self.item.separator {
            painter.fill_rect(
                Rect {
                    x: rect.x + 8.0,
                    y: rect.y + 4.0,
                    width: (rect.width - 16.0).max(0.0),
                    height: 1.0,
                },
                t.border,
                0.0,
            );
            return;
        }
        if self.active || (self.item.selectable() && painter.hovered(rect)) {
            painter.fill_rect(rect, t.active, t.item_radius);
        }
        let color = if self.item.selectable() {
            t.text
        } else {
            t.muted
        };
        let trailing = if self.item.children.is_empty() {
            self.item.shortcut.as_str()
        } else {
            "›"
        };
        let trailing_width =
            crate::text_metrics::measure_family(trailing, t.font_size, rect.width, None, false).0
                + 16.0;
        painter.fill_text(
            Rect {
                x: rect.x + 10.0,
                width: (rect.width - 20.0 - trailing_width).max(0.0),
                ..rect
            },
            &self.item.label,
            color,
            t.font_size,
            TextAlign::Start,
        );
        painter.fill_text(
            Rect {
                x: rect.x + rect.width - trailing_width,
                width: (trailing_width - 10.0).max(0.0),
                ..rect
            },
            trailing,
            t.muted,
            t.font_size,
            TextAlign::End,
        );
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let controller = self.controller.clone();
        let path = self.path.clone();
        Some(Rc::new(move || controller.activate(path.clone())))
    }
    fn on_context_menu(&self) -> Option<Rc<dyn Fn(Point)>> {
        let controller = self.controller.clone();
        Some(Rc::new(move |_| controller.close()))
    }
    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        let controller = self.controller.clone();
        let path = self.path.clone();
        Some(Rc::new(move |hover| {
            if hover {
                controller.select(path.clone(), true);
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::Renderer;
    #[derive(Default)]
    struct Recorder {
        text: Vec<(String, Rect)>,
    }
    impl Painter for Recorder {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, rect: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
            self.text.push((text.into(), rect));
        }
    }
    fn tokens() -> ContextMenuStyle {
        ContextMenuStyle {
            background: Color::rgb(255, 255, 255),
            border: Color::rgb(100, 100, 100),
            text: Color::rgb(0, 0, 0),
            muted: Color::rgb(120, 120, 120),
            active: Color::rgb(220, 220, 220),
            radius: 10.0,
            item_radius: 6.0,
            row_height: 32.0,
            font_size: 13.0,
            width: 220.0,
            shadow: None,
        }
    }
    #[test]
    fn keyboard_skips_unavailable_rows_and_closes_before_callback() {
        let controller = ContextMenuController::new();
        let clicked = Rc::new(Cell::new(false));
        let action = clicked.clone();
        let menu = controller.clone();
        controller.open(
            Point::default(),
            vec![
                ContextMenuItem::separator(),
                ContextMenuItem::new("disabled", "Disabled", || panic!()).enabled(false),
                ContextMenuItem::submenu(
                    "parent",
                    "Parent",
                    vec![ContextMenuItem::submenu(
                        "nested",
                        "Nested",
                        vec![ContextMenuItem::new("leaf", "Leaf", move || {
                            assert!(!menu.is_open());
                            action.set(true);
                        })],
                    )],
                ),
            ],
        );
        for key in [Key::Down, Key::Right, Key::Right, Key::Enter] {
            assert!(controller.handle_key(&key));
        }
        assert!(clicked.get());
        assert!(!controller.is_open());
    }
    #[test]
    fn item_updates_preserve_pane_size_and_cannot_reopen_a_closed_menu() {
        let controller = ContextMenuController::new();
        controller.open(
            Point::default(),
            vec![
                ContextMenuItem::submenu("apps", "Open with", vec![]),
                ContextMenuItem::new("copy", "Copy", || {}),
            ],
        );
        let height = controller.pane_height(&[], 32.0);
        let activated = Rc::new(Cell::new(false));
        let action = activated.clone();
        assert!(controller.update_item(ContextMenuItem::submenu(
            "apps",
            "Open with",
            vec![ContextMenuItem::new("editor", "Editor", move || action.set(true))]
        )));
        assert_eq!(controller.pane_height(&[], 32.0), height);
        for key in [Key::Down, Key::Right, Key::Enter] {
            controller.handle_key(&key);
        }
        assert!(activated.get());
        assert!(!controller.update_item(ContextMenuItem::new("copy", "Copy", || {})));
        assert!(!controller.is_open());
    }

    #[test]
    fn left_and_escape_close_submenus_without_activating() {
        let controller = ContextMenuController::new();
        controller.open(
            Point::default(),
            vec![ContextMenuItem::submenu(
                "parent",
                "Parent",
                vec![ContextMenuItem::new("leaf", "Leaf", || panic!())],
            )],
        );
        for key in [Key::Down, Key::Right, Key::Left] {
            controller.handle_key(&key);
        }
        let state = controller.state.peek().unwrap();
        assert!(state.expanded.is_empty());
        assert_eq!(state.selected, vec![0]);
        controller.handle_key(&Key::Escape);
        assert!(!controller.is_open());
    }
    #[test]
    fn scene_keeps_child_menus_open_and_shields_the_background() {
        let controller = ContextMenuController::new();
        let clicked = Rc::new(Cell::new(0));
        let action = clicked.clone();
        controller.open(
            Point { x: 660.0, y: 440.0 },
            vec![ContextMenuItem::submenu(
                "new",
                "New",
                vec![ContextMenuItem::new("file", "Empty file", move || {
                    action.set(action.get() + 1)
                })],
            )],
        );
        let size = Size {
            width: 700.0,
            height: 480.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = Recorder::default();
        let scene = renderer.render(
            Box::new(RawContextMenu::new(&controller, size, tokens())),
            size,
            &mut painter,
        );
        let rect = painter
            .text
            .iter()
            .find(|(text, _)| text == "New")
            .unwrap()
            .1;
        let point = Point {
            x: rect.x + 2.0,
            y: rect.y + 16.0,
        };
        scene.hover_hit_test(point).unwrap().1(true);
        painter.text.clear();
        let scene = renderer.render(
            Box::new(RawContextMenu::new(&controller, size, tokens())),
            size,
            &mut painter,
        );
        let child = painter
            .text
            .iter()
            .find(|(text, _)| text == "Empty file")
            .unwrap()
            .1;
        assert!(child.x < rect.x);
        assert!(child.y + child.height <= size.height);
        let point = Point {
            x: child.x + 2.0,
            y: child.y + 16.0,
        };
        scene.hit_test(point).unwrap()();
        assert_eq!(clicked.get(), 1);
        assert!(!controller.is_open());
        controller.open(
            Point::default(),
            vec![ContextMenuItem::new("test", "Test", || {})],
        );
        let scene = renderer.render(
            Box::new(RawContextMenu::new(&controller, size, tokens())),
            size,
            &mut Recorder::default(),
        );
        scene.context_menu_at(Point { x: 680.0, y: 460.0 }).unwrap()(Point::default());
        assert!(!controller.is_open());
    }
    #[test]
    fn keyboard_selection_scrolls_long_menus_into_view() {
        let controller = ContextMenuController::new();
        controller.open(
            Point::default(),
            (0..50)
                .map(|i| ContextMenuItem::new(i.to_string(), format!("Item {i}"), || {}))
                .collect(),
        );
        controller.handle_key(&Key::End);
        let size = Size {
            width: 300.0,
            height: 180.0,
        };
        let mut painter = Recorder::default();
        let scene = Renderer::new().render(
            Box::new(RawContextMenu::new(&controller, size, tokens())),
            size,
            &mut painter,
        );
        let rect = painter
            .text
            .iter()
            .find(|(text, _)| text == "Item 49")
            .unwrap()
            .1;
        assert!(rect.y >= 0.0 && rect.y + rect.height <= size.height);
        assert!(scene
            .hit_test(Point {
                x: rect.x + 1.0,
                y: rect.y + 16.0
            })
            .is_some());
    }
    #[test]
    fn popup_panes_report_parent_rows_and_render_only_their_branch() {
        let controller = ContextMenuController::new();
        controller.open(
            Point::default(),
            vec![ContextMenuItem::submenu(
                "root",
                "Root",
                vec![ContextMenuItem::new("leaf", "Leaf", || {})],
            )],
        );
        let size = Size {
            width: 248.0,
            height: 42.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = Recorder::default();
        renderer.render(
            Box::new(RawContextMenu::pane(&controller, vec![], size, tokens())),
            size,
            &mut painter,
        );
        controller.handle_key(&Key::Down);
        controller.handle_key(&Key::Right);
        let panes = controller.visible_panes();
        assert_eq!(panes.len(), 2);
        assert!(panes[1].anchor.is_some());
        painter.text.clear();
        renderer.render(
            Box::new(RawContextMenu::pane(&controller, vec![0], size, tokens())),
            size,
            &mut painter,
        );
        assert!(painter.text.iter().any(|(text, _)| text == "Leaf"));
        assert!(!painter.text.iter().any(|(text, _)| text == "Root"));
        controller.close_submenu(&[0]);
        painter.text.clear();
        renderer.render(
            Box::new(RawContextMenu::pane(&controller, vec![0], size, tokens())),
            size,
            &mut painter,
        );
        assert!(painter.text.is_empty());
        assert!(controller.is_open());
    }

    #[test]
    fn wheel_scrolling_over_rows_closes_submenus_and_keeps_the_root_open() {
        let controller = ContextMenuController::new();
        let mut items = vec![ContextMenuItem::submenu(
            "root",
            "Root",
            vec![ContextMenuItem::new("leaf", "Leaf", || {})],
        )];
        items.extend(
            (0..20).map(|index| {
                ContextMenuItem::new(index.to_string(), format!("Item {index}"), || {})
            }),
        );
        controller.open(Point::default(), items);
        controller.handle_key(&Key::Down);
        controller.handle_key(&Key::Right);
        let size = Size {
            width: 248.0,
            height: 100.0,
        };
        let scene = Renderer::new().render(
            Box::new(RawContextMenu::pane(&controller, vec![], size, tokens())),
            size,
            &mut Recorder::default(),
        );
        let index = scene
            .scroll_hit_test(Point { x: 20.0, y: 20.0 })
            .expect("menu row permits scrolling");
        scene.on_scroll_at(index).unwrap()(60.0);
        assert_eq!(controller.visible_panes().len(), 1);
        assert!(controller.state.peek().unwrap().scrolls[0].offset() > 0.0);
    }

    #[test]
    fn context_hit_testing_prefers_visible_children_and_honors_modal_layers() {
        let parent = Rc::new(Cell::new(0));
        let child = Rc::new(Cell::new(0));
        let p = parent.clone();
        let c = child.clone();
        let root = RawView::new(creamui_core::layout::Style {
            size: crate::layout::fixed(400.0, 300.0),
            ..Default::default()
        })
        .with_context_menu(move |_| p.set(p.get() + 1))
        .child(Box::new(
            RawButton::new(
                creamui_core::layout::Style {
                    size: crate::layout::fixed(80.0, 40.0),
                    ..Default::default()
                },
                || panic!(),
            )
            .with_context_menu(move |_| c.set(c.get() + 1)),
        ));
        let size = Size {
            width: 400.0,
            height: 300.0,
        };
        let scene = Renderer::new().render(Box::new(root), size, &mut Recorder::default());
        for point in [Point { x: 10.0, y: 10.0 }, Point { x: 300.0, y: 200.0 }] {
            scene.context_menu_at(point).unwrap()(point);
        }
        assert_eq!(child.get(), 1);
        assert_eq!(parent.get(), 1);
    }
}
