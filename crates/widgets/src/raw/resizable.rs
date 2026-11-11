use super::*;
use crate::raw::{RawSplitView, SplitAxis, SplitState};
use creamui_core::layout::Dimension;

pub struct RawResizablePanels {
    pub style: creamui_core::Style,
    axis: SplitAxis,
    states: Vec<SplitState>,
    panes: Vec<(BoxedWidget, f32, Option<f32>)>,
    divider: f32,
    color: Color,
}

impl RawResizablePanels {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        axis: SplitAxis,
        states: Vec<SplitState>,
    ) -> Self {
        Self {
            style: style.into(),
            axis,
            states,
            panes: Vec::new(),
            divider: 6.0,
            color: Color::rgb(100, 100, 100),
        }
    }

    pub fn pane(mut self, content: BoxedWidget, min: f32, max: Option<f32>) -> Self {
        self.panes
            .push((content, min.max(0.0), max.map(|value| value.max(min))));
        self
    }

    pub fn divider(mut self, width: f32, color: Color) -> Self {
        self.divider = width.max(1.0);
        self.color = color;
        self
    }
}

impl Widget for RawResizablePanels {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        assert_eq!(
            self.states.len(),
            self.panes.len().saturating_sub(1),
            "one persistent SplitState is required per divider"
        );
        let mut panes = std::mem::take(&mut self.panes);
        let Some((last, last_min, last_max)) = panes.pop() else {
            return Vec::new();
        };
        let mut tail = last;
        let mut remaining_min = last_min;
        let mut remaining_max = last_max;
        for (index, (pane, min, max)) in panes.into_iter().enumerate().rev() {
            let next_min = remaining_min + self.divider;
            let next_max = remaining_max.map(|value| value + self.divider);
            let style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                ..Default::default()
            };
            tail = Box::new(
                RawSplitView::new(style, self.states[index].clone(), self.axis, pane, tail)
                    .divider(self.divider, self.color)
                    .min_sizes(min, next_min)
                    .max_sizes(max, next_max),
            );
            remaining_min = min + next_min;
            remaining_max = match (max, next_max) {
                (Some(max), Some(next)) => Some(max + next),
                _ => None,
            };
        }
        vec![tail]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane() -> BoxedWidget {
        Box::new(RawView::new(Style::default()))
    }

    #[test]
    fn nested_panels_keep_explicit_split_handles() {
        let states = vec![SplitState::new(0.3), SplitState::new(0.5)];
        let mut panels =
            RawResizablePanels::new(Style::default(), SplitAxis::Horizontal, states.clone())
                .pane(pane(), 40.0, Some(150.0))
                .pane(pane(), 50.0, None)
                .pane(pane(), 60.0, None);
        let mut root = panels.children();
        assert_eq!(root.len(), 1);
        let mut split = root[0].children();
        assert_eq!(split.len(), 3);
        assert_eq!(split[2].children().len(), 1);
        states[0].set_fraction(0.6);
        assert_eq!(states[0].fraction(), 0.6);
    }
}
