use super::*;

pub struct RawKbd {
    pub style: creamui_core::Style,
    keys: Vec<String>,
    color: Color,
    size: f32,
}

impl RawKbd {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        keys: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            style: style.into(),
            keys: keys.into_iter().collect(),
            color: Color::rgb(0, 0, 0),
            size: 12.0,
        }
    }

    pub fn text_style(mut self, color: Color, size: f32) -> Self {
        self.color = color;
        self.size = size.max(1.0);
        self
    }

    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    fn formatted(&self) -> String {
        self.keys
            .iter()
            .map(|key| {
                if key.eq_ignore_ascii_case("mod") {
                    if cfg!(target_os = "macos") {
                        "Cmd"
                    } else {
                        "Ctrl"
                    }
                } else {
                    key.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

impl Widget for RawKbd {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        vec![Box::new(RawText::new(
            self.formatted(),
            self.color,
            self.size,
        ))]
    }
}

pub struct RawTag {
    pub style: creamui_core::Style,
    pub selected: bool,
    label: Option<BoxedWidget>,
    on_select: Option<Rc<dyn Fn(bool)>>,
    remove: Option<(BoxedWidget, Rc<dyn Fn()>)>,
}

impl RawTag {
    pub fn new(style: impl Into<creamui_core::Style>, label: BoxedWidget) -> Self {
        Self {
            style: style.into(),
            selected: false,
            label: Some(label),
            on_select: None,
            remove: None,
        }
    }

    pub fn selectable(mut self, selected: bool, on_select: impl Fn(bool) + 'static) -> Self {
        self.selected = selected;
        self.on_select = Some(Rc::new(on_select));
        self
    }

    pub fn label(mut self, label: BoxedWidget) -> Self {
        self.label = Some(label);
        self
    }

    pub fn removable(mut self, label: BoxedWidget, on_remove: impl Fn() + 'static) -> Self {
        self.remove = Some((label, Rc::new(on_remove)));
        self
    }
}

impl Widget for RawTag {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        if let Some(label) = self.label.take() {
            if let Some(callback) = self.on_select.clone() {
                let selected = self.selected;
                children.push(Box::new(
                    RawButton::new(Style::default(), move || callback(!selected)).child(label),
                ) as BoxedWidget);
            } else {
                children.push(label);
            }
        }
        if let Some((label, callback)) = self.remove.take() {
            children.push(Box::new(
                RawButton::new(Style::default(), move || callback()).child(label),
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
    fn shortcuts_format_platform_modifiers() {
        let mut shortcut = RawKbd::new(Style::default(), ["Mod".into(), "K".into()]);
        assert_eq!(shortcut.children().len(), 1);
        assert_eq!(
            shortcut.formatted(),
            if cfg!(target_os = "macos") {
                "Cmd + K"
            } else {
                "Ctrl + K"
            }
        );
    }

    #[test]
    fn tag_actions_do_not_share_callbacks() {
        let selected = Rc::new(Cell::new(false));
        let removed = Rc::new(Cell::new(false));
        let selected_target = selected.clone();
        let removed_target = removed.clone();
        let mut tag = RawTag::new(Style::default(), slot())
            .selectable(false, move |next| selected_target.set(next))
            .removable(slot(), move || removed_target.set(true));
        let parts = tag.children();
        parts[0].on_click().unwrap()();
        assert!(selected.get());
        assert!(!removed.get());
        parts[1].on_click().unwrap()();
        assert!(removed.get());
    }
}
