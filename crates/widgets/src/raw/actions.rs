use super::*;
use creamui_core::layout::FlexDirection;

pub struct RawInputGroup {
    pub style: creamui_core::Style,
    leading: Vec<BoxedWidget>,
    control: Option<BoxedWidget>,
    trailing: Vec<BoxedWidget>,
}

impl RawInputGroup {
    pub fn new(style: impl Into<creamui_core::Style>, control: BoxedWidget) -> Self {
        Self {
            style: style.into(),
            leading: Vec::new(),
            control: Some(control),
            trailing: Vec::new(),
        }
    }

    pub fn leading(mut self, widget: BoxedWidget) -> Self {
        self.leading.push(widget);
        self
    }

    pub fn trailing(mut self, widget: BoxedWidget) -> Self {
        self.trailing.push(widget);
        self
    }
}

impl Widget for RawInputGroup {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = std::mem::take(&mut self.leading);
        children.extend(self.control.take());
        children.append(&mut self.trailing);
        children
    }
}

pub struct RawDropdownButton {
    pub style: creamui_core::Style,
    primary: Option<BoxedWidget>,
    menu: Option<BoxedWidget>,
    on_primary: Rc<dyn Fn()>,
    on_menu: Rc<dyn Fn()>,
    primary_style: creamui_core::Style,
    menu_style: creamui_core::Style,
    disabled: bool,
}

impl RawDropdownButton {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        primary: BoxedWidget,
        menu: BoxedWidget,
        on_primary: impl Fn() + 'static,
        on_menu: impl Fn() + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            primary: Some(primary),
            menu: Some(menu),
            on_primary: Rc::new(on_primary),
            on_menu: Rc::new(on_menu),
            primary_style: Style::default().into(),
            menu_style: Style::default().into(),
            disabled: false,
        }
    }

    pub fn button_styles(
        mut self,
        primary: impl Into<creamui_core::Style>,
        menu: impl Into<creamui_core::Style>,
    ) -> Self {
        self.primary_style = primary.into();
        self.menu_style = menu.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Widget for RawDropdownButton {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        if let Some(primary) = self.primary.take() {
            let callback = self.on_primary.clone();
            children.push(Box::new(
                RawButton::new(self.primary_style.clone(), move || callback())
                    .disabled(self.disabled)
                    .child(primary),
            ) as BoxedWidget);
        }
        if let Some(menu) = self.menu.take() {
            let callback = self.on_menu.clone();
            children.push(Box::new(
                RawButton::new(self.menu_style.clone(), move || callback())
                    .disabled(self.disabled)
                    .child(menu),
            ));
        }
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
    fn input_group_keeps_addons_on_both_sides() {
        let mut group = RawInputGroup::new(Style::default(), slot())
            .leading(slot())
            .trailing(slot());
        assert_eq!(group.children().len(), 3);
    }

    #[test]
    fn split_button_actions_are_independent_and_disable_together() {
        let primary = Rc::new(Cell::new(0));
        let menu = Rc::new(Cell::new(0));
        let p = primary.clone();
        let m = menu.clone();
        let mut split = RawDropdownButton::new(
            Style::default(),
            slot(),
            slot(),
            move || p.set(p.get() + 1),
            move || m.set(m.get() + 1),
        );
        let children = split.children();
        children[0].on_click().unwrap()();
        children[1].on_click().unwrap()();
        assert_eq!((primary.get(), menu.get()), (1, 1));
        let mut disabled =
            RawDropdownButton::new(Style::default(), slot(), slot(), || {}, || {}).disabled(true);
        assert!(disabled
            .children()
            .iter()
            .all(|child| child.on_click().is_none()));
    }
}
