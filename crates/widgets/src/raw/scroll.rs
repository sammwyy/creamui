use super::*;
use crate::ScrollController;

/// An unstyled vertically-scrollable container. [`RawScrollView::new`] lets
/// the caller own the offset; [`RawScrollView::controlled`] clamps a shared
/// [`ScrollController`] to the resolved content height. Children are laid out at their natural height
/// (never flex-shrunk to fit the visible viewport, which would defeat the
/// point of scrolling) and clipped + offset to this widget's own rect.
///
/// When built via [`RawScrollView::controlled`], a thin draggable
/// [`RawScrollbar`] is overlaid on the right edge by default (disable with
/// [`RawScrollView::scrollbar`]) — sized and positioned from the same
/// [`ScrollController`], so dragging it and turning the mouse wheel stay in
/// sync automatically. It has no effect on [`RawScrollView::new`], which
/// has no controller to write a dragged position back into.
///
/// Internally this composes two children: a [`ScrollClip`] that does the
/// actual clipping/offsetting (kept separate so the scrollbar, painted as
/// its sibling, is never itself shifted by the content's own scroll
/// offset), and the optional [`RawScrollbar`] overlay.
pub struct RawScrollView {
    pub style: creamui_core::Style,
    pub scroll_y: f32,
    pub controller: Option<ScrollController>,
    pub children: Vec<BoxedWidget>,
    pub on_scroll: Rc<dyn Fn(f32)>,
    pub on_scroll_bounded: Option<Rc<dyn Fn(f32, f32)>>,
    pub scrollbar: bool,
    pub scrollbar_width: f32,
    pub scrollbar_margin: f32,
    pub scrollbar_color: Color,
    pub scrollbar_hover_color: Option<Color>,
    pub scrollbar_pressed_color: Option<Color>,
    pub scrollbar_track_color: Option<Color>,
    pub content_gap: f32,
    pub scrollbar_gap: f32,
}

impl RawScrollView {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        scroll_y: f32,
        on_scroll: impl Fn(f32) + 'static,
    ) -> Self {
        RawScrollView {
            style: style.into(),
            scroll_y,
            controller: None,
            children: Vec::new(),
            on_scroll: Rc::new(on_scroll),
            on_scroll_bounded: None,
            scrollbar: true,
            scrollbar_width: 6.0,
            scrollbar_margin: 2.0,
            // Faint at rest so it reads as a hint rather than competing
            // with full-width row content; brightens on hover/press so it's
            // still easy to find and grab.
            scrollbar_color: Color::rgba(128, 128, 128, 80),
            scrollbar_hover_color: Some(Color::rgba(128, 128, 128, 170)),
            scrollbar_pressed_color: Some(Color::rgba(128, 128, 128, 220)),
            scrollbar_track_color: None,
            content_gap: 0.0,
            scrollbar_gap: 0.0,
        }
    }

    pub fn controlled(style: impl Into<creamui_core::Style>, controller: ScrollController) -> Self {
        let mut view = Self::new(style, controller.offset(), |_| {});
        view.controller = Some(controller.clone());
        view.on_scroll_bounded = Some(Rc::new(move |delta, max_offset| {
            controller.scroll_by(delta, max_offset);
        }));
        view
    }

    pub fn background(mut self, color: Color) -> Self {
        self.style.paint.background = Some(color.into());
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.paint.corner_radius = Some(radius);
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style.layout = style;
        self
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }

    /// Shows or hides the draggable scrollbar overlay. Only takes effect on
    /// a controller-backed view (see [`RawScrollView::controlled`]); a
    /// plain [`RawScrollView::new`] view never draws one, since there is no
    /// controller for a drag to write a jumped-to position into. Default:
    /// `true`.
    pub fn scrollbar(mut self, visible: bool) -> Self {
        self.scrollbar = visible;
        self
    }

    /// Total width of the scrollbar's hit area, in logical pixels. The
    /// painted thumb is inset within this by [`RawScrollView::scrollbar_inset`].
    /// Default: `10.0`.
    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbar_width = width.max(1.0);
        self
    }

    /// Gap between the scrollbar and the view's right edge. Default: `2.0`.
    pub fn scrollbar_margin(mut self, margin: f32) -> Self {
        self.scrollbar_margin = margin.max(0.0);
        self
    }

    /// Thumb color at rest. Default: a translucent mid-gray.
    pub fn scrollbar_color(mut self, color: Color) -> Self {
        self.scrollbar_color = color;
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

    /// Background painted behind the thumb across the full scrollbar
    /// width/height. Default: `None` (no track chrome, just the thumb).
    pub fn scrollbar_track_color(mut self, color: Color) -> Self {
        self.scrollbar_track_color = Some(color);
        self
    }

    /// Fixed pixel gap between consecutive children, applied the same way
    /// [`crate::layout::column`]'s `gap` is. Default: `0.0`.
    pub fn content_gap(mut self, gap: f32) -> Self {
        self.content_gap = gap.max(0.0);
        self
    }

    /// Extra space between content's reserved right edge and the scrollbar
    /// track itself (on top of [`RawScrollView::scrollbar_margin`], which
    /// only separates the track from the viewport's outer edge). Default:
    /// `0.0` — content and track sit flush.
    pub fn scrollbar_gap(mut self, gap: f32) -> Self {
        self.scrollbar_gap = gap.max(0.0);
        self
    }
}

