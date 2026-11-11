use super::*;
use crate::layout::StyleExt;
use crate::raw::{DiffColors, DiffController, DiffHunk, DiffView, RawDiff};
use crate::{ScrollController, VirtualListState};

pub struct Diff {
    pub style: creamui_core::Style,
    raw: Option<RawDiff>,
    controller: DiffController,
    theme: Theme,
}

impl Diff {
    pub fn new(
        controller: &DiffController,
        state: VirtualListState,
        scroll: ScrollController,
        viewport_height: f32,
        hunks: &[DiffHunk],
    ) -> Self {
        let theme = use_theme();
        let raw = RawDiff::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(viewport_height.max(0.0)),
                },
                ..Default::default()
            },
            controller,
            state,
            scroll,
            viewport_height,
            hunks,
        )
        .colors(DiffColors {
            context: theme.surface,
            addition: theme.success.mix(theme.surface, 0.87),
            deletion: theme.danger.mix(theme.surface, 0.87),
            conflict: theme.warning.mix(theme.surface, 0.84),
            selected: theme.accent.mix(theme.surface, 0.65),
            header: theme.surface_elevated,
            text: theme.text_primary,
            gutter: theme.text_secondary,
        });
        Self {
            style: Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(viewport_height.max(0.0) + 35.0),
                },
                ..Style::default().flex_column().gap(theme.spacing_small)
            }
            .into(),
            raw: Some(raw),
            controller: controller.clone(),
            theme,
        }
    }
}

impl Styled for Diff {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.style = style;
    }
}

impl Widget for Diff {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let unified = self.controller.clone();
        let split = self.controller.clone();
        let view = self.controller.view();
        let controls = RawView::new(Style::default().flex_row().gap(self.theme.spacing_small))
            .child(Box::new(Button::secondary(
                ButtonSize::Sm,
                if view == DiffView::Unified {
                    "Unified *"
                } else {
                    "Unified"
                },
                move || unified.set_view(DiffView::Unified),
            )))
            .child(Box::new(Button::secondary(
                ButtonSize::Sm,
                if view == DiffView::SideBySide {
                    "Side by side *"
                } else {
                    "Side by side"
                },
                move || split.set_view(DiffView::SideBySide),
            )));
        let mut children: Vec<BoxedWidget> = vec![Box::new(controls)];
        if let Some(raw) = self.raw.take() {
            children.push(Box::new(raw));
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::{DiffKind, DiffLine};

    #[test]
    fn themed_diff_mounts_actions_and_virtual_rows() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = DiffController::new();
            let mut diff = Diff::new(
                &controller,
                VirtualListState::new(2, 26.0),
                ScrollController::new(0.0),
                110.0,
                &[DiffHunk::new("Hunk").line(DiffLine::new(
                    DiffKind::Addition,
                    None,
                    Some(1),
                    "new",
                ))],
            );
            assert_eq!(diff.children().len(), 2);
        });
    }
}
