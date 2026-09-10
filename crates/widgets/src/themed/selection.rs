use super::*;
use crate::layout::{column, fill, fixed, padding, row};
use crate::ScrollController;
use creamui_core::layout::{AlignItems, Dimension, LengthPercentageAuto, Position, Style};
use creamui_core::Key;

/// A controlled, non-editable combo box. Its popup is an absolute child, so
/// it floats over following content instead of making the surrounding form
/// jump. Keep a [`crate::SelectController`] alive in the application.
pub struct Select {
    theme: Theme,
    options: Vec<String>,
    controller: crate::SelectController,
    style: creamui_core::Style,
}
impl_styled_field!(Select);

impl Select {
    pub fn controlled(options: &[&str], controller: crate::SelectController) -> Self {
        let theme = use_theme();
        Self {
            theme,
            options: options.iter().map(|option| (*option).to_owned()).collect(),
            controller,
            style: Self::default_style().into(),
        }
    }

    pub fn default_style() -> Style {
        Style {
            size: fixed(220.0, 36.0),
            flex_shrink: 0.0,
            ..Default::default()
        }
    }

    fn selected_label(&self) -> &str {
        self.options
            .get(
                self.controller
                    .selected()
                    .min(self.options.len().saturating_sub(1)),
            )
            .map(String::as_str)
            .unwrap_or("Select…")
    }
}

impl Widget for Select {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let hovered = painter.hovered(rect);
        painter.fill_rect(
            rect,
            if hovered {
                self.theme.surface_hover
            } else {
                self.theme.surface_elevated
            },
            self.theme.input_radius,
        );
        painter.stroke_rect(
            rect,
            self.theme.border_strong,
            self.theme.input_border_width,
            self.theme.input_radius,
        );
        painter.fill_text(
            Rect {
                x: rect.x + 11.0,
                y: rect.y,
                width: (rect.width - 34.0).max(0.0),
                height: rect.height,
            },
            self.selected_label(),
            self.theme.text_primary,
            self.theme.typography.body,
            TextAlign::Start,
        );
        let cx = rect.x + rect.width - 17.0;
        let cy = rect.y + rect.height / 2.0;
        let down = self.controller.is_open();
        let (a, b, c) = if down {
            (
                Point {
                    x: cx - 5.0,
                    y: cy + 2.0,
                },
                Point {
                    x: cx + 5.0,
                    y: cy + 2.0,
                },
                Point { x: cx, y: cy - 3.0 },
            )
        } else {
            (
                Point {
                    x: cx - 5.0,
                    y: cy - 2.0,
                },
                Point {
                    x: cx + 5.0,
                    y: cy - 2.0,
                },
                Point { x: cx, y: cy + 3.0 },
            )
        };
        painter.stroke_line(a, b, self.theme.text_secondary, 1.5);
        painter.stroke_line(b, c, self.theme.text_secondary, 1.5);
        painter.stroke_line(c, a, self.theme.text_secondary, 1.5);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        if !self.controller.is_open() || self.options.is_empty() {
            return Vec::new();
        }
        let option_height = 34.0;
        let popup_style = padding(
            Style {
                position: Position::Absolute,
                inset: creamui_core::layout::Rect {
                    left: LengthPercentageAuto::Length(0.0),
                    right: LengthPercentageAuto::Auto,
                    top: LengthPercentageAuto::Length(40.0),
                    bottom: LengthPercentageAuto::Auto,
                },
                size: creamui_core::layout::Size {
                    width: match self.style.size.width {
                        Dimension::Length(width) => Dimension::Length(width),
                        _ => Dimension::Length(220.0),
                    },
                    // Keep the popup's opaque surface and hit region
                    // deterministic across absolute-layout parents.
                    height: Dimension::Length(self.options.len() as f32 * option_height + 6.0),
                },
                ..column(2.0)
            },
            3.0,
        );
        let mut popup = Popover::new(popup_style);
        for (index, label) in self.options.iter().enumerate() {
            let selected = self.controller.selected() == index;
            let controller = self.controller.clone();
            let label = label.clone();
            let item_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(option_height),
                },
                padding: creamui_core::layout::Rect {
                    left: creamui_core::layout::LengthPercentage::Length(9.0),
                    right: creamui_core::layout::LengthPercentage::Length(9.0),
                    top: creamui_core::layout::LengthPercentage::Length(0.0),
                    bottom: creamui_core::layout::LengthPercentage::Length(0.0),
                },
                align_items: Some(AlignItems::Center),
                ..Default::default()
            };
            let background = if selected {
                self.theme.accent
            } else {
                self.theme.surface_elevated
            };
            let foreground = if selected {
                self.theme.selection_text
            } else {
                self.theme.text_primary
            };
            let mut item = RawButton::new(item_style, move || controller.select(index))
                .background(background)
                .corner_radius(self.theme.menu_item_radius)
                .child(Box::new(
                    RawText::new(label, foreground, self.theme.typography.body)
                        .text_align(TextAlign::Start),
                ));
            item = item
                .hover_style(creamui_core::StateStyle::new().background(if selected {
                    self.theme.accent_hover
                } else {
                    self.theme.surface_hover
                }))
                .focus_style(creamui_core::StateStyle::new().outline(self.theme.accent, 2.0));
            popup = popup.child(Box::new(item));
        }
        let dismiss = self.controller.clone();
        vec![
            super::portal_dismiss_layer(move || dismiss.set_open(false)),
            Box::new(popup),
        ]
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let controller = self.controller.clone();
        Some(Rc::new(move || controller.toggle()))
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let controller = self.controller.clone();
        let len = self.options.len();
        Some(Rc::new(move |input| match input.key {
            Key::Enter | Key::Char(' ') => controller.toggle(),
            Key::Escape => controller.set_open(false),
            Key::Down if len > 0 => {
                controller.select((controller.peek_selected() + 1).min(len - 1))
            }
            Key::Up if len > 0 => controller.select(controller.peek_selected().saturating_sub(1)),
            _ => {}
        }))
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
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
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}

