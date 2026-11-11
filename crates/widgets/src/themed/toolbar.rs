use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawToolbar, ToolbarCommand, ToolbarController};

pub struct Toolbar {
    inner: RawToolbar,
    groups: Vec<Vec<ToolbarCommand>>,
}
impl_styled_inner!(Toolbar);

impl Toolbar {
    pub fn new(controller: &ToolbarController) -> Self {
        let theme = use_theme();
        let menu_style = RawView::new(
            Style::default()
                .flex_column()
                .padding_all(theme.spacing_small),
        )
        .background(theme.surface_elevated)
        .border(theme.border, 1.0)
        .corner_radius(theme.radius_small)
        .style();
        Self {
            inner: RawToolbar::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(36.0),
                    },
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .padding_xy(theme.spacing_small, 0.0)
                },
                controller,
            )
            .group_gap(theme.spacing_small)
            .menu_style(menu_style)
            .overflow_trigger(
                Box::new(RawText::new(
                    "More",
                    theme.text_primary,
                    theme.typography.caption,
                )),
                44.0,
            ),
            groups: vec![Vec::new()],
        }
    }

    pub fn action(mut self, label: impl Into<String>, on_activate: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let label = label.into();
        let width = label.chars().count() as f32 * 7.0 + 22.0;
        let style = RawView::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .padding_xy(theme.spacing_small, 0.0),
        )
        .hover_style(creamui_core::StateStyle::new().background(theme.surface_hover))
        .pressed_style(creamui_core::StateStyle::new().background(theme.accent_hover))
        .corner_radius(theme.radius_small)
        .style();
        let command = ToolbarCommand::new(
            width,
            style,
            Box::new(RawText::new(
                label,
                theme.text_primary,
                theme.typography.caption,
            )),
            on_activate,
        );
        self.groups.last_mut().unwrap().push(command);
        self
    }

    pub fn separator(mut self) -> Self {
        if !self.groups.last().is_some_and(Vec::is_empty) {
            self.groups.push(Vec::new());
        }
        self
    }

    pub fn floating(mut self) -> Self {
        let theme = use_theme();
        self.inner.style.paint.background = Some(theme.surface_elevated.into());
        self.inner.style.paint.border = Some(creamui_core::Border {
            color: theme.border.into(),
            width: 1.0,
        });
        self
    }
}

impl Widget for Toolbar {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn focusable(&self) -> bool {
        self.inner.focusable()
    }

    fn reports_layout(&self) -> bool {
        self.inner.reports_layout()
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.inner.on_layout(rect)
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        for group in std::mem::take(&mut self.groups) {
            if !group.is_empty() {
                self.inner.push_group(group);
            }
        }
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_toolbar_mounts_groups_and_overflow() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = ToolbarController::new();
            let mut toolbar = Toolbar::new(&controller)
                .action("Save", || {})
                .separator()
                .action("Preview", || {});
            assert_eq!(toolbar.children().len(), 2);
        });
    }
}
