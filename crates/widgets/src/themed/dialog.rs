use super::*;
use crate::layout::{column, padding, row};
use crate::{Icon, IconSource, Symbol};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogPlacement {
    #[default]
    Center,
    Top,
    TopLeft,
    Left,
}

struct DialogAction {
    label: String,
    variant: ButtonVariant,
    on_click: Rc<dyn Fn()>,
}

pub struct Dialog {
    dismiss: Rc<dyn Fn()>,
    width: f32,
    placement: DialogPlacement,
    header: Option<BoxedWidget>,
    content: Vec<BoxedWidget>,
    footer: Option<BoxedWidget>,
    actions: Vec<DialogAction>,
}
impl Dialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        Self::custom(on_dismiss)
            .header(Box::new(DialogHeader::new(title)))
            .content(Box::new(
                DialogContent::new().child(Box::new(DialogMessage::new(message))),
            ))
    }
    pub fn custom(on_dismiss: impl Fn() + 'static) -> Self {
        Self {
            dismiss: Rc::new(on_dismiss),
            width: 440.0,
            placement: DialogPlacement::Center,
            header: None,
            content: Vec::new(),
            footer: None,
            actions: Vec::new(),
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(160.0);
        self
    }
    pub fn placement(mut self, placement: DialogPlacement) -> Self {
        self.placement = placement;
        self
    }
    pub fn header(mut self, header: BoxedWidget) -> Self {
        self.header = Some(header);
        self
    }
    pub fn content(mut self, content: BoxedWidget) -> Self {
        self.content.push(content);
        self
    }
    pub fn footer(mut self, footer: BoxedWidget) -> Self {
        self.footer = Some(footer);
        self
    }
    pub fn action(
        mut self,
        label: impl Into<String>,
        variant: ButtonVariant,
        on_click: impl Fn() + 'static,
    ) -> Self {
        self.actions.push(DialogAction {
            label: label.into(),
            variant,
            on_click: Rc::new(on_click),
        });
        self
    }
    pub fn dismiss_action(mut self, label: impl Into<String>) -> Self {
        self.actions.push(DialogAction {
            label: label.into(),
            variant: ButtonVariant::Secondary,
            on_click: self.dismiss.clone(),
        });
        self
    }
}
impl Widget for Dialog {
    fn is_modal(&self) -> bool {
        true
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn style(&self) -> creamui_core::Style {
        let (align, justify) = match self.placement {
            DialogPlacement::Center => (AlignItems::Center, JustifyContent::Center),
            DialogPlacement::Top => (AlignItems::Center, JustifyContent::Start),
            DialogPlacement::TopLeft => (AlignItems::Start, JustifyContent::Start),
            DialogPlacement::Left => (AlignItems::Start, JustifyContent::Center),
        };
        let mut style = Overlay::fullscreen(|| {}).style().layout;
        style.flex_direction = creamui_core::layout::FlexDirection::Column;
        style.align_items = Some(align);
        style.justify_content = Some(justify);
        padding(style, use_theme().spacing_large).into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, Color::rgba(0, 0, 0, 104), 0.0);
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.dismiss.clone())
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut sections = Vec::new();
        if let Some(header) = self.header.take() {
            sections.push(header);
        }
        sections.append(&mut self.content);
        if let Some(footer) = self.footer.take() {
            sections.push(footer);
        } else if !self.actions.is_empty() {
            let mut actions = ModalActions::new();
            for action in std::mem::take(&mut self.actions) {
                actions = actions.child(Button::styled(
                    action.variant,
                    ButtonSize::Md,
                    action.label,
                    ButtonState::Normal,
                    move || (action.on_click)(),
                ));
            }
            sections.push(Box::new(DialogFooter::new().child(Box::new(actions))));
        }
        vec![Box::new(
            Popover::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(self.width),
                    height: Dimension::Auto,
                },
                max_size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                flex_shrink: 0.0,
                ..column(0.0)
            })
            .with_children(sections),
        )]
    }
}