/// Alias for [`Select`]. The current version is a non-editable combo box;
/// searchable text entry can be layered on it without changing controller
/// ownership or popup behaviour.
pub type ComboBox = Select;

/// One accessible-looking radio option, controlled by its parent state.
pub struct Radio {
    theme: Theme,
    label: String,
    selected: bool,
    style: creamui_core::Style,
    on_click: Rc<dyn Fn()>,
}
impl_styled_field!(Radio);

impl Radio {
    pub fn new(label: impl Into<String>, selected: bool, on_click: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        Self {
            theme,
            label: label.into(),
            selected,
            style: Self::default_style().into(),
            on_click: Rc::new(on_click),
        }
    }
    pub fn default_style() -> Style {
        Style {
            size: fixed(200.0, 30.0),
            ..Default::default()
        }
    }
}

impl Widget for Radio {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let diameter = 18.0;
        let ring = Rect {
            x: rect.x,
            y: rect.y + (rect.height - diameter) / 2.0,
            width: diameter,
            height: diameter,
        };
        painter.fill_rect(ring, self.theme.surface_elevated, diameter / 2.0);
        painter.stroke_rect(
            ring,
            if self.selected {
                self.theme.accent
            } else {
                self.theme.border_strong
            },
            1.5,
            diameter / 2.0,
        );
        if self.selected {
            painter.fill_rect(
                Rect {
                    x: ring.x + 5.0,
                    y: ring.y + 5.0,
                    width: 8.0,
                    height: 8.0,
                },
                self.theme.accent,
                4.0,
            );
        }
        painter.fill_text(
            Rect {
                x: rect.x + 27.0,
                y: rect.y,
                width: (rect.width - 27.0).max(0.0),
                height: rect.height,
            },
            &self.label,
            self.theme.text_primary,
            self.theme.typography.body,
            TextAlign::Start,
        );
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.on_click.clone())
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let on_click = self.on_click.clone();
        Some(Rc::new(move |input| {
            if matches!(input.key, Key::Enter | Key::Char(' ')) {
                on_click()
            }
        }))
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(
            Rect {
                x: rect.x - 2.0,
                y: rect.y - 2.0,
                width: rect.width + 4.0,
                height: rect.height + 4.0,
            },
            self.theme.accent,
            2.0,
            self.theme.checkbox_radius + 2.0,
        );
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}

/// A vertical group of mutually exclusive [`Radio`] controls.
pub struct RadioGroup {
    selected: usize,
    on_change: Rc<dyn Fn(usize)>,
    options: Vec<String>,
    style: creamui_core::Style,
}
impl_styled_field!(RadioGroup);

impl RadioGroup {
    pub fn new(selected: usize, on_change: impl Fn(usize) + 'static) -> Self {
        let theme = use_theme();
        Self {
            selected,
            on_change: Rc::new(on_change),
            options: Vec::new(),
            style: column(theme.spacing_small).into(),
        }
    }
    pub fn option(mut self, label: impl Into<String>) -> Self {
        self.options.push(label.into());
        self
    }
}

