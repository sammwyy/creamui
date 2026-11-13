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
    on_select: Option<Rc<dyn Fn(usize)>>,
    searchable: bool,
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
            on_select: None,
            searchable: false,
        }
    }

    pub fn on_select(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(callback));
        self
    }

    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }

    pub fn default_style() -> Style {
        Style {
            size: fixed(220.0, 34.0),
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
        let theme = &self.theme;
        let fill = elevation::control_fill(theme);
        let fill = if painter.pressed(rect) {
            fill.mix(theme.text_primary, 0.08)
        } else if painter.hovered(rect) {
            fill.mix(theme.text_primary, 0.04)
        } else {
            fill
        };
        elevation::paint_control(painter, theme, rect, fill, theme.input_radius);
        if self.controller.is_open() {
            painter.stroke_rect_inside(rect, theme.accent, 1.0, theme.input_radius);
            painter.stroke_rect(
                rect.inflate(3.0),
                elevation::with_alpha(theme.accent, 70),
                3.0,
                theme.input_radius + 3.0,
            );
        }
        painter.fill_text(
            Rect {
                x: rect.x + 12.0,
                y: rect.y,
                width: (rect.width - 40.0).max(0.0),
                height: rect.height,
            },
            self.selected_label(),
            theme.text_primary,
            theme.typography.body,
            TextAlign::Start,
        );
        let glyph = theme.text_secondary;
        let cx = rect.x + rect.width - 16.0;
        let cy = rect.y + rect.height / 2.0;
        for direction in [-1.0f32, 1.0] {
            let tip = cy + direction * 5.0;
            let base = cy + direction * 1.5;
            painter.stroke_line(
                Point {
                    x: cx - 3.5,
                    y: base,
                },
                Point { x: cx, y: tip },
                glyph,
                1.5,
            );
            painter.stroke_line(
                Point {
                    x: cx + 3.5,
                    y: base,
                },
                Point { x: cx, y: tip },
                glyph,
                1.5,
            );
        }
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        if !self.controller.is_open() || self.options.is_empty() {
            return Vec::new();
        }
        let option_height = 32.0;
        let query_controller = self.controller.query();
        let query = query_controller.value().to_ascii_lowercase();
        let matching: Vec<_> = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, label)| query.is_empty() || label.to_ascii_lowercase().contains(&query))
            .collect();
        let list_height = (matching.len().max(1) as f32 * option_height).min(204.0);
        let popup_style = padding(
            Style {
                position: Position::Absolute,
                inset: creamui_core::layout::Rect {
                    left: LengthPercentageAuto::Length(0.0),
                    right: LengthPercentageAuto::Auto,
                    top: LengthPercentageAuto::Percent(1.0),
                    bottom: LengthPercentageAuto::Auto,
                },
                margin: creamui_core::layout::Rect {
                    top: LengthPercentageAuto::Length(4.0),
                    ..Style::default().margin
                },
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(
                        list_height + if self.searchable { 50.0 } else { 12.0 },
                    ),
                },
                ..column(2.0)
            },
            6.0,
        );
        let mut popup = Popover::new(popup_style);
        if self.searchable {
            popup = popup.child(Box::new(
                TextInput::controlled(&query_controller)
                    .placeholder("Search…")
                    .layout(Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Length(34.0),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    }),
            ));
        }
        let mut options = RawView::new(Style {
            flex_direction: creamui_core::layout::FlexDirection::Column,
            flex_shrink: 0.0,
            ..Default::default()
        });
        if matching.is_empty() {
            options = options.child(Box::new(
                RawText::new(
                    "No results",
                    self.theme.text_secondary,
                    self.theme.typography.body,
                )
                .height(option_height)
                .text_align(TextAlign::Center),
            ));
        }
        for (index, label) in matching {
            let selected = self.controller.selected() == index;
            let controller = self.controller.clone();
            let on_select = self.on_select.clone();
            let label = label.clone();
            let item_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(option_height),
                },
                padding: creamui_core::layout::Rect {
                    left: creamui_core::layout::LengthPercentage::Length(12.0),
                    right: creamui_core::layout::LengthPercentage::Length(12.0),
                    top: creamui_core::layout::LengthPercentage::Length(0.0),
                    bottom: creamui_core::layout::LengthPercentage::Length(0.0),
                },
                align_items: Some(AlignItems::Center),
                flex_direction: creamui_core::layout::FlexDirection::Row,
                flex_shrink: 0.0,
                ..Default::default()
            };
            let background = if selected {
                elevation::with_alpha(self.theme.accent, 40)
            } else {
                elevation::with_alpha(self.theme.text_primary, 0)
            };
            let foreground = self.theme.text_primary;
            let mut item = RawButton::new(item_style, move || {
                controller.select(index);
                if let Some(callback) = &on_select {
                    callback(index);
                }
            })
            .background(background)
            .corner_radius(self.theme.menu_item_radius)
            .child(Box::new(
                RawText::new(label, foreground, self.theme.typography.body)
                    .text_align(TextAlign::Start)
                    .font_family(self.theme.font_family)
                    .layout(Style {
                        flex_grow: 1.0,
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Auto,
                        },
                        ..Default::default()
                    }),
            ));
            if selected {
                item = item.child(Box::new(
                    crate::Icon::new(crate::Symbol::Check, self.theme.accent).size(14.0),
                ));
            }
            item = item
                .hover_style(creamui_core::StateStyle::new().background(if selected {
                    elevation::with_alpha(self.theme.accent, 60)
                } else {
                    elevation::with_alpha(self.theme.text_primary, 16)
                }))
                .focus_style(creamui_core::StateStyle::new().border(self.theme.accent, 1.0));
            options = options.child(Box::new(item));
        }
        let list_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(list_height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };
        popup = popup.child(Box::new(
            RawScrollView::controlled(list_style, self.controller.scroll())
                .child(Box::new(options)),
        ));
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
            rect.inflate(3.0),
            elevation::focus_ring(&self.theme),
            2.0,
            self.theme.input_radius + 3.0,
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
        let hovered = painter.hovered(rect);
        if self.selected {
            let fill = if hovered {
                self.theme.accent_hover
            } else {
                self.theme.accent
            };
            painter.fill_rect(ring, fill, diameter / 2.0);
            let dot = 6.0;
            painter.fill_rect(
                Rect {
                    x: ring.x + (diameter - dot) / 2.0,
                    y: ring.y + (diameter - dot) / 2.0,
                    width: dot,
                    height: dot,
                },
                self.theme.accent.readable_text(),
                dot / 2.0,
            );
        } else {
            painter.fill_rect(ring, elevation::field_fill(&self.theme), diameter / 2.0);
            painter.stroke_rect_inside(
                ring,
                elevation::with_alpha(self.theme.text_primary, if hovered { 140 } else { 90 }),
                1.5,
                diameter / 2.0,
            );
        }
        painter.fill_text(
            Rect {
                x: rect.x + 28.0,
                y: rect.y,
                width: (rect.width - 28.0).max(0.0),
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
            style: creamui_core::Style::new()
                .layout(Style {
                    align_self: Some(creamui_core::layout::AlignSelf::Start),
                    ..padding(row(2.0), 3.0)
                })
                .background(elevation::track_fill(&theme))
                .border(elevation::with_alpha(theme.text_primary, 10), 1.0)
                .corner_radius(theme.tab_radius + 3.0),
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
            RawScrollView::controlled(padding(fill(Style::default()), 4.0), self.scroll.clone())
                .background(elevation::group_fill(&self.theme))
                .border(self.theme.border, 1.0)
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
                    flex_direction: creamui_core::layout::FlexDirection::Row,
                    ..Default::default()
                },
                self.theme.spacing_medium,
            );
            let background = if selected {
                elevation::with_alpha(self.theme.accent, 40)
            } else {
                elevation::with_alpha(self.theme.text_primary, 0)
            };
            let foreground = self.theme.text_primary;
            let mut item = RawButton::new(row_style, move || on_change(index))
                .background(background)
                .corner_radius(self.theme.menu_item_radius);
            item = item.hover_style(creamui_core::StateStyle::new().background(if selected {
                elevation::with_alpha(self.theme.accent, 60)
            } else {
                elevation::with_alpha(self.theme.text_primary, 14)
            }));
            item = item.child(Box::new(
                RawText::new(label.clone(), foreground, self.theme.typography.body)
                    .text_align(TextAlign::Start)
                    .font_family(self.theme.font_family)
                    .layout(Style {
                        flex_grow: 1.0,
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Auto,
                        },
                        ..Default::default()
                    }),
            ));
            if selected {
                item = item.child(Box::new(
                    crate::Icon::new(crate::Symbol::Check, self.theme.accent).size(14.0),
                ));
            }
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
            rect.inflate(3.0),
            elevation::focus_ring(&self.theme),
            2.0,
            self.theme.input_radius + 3.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_popup_has_breathing_room_and_a_quiet_selected_row() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let controller = crate::SelectController::default();
            controller.set_open(true);
            let mut select = Select::controlled(&["System", "Light", "Dark"], controller);
            let mut children = select.children();
            let mut popup = children.remove(1);
            assert_eq!(
                popup.style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(6.0)
            );
            let mut list = popup.children().remove(0);
            let mut clip = list.children().remove(0);
            let mut content = clip.children().remove(0);
            let mut rows = content.children().remove(0);
            let mut selected = rows.children().remove(0);
            assert_eq!(
                selected.style().paint.background,
                Some(elevation::with_alpha(theme.accent, 40).into())
            );
            assert_eq!(selected.children().len(), 2);
        });
    }

    #[test]
    fn list_box_selected_row_matches_the_popup_treatment() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let mut list = ListBox::new(
                Style {
                    size: fixed(220.0, 200.0),
                    ..Default::default()
                },
                ScrollController::default(),
                0,
                |_| {},
            )
            .options(&["Apple", "Banana"]);
            let mut scroll = list.children().remove(0);
            let mut clip = scroll.children().remove(0);
            let mut content = clip.children().remove(0);
            let mut selected = content.children().remove(0);
            assert_eq!(
                selected.style().paint.background,
                Some(elevation::with_alpha(theme.accent, 40).into())
            );
            assert_eq!(selected.children().len(), 2);
        });
    }

    #[test]
    fn segmented_options_share_a_single_control_track() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let mut segments = SegmentedControl::new(1, |_| {})
                .option("Day")
                .option("Week")
                .option("Month");
            let style = segments.style();
            assert_eq!(
                style.paint.background,
                Some(elevation::track_fill(&theme).into())
            );
            assert_eq!(
                style.layout.align_self,
                Some(creamui_core::layout::AlignSelf::Start)
            );
            assert_eq!(
                style.layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(3.0)
            );
            assert_eq!(segments.children().len(), 3);
        });
    }
}