pub struct DialogHeader {
    inner: RawView,
    title: String,
    text_index: usize,
}
impl_styled_inner!(DialogHeader);
impl DialogHeader {
    pub fn new(title: impl Into<String>) -> Self {
        let theme = use_theme();
        let title = title.into();
        Self {
            title: title.clone(),
            text_index: 0,
            inner: RawView::new(padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    flex_shrink: 0.0,
                    align_items: Some(AlignItems::Center),
                    ..row(theme.spacing_medium)
                },
                theme.spacing_large,
            ))
            .child(Box::new(
                RawView::new(Style {
                    flex_grow: 1.0,
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Length(0.0),
                    },
                    ..column(theme.spacing_small)
                })
                .child(Box::new(
                    Heading::md(title.clone()).text_align(TextAlign::Start),
                )),
            )),
        }
    }
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.inner.children[self.text_index] = Box::new(
            RawView::new(Style {
                flex_grow: 1.0,
                ..column(use_theme().spacing_small)
            })
            .with_children(vec![
                Box::new(Heading::md(self.title.clone()).text_align(TextAlign::Start)),
                Box::new(
                    Text::secondary(description)
                        .font_size(12.0)
                        .text_align(TextAlign::Start),
                ),
            ]),
        );
        self
    }
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.inner.children.insert(
            0,
            Box::new(Icon::new(icon, use_theme().text_secondary).size(22.0)),
        );
        self.text_index += 1;
        self
    }
    pub fn trailing(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.child(widget);
        self
    }
    pub fn on_close(self, on_close: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        self.trailing(Box::new(
            RawButton::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(28.0),
                        height: Dimension::Length(28.0),
                    },
                    justify_content: Some(JustifyContent::Center),
                    align_items: Some(AlignItems::Center),
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                on_close,
            )
            .corner_radius(theme.radius_small)
            .hover_style(creamui_core::StateStyle::new().background(theme.surface_hover))
            .child(Box::new(
                Icon::new(Symbol::Close, theme.text_secondary).size(14.0),
            )),
        ))
    }
}