impl Widget for RadioGroup {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn children(&mut self) -> Vec<BoxedWidget> {
        let selected = self.selected;
        let options = &self.options;
        let on_change = &self.on_change;
        options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let on_change = on_change.clone();
                Box::new(Radio::new(label.clone(), selected == index, move || {
                    on_change(index)
                })) as BoxedWidget
            })
            .collect()
    }
}

/// A compact horizontal exclusive-choice control built from the existing
/// `Choice` primitive. Use it when each option is short and immediately
/// comparable; use [`RadioGroup`] for explanatory labels.
pub struct SegmentedControl {
    selected: usize,
    on_change: Rc<dyn Fn(usize)>,
    options: Vec<String>,
    style: creamui_core::Style,
}
impl_styled_field!(SegmentedControl);

impl SegmentedControl {
    pub fn new(selected: usize, on_change: impl Fn(usize) + 'static) -> Self {
        let theme = use_theme();
        Self {
            selected,
            on_change: Rc::new(on_change),
            options: Vec::new(),
            style: row(theme.spacing_small).into(),
        }
    }
    pub fn option(mut self, label: impl Into<String>) -> Self {
        self.options.push(label.into());
        self
    }
}

impl Widget for SegmentedControl {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn children(&mut self) -> Vec<BoxedWidget> {
        let selected = self.selected;
        let options = &self.options;
        let on_change = &self.on_change;
        options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let on_change = on_change.clone();
                Box::new(crate::Choice::new(
                    label.clone(),
                    selected == index,
                    move || on_change(index),
                )) as BoxedWidget
            })
            .collect()
    }
}

/// A scrollable, single-select list of plain-text rows. Selection is fully
/// controlled — like [`RadioGroup`], it takes the current index and an
/// `on_change` callback rather than owning state itself — while scrolling
/// (including the draggable thumb) is delegated to a [`crate::RawScrollView`]
/// built from the given [`ScrollController`]. For hierarchical or
/// column-based data, see [`crate::themed::TreeView`] instead.
pub struct ListBox {
    theme: Theme,
    style: creamui_core::Style,
    scroll: ScrollController,
    options: Vec<String>,
    selected: usize,
    on_change: Rc<dyn Fn(usize)>,
    row_height: f32,
}
impl_styled_field!(ListBox);

impl ListBox {
    pub fn new(
        style: Style,
        scroll: ScrollController,
        selected: usize,
        on_change: impl Fn(usize) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            style: style.into(),
            scroll,
            options: Vec::new(),
            selected,
            on_change: Rc::new(on_change),
            row_height: 32.0,
        }
    }

    pub fn option(mut self, label: impl Into<String>) -> Self {
        self.options.push(label.into());
        self
    }

    pub fn options(mut self, labels: &[&str]) -> Self {
        self.options = labels.iter().map(|label| (*label).to_owned()).collect();
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(1.0);
        self
    }
}

impl Widget for ListBox {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut scroll_view =
            RawScrollView::controlled(fill(Style::default()), self.scroll.clone())
                .background(self.theme.surface_elevated)
                .corner_radius(self.theme.input_radius);
        for (index, label) in self.options.iter().enumerate() {
            let selected = index == self.selected;
            let on_change = self.on_change.clone();
            let row_style = padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(self.row_height),
                    },
                    flex_shrink: 0.0,
                    align_items: Some(AlignItems::Center),
                    ..Default::default()
                },
                self.theme.spacing_medium,
            );
            let background = if selected {
                self.theme.accent
            } else {
                self.theme.surface_elevated
            };
            let foreground = if selected {
                self.theme.selection_text
            } else {
                self.theme.text_primary
            };
            let mut item =
                RawButton::new(row_style, move || on_change(index)).background(background);
            item = item.hover_style(creamui_core::StateStyle::new().background(if selected {
                self.theme.accent_hover
            } else {
                self.theme.surface_hover
            }));
            item = item.child(Box::new(
                RawText::new(label.clone(), foreground, self.theme.typography.body)
                    .text_align(TextAlign::Start),
            ));
            scroll_view = scroll_view.child(Box::new(item));
        }
        vec![Box::new(scroll_view)]
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let on_change = self.on_change.clone();
        let len = self.options.len();
        let selected = self.selected;
        Some(Rc::new(move |input| {
            if len == 0 {
                return;
            }
            match input.key {
                Key::Down => on_change((selected + 1).min(len - 1)),
                Key::Up => on_change(selected.saturating_sub(1)),
                Key::Home => on_change(0),
                Key::End => on_change(len - 1),
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
