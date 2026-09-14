//! Reproducible synthetic scenes for benchmarking reconcile/layout/paint
//! work.

use creamui_core::{BoxedWidget, Painter, Rect, Style, Styled, Widget};
use creamui_theme::Color;
use creamui_widgets::layout::{Flex, Wrap};
use creamui_widgets::raw::{RawText, RawView};

fn leaf_color(seed: usize) -> Color {
    let h = seed.wrapping_mul(2654435761);
    Color::rgb(
        (h & 0xff) as u8,
        ((h >> 8) & 0xff) as u8,
        ((h >> 16) & 0xff) as u8,
    )
}

fn leaf(seed: usize) -> BoxedWidget {
    Box::new(RawView::new(
        Style::new()
            .width(16.0)
            .height(16.0)
            .background(leaf_color(seed)),
    ))
}

/// A chain of `depth` nested containers, one child each — stresses
/// ancestor-chain invalidation.
pub fn deep_tree(depth: usize) -> BoxedWidget {
    fn build(remaining: usize) -> BoxedWidget {
        if remaining == 0 {
            return Box::new(RawView::new(Style::new().width(16.0).height(16.0)));
        }
        Box::new(RawView::new(Style::new().width(20.0).height(20.0)).child(build(remaining - 1)))
    }
    build(depth)
}

/// A root with `count` direct leaf children — stresses linear scans and
/// per-leaf event-region bookkeeping.
pub fn wide_tree(count: usize) -> BoxedWidget {
    let children = (0..count).map(leaf).collect();
    Box::new(Flex::row().wrap(Wrap::Wrap).with_children(children))
}

/// [`wide_tree`], with leaf `changed_index`'s background overridden to
/// `color` — lets a benchmark change exactly one leaf's paint output across
/// otherwise-identical rerenders.
pub fn wide_tree_with_leaf_color(count: usize, changed_index: usize, color: Color) -> BoxedWidget {
    let children = (0..count)
        .map(|i| {
            if i == changed_index {
                Box::new(RawView::new(
                    Style::new().width(16.0).height(16.0).background(color),
                )) as BoxedWidget
            } else {
                leaf(i)
            }
        })
        .collect();
    Box::new(Flex::row().wrap(Wrap::Wrap).with_children(children))
}

/// A `rows` x `cols` nested flex grid — a more realistic laid-out tree than
/// [`wide_tree`], still roughly `rows * cols` nodes.
pub fn grid(rows: usize, cols: usize) -> BoxedWidget {
    let row_widgets = (0..rows)
        .map(|r| {
            let cells = (0..cols).map(|c| leaf(r * cols + c)).collect();
            Box::new(Flex::row().gap(1.0).with_children(cells)) as BoxedWidget
        })
        .collect();
    Box::new(Flex::column().gap(1.0).with_children(row_widgets))
}

/// A realistic mixed desktop layout: sidebar, toolbar, and a card grid.
pub fn dashboard() -> BoxedWidget {
    let sidebar_items = (0..24)
        .map(|i| {
            Box::new(RawText::new(
                format!("Item {i}"),
                Color::rgb(30, 30, 30),
                13.0,
            )) as BoxedWidget
        })
        .collect();
    let sidebar = Flex::column()
        .gap(4.0)
        .padding(8.0)
        .width(200.0)
        .full_height()
        .background(Color::rgb(240, 240, 245))
        .with_children(sidebar_items);

    let toolbar_buttons = (0..6)
        .map(|i| {
            Box::new(
                RawView::new(
                    Style::new()
                        .width(80.0)
                        .height(28.0)
                        .background(Color::rgb(220, 220, 230)),
                )
                .child(Box::new(RawText::new(
                    format!("Action {i}"),
                    Color::rgb(20, 20, 20),
                    12.0,
                ))),
            ) as BoxedWidget
        })
        .collect();
    let toolbar = Flex::row()
        .gap(8.0)
        .padding(8.0)
        .full_width()
        .background(Color::rgb(250, 250, 252))
        .with_children(toolbar_buttons);

    let cards = (0..48)
        .map(|i| {
            Box::new(
                Flex::column()
                    .gap(4.0)
                    .padding(12.0)
                    .width(160.0)
                    .height(100.0)
                    .background(Color::rgb(255, 255, 255))
                    .child(Box::new(RawText::new(
                        format!("Card {i}"),
                        Color::rgb(10, 10, 10),
                        14.0,
                    )))
                    .child(Box::new(RawText::new(
                        "Some supporting text",
                        Color::rgb(90, 90, 90),
                        12.0,
                    ))),
            ) as BoxedWidget
        })
        .collect();
    let content = Flex::row()
        .wrap(Wrap::Wrap)
        .gap(12.0)
        .padding(12.0)
        .grow(1.0)
        .background(Color::rgb(245, 245, 248))
        .with_children(cards);

    let main = Flex::column()
        .grow(1.0)
        .child(Box::new(toolbar))
        .child(Box::new(content));

    Box::new(
        Flex::row()
            .full_width()
            .full_height()
            .child(Box::new(sidebar))
            .child(Box::new(main)),
    )
}

/// A scrollable list of `count` chat message bubbles, with message
/// `changed_index`'s text overridden to `changed_text` — lets a benchmark
/// change exactly one leaf's content across otherwise-identical rerenders.
pub fn chat_with_message(count: usize, changed_index: usize, changed_text: &str) -> BoxedWidget {
    let rows = (0..count)
        .map(|i| {
            let bg = if i % 2 == 0 {
                Color::rgb(230, 230, 235)
            } else {
                Color::rgb(210, 225, 255)
            };
            let text = if i == changed_index {
                changed_text.to_string()
            } else {
                format!("message {i}")
            };
            Box::new(
                Flex::row()
                    .padding(8.0)
                    .gap(4.0)
                    .background(bg)
                    .child(Box::new(RawText::new(text, Color::rgb(20, 20, 20), 14.0))),
            ) as BoxedWidget
        })
        .collect();
    Box::new(Flex::column().gap(2.0).with_children(rows))
}

/// A scrollable list of `count` chat message bubbles.
pub fn chat(count: usize) -> BoxedWidget {
    chat_with_message(count, usize::MAX, "")
}

/// A leaf that calls [`Painter::animation_time`] every paint, so the
/// renderer treats it as animating — see `creamui_core::scene`'s
/// layer-promotion doc comments.
struct AnimatedLeaf;

impl Widget for AnimatedLeaf {
    fn style(&self) -> Style {
        Style::new().width(24.0).height(24.0)
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.animation_time();
        painter.fill_rect(rect, Color::rgb(220, 60, 60), 4.0);
    }
}

/// `static_count` plain leaves plus `animated_count` leaves that request a
/// redraw every frame — stresses layer promotion and animated-only repaint
/// pruning.
pub fn animated_cards(static_count: usize, animated_count: usize) -> BoxedWidget {
    let mut children: Vec<BoxedWidget> = (0..static_count).map(leaf).collect();
    children.extend((0..animated_count).map(|_| Box::new(AnimatedLeaf) as BoxedWidget));
    Box::new(Flex::row().wrap(Wrap::Wrap).with_children(children))
}
