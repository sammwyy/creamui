use super::*;
use crate::ScrollController;
use creamui_core::{visible_range, HeightIndex};
use std::cell::{Cell, RefCell};

/// Persistent per-item height bookkeeping for [`RawVirtualList`], owned by
/// the caller and cloned into the widget on every rebuild — the same
/// discipline [`crate::ScrollController`] already requires, since nothing
/// in this crate rebuilds widgets in place across renders.
#[derive(Clone)]
pub struct VirtualListState {
    heights: Rc<RefCell<HeightIndex>>,
    viewport_height: Rc<Cell<Option<f32>>>,
}

impl VirtualListState {
    pub fn new(item_count: usize, estimated_height: f32) -> Self {
        Self {
            heights: Rc::new(RefCell::new(HeightIndex::uniform(
                item_count,
                estimated_height.max(0.0),
            ))),
            viewport_height: Rc::new(Cell::new(None)),
        }
    }

    pub fn len(&self) -> usize {
        self.heights.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.heights.borrow().is_empty()
    }

    pub fn total_height(&self) -> f32 {
        self.heights.borrow().total_height()
    }

    pub fn height(&self, index: usize) -> f32 {
        self.heights.borrow().height(index)
    }

    /// Sets a row height until that row is measured again during layout.
    pub fn set_height(&self, index: usize, height: f32) {
        self.heights.borrow_mut().set_height(index, height.max(0.0));
    }

    /// Replaces the item count, resetting every height to `estimated_height`.
    /// Call this when the underlying list's length changes; per-item
    /// heights set via [`VirtualListState::set_height`] do not survive it.
    pub fn set_item_count(&self, item_count: usize, estimated_height: f32) {
        *self.heights.borrow_mut() = HeightIndex::uniform(item_count, estimated_height.max(0.0));
    }

    fn offset(&self, index: usize) -> f32 {
        self.heights.borrow().offset(index)
    }

    fn visible_range(
        &self,
        scroll_offset: f32,
        viewport_height: f32,
        overscan: usize,
    ) -> std::ops::Range<usize> {
        visible_range(
            &self.heights.borrow(),
            scroll_offset,
            viewport_height,
            overscan,
        )
    }

    fn report_viewport_height(&self, height: f32) -> bool {
        let height = height.max(0.0);
        if self.viewport_height.get() == Some(height) {
            return false;
        }
        self.viewport_height.set(Some(height));
        true
    }

    fn report_row_height(&self, index: usize, height: f32) -> bool {
        if !height.is_finite() {
            return false;
        }
        let height = height.max(0.0);
        let mut heights = self.heights.borrow_mut();
        if heights.height(index) == height {
            return false;
        }
        heights.set_height(index, height);
        true
    }
}

/// A vertically-scrollable list that mounts only the rows within
/// `viewport_height` of the current scroll offset (plus [`RawVirtualList::overscan`]
/// rows on each side), instead of every row in [`VirtualListState`]. The
/// list measures mounted rows after layout and updates their heights in
/// `state`.
///
/// Wraps [`RawScrollView`], so it shares its draggable scrollbar and wheel
/// routing. `viewport_height` is used until layout resolves the widget's
/// height. The caller should keep `state` across rebuilds.
pub struct RawVirtualList {
    pub style: creamui_core::Style,
    pub state: VirtualListState,
    pub controller: ScrollController,
    pub viewport_height: f32,
    pub overscan: usize,
    pub scrollbar: bool,
    pub scrollbar_color: Option<Color>,
    pub scrollbar_hover_color: Option<Color>,
    pub scrollbar_pressed_color: Option<Color>,
    pub item: Rc<dyn Fn(usize) -> BoxedWidget>,
}

