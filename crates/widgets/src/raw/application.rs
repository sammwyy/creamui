use super::*;
use creamui_core::layout::FlexDirection;

pub struct RawAccordion {
    pub style: creamui_core::Style,
    items: Vec<(BoxedWidget, BoxedWidget, bool)>,
    on_toggle: Rc<dyn Fn(usize, bool)>,
}

impl RawAccordion {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        on_toggle: impl Fn(usize, bool) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            items: Vec::new(),
            on_toggle: Rc::new(on_toggle),
        }
    }

    pub fn item(mut self, heading: BoxedWidget, content: BoxedWidget, expanded: bool) -> Self {
        self.items.push((heading, content, expanded));
        self
    }
}

impl Widget for RawAccordion {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.items)
            .into_iter()
            .enumerate()
            .map(|(index, (heading, content, expanded))| {
                let on_toggle = self.on_toggle.clone();
                Box::new(
                    RawCollapsible::new(Style::default(), expanded, move |open| {
                        on_toggle(index, open)
                    })
                    .trigger(heading)
                    .content(content),
                ) as BoxedWidget
            })
            .collect()
    }
}

pub struct RawField {
    pub style: creamui_core::Style,
    pub required: bool,
    label: Option<BoxedWidget>,
    control: Option<BoxedWidget>,
    hint: Option<BoxedWidget>,
    error: Option<BoxedWidget>,
}

impl RawField {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        label: BoxedWidget,
        control: BoxedWidget,
    ) -> Self {
        Self {
            style: style.into(),
            required: false,
            label: Some(label),
            control: Some(control),
            hint: None,
            error: None,
        }
    }

    pub fn required(mut self, value: bool) -> Self {
        self.required = value;
        self
    }

    pub fn hint(mut self, hint: BoxedWidget) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn error(mut self, error: BoxedWidget) -> Self {
        self.error = Some(error);
        self
    }
}

impl Widget for RawField {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = Vec::new();
        children.extend(self.label.take());
        children.extend(self.control.take());
        if self.error.is_some() {
            children.extend(self.error.take());
        } else {
            children.extend(self.hint.take());
        }
        children
    }
}

pub struct RawForm {
    pub style: creamui_core::Style,
    fields: Vec<BoxedWidget>,
    summary: Option<BoxedWidget>,
    submit: Option<(BoxedWidget, Rc<dyn Fn()>)>,
    submit_style: creamui_core::Style,
    submitting: bool,
}

impl RawForm {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        Self {
            style: style.into(),
            fields: Vec::new(),
            summary: None,
            submit: None,
            submit_style: Style::default().into(),
            submitting: false,
        }
    }

    pub fn field(mut self, field: BoxedWidget) -> Self {
        self.fields.push(field);
        self
    }

    pub fn summary(mut self, summary: BoxedWidget) -> Self {
        self.summary = Some(summary);
        self
    }

    pub fn submit(mut self, label: BoxedWidget, on_submit: impl Fn() + 'static) -> Self {
        self.submit = Some((label, Rc::new(on_submit)));
        self
    }

    pub fn submitting(mut self, submitting: bool) -> Self {
        self.submitting = submitting;
        self
    }

    pub fn submit_style(mut self, style: impl Into<creamui_core::Style>) -> Self {
        self.submit_style = style.into();
        self
    }
}

impl Widget for RawForm {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children = std::mem::take(&mut self.fields);
        children.extend(self.summary.take());
        if let Some((label, callback)) = self.submit.take() {
            children.push(Box::new(
                RawButton::new(self.submit_style.clone(), move || callback())
                    .disabled(self.submitting)
                    .child(label),
            ));
        }
        children
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageItem {
    Page(usize),
    Ellipsis,
}

pub fn page_items(current: usize, total: usize) -> Vec<PageItem> {
    if total == 0 {
        return Vec::new();
    }
    let current = current.clamp(1, total);
    let mut pages = Vec::new();
    for page in 1..=total {
        if page == 1 || page == total || page.abs_diff(current) <= 1 {
            if let Some(PageItem::Page(previous)) = pages.last() {
                if page > previous + 1 {
                    pages.push(PageItem::Ellipsis);
                }
            }
            pages.push(PageItem::Page(page));
        }
    }
    pages
}

pub struct RawPagination {
    pub style: creamui_core::Style,
    pub current: usize,
    pub total: usize,
    on_change: Rc<dyn Fn(usize)>,
    colors: Option<(Color, Color, Color)>,
    item_styles: Option<PaginationItemStyles>,
    compact: bool,
}

/// Explicit per-item styles for [`RawPagination`]; they take precedence
/// over [`RawPagination::colors`].
#[derive(Clone)]
pub struct PaginationItemStyles {
    pub item: creamui_core::Style,
    pub selected: creamui_core::Style,
    pub text: Color,
    pub selected_text: Color,
}

impl RawPagination {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        current: usize,
        total: usize,
        on_change: impl Fn(usize) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            current,
            total,
            on_change: Rc::new(on_change),
            colors: None,
            item_styles: None,
            compact: false,
        }
    }

    pub fn item_styles(mut self, styles: PaginationItemStyles) -> Self {
        self.item_styles = Some(styles);
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn colors(mut self, background: Color, text: Color, selected: Color) -> Self {
        self.colors = Some((background, text, selected));
        self
    }
}