impl Widget for RawScrollView {
    fn style(&self) -> creamui_core::Style {
        // Always Column, regardless of what the caller passes: the sole
        // in-flow child is the `ScrollClip` (see `children()` below), and
        // it needs Column's cross axis (width) to `align-items: stretch` to
        // the container's width while its main axis (height) stays
        // whatever the caller asked for — the combination that makes "as
        // wide as the viewport, as tall as the caller wants" happen. Row
        // direction would stretch the wrong axis instead.
        // A caller that gives this a `flex_grow` (rather than an explicit
        // size) — e.g. a sidebar's item list filling whatever room is left
        // between a header and a footer — hits the same CSS "min-height:
        // auto" trap `ScrollClip` already works around: without an explicit
        // `min_size`, this would still refuse to shrink below its own
        // content's height, so `flex_grow` could only ever grow it, never
        // let it actually shrink to fit and scroll. `shrinkable` leaves an
        // axis alone if the caller already set one explicitly.
        crate::layout::shrinkable(Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..self.style.layout.clone()
        })
        .into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let _ = (painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        // Keeps full-width row backgrounds from sitting under the thumb.
        let scrollbar_gutter = if self.scrollbar && self.controller.is_some() {
            self.scrollbar_gap + self.scrollbar_width + self.scrollbar_margin
        } else {
            0.0
        };
        let content_style = Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            flex_shrink: 0.0,
            gap: creamui_core::layout::Size {
                width: creamui_core::layout::LengthPercentage::Length(self.content_gap),
                height: creamui_core::layout::LengthPercentage::Length(self.content_gap),
            },
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(0.0),
                right: creamui_core::layout::LengthPercentage::Length(scrollbar_gutter),
                top: creamui_core::layout::LengthPercentage::Length(0.0),
                bottom: creamui_core::layout::LengthPercentage::Length(0.0),
            },
            ..Default::default()
        };
        let content = RawView::new(content_style).with_children(std::mem::take(&mut self.children));

        let clip = ScrollClip {
            // Without `shrinkable`, the CSS "min-height: auto" trap floors
            // `flex_grow` at the overflowing content's own height.
            style: crate::layout::shrinkable(Style {
                display: creamui_core::layout::Display::Flex,
                flex_direction: creamui_core::layout::FlexDirection::Column,
                flex_grow: 1.0,
                ..Default::default()
            })
            .into(),
            controller: self.controller.clone(),
            scroll_y: self.scroll_y,
            on_scroll: self.on_scroll.clone(),
            on_scroll_bounded: self.on_scroll_bounded.clone(),
            corner_radius: self.style.paint.corner_radius.unwrap_or(0.0),
            content: Some(Box::new(content)),
        };

        let mut out: Vec<BoxedWidget> = vec![Box::new(clip)];
        if self.scrollbar {
            if let Some(controller) = &self.controller {
                let bar_style = Style {
                    position: creamui_core::layout::Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        top: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                        right: creamui_core::layout::LengthPercentageAuto::Length(
                            self.scrollbar_margin,
                        ),
                        bottom: creamui_core::layout::LengthPercentageAuto::Auto,
                        left: creamui_core::layout::LengthPercentageAuto::Auto,
                    },
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Length(self.scrollbar_width),
                        height: creamui_core::layout::Dimension::Percent(1.0),
                    },
                    ..Default::default()
                };
                let mut bar =
                    RawScrollbar::new(bar_style, controller.clone(), self.scrollbar_color);
                bar.hover_color = self.scrollbar_hover_color;
                bar.pressed_color = self.scrollbar_pressed_color;
                bar.track_color = self.scrollbar_track_color;
                out.push(Box::new(bar));
            }
        }
        out
    }
}

/// The actual clipping/offsetting/wheel-handling half of [`RawScrollView`],
/// kept as a separate widget so a sibling [`RawScrollbar`] can sit beside
/// it — as a child of the same undipped, unoffset parent — instead of
/// being caught by its own [`Widget::scroll_offset`].
struct ScrollClip {
    style: creamui_core::Style,
    scroll_y: f32,
    controller: Option<ScrollController>,
    on_scroll: Rc<dyn Fn(f32)>,
    on_scroll_bounded: Option<Rc<dyn Fn(f32, f32)>>,
    corner_radius: f32,
    content: Option<BoxedWidget>,
}

