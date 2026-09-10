use super::{layout_container_methods, shrinkable, Align, Justify, StyleExt, Wrap};
use creamui_core::layout::{Dimension, Display, FlexDirection, Style};
use creamui_core::{BoxedWidget, Painter, Rect, Widget};

/// An unstyled flex container, equivalent to `<div style="display: flex">`.
///
/// ```
/// use creamui_widgets::layout::{Align, Flex, Justify};
///
/// let toolbar = Flex::row()
///     .gap(12.0)
///     .align(Align::Center)
///     .justify(Justify::Between);
/// ```
pub struct Flex {
    inner: crate::raw::RawView,
}
impl_styled_inner!(Flex);

impl Flex {
    /// Creates a left-to-right flex container. Its defaults match CSS:
    /// `align-items` and `justify-content` are unset, and wrapping is off.
    pub fn row() -> Self {
        Self::new(FlexDirection::Row)
    }

    /// Creates a top-to-bottom flex container.
    pub fn column() -> Self {
        Self::new(FlexDirection::Column)
    }

    /// Creates a flex container with an explicit direction.
    pub fn new(direction: FlexDirection) -> Self {
        Self {
            inner: crate::raw::RawView::new(shrinkable(Style {
                display: Display::Flex,
                flex_direction: direction,
                ..Default::default()
            })),
        }
    }

    /// Changes the flex direction, including reverse directions when needed.
    pub fn direction(mut self, direction: FlexDirection) -> Self {
        self.inner.style.flex_direction = direction;
        self
    }

    /// Sets equal row and column gaps, equivalent to CSS `gap`.
    pub fn gap(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap(value);
        self
    }

    /// Sets the horizontal gap (`column-gap`).
    pub fn gap_x(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap_x(value);
        self
    }

    /// Sets the vertical gap (`row-gap`).
    pub fn gap_y(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap_y(value);
        self
    }

    /// Sets `align-items` on the cross axis.
    pub fn align(mut self, value: Align) -> Self {
        self.inner.style = self.inner.style.align(value);
        self
    }

    /// Sets `justify-content` on the main axis.
    pub fn justify(mut self, value: Justify) -> Self {
        self.inner.style = self.inner.style.justify(value);
        self
    }

    /// Sets `align-content`, used when wrapped lines have extra cross-axis space.
    pub fn align_content(mut self, value: Justify) -> Self {
        self.inner.style = self.inner.style.align_content(value);
        self
    }

    /// Sets `flex-wrap`.
    pub fn wrap(mut self, value: Wrap) -> Self {
        self.inner.style = self.inner.style.wrap(value);
        self
    }

    layout_container_methods!();
}

impl Widget for Flex {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::layout::{AlignItems, FlexWrap, JustifyContent, LengthPercentage};

    #[test]
    fn flex_container_uses_css_defaults_until_configured() {
        let style = Flex::row()
            .gap(10.0)
            .align(Align::Center)
            .justify(Justify::End)
            .wrap(Wrap::Reverse)
            .style();

        assert_eq!(style.display, creamui_core::layout::Display::Flex);
        assert_eq!(style.flex_direction, FlexDirection::Row);
        assert_eq!(style.gap.width, LengthPercentage::Length(10.0));
        assert_eq!(style.gap.height, LengthPercentage::Length(10.0));
        assert_eq!(style.align_items, Some(AlignItems::Center));
        assert_eq!(style.justify_content, Some(JustifyContent::End));
        assert_eq!(style.flex_wrap, FlexWrap::WrapReverse);
    }
}
