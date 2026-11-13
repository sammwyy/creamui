use super::*;
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentageAuto, Position};
use creamui_reactive::Signal;

#[derive(Clone)]
pub struct ToolbarController {
    width: Signal<f32>,
    active: Signal<usize>,
    open: Signal<bool>,
}

impl ToolbarController {
    pub fn new() -> Self {
        Self {
            width: Signal::new(0.0),
            active: Signal::new(0),
            open: Signal::new(false),
        }
    }

    pub fn active(&self) -> usize {
        self.active.get()
    }

    pub fn set_active(&self, active: usize) {
        self.active.set_if_changed(active);
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn set_open(&self, open: bool) {
        self.open.set_if_changed(open);
    }
}

impl Default for ToolbarController {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ToolbarCommand {
    pub width: f32,
    pub style: creamui_core::Style,
    content: Option<BoxedWidget>,
    on_activate: Rc<dyn Fn()>,
    disabled: bool,
}

impl ToolbarCommand {
    pub fn new(
        width: f32,
        style: impl Into<creamui_core::Style>,
        content: BoxedWidget,
        on_activate: impl Fn() + 'static,
    ) -> Self {
        Self {
            width: width.max(1.0),
            style: style.into(),
            content: Some(content),
            on_activate: Rc::new(on_activate),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

struct ToolbarAction {
    style: creamui_core::Style,
    content: Option<BoxedWidget>,
    on_activate: Rc<dyn Fn()>,
    controller: ToolbarController,
    index: usize,
    disabled: bool,
    overflow: bool,
}

impl Widget for ToolbarAction {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        creamui_core::StyleState::NORMAL
            .with_pressed(
                self.controller.active() == self.index
                    && (!self.overflow || self.controller.is_open()),
            )
            .with_disabled(self.disabled)
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.content.take().into_iter().collect()
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        if self.disabled {
            return None;
        }
        let controller = self.controller.clone();
        let index = self.index;
        let action = self.on_activate.clone();
        let overflow = self.overflow;
        Some(Rc::new(move || {
            controller.set_active(index);
            if overflow {
                controller.set_open(false);
            }
            action();
        }))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

pub struct RawToolbar {
    pub style: creamui_core::Style,
    controller: ToolbarController,
    groups: Vec<Vec<ToolbarCommand>>,
    actions: Vec<(Rc<dyn Fn()>, bool)>,
    overflow_trigger: Option<BoxedWidget>,
    menu_style: creamui_core::Style,
    group_gap: f32,
    overflow_width: f32,
    menu_above: bool,
    visible_commands: Rc<Cell<usize>>,
}

impl RawToolbar {
    pub fn new(style: impl Into<creamui_core::Style>, controller: &ToolbarController) -> Self {
        let _ = controller.width.get();
        let _ = controller.active();
        let _ = controller.is_open();
        Self {
            style: style.into(),
            controller: controller.clone(),
            groups: Vec::new(),
            actions: Vec::new(),
            overflow_trigger: None,
            menu_style: Style::default().into(),
            group_gap: 8.0,
            overflow_width: 44.0,
            menu_above: false,
            visible_commands: Rc::new(Cell::new(0)),
        }
    }

    pub fn group(mut self, commands: Vec<ToolbarCommand>) -> Self {
        self.push_group(commands);
        self
    }

    pub fn push_group(&mut self, commands: Vec<ToolbarCommand>) {
        self.actions.extend(
            commands
                .iter()
                .map(|command| (command.on_activate.clone(), command.disabled)),
        );
        self.groups.push(commands);
    }

    pub fn overflow_trigger(mut self, trigger: BoxedWidget, width: f32) -> Self {
        self.overflow_trigger = Some(trigger);
        self.overflow_width = width.max(1.0);
        self
    }

    pub fn menu_style(mut self, style: impl Into<creamui_core::Style>) -> Self {
        self.menu_style = style.into();
        self
    }

    pub fn group_gap(mut self, gap: f32) -> Self {
        self.group_gap = gap.max(0.0);
        self
    }

    pub fn menu_above(mut self, above: bool) -> Self {
        self.menu_above = above;
        self
    }

    fn visible_groups(&self) -> usize {
        let width = self.controller.width.peek();
        if width <= 0.0 {
            return self.groups.len();
        }
        let total = self
            .groups
            .iter()
            .map(|group| group.iter().map(|action| action.width).sum::<f32>())
            .sum::<f32>()
            + self.group_gap * self.groups.len().saturating_sub(1) as f32;
        if total <= width {
            return self.groups.len();
        }
        let available = (width - self.overflow_width - self.group_gap).max(0.0);
        let mut used = 0.0;
        for (index, group) in self.groups.iter().enumerate() {
            let group_width: f32 = group.iter().map(|action| action.width).sum();
            let next = used + group_width + if index == 0 { 0.0 } else { self.group_gap };
            if next > available {
                return index;
            }
            used = next;
        }
        self.groups.len()
    }
}

impl Widget for RawToolbar {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style.layout.gap.width = creamui_core::layout::LengthPercentage::Length(self.group_gap);
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn focusable(&self) -> bool {
        true
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.controller.width.set_if_changed(rect.width);
        false
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let controller = self.controller.clone();
        let actions = self.actions.clone();
        let visible_commands = self.visible_commands.clone();
        Some(Rc::new(move |input| {
            if actions.is_empty() {
                return;
            }
            match input.key {
                Key::Left | Key::Right => {
                    let direction = if input.key == Key::Left {
                        actions.len() - 1
                    } else {
                        1
                    };
                    let mut index = controller.active.peek().min(actions.len() - 1);
                    for _ in 0..actions.len() {
                        index = (index + direction) % actions.len();
                        if !actions[index].1 {
                            controller.set_active(index);
                            controller.set_open(index >= visible_commands.get());
                            break;
                        }
                    }
                }
                Key::Home => controller.set_active(0),
                Key::End => controller.set_active(actions.len() - 1),
                Key::Enter | Key::Char(' ') => {
                    let index = controller.active.peek().min(actions.len() - 1);
                    if !actions[index].1 {
                        actions[index].0();
                    }
                }
                Key::Escape => controller.set_open(false),
                _ => {}
            }
        }))
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let visible = self.visible_groups();
        self.visible_commands
            .set(self.groups.iter().take(visible).map(Vec::len).sum());
        let mut children = Vec::new();
        let mut menu = Vec::new();
        let mut index = 0;
        for (group_index, group) in std::mem::take(&mut self.groups).into_iter().enumerate() {
            let mut group_view = RawView::new(Style {
                flex_direction: FlexDirection::Row,
                flex_shrink: 0.0,
                ..Default::default()
            });
            for mut command in group {
                let mut style = command.style.clone();
                style.layout.size.width = if group_index < visible {
                    Dimension::Length(command.width)
                } else {
                    Dimension::Percent(1.0)
                };
                let action = Box::new(ToolbarAction {
                    style,
                    content: command.content.take(),
                    on_activate: command.on_activate,
                    controller: self.controller.clone(),
                    index,
                    disabled: command.disabled,
                    overflow: group_index >= visible,
                }) as BoxedWidget;
                if group_index < visible {
                    group_view = group_view.child(action);
                } else {
                    menu.push(action);
                }
                index += 1;
            }
            if group_index < visible {
                children.push(Box::new(group_view) as BoxedWidget);
            }
        }
        if !menu.is_empty() {
            let controller = self.controller.clone();
            let trigger = self
                .overflow_trigger
                .take()
                .unwrap_or_else(|| Box::new(RawText::new("More", Color::rgb(0, 0, 0), 13.0)));
            children.push(Box::new(
                RawButton::new(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(self.overflow_width),
                            height: Dimension::Percent(1.0),
                        },
                        ..Default::default()
                    },
                    move || controller.set_open(!controller.is_open()),
                )
                .child(trigger),
            ));
            if self.controller.is_open() {
                let mut style = self.menu_style.clone();
                style.layout.position = Position::Absolute;
                style.layout.flex_direction = FlexDirection::Column;
                style.layout.inset = creamui_core::layout::Rect {
                    left: LengthPercentageAuto::Auto,
                    right: LengthPercentageAuto::Length(0.0),
                    top: if self.menu_above {
                        LengthPercentageAuto::Auto
                    } else {
                        LengthPercentageAuto::Length(38.0)
                    },
                    bottom: if self.menu_above {
                        LengthPercentageAuto::Length(38.0)
                    } else {
                        LengthPercentageAuto::Auto
                    },
                };
                style.layout.size.width = Dimension::Length(180.0);
                children.push(Box::new(RawView::new(style).with_children(menu)));
            }
        }
        children
    }
}

impl Styled for RawToolbar {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.style = style;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(label: &str, counter: Rc<Cell<usize>>) -> ToolbarCommand {
        ToolbarCommand::new(
            60.0,
            Style::default(),
            Box::new(RawText::new(label, Color::rgb(0, 0, 0), 14.0)),
            move || counter.set(counter.get() + 1),
        )
    }

    #[test]
    fn toolbar_preserves_groups_and_routes_keyboard_activation() {
        let controller = ToolbarController::new();
        let count = Rc::new(Cell::new(0));
        let mut toolbar = RawToolbar::new(Style::default(), &controller)
            .group(vec![command("A", count.clone())])
            .group(vec![command("B", count.clone())]);
        assert_eq!(toolbar.children().len(), 2);
        toolbar.on_key().unwrap()(KeyInput {
            key: Key::Right,
            modifiers: Default::default(),
        });
        toolbar.on_key().unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(controller.active(), 1);
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn overflow_is_group_atomic_and_actions_remain_clickable() {
        let controller = ToolbarController::new();
        controller.width.set(105.0);
        controller.set_open(true);
        let count = Rc::new(Cell::new(0));
        let mut toolbar = RawToolbar::new(Style::default(), &controller)
            .group(vec![command("A", count.clone())])
            .group(vec![command("B", count.clone())]);
        let mut children = toolbar.children();
        assert_eq!(children.len(), 2);
        let mut menu = children.pop().unwrap();
        let actions = menu.children();
        actions[0].on_click().unwrap()();
        assert_eq!(count.get(), 1);
        assert!(!controller.is_open());
    }
}
