use super::*;
use creamui_core::layout::FlexDirection;

pub struct RawCollapsible {
    pub style: creamui_core::Style,
    pub expanded: bool,
    trigger: Option<BoxedWidget>,
    content: Option<BoxedWidget>,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl RawCollapsible {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        expanded: bool,
        on_toggle: impl Fn(bool) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            expanded,
            trigger: None,
            content: None,
            on_toggle: Rc::new(on_toggle),
        }
    }

    pub fn trigger(mut self, trigger: BoxedWidget) -> Self {
        self.trigger = Some(trigger);
        self
    }

    pub fn content(mut self, content: BoxedWidget) -> Self {
        self.content = Some(content);
        self
    }
}

impl Widget for RawCollapsible {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        if let Some(trigger) = self.trigger.take() {
            let on_toggle = self.on_toggle.clone();
            let expanded = self.expanded;
            children.push(Box::new(
                RawButton::new(Style::default(), move || on_toggle(!expanded)).child(trigger),
            ) as BoxedWidget);
        }
        if self.expanded {
            children.extend(self.content.take());
        }
        children
    }
}

pub struct RawAlert {
    pub style: creamui_core::Style,
    pub status: AlertStatus,
    icon: Option<BoxedWidget>,
    title: Option<BoxedWidget>,
    message: Option<BoxedWidget>,
    action: Option<BoxedWidget>,
    dismiss: Option<(BoxedWidget, Rc<dyn Fn()>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertStatus {
    Info,
    Success,
    Warning,
    Error,
}

impl RawAlert {
    pub fn new(style: impl Into<creamui_core::Style>, status: AlertStatus) -> Self {
        Self {
            style: style.into(),
            status,
            icon: None,
            title: None,
            message: None,
            action: None,
            dismiss: None,
        }
    }

    pub fn icon(mut self, icon: BoxedWidget) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn title(mut self, title: BoxedWidget) -> Self {
        self.title = Some(title);
        self
    }

    pub fn message(mut self, message: BoxedWidget) -> Self {
        self.message = Some(message);
        self
    }

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.action = Some(action);
        self
    }

    pub fn dismiss(mut self, label: BoxedWidget, callback: impl Fn() + 'static) -> Self {
        self.dismiss = Some((label, Rc::new(callback)));
        self
    }
}

impl Widget for RawAlert {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        children.extend(self.icon.take());
        let mut body = RawView::new(Style {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            ..Default::default()
        });
        if let Some(title) = self.title.take() {
            body = body.child(title);
        }
        if let Some(message) = self.message.take() {
            body = body.child(message);
        }
        children.push(Box::new(body) as BoxedWidget);
        children.extend(self.action.take());
        if let Some((label, callback)) = self.dismiss.take() {
            children.push(Box::new(
                RawButton::new(Style::default(), move || callback()).child(label),
            ));
        }
        children
    }
}

pub struct RawSkeleton {
    pub style: creamui_core::Style,
}

impl RawSkeleton {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        Self {
            style: style.into(),
        }
    }
}

impl Widget for RawSkeleton {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}

pub struct RawEmptyState {
    pub style: creamui_core::Style,
    media: Option<BoxedWidget>,
    title: Option<BoxedWidget>,
    description: Option<BoxedWidget>,
    action: Option<BoxedWidget>,
}

impl RawEmptyState {
    pub fn new(style: impl Into<creamui_core::Style>, title: BoxedWidget) -> Self {
        Self {
            style: style.into(),
            media: None,
            title: Some(title),
            description: None,
            action: None,
        }
    }

    pub fn media(mut self, media: BoxedWidget) -> Self {
        self.media = Some(media);
        self
    }

    pub fn description(mut self, description: BoxedWidget) -> Self {
        self.description = Some(description);
        self
    }

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.action = Some(action);
        self
    }
}

impl Widget for RawEmptyState {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        children.extend(self.media.take());
        children.extend(self.title.take());
        children.extend(self.description.take());
        children.extend(self.action.take());
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> BoxedWidget {
        Box::new(RawView::new(Style::default()))
    }

    #[test]
    fn collapsible_only_mounts_open_content_and_toggles_from_trigger() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        for expanded in [false, true] {
            let changes_for_callback = changes.clone();
            let mut widget = RawCollapsible::new(Style::default(), expanded, move |next| {
                changes_for_callback.borrow_mut().push(next)
            })
            .trigger(slot())
            .content(slot());
            let children = widget.children();
            assert_eq!(children.len(), if expanded { 2 } else { 1 });
            children[0].on_click().unwrap()();
        }
        assert_eq!(*changes.borrow(), [true, false]);
    }

    #[test]
    fn alert_slots_and_dismissal_are_independent() {
        let dismissed = Rc::new(Cell::new(false));
        let flag = dismissed.clone();
        let mut alert = RawAlert::new(Style::default(), AlertStatus::Warning)
            .icon(slot())
            .title(slot())
            .message(slot())
            .action(slot())
            .dismiss(slot(), move || flag.set(true));
        let mut children = alert.children();
        assert_eq!(children.len(), 4);
        assert_eq!(children[1].children().len(), 2);
        children[3].on_click().unwrap()();
        assert!(dismissed.get());
    }

    #[test]
    fn empty_state_keeps_slot_order() {
        let mut state = RawEmptyState::new(Style::default(), slot())
            .media(slot())
            .description(slot())
            .action(slot());
        assert_eq!(state.children().len(), 4);
    }
}
