use super::*;
use crate::layout::{column, padding, row};
use crate::{Dialog, DialogContent, DialogFooter, Icon, ScrollController, Symbol, TextController};
use creamui_reactive::{batch, Signal};

#[derive(Clone)]
pub struct CommandPaletteItem {
    pub id: String,
    pub label: String,
    pub category: String,
    pub shortcut: String,
    action: Rc<dyn Fn()>,
}
impl CommandPaletteItem {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: String::new(),
            shortcut: String::new(),
            action: Rc::new(action),
        }
    }
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }
    pub fn invoke(&self) {
        (self.action)();
    }
}
#[derive(Clone)]
pub struct CommandPaletteController {
    query: TextController,
    selected: Signal<usize>,
    scroll: ScrollController,
}
impl CommandPaletteController {
    pub fn new() -> Self {
        Self::with_query(TextController::default())
    }
    pub fn with_query(query: TextController) -> Self {
        let selected = Signal::new(0);
        let scroll = ScrollController::new(0.0);
        let reset = selected.clone();
        let position = scroll.clone();
        query.on_change(move |_, next| {
            batch(|| {
                reset.set(0);
                position.set(0.0);
            });
            Some(next.to_owned())
        });
        Self {
            query,
            selected,
            scroll,
        }
    }
    pub fn query(&self) -> TextController {
        self.query.clone()
    }
    pub fn reset(&self) {
        batch(|| {
            self.query.set_value("");
            self.selected.set(0);
            self.scroll.set(0.0);
        });
    }
    pub fn matching(&self, items: &[CommandPaletteItem]) -> Vec<CommandPaletteItem> {
        filter(items, &self.query.value())
    }
    pub fn selected(&self) -> usize {
        self.selected.get()
    }
    fn navigate(&self, items: &[CommandPaletteItem], direction: isize) {
        let matches = self.matching(items);
        if matches.is_empty() {
            return;
        }
        let selected =
            (self.selected.peek() as isize + direction).rem_euclid(matches.len() as isize) as usize;
        let mut offset = selected as f32 * 33.0;
        let mut previous = "";
        if self.query.peek().is_empty() {
            for item in matches.iter().take(selected + 1) {
                if !item.category.is_empty() && item.category != previous {
                    offset += 20.0;
                    previous = &item.category;
                }
            }
        }
        batch(|| {
            self.selected.set(selected);
            self.scroll.set((offset - 110.0).max(0.0));
        });
    }
    pub fn activate(&self, items: &[CommandPaletteItem]) {
        if let Some(item) = self.matching(items).get(self.selected.peek()) {
            item.invoke();
        }
    }
    pub fn handle_key(
        &self,
        input: KeyInput,
        items: &[CommandPaletteItem],
        dismiss: impl Fn(),
    ) -> bool {
        match input.key {
            creamui_core::Key::Up => self.navigate(items, -1),
            creamui_core::Key::Down => self.navigate(items, 1),
            creamui_core::Key::Enter => self.activate(items),
            creamui_core::Key::Escape => dismiss(),
            creamui_core::Key::Tab => return false,
            _ => {
                let change = self.query.clone();
                let cursor = self.query.clone();
                let selection = self.query.clone();
                let paste = self.query.clone();
                let editor = RawTextInput::new(
                    Style::default(),
                    self.query.peek(),
                    14.0,
                    Color::rgba(0, 0, 0, 0),
                    move |value| change.set_value(value),
                )
                .cursor(self.query.cursor(), move |value| cursor.set_cursor(value))
                .selection(self.query.selection(), move |value| {
                    selection.set_selection(value)
                })
                .paste_handler(move || paste.paste());
                if let Some(handler) = editor.on_key() {
                    handler(input);
                } else {
                    return false;
                }
            }
        }
        true
    }
}
impl Default for CommandPaletteController {
    fn default() -> Self {
        Self::new()
    }
}
fn filter(items: &[CommandPaletteItem], query: &str) -> Vec<CommandPaletteItem> {
    let query = query.to_lowercase();
    let mut result: Vec<_> = items
        .iter()
        .filter(|item| {
            let text = format!(
                "{} {} {} {}",
                item.id, item.label, item.category, item.shortcut
            )
            .to_lowercase();
            query.split_whitespace().all(|word| text.contains(word))
        })
        .cloned()
        .collect();
    if !query.is_empty() {
        result.sort_by_key(|item| !item.label.to_lowercase().starts_with(&query));
    }
    result
}