impl RawVirtualList {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        state: VirtualListState,
        controller: ScrollController,
        viewport_height: f32,
        item: impl Fn(usize) -> BoxedWidget + 'static,
    ) -> Self {
        RawVirtualList {
            style: style.into(),
            state,
            controller,
            viewport_height: viewport_height.max(0.0),
            overscan: 4,
            scrollbar: true,
            scrollbar_color: None,
            scrollbar_hover_color: None,
            scrollbar_pressed_color: None,
            item: Rc::new(item),
        }
    }

    /// Extra rows mounted beyond each edge of the viewport, so a fast
    /// scroll or a focus jump doesn't flash an unmounted row. Default: `4`.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn scrollbar(mut self, visible: bool) -> Self {
        self.scrollbar = visible;
        self
    }

    pub fn scrollbar_color(mut self, color: Color) -> Self {
        self.scrollbar_color = Some(color);
        self
    }

    pub fn scrollbar_hover_color(mut self, color: Color) -> Self {
        self.scrollbar_hover_color = Some(color);
        self
    }

    pub fn scrollbar_pressed_color(mut self, color: Color) -> Self {
        self.scrollbar_pressed_color = Some(color);
        self
    }

    fn effective_viewport_height(&self) -> f32 {
        self.state
            .viewport_height
            .get()
            .unwrap_or_else(|| match self.style.layout.size.height {
                creamui_core::layout::Dimension::Length(height) => height.max(0.0),
                _ => self.viewport_height,
            })
    }
}

impl Widget for RawVirtualList {
    fn style(&self) -> creamui_core::Style {
        crate::layout::shrinkable(Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..self.style.layout.clone()
        })
        .into()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn on_layout(&self, rect: Rect) -> bool {
        self.state.report_viewport_height(rect.height)
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let range = self.state.visible_range(
            self.controller.peek(),
            self.effective_viewport_height(),
            self.overscan,
        );

        let rows: Vec<BoxedWidget> = range
            .map(|index| {
                let row_style = Style {
                    position: creamui_core::layout::Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        top: creamui_core::layout::LengthPercentageAuto::Length(
                            self.state.offset(index),
                        ),
                        left: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                        right: creamui_core::layout::LengthPercentageAuto::Auto,
                        bottom: creamui_core::layout::LengthPercentageAuto::Auto,
                    },
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Percent(1.0),
                        height: creamui_core::layout::Dimension::Auto,
                    },
                    ..Default::default()
                };
                Box::new(MeasuredRow {
                    style: row_style.into(),
                    state: self.state.clone(),
                    index,
                    child: Some((self.item)(index)),
                }) as BoxedWidget
            })
            .collect();

        let spacer_style = Style {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Percent(1.0),
                height: creamui_core::layout::Dimension::Length(self.state.total_height()),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };
        let spacer = RawView::new(spacer_style).with_children(rows);

        let mut scroll_view =
            RawScrollView::controlled(self.style.clone(), self.controller.clone())
                .scrollbar(self.scrollbar)
                .rebuild_on_scroll(true);
        if let Some(color) = self.scrollbar_color {
            scroll_view = scroll_view.scrollbar_color(color);
        }
        if let Some(color) = self.scrollbar_hover_color {
            scroll_view = scroll_view.scrollbar_hover_color(color);
        }
        if let Some(color) = self.scrollbar_pressed_color {
            scroll_view = scroll_view.scrollbar_pressed_color(color);
        }
        let scroll_view = scroll_view.child(Box::new(spacer));

        vec![Box::new(scroll_view)]
    }
}

struct MeasuredRow {
    style: creamui_core::Style,
    state: VirtualListState,
    index: usize,
    child: Option<BoxedWidget>,
}

