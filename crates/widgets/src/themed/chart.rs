use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{PlotController, PlotSeries, RawPlot};

pub struct Chart {
    pub style: creamui_core::Style,
    plot: Option<RawPlot>,
    legend: Vec<(String, Color)>,
    controller: PlotController,
    theme: Theme,
}

impl Chart {
    pub fn new(controller: &PlotController, series: Vec<PlotSeries>) -> Self {
        let theme = use_theme();
        let legend = series
            .iter()
            .map(|series| (series.label.clone(), series.color))
            .collect();
        let mut plot = RawPlot::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(180.0),
                },
                ..Default::default()
            },
            controller,
            series,
        );
        plot.axis_color = theme.border_strong;
        plot.grid_color = Color::rgba(theme.border.r, theme.border.g, theme.border.b, 120);
        plot.label_color = theme.text_secondary;
        Self {
            style: Style::default()
                .flex_column()
                .gap(theme.spacing_small)
                .into(),
            plot: Some(plot),
            legend,
            controller: controller.clone(),
            theme,
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        if let Some(plot) = &mut self.plot {
            plot.style.layout.size.height = Dimension::Length(height.max(40.0));
        }
        self
    }
}

impl Styled for Chart {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.style = style;
    }
}

impl Widget for Chart {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children: Vec<BoxedWidget> = self
            .plot
            .take()
            .map(|plot| Box::new(plot) as BoxedWidget)
            .into_iter()
            .collect();
        let mut legend = RawView::new(Style::default().flex_row().gap(self.theme.spacing_medium));
        for (label, color) in &self.legend {
            legend = legend.child(Box::new(
                RawView::new(Style::default().flex_row().align(Align::Center).gap(5.0))
                    .child(Box::new(
                        RawView::new(Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Length(9.0),
                                height: Dimension::Length(9.0),
                            },
                            ..Default::default()
                        })
                        .background(*color)
                        .corner_radius(3.0),
                    ))
                    .child(Box::new(RawText::new(
                        label,
                        self.theme.text_secondary,
                        self.theme.typography.caption,
                    ))),
            ));
        }
        children.push(Box::new(legend));
        if let Some(hit) = self.controller.hovered() {
            if let Some((label, _)) = self.legend.get(hit.series) {
                children.push(Box::new(RawText::new(
                    format!("{}: {:.1}, {:.1}", label, hit.value.0, hit.value.1),
                    self.theme.text_primary,
                    self.theme.typography.caption,
                )));
            }
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::PlotKind;

    #[test]
    fn themed_chart_mounts_plot_and_legend() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = PlotController::new();
            let mut chart = Chart::new(
                &controller,
                vec![PlotSeries::new(
                    "Revenue",
                    PlotKind::Line,
                    Color::rgb(120, 120, 255),
                    vec![(0.0, 1.0), (1.0, 2.0)],
                )],
            );
            assert_eq!(chart.children().len(), 2);
        });
    }
}
