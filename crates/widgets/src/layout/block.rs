use super::{layout_container_methods, shrinkable, StyleExt};
use creamui_core::layout::{Dimension, Display, Style};
use creamui_core::{BoxedWidget, Painter, Rect, Widget};

/// An unstyled block container, equivalent to a semantic HTML `<div>`.
///
/// `Block` is intentionally the base container. Use [`super::Flex`] only
/// when its children need flexbox behavior; the name `View` is kept free
/// for a future navigation/view abstraction.
pub struct Block {
    inner: crate::raw::RawView,
}

impl Block {
    /// Creates an empty block-layout container.
    pub fn new() -> Self {
        Self::with_style(Style::default())
    }

    /// Creates a block container from a Taffy style. `display` is always
    /// normalized to `Block` so the component keeps its semantic contract.
    pub fn with_style(mut style: Style) -> Self {
        style.display = Display::Block;
        Self {
            inner: crate::raw::RawView::new(shrinkable(style)),
        }
    }

    layout_container_methods!();
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Block {
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

    #[test]
    fn block_is_always_a_block_layout_container() {
        let style = Block::with_style(Style {
            display: creamui_core::layout::Display::Flex,
            ..Default::default()
        })
        .style();

        assert_eq!(style.display, creamui_core::layout::Display::Block);
    }
}