impl Widget for MeasuredRow {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.state.report_row_height(self.index, rect.height)
    }

    fn reports_layout(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn only_the_visible_plus_overscan_range_is_built() {
        let item_count = 10_000;
        let item_height = 20.0;
        let scroll_offset = 500.0;
        let viewport_height = 100.0;
        let overscan = 2;
        let expected: Vec<usize> = visible_range(
            &HeightIndex::uniform(item_count, item_height),
            scroll_offset,
            viewport_height,
            overscan,
        )
        .collect();

        let state = VirtualListState::new(item_count, item_height);
        let controller = ScrollController::new(scroll_offset);
        let built: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
        let built_for_closure = built.clone();
        let mut list = RawVirtualList::new(
            Style::default(),
            state,
            controller,
            viewport_height,
            move |index| {
                built_for_closure.borrow_mut().push(index);
                Box::new(RawView::new(Style::default())) as BoxedWidget
            },
        )
        .overscan(overscan);

        let _ = Widget::children(&mut list);

        assert_eq!(*built.borrow(), expected);
        assert!(expected.len() < item_count);
    }

    #[test]
    fn scrolling_changes_which_rows_are_built() {
        let item_count = 1_000;
        let item_height = 10.0;
        let viewport_height = 50.0;
        let index = HeightIndex::uniform(item_count, item_height);
        let first_range = visible_range(&index, 0.0, viewport_height, 0);
        let second_range = visible_range(&index, 500.0, viewport_height, 0);

        let call_count = Rc::new(Cell::new(0usize));
        let state = VirtualListState::new(item_count, item_height);
        let controller = ScrollController::new(0.0);
        let counter = call_count.clone();
        let mut list = RawVirtualList::new(
            Style::default(),
            state,
            controller.clone(),
            viewport_height,
            move |_| {
                counter.set(counter.get() + 1);
                Box::new(RawView::new(Style::default())) as BoxedWidget
            },
        )
        .overscan(0);

        let _ = Widget::children(&mut list);
        assert_eq!(call_count.get(), first_range.len());

        controller.set(500.0);
        let _ = Widget::children(&mut list);
        assert_eq!(call_count.get(), first_range.len() + second_range.len());
    }

    #[test]
    fn virtual_list_state_tracks_per_item_heights() {
        let state = VirtualListState::new(3, 10.0);
        assert_eq!(state.total_height(), 30.0);
        state.set_height(1, 40.0);
        assert_eq!(state.height(1), 40.0);
        assert_eq!(state.total_height(), 60.0);
    }

    #[test]
    fn a_fixed_style_height_is_used_as_the_viewport() {
        let state = VirtualListState::new(100, 10.0);
        let controller = ScrollController::new(200.0);
        let built = Rc::new(RefCell::new(Vec::new()));
        let built_for_item = built.clone();
        let mut list = RawVirtualList::new(
            creamui_core::Style::new().height(30.0),
            state,
            controller,
            1.0,
            move |index| {
                built_for_item.borrow_mut().push(index);
                Box::new(RawView::new(Style::default())) as BoxedWidget
            },
        )
        .overscan(0);

        let _ = Widget::children(&mut list);

        assert_eq!(*built.borrow(), vec![20, 21, 22, 23]);
    }

    #[test]
    fn layout_reports_flex_viewport_and_measured_row_heights() {
        use creamui_core::{Renderer, Size};

        let state = VirtualListState::new(100, 10.0);
        let controller = ScrollController::new(0.0);
        let mut renderer = Renderer::new();
        let viewport = Size {
            width: 200.0,
            height: 100.0,
        };
        let build = || {
            let list = RawVirtualList::new(
                Style {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                state.clone(),
                controller.clone(),
                1.0,
                |index| {
                    let height = if index % 2 == 0 { 20.0 } else { 40.0 };
                    Box::new(RawView::new(creamui_core::Style::new().height(height)))
                },
            )
            .overscan(0);
            Box::new(
                RawView::new(creamui_core::Style::new().width(200.0).height(75.0))
                    .child(Box::new(list)),
            ) as BoxedWidget
        };

        for _ in 0..8 {
            renderer.update(build(), viewport);
            if !renderer.layout_feedback() {
                break;
            }
        }

        assert!(!renderer.layout_feedback());
        assert_eq!(state.viewport_height.get(), Some(75.0));
        assert_eq!(state.height(0), 20.0);
        assert_eq!(state.height(1), 40.0);
        assert_eq!(state.height(2), 20.0);
    }
}