pub struct CommandPalette {
    controller: CommandPaletteController,
    items: Rc<Vec<CommandPaletteItem>>,
    dismiss: Rc<dyn Fn()>,
    query: String,
    selected: usize,
    viewport: creamui_core::Size,
    width: f32,
    max_results_height: f32,
    placeholder: String,
    hint: String,
    reference: bool,
}
impl CommandPalette {
    pub fn new(
        controller: &CommandPaletteController,
        items: Vec<CommandPaletteItem>,
        dismiss: impl Fn() + 'static,
    ) -> Self {
        Self {
            controller: controller.clone(),
            items: Rc::new(items),
            dismiss: Rc::new(dismiss),
            query: controller.query.value(),
            selected: controller.selected.get(),
            viewport: creamui_core::Size {
                width: 1000.0,
                height: 700.0,
            },
            width: 520.0,
            max_results_height: 300.0,
            placeholder: "Search commands…".into(),
            hint: "↑ ↓  Navigate    Enter  Run    Esc  Close".into(),
            reference: false,
        }
    }
    pub fn viewport(mut self, size: creamui_core::Size) -> Self {
        self.viewport = size;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(200.0);
        self
    }
    pub fn max_results_height(mut self, height: f32) -> Self {
        self.max_results_height = height.max(48.0);
        self
    }
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
    pub fn reference(mut self, reference: bool) -> Self {
        self.reference = reference;
        self
    }
}
impl Widget for CommandPalette {
    fn is_modal(&self) -> bool {
        true
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn style(&self) -> creamui_core::Style {
        Dialog::custom(|| {}).style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, Color::rgba(0, 0, 0, 104), 0.0);
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.dismiss.clone())
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let theme = use_theme();
        let width = self.width.min((self.viewport.width - 48.0).max(200.0));
        let submit = self.controller.clone();
        let commands = self.items.clone();
        let keys = self.controller.clone();
        let navigation = self.items.clone();
        let escape = self.dismiss.clone();
        let input = TextInput::controlled_with_style(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(36.0),
                },
                ..Default::default()
            },
            &self.controller.query,
        )
        .placeholder(self.placeholder.clone())
        .background(theme.surface_elevated)
        .border(theme.surface_elevated, 0.0)
        .corner_radius(0.0)
        .on_submit(move || submit.activate(&commands))
        .on_key_press(move |input| match input.key {
            creamui_core::Key::Up => keys.navigate(&navigation, -1),
            creamui_core::Key::Down => keys.navigate(&navigation, 1),
            creamui_core::Key::Escape => escape(),
            _ => {}
        });
        let close = self.dismiss.clone();
        let header = Box::new(
            RawView::new(padding(
                Style {
                    align_items: Some(AlignItems::Center),
                    ..row(8.0)
                },
                8.0,
            ))
            .with_children(vec![
                Box::new(
                    Icon::new(
                        if self.reference {
                            Symbol::Keyboard
                        } else {
                            Symbol::Search
                        },
                        theme.text_secondary,
                    )
                    .size(16.0),
                ),
                Box::new(input),
                Box::new(Button::styled(
                    ButtonVariant::Tertiary,
                    ButtonSize::Xs,
                    "Esc",
                    ButtonState::Normal,
                    move || close(),
                )),
            ]),
        );
        let items = filter(&self.items, &self.query);
        let selected = self.selected.min(items.len().saturating_sub(1));
        let mut rows = Vec::new();
        let mut previous = "";
        let mut height: f32 = 0.0;
        for (index, item) in items.iter().enumerate() {
            if self.query.is_empty() && !item.category.is_empty() && item.category != previous {
                rows.push(Box::new(
                    RawView::new(padding(row(0.0), 3.0))
                        .child(Box::new(Text::secondary(&item.category).font_size(10.0))),
                ) as BoxedWidget);
                height += 20.0;
                previous = &item.category;
            }
            let action = item.clone();
            let shortcut = RawView::new(padding(row(0.0), 4.0))
                .background(theme.surface)
                .corner_radius(4.0)
                .child(Box::new(Text::secondary(&item.shortcut).font_size(10.0)));
            rows.push(Box::new(
                RawButton::new(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(width - 24.0),
                            height: Dimension::Length(32.0),
                        },
                        padding: creamui_core::layout::Rect {
                            left: LengthPercentage::Length(8.0),
                            right: LengthPercentage::Length(8.0),
                            top: LengthPercentage::Length(0.0),
                            bottom: LengthPercentage::Length(0.0),
                        },
                        align_items: Some(AlignItems::Center),
                        flex_shrink: 0.0,
                        ..Default::default()
                    },
                    move || action.invoke(),
                )
                .background(if !self.reference && index == selected {
                    theme.surface_hover
                } else {
                    Color::rgba(0, 0, 0, 0)
                })
                .corner_radius(6.0)
                .keyboard_focus(false)
                .hover_style(creamui_core::StateStyle::new().background(theme.surface_hover))
                .with_children(vec![
                    Box::new(Text::new(&item.label).font_size(12.0)),
                    Box::new(RawView::new(Style {
                        flex_grow: 1.0,
                        ..Default::default()
                    })),
                    if item.shortcut.is_empty() {
                        Box::new(RawView::new(Style {
                            display: creamui_core::layout::Display::None,
                            ..Default::default()
                        })) as BoxedWidget
                    } else {
                        Box::new(shortcut)
                    },
                ]),
            ) as BoxedWidget);
            height += 33.0;
        }
        if rows.is_empty() {
            rows.push(Box::new(RawView::new(padding(column(0.0), 12.0)).child(
                Box::new(Text::secondary("No matching commands").font_size(12.0)),
            )));
            height = 48.0;
        }
        let height =
            height.min((self.viewport.height - 136.0).clamp(48.0, self.max_results_height));
        let list = RawScrollView::controlled(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(height),
                },
                ..Default::default()
            },
            self.controller.scroll.clone(),
        )
        .child(Box::new(RawView::new(column(1.0)).with_children(rows)));
        let dismiss = self.dismiss.clone();
        let mut dialog = Dialog::custom(move || dismiss())
            .width(width)
            .header(header)
            .content(Box::new(
                DialogContent::new().compact().child(Box::new(list)),
            ))
            .footer(Box::new(
                DialogFooter::new()
                    .compact()
                    .child(Box::new(Text::secondary(&self.hint).font_size(10.0))),
            ));
        dialog.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Key, Modifiers, Renderer, Size};
    use creamui_theme::ThemeProvider;
    use std::cell::Cell;
    fn input(key: Key) -> KeyInput {
        KeyInput {
            key,
            modifiers: Modifiers::default(),
        }
    }
    #[test]
    fn filtering_keyboard_navigation_and_query_reset_share_one_controller() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            let controller = CommandPaletteController::new();
            let chosen = Rc::new(Cell::new(0));
            let first = chosen.clone();
            let second = chosen.clone();
            let items = vec![
                CommandPaletteItem::new("new_tab", "New tab", move || first.set(1))
                    .shortcut("Ctrl+T")
                    .category("Tabs"),
                CommandPaletteItem::new("close_tab", "Close tab", move || second.set(2))
                    .shortcut("Ctrl+W")
                    .category("Tabs"),
            ];
            controller.handle_key(input(Key::Down), &items, || {});
            controller.handle_key(input(Key::Enter), &items, || {});
            assert_eq!(chosen.get(), 2);
            controller.query().set_value("ctrl t");
            assert_eq!(controller.selected(), 0);
            assert_eq!(controller.matching(&items)[0].id, "new_tab");
            controller.handle_key(input(Key::Enter), &items, || {});
            assert_eq!(chosen.get(), 1);
            controller.reset();
            for character in "close".chars() {
                controller.handle_key(input(Key::Char(character)), &items, || {});
            }
            assert_eq!(controller.query().peek(), "close");
            assert_eq!(controller.matching(&items)[0].id, "close_tab");
            let dismissed = Cell::new(false);
            controller.handle_key(input(Key::Escape), &items, || dismissed.set(true));
            assert!(dismissed.get());
        });
    }
    #[derive(Default)]
    struct Recorder {
        surfaces: Vec<(Rect, Color)>,
    }
    impl Painter for Recorder {
        fn fill_rect(&mut self, rect: Rect, color: Color, _: f32) {
            self.surfaces.push((rect, color));
        }
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }
    #[test]
    fn palette_shrinks_to_filtered_results_and_keeps_text_focus() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let controller = CommandPaletteController::new();
            let items: Vec<_> = (0..20)
                .map(|index| {
                    CommandPaletteItem::new(index.to_string(), format!("Command {index}"), || {})
                })
                .collect();
            let size = Size {
                width: 900.0,
                height: 600.0,
            };
            let mut heights = Vec::new();
            for query in ["", "Command 19"] {
                controller.query().set_value(query);
                let mut recorder = Recorder::default();
                let scene = Renderer::new().render(
                    Box::new(CommandPalette::new(&controller, items.clone(), || {}).viewport(size)),
                    size,
                    &mut recorder,
                );
                assert!(scene.first_text_input().is_some());
                heights.push(
                    recorder
                        .surfaces
                        .iter()
                        .filter(|(rect, color)| {
                            rect.width > 400.0
                                && color.r == theme.surface_elevated.r
                                && color.g == theme.surface_elevated.g
                                && color.b == theme.surface_elevated.b
                        })
                        .map(|(rect, _)| rect.height)
                        .reduce(f32::max)
                        .unwrap(),
                );
            }
            assert!(heights[1] < heights[0]);
        });
    }
}
