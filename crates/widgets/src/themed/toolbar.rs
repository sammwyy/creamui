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
                .gap(theme.spacing_small)
                .padding_all(theme.spacing_medium),
        )
        .background(elevation::overlay_fill(&theme))
        .border(elevation::overlay_border(&theme), 1.0)
        .corner_radius(theme.menu_radius)
        .box_shadow(elevation::Elevation::Overlay.shadow(&theme))
        .style();
        Self {
            inner: RawToolbar::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(40.0),
                    },
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .padding_xy(theme.spacing_medium, 0.0)
                },
                controller,
            )
            .group_gap(theme.spacing_medium)
            .menu_style(menu_style)
            .overflow_trigger(
                Box::new(RawText::new(
                    "More",
                    theme.text_primary,
                    theme.typography.body,
                )),
                56.0,
            ),
            groups: vec![Vec::new()],
        }
    }

    pub fn action(mut self, label: impl Into<String>, on_activate: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let label = label.into();
        let horizontal_padding = theme.spacing_medium + theme.spacing_small;
        let font_size = theme.typography.body;
        let text_width =
            crate::text_metrics::measure(&label, font_size, crate::text_metrics::unbounded_width())
                .0;
        let width = (text_width + horizontal_padding * 2.0 + 2.0).ceil();
        let style = RawView::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .padding_xy(horizontal_padding, theme.spacing_small + 2.0),
        )
        .hover_style(
            creamui_core::StateStyle::new()
                .background(elevation::with_alpha(theme.text_primary, 16)),
        )
        .pressed_style(
            creamui_core::StateStyle::new()
                .background(elevation::with_alpha(theme.text_primary, 28)),
        )
        .focus_style(creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0))
        .corner_radius(theme.menu_item_radius)
        .style();
        let command = ToolbarCommand::new(
            width,
            style,
            Box::new(
                RawText::new(label, theme.text_primary, font_size).font_family(theme.font_family),
            ),
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
        self.inner.style.paint.background = Some(elevation::group_fill(&theme).into());
        self.inner.style.paint.border = Some(creamui_core::Border {
            color: theme.border.into(),
            width: 1.0,
        });
        self.inner.style.paint.corner_radius = Some(theme.radius_medium);
        if !theme.high_contrast {
            self.inner.style.paint.box_shadow = Some(elevation::Elevation::Raised.shadow(&theme));
        }
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
