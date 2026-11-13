use super::*;
use crate::layout::StyleExt;
use crate::raw::RawCarousel;

pub struct Carousel {
    inner: RawCarousel,
}
impl_styled_inner!(Carousel);

impl Carousel {
    pub fn new(index: usize, on_change: impl Fn(usize) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawCarousel::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    ..Style::default()
                        .flex_column()
                        .gap(theme.spacing_medium)
                        .padding_all(theme.spacing_medium)
                },
                index,
                on_change,
            )
            .controls(
                Box::new(
                    crate::Icon::new(crate::Symbol::ChevronLeft, theme.text_primary).size(16.0),
                ),
                Box::new(
                    crate::Icon::new(crate::Symbol::ChevronRight, theme.text_primary).size(16.0),
                ),
            )
            .indicators(elevation::rail_fill(&theme), theme.accent)
            .background(elevation::group_fill(&theme))
            .border(theme.border, 1.0)
            .corner_radius(theme.card_radius),
        }
    }

    pub fn slide(mut self, slide: BoxedWidget) -> Self {
        self.inner = self.inner.slide(slide);
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.inner = self.inner.looping(looping);
        self
    }
}

impl Widget for Carousel {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag_start()
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag()
    }
    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_drag_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_carousel_mounts_current_slide_and_controls() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let mut carousel = Carousel::new(1, |_| {})
                .slide(Box::new(RawView::new(Style::default())))
                .slide(Box::new(RawView::new(Style::default())));
            let mut children = carousel.children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[1].children().len(), 3);
        });
    }
}