pub struct DialogContent {
    inner: RawView,
}
impl_styled_inner!(DialogContent);
impl DialogContent {
    pub fn new() -> Self {
        let theme = use_theme();
        let mut style = padding(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                flex_shrink: 0.0,
                ..column(theme.spacing_medium)
            },
            theme.spacing_large,
        );
        style.padding.top = LengthPercentage::Length(0.0);
        Self {
            inner: RawView::new(style),
        }
    }
    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.child(widget);
        self
    }
    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }
}
impl Default for DialogContent {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DialogFooter {
    inner: RawView,
}
impl_styled_inner!(DialogFooter);
impl DialogFooter {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            inner: RawView::new(padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    flex_shrink: 0.0,
                    ..column(theme.spacing_small)
                },
                theme.spacing_large,
            )),
        }
    }
    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.child(widget);
        self
    }
    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }
}
impl Default for DialogFooter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ModalActions {
    inner: RawView,
}
impl_styled_inner!(ModalActions);
impl ModalActions {
    pub fn new() -> Self {
        Self {
            inner: RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                flex_shrink: 0.0,
                ..row(use_theme().spacing_small)
            }),
        }
    }
    pub fn gap(mut self, gap: f32) -> Self {
        self.inner.style.layout.gap = creamui_core::layout::Size {
            width: LengthPercentage::Length(gap.max(0.0)),
            height: LengthPercentage::Length(gap.max(0.0)),
        };
        self
    }
    pub fn child<W: Widget + Styled + 'static>(mut self, mut widget: W) -> Self {
        let mut style = widget.style();
        style.layout.flex_grow = 1.0;
        style.layout.flex_basis = Dimension::Length(0.0);
        style.layout.size.width = Dimension::Length(0.0);
        style.layout.min_size.width = Dimension::Length(0.0);
        widget.set_style(style);
        self.inner = self.inner.child(Box::new(widget));
        self
    }
}
impl Default for ModalActions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DialogMessage {
    inner: RawView,
}
impl_styled_inner!(DialogMessage);
impl DialogMessage {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            inner: RawView::new(Style {
                align_items: Some(AlignItems::Start),
                ..row(use_theme().spacing_medium)
            })
            .child(Box::new(
                Text::secondary(message)
                    .text_align(TextAlign::Start)
                    .layout(Style {
                        flex_grow: 1.0,
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Length(0.0),
                        },
                        ..Default::default()
                    }),
            )),
        }
    }
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.inner.children.insert(
            0,
            Box::new(Icon::new(icon, use_theme().text_secondary).size(24.0)),
        );
        self
    }
}
macro_rules! compact_section {
    ($($name:ident),+)=>{$(impl $name {
        pub fn compact(mut self)->Self {
            let inset=LengthPercentage::Length(use_theme().spacing_medium+use_theme().spacing_small);
            self.inner.style.layout.padding=creamui_core::layout::Rect{left:inset,right:inset,top:inset,bottom:inset};
            self
        }
    })+};
}
compact_section!(DialogHeader, DialogFooter);
impl DialogContent {
    pub fn compact(mut self) -> Self {
        let inset =
            LengthPercentage::Length(use_theme().spacing_medium + use_theme().spacing_small);
        self.inner.style.layout.padding = creamui_core::layout::Rect {
            left: inset,
            right: inset,
            top: LengthPercentage::Length(0.0),
            bottom: inset,
        };
        self
    }
}
macro_rules! container_widget {
    ($($name:ident),+)=>{$(impl Widget for $name {
        fn style(&self)->creamui_core::Style{self.inner.style()}
        fn paint(&self,painter:&mut dyn Painter,rect:Rect){self.inner.paint(painter,rect)}
        fn children(&mut self)->Vec<BoxedWidget>{self.inner.children()}
    })+};
}
container_widget!(
    DialogHeader,
    DialogContent,
    DialogFooter,
    ModalActions,
    DialogMessage
);

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Key, Modifiers, Renderer, Size};
    use creamui_theme::ThemeProvider;
    use std::cell::Cell;
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
    #[test]
    fn actions_share_the_available_width_and_keep_callbacks() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
            let clicked = Rc::new(Cell::new(0));
            let mut actions = ModalActions::new().gap(8.0);
            for index in 0..3 {
                let clicked = clicked.clone();
                actions = actions.child(Button::styled(
                    ButtonVariant::Secondary,
                    ButtonSize::Md,
                    format!("Action {index}"),
                    ButtonState::Normal,
                    move || clicked.set(clicked.get() + 1),
                ));
            }
            let size = Size {
                width: 360.0,
                height: 36.0,
            };
            let root = RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(360.0),
                    height: Dimension::Length(36.0),
                },
                ..column(0.0)
            })
            .child(Box::new(actions));
            let scene = Renderer::new().render(Box::new(root), size, &mut Recorder::default());
            let mut widths = [0; 3];
            for x in 0..360 {
                if let Some(index) = scene.focus_hit_test(Point {
                    x: x as f32 + 0.5,
                    y: 18.0,
                }) {
                    widths[index] += 1;
                }
            }
            assert!(*widths.iter().max().unwrap() - *widths.iter().min().unwrap() <= 1);
            assert_eq!(widths.iter().sum::<i32>(), 344);
            for x in [50.0, 180.0, 310.0] {
                scene.hit_test(Point { x, y: 18.0 }).unwrap()();
            }
            assert_eq!(clicked.get(), 3);
        });
    }
    #[test]
    fn custom_dialog_hosts_inputs_and_isolates_background_interactions() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            let dismissed = Rc::new(Cell::new(false));
            let dismiss = dismissed.clone();
            let text = crate::TextController::default();
            let dialog = Dialog::custom(move || dismiss.set(true))
                .width(360.0)
                .placement(DialogPlacement::TopLeft)
                .header(Box::new(
                    DialogHeader::new("Position")
                        .icon(Symbol::Search)
                        .description("Choose a location"),
                ))
                .content(Box::new(
                    DialogContent::new().child(Box::new(TextInput::controlled(&text))),
                ));
            let size = Size {
                width: 800.0,
                height: 600.0,
            };
            let mut painter = Recorder::default();
            let scene = Renderer::new().render(Box::new(dialog), size, &mut painter);
            let title = painter
                .text
                .iter()
                .find(|(text, _)| text == "Position")
                .unwrap()
                .1;
            assert!(title.x < 100.0 && title.y < 100.0);
            let field = scene.first_text_input().unwrap();
            scene.on_key_at(field).unwrap()(KeyInput {
                key: Key::Char('4'),
                modifiers: Modifiers::default(),
            });
            assert_eq!(text.peek(), "4");
            scene.hit_test(Point { x: 750.0, y: 550.0 }).unwrap()();
            assert!(dismissed.get());
        });
    }
}