impl Widget for ScrollClip {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.content.take().into_iter().collect()
    }

    fn clips_children(&self) -> bool {
        true
    }

    fn clip_corner_radius(&self) -> f32 {
        self.corner_radius
    }

    fn scroll_offset(&self) -> Point {
        Point {
            x: 0.0,
            y: self
                .controller
                .as_ref()
                .map_or(self.scroll_y, ScrollController::peek),
        }
    }

    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        Some(self.on_scroll.clone())
    }

    fn on_scroll_bounded(&self) -> Option<Rc<dyn Fn(f32, f32)>> {
        self.on_scroll_bounded.clone()
    }

    fn on_content_overflow(&self) -> Option<Rc<dyn Fn(f32)>> {
        let controller = self.controller.clone()?;
        Some(Rc::new(move |max_offset| {
            controller.report_max_offset(max_offset);
        }))
    }
}

/// Computes the painted thumb's length and its offset from the track's
/// start, or `None` when there isn't enough overflow to justify a thumb
/// (including the not-yet-measured `max_offset == f32::INFINITY` case).
/// Shared between [`RawScrollbar::paint`] and its drag handler so both
/// agree on exactly where the thumb is.
fn thumb_geometry(
    track_length: f32,
    max_offset: f32,
    offset: f32,
    min_length: f32,
) -> Option<(f32, f32)> {
    if !max_offset.is_finite() || max_offset <= 0.5 || track_length <= 0.0 {
        return None;
    }
    let content_length = track_length + max_offset;
    let thumb_length = (track_length * (track_length / content_length))
        .clamp(min_length.min(track_length), track_length);
    let usable = (track_length - thumb_length).max(0.0);
    let fraction = (offset / max_offset).clamp(0.0, 1.0);
    Some((thumb_length, usable * fraction))
}

/// A draggable vertical scrollbar thumb, self-contained enough to be used
/// on its own (outside [`RawScrollView`]) as long as it shares a
/// [`ScrollController`] with whatever it's meant to scroll. Sizes and
/// positions its thumb from [`ScrollController::max_offset`] — kept fresh
/// every paint by the scroll view's [`Widget::on_content_overflow`] hook —
/// so it never needs its own access to the scrolled content's layout.
pub struct RawScrollbar {
    pub style: creamui_core::Style,
    pub controller: ScrollController,
    pub color: Color,
    pub hover_color: Option<Color>,
    pub pressed_color: Option<Color>,
    pub track_color: Option<Color>,
    pub thumb_inset: f32,
    pub thumb_radius: Option<f32>,
    pub min_thumb_length: f32,
}

impl RawScrollbar {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: ScrollController,
        color: Color,
    ) -> Self {
        RawScrollbar {
            style: style.into(),
            controller,
            color,
            hover_color: None,
            pressed_color: None,
            track_color: None,
            thumb_inset: 1.0,
            thumb_radius: None,
            min_thumb_length: 24.0,
        }
    }

    pub fn hover_color(mut self, color: Color) -> Self {
        self.hover_color = Some(color);
        self
    }

    pub fn pressed_color(mut self, color: Color) -> Self {
        self.pressed_color = Some(color);
        self
    }

    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = Some(color);
        self
    }

    pub fn thumb_inset(mut self, inset: f32) -> Self {
        self.thumb_inset = inset.max(0.0);
        self
    }

    pub fn thumb_radius(mut self, radius: f32) -> Self {
        self.thumb_radius = Some(radius.max(0.0));
        self
    }

    pub fn min_thumb_length(mut self, length: f32) -> Self {
        self.min_thumb_length = length.max(0.0);
        self
    }
}

impl Widget for RawScrollbar {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(track_color) = self.track_color {
            painter.fill_rect(
                rect,
                track_color,
                self.thumb_radius.unwrap_or(rect.width / 2.0),
            );
        }
        let Some((thumb_length, thumb_offset)) = thumb_geometry(
            rect.height,
            self.controller.max_offset(),
            self.controller.peek(),
            self.min_thumb_length,
        ) else {
            return;
        };
        let thumb_rect = Rect {
            x: rect.x + self.thumb_inset,
            y: rect.y + thumb_offset,
            width: (rect.width - self.thumb_inset * 2.0).max(1.0),
            height: thumb_length,
        };
        let color = if painter.pressed(rect) {
            self.pressed_color
                .or(self.hover_color)
                .unwrap_or(self.color)
        } else if painter.hovered(rect) {
            self.hover_color.unwrap_or(self.color)
        } else {
            self.color
        };
        painter.fill_rect(
            thumb_rect,
            color,
            self.thumb_radius.unwrap_or(thumb_rect.width / 2.0),
        );
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let max_offset = self.controller.max_offset();
        if !max_offset.is_finite() || max_offset <= 0.5 {
            return None;
        }
        let controller = self.controller.clone();
        let min_length = self.min_thumb_length;
        Some(Rc::new(move |local: Point, rect: Rect| {
            let max_offset = controller.max_offset();
            let Some((thumb_length, _)) =
                thumb_geometry(rect.height, max_offset, controller.peek(), min_length)
            else {
                return;
            };
            let usable = (rect.height - thumb_length).max(1.0);
            let fraction = ((local.y - thumb_length / 2.0) / usable).clamp(0.0, 1.0);
            controller.set(fraction * max_offset);
        }))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        let max_offset = self.controller.max_offset();
        (max_offset.is_finite() && max_offset > 0.5).then_some(CursorIcon::Pointer)
    }
}