impl Widget for RawPagination {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        if self.total == 0 {
            return Vec::new();
        }
        let current = self.current.clamp(1, self.total);
        let button = |label: String, target: usize, disabled: bool, selected: bool| {
            let callback = self.on_change.clone();
            let mut button =
                RawButton::new(Style::default(), move || callback(target)).disabled(disabled);
            let mut text = RawText::unstyled(label);
            if let Some(styles) = &self.item_styles {
                button = button.with_style(if selected {
                    styles.selected.clone()
                } else {
                    styles.item.clone()
                });
                text = text
                    .color(if selected {
                        styles.selected_text
                    } else {
                        styles.text
                    })
                    .bold(selected);
            } else if let Some((background, foreground, active)) = self.colors {
                button = button
                    .background(if selected { active } else { background })
                    .corner_radius(6.0)
                    .padding(6.0)
                    .height(30.0);
                text = text.color(foreground);
            }
            Box::new(button.child(Box::new(text))) as BoxedWidget
        };
        let mut children = vec![button(
            "‹".into(),
            current.saturating_sub(1),
            current == 1,
            false,
        )];
        let pages = if self.compact {
            vec![PageItem::Page(current)]
        } else {
            page_items(current, self.total)
        };
        for item in pages {
            match item {
                PageItem::Page(page) => children.push(button(
                    page.to_string(),
                    page,
                    page == current,
                    page == current,
                )),
                PageItem::Ellipsis => {
                    let mut text = RawText::unstyled("…");
                    if let Some(styles) = &self.item_styles {
                        text = text.color(styles.text);
                    } else if let Some((_, foreground, _)) = self.colors {
                        text = text.color(foreground);
                    }
                    children.push(Box::new(text));
                }
            }
        }
        children.push(button(
            "›".into(),
            current.saturating_add(1),
            current == self.total,
            false,
        ));
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
    fn accordion_reports_index_and_requested_state() {
        let values = Rc::new(RefCell::new(Vec::new()));
        let received = values.clone();
        let mut accordion = RawAccordion::new(Style::default(), move |index, open| {
            received.borrow_mut().push((index, open));
        })
        .item(slot(), slot(), false)
        .item(slot(), slot(), true);
        let mut items = accordion.children();
        assert_eq!(items[0].children().len(), 1);
        let trigger = items[1].children().remove(0);
        trigger.on_click().unwrap()();
        assert_eq!(*values.borrow(), [(1, false)]);
    }

    #[test]
    fn field_shows_error_instead_of_hint() {
        let mut field = RawField::new(Style::default(), slot(), slot())
            .hint(slot())
            .error(slot())
            .required(true);
        assert!(field.required);
        assert_eq!(field.children().len(), 3);
    }

    #[test]
    fn submitting_form_disables_action() {
        let mut form = RawForm::new(Style::default())
            .field(slot())
            .submit(slot(), || {})
            .submitting(true);
        let children = form.children();
        assert_eq!(children.len(), 2);
        assert!(children[1].on_click().is_none());
    }

    #[test]
    fn pagination_handles_edges_and_gaps() {
        assert!(page_items(1, 0).is_empty());
        assert_eq!(page_items(1, 2), [PageItem::Page(1), PageItem::Page(2)]);
        assert_eq!(
            page_items(6, 12),
            [
                PageItem::Page(1),
                PageItem::Ellipsis,
                PageItem::Page(5),
                PageItem::Page(6),
                PageItem::Page(7),
                PageItem::Ellipsis,
                PageItem::Page(12)
            ]
        );
    }
}
