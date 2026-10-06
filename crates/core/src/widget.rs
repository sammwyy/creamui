use crate::geometry::{Point, Rect};
use std::ops::Range;
use std::rc::Rc;
use taffy::geometry::Size;
use taffy::style::AvailableSpace;

/// A logical key, decoded from the backend's native key event into
/// something widgets can match on without depending on a specific backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// A keyboard event delivered to whichever widget currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub modifiers: Modifiers,
}

/// Keyboard modifiers carried with every [`KeyInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

/// A widget's intrinsic-size function, used by `taffy`'s layout algorithm
/// for leaves whose size depends on content (e.g. text) rather than being
/// fully determined by their `Style`. Called with the dimensions already
/// pinned down by layout (`known_dimensions`) and the space available on
/// each axis; must return a definite size.
pub type MeasureFn = Box<dyn Fn(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>>;

/// Horizontal text alignment within a widget's painted rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// The system mouse cursor shape to show while the pointer hovers a widget.
/// Backend-agnostic, mirroring [`Key`]'s role for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorIcon {
    #[default]
    Default,
    /// An I-beam, shown over editable text (e.g. a text input).
    Text,
    /// A hand/pointer, shown over clickable widgets.
    Pointer,
    /// A "no" circle-with-a-line, shown over a disabled control.
    NotAllowed,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNwse,
    ResizeNesw,
}

/// An explicit structural identity a widget can opt into, so reconciliation
/// matches it against last frame's widget by identity instead of by
/// position among its siblings. See [`Widget::key`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetKey {
    U64(u64),
    String(Rc<str>),
}

impl From<u64> for WidgetKey {
    fn from(value: u64) -> Self {
        WidgetKey::U64(value)
    }
}

impl From<&str> for WidgetKey {
    fn from(value: &str) -> Self {
        WidgetKey::String(Rc::from(value))
    }
}

impl From<String> for WidgetKey {
    fn from(value: String) -> Self {
        WidgetKey::String(Rc::from(value))
    }
}

#[derive(Clone)]
pub struct WindowDragHandle(Rc<dyn Fn()>);

impl WindowDragHandle {
    pub fn new(drag: impl Fn() + 'static) -> Self {
        Self(Rc::new(drag))
    }

    pub fn start_drag(&self) {
        (self.0)();
    }
}

/// Backend-agnostic drawing surface a [`Widget`] paints itself onto.
///
/// Implemented by `creamui-render`'s display-list recorder and by test
/// doubles; widgets never depend on a specific backend.
pub trait Painter {
    /// Color scheme used to resolve semantic [`crate::ColorToken`] values.
    fn color_scheme(&self) -> creamui_theme::ColorScheme {
        creamui_theme::ColorScheme::default()
    }

    /// Optional pointer paint context. Coordinates are logical pixels.
    fn hovered(&self, _rect: Rect) -> bool {
        false
    }
    fn pressed(&self, _rect: Rect) -> bool {
        false
    }
    /// Request another frame only while an animated control is visible.
    fn animation_time(&mut self) -> f32 {
        0.0
    }

    fn stroke_line(&mut self, from: Point, to: Point, color: creamui_theme::Color, width: f32) {
        let steps = ((to.x - from.x).abs().max((to.y - from.y).abs()) * 2.0)
            .ceil()
            .max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.fill_rect(
                Rect {
                    x: from.x + (to.x - from.x) * t - width / 2.,
                    y: from.y + (to.y - from.y) * t - width / 2.,
                    width,
                    height: width,
                },
                color,
                width / 2.,
            );
        }
    }
    fn fill_text_weight(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: TextAlign,
        _bold: bool,
        _italic: bool,
    ) {
        self.fill_text(rect, text, color, font_size, align);
    }

    /// Like [`Painter::fill_text_weight`], resolving `family` (a CSS-style
    /// stack, e.g. `"Inter, system-ui"`) against the font registry instead
    /// of the system default. `None` behaves exactly like
    /// [`Painter::fill_text_weight`].
    #[allow(clippy::too_many_arguments)]
    fn fill_text_font(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) {
        let _ = family;
        self.fill_text_weight(rect, text, color, font_size, align, bold, italic);
    }
    /// Declares a translucent widget background for compositor blur.
    /// Separate from draw calls so text, borders and shadows never enlarge it.
    fn background_blur_region(&mut self, _rect: Rect, _corner_radius: f32) {}

    fn fill_rect(&mut self, rect: Rect, color: creamui_theme::Color, corner_radius: f32);
    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        _angle_degrees: f32,
        corner_radius: f32,
    ) {
        self.fill_rect(rect, start.mix(end, 0.5), corner_radius);
    }
    #[allow(clippy::too_many_arguments)]
    fn fill_radial_gradient(
        &mut self,
        rect: Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        _center: Point,
        _radius: f32,
        corner_radius: f32,
    ) {
        self.fill_rect(rect, start.mix(end, 0.5), corner_radius);
    }
    fn fill_radial_gradient_ellipse(
        &mut self,
        rect: Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        center: Point,
        radii: [f32; 2],
        corner_radius: f32,
    ) {
        self.fill_radial_gradient(
            rect,
            start,
            end,
            center,
            radii[0].max(radii[1]),
            corner_radius,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn fill_radial_gradient_ellipse_repeating(
        &mut self,
        rect: Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        center: Point,
        radii: [f32; 2],
        _repeating: bool,
        corner_radius: f32,
    ) {
        self.fill_radial_gradient_ellipse(rect, start, end, center, radii, corner_radius);
    }
    fn fill_radial_gradient_stops(
        &mut self,
        rect: Rect,
        stops: &[crate::ResolvedRadialColorStop],
        center: Point,
        radii: [f32; 2],
        repeating: bool,
        corner_radius: f32,
    ) {
        let Some((start, end)) = stops.first().zip(stops.last()) else {
            return;
        };
        self.fill_radial_gradient_ellipse_repeating(
            rect,
            start.color,
            end.color,
            center,
            radii,
            repeating,
            corner_radius,
        );
    }
    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        color: creamui_theme::Color,
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        spread: f32,
        corner_radius: f32,
    ) {
        if color.a == 0 {
            return;
        }
        let blur = blur.max(0.0);
        let steps = blur.ceil().clamp(1.0, 20.0) as usize;
        let weight_sum = (0..steps)
            .map(|index| {
                let t = index as f32 / steps as f32;
                (1.0 - t) * (1.0 - t)
            })
            .sum::<f32>()
            .max(1.0);
        for index in (0..steps).rev() {
            let t = index as f32 / steps as f32;
            let expansion = spread + blur * t;
            let weight = (1.0 - t) * (1.0 - t) / weight_sum;
            let layer = creamui_theme::Color::rgba(
                color.r,
                color.g,
                color.b,
                (color.a as f32 * weight).round() as u8,
            );
            self.fill_rect(
                Rect {
                    x: rect.x + offset_x - expansion,
                    y: rect.y + offset_y - expansion,
                    width: rect.width + expansion * 2.0,
                    height: rect.height + expansion * 2.0,
                },
                layer,
                (corner_radius + expansion).max(0.0),
            );
        }
    }
    fn stroke_rect(
        &mut self,
        rect: Rect,
        color: creamui_theme::Color,
        width: f32,
        corner_radius: f32,
    );
    fn stroke_rect_inside(
        &mut self,
        rect: Rect,
        color: creamui_theme::Color,
        width: f32,
        corner_radius: f32,
    ) {
        self.stroke_rect(rect.inflate(-width / 2.0), color, width, corner_radius);
    }
    fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: TextAlign,
    );

    /// Draws `image` stretched into `rect`. With `tint`, only the image's
    /// alpha is kept and every visible pixel takes the tint color.
    fn draw_image(
        &mut self,
        _rect: Rect,
        _image: &crate::RgbaImage,
        _tint: Option<creamui_theme::Color>,
    ) {
    }

    fn draw_image_opacity(
        &mut self,
        rect: Rect,
        image: &crate::RgbaImage,
        tint: Option<creamui_theme::Color>,
        opacity: f32,
    ) {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity == 0.0 {
            return;
        }
        if opacity == 1.0 {
            self.draw_image(rect, image, tint);
        } else if let Some(tint) = tint {
            self.draw_image(
                rect,
                image,
                Some(creamui_theme::Color::rgba(
                    tint.r,
                    tint.g,
                    tint.b,
                    (tint.a as f32 * opacity).round() as u8,
                )),
            );
        } else {
            let faded = image.clone().map_pixels(|pixel| {
                for channel in pixel {
                    *channel = (*channel as f32 * opacity).round() as u8;
                }
            });
            self.draw_image(rect, &faded, None);
        }
    }

    /// Draws a text run whose glyph layout stays intact while a byte range
    /// receives a different foreground color. Backends that do not support
    /// per-glyph coloring may use the stable normal-color fallback.
    fn fill_text_selected(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: Range<usize>,
        font_size: f32,
        align: TextAlign,
    ) {
        let _ = (selected_color, selected);
        self.fill_text(rect, text, color, font_size, align);
    }

    /// Like [`Painter::fill_text_selected`], resolving `family` through the
    /// font registry before drawing.
    #[allow(clippy::too_many_arguments)]
    fn fill_text_selected_font(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: Range<usize>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
    ) {
        let _ = family;
        self.fill_text_selected(
            rect,
            text,
            color,
            selected_color,
            selected,
            font_size,
            align,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_text_selected_weight_font(
        &mut self,
        rect: Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: Range<usize>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
        _bold: bool,
        _italic: bool,
    ) {
        self.fill_text_selected_font(
            rect,
            text,
            color,
            selected_color,
            selected,
            font_size,
            align,
            family,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text_decorations(
        &mut self,
        rect: Rect,
        _text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        _align: TextAlign,
        _family: Option<&str>,
        _bold: bool,
        underline: bool,
        strikethrough: bool,
    ) {
        let center_y = rect.y + rect.height / 2.0;
        let thickness = (font_size * 0.06).max(1.0);
        for y in [
            underline.then_some(center_y + font_size * 0.32),
            strikethrough.then_some(center_y - font_size * 0.02),
        ]
        .into_iter()
        .flatten()
        {
            self.fill_rect(
                Rect {
                    x: rect.x,
                    y: y - thickness / 2.0,
                    width: rect.width,
                    height: thickness,
                },
                color,
                0.0,
            );
        }
    }

    /// Restricts all subsequent drawing (until the matching [`Painter::pop_clip`])
    /// to `rect`, intersected with any already-active clip. Used by
    /// scrollable containers to hide content outside their own bounds.
    /// Default: a no-op, for backends/tests that don't need real clipping.
    fn push_clip(&mut self, _rect: Rect) {}

    /// Like [`Painter::push_clip`], preserving rounded corners when supported.
    fn push_clip_rounded(&mut self, rect: Rect, _corner_radius: f32) {
        self.push_clip(rect);
    }

    /// Removes the most recently pushed clip. Must be paired 1:1 with
    /// [`Painter::push_clip`] calls. Default: a no-op.
    fn pop_clip(&mut self) {}

    /// Like [`Painter::push_clip_rounded`] for the viewport of content
    /// scrolled by `offset` (the content's origin sits at the viewport's
    /// origin minus `offset`). Painters that retain frames can then move
    /// already drawn content instead of redrawing it. Paired with
    /// [`Painter::pop_scroll_layer`].
    fn push_scroll_layer(&mut self, viewport: Rect, corner_radius: f32, offset: Point) {
        let _ = offset;
        self.push_clip_rounded(viewport, corner_radius);
    }

    fn pop_scroll_layer(&mut self) {
        self.pop_clip();
    }
}

/// A node in a CreamUI widget tree.
///
/// Widgets are cheap, immutable descriptions rebuilt on every reactive
/// re-render (similar to an immediate-mode `view()` function); the expensive
/// work — layout and painting — only happens through [`crate::render_frame`].
/// This keeps the model simple now and leaves room for a future retained /
/// diffed tree without changing the trait.
pub trait Widget {
    /// This widget's common style declaration. The layout engine consumes
    /// only [`crate::Style::layout`]; the widget consumes whichever paint,
    /// typography, and state properties it understands.
    fn style(&self) -> crate::Style;

    /// Component-owned pseudo states. Pointer and focus flags are added by
    /// the renderer; widgets use this hook for states such as `disabled`.
    fn style_state(&self) -> crate::StyleState {
        crate::StyleState::NORMAL
    }

    /// Paints component-specific content into `rect` (already laid out in
    /// the parent's coordinate space). The renderer paints the common
    /// background, border, radius, and outline first. Does not paint children.
    fn paint(&self, painter: &mut dyn Painter, rect: Rect);

    fn paint_content(&self, painter: &mut dyn Painter, rect: Rect, _content: Rect) {
        self.paint(painter, rect);
    }

    /// This widget's fundamental content, for [`crate::runtime::mount_legacy_widget`]
    /// to classify as [`crate::runtime::NodeKind::Text`]/[`crate::runtime::NodeKind::Image`]
    /// instead of the default opaque [`crate::runtime::NodeKind::Custom`].
    /// `None` (the default) mounts as `Custom` — correct for any widget,
    /// just opaque to whatever inspects the runtime tree by `NodeKind`
    /// (e.g. a future devtools inspector). Layout and paint still come
    /// entirely from [`Widget::style`]/[`Widget::paint`] either way.
    fn legacy_node_kind(&self) -> Option<crate::runtime::NodeKind> {
        None
    }

    /// Takes ownership of this widget's children, in layout order, leaving
    /// it childless. Takes `&mut self` (rather than consuming the widget)
    /// so implementors can `std::mem::take` an owned `Vec<BoxedWidget>`
    /// field while keeping the rest of `self` around for [`Widget::paint`].
    /// Default: a leaf widget with no children.
    fn children(&mut self) -> Vec<BoxedWidget> {
        Vec::new()
    }

    /// Receives the resolved window-local rectangle after layout. Return
    /// `true` when the result changes data used to build this widget tree.
    fn on_layout(&self, _rect: Rect) -> bool {
        false
    }

    /// Whether this widget needs [`Widget::on_layout`] after layout.
    fn reports_layout(&self) -> bool {
        false
    }

    /// Optional click handler. When present, this widget's laid-out rect
    /// becomes part of hit-testing for pointer clicks.
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        None
    }

    /// Optional click handler that receives the pointer position in the
    /// widget's window-local coordinate space.
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        None
    }

    /// Optional intrinsic-size function for content-sized leaves (e.g.
    /// text). Default: `None`, meaning this widget's size is fully
    /// determined by its `Style` (the common case for containers).
    fn measure(&self) -> Option<MeasureFn> {
        None
    }

    /// A cheap, stable hash of everything [`Widget::measure`]'s closure
    /// would capture — e.g. text + font face + font size + wrap width for a
    /// text widget. `None` (the default) opts out: reconciliation always
    /// re-registers the measure closure with the layout engine, which is
    /// always correct, just forgoes the cache. A widget with no
    /// [`Widget::measure`] has nothing worth fingerprinting and should
    /// leave this `None`.
    fn measure_fingerprint(&self) -> Option<u64> {
        None
    }

    /// An explicit structural identity for this widget, opting its parent's
    /// child list into keyed reconciliation: children are matched against
    /// last frame's by key instead of by position, so inserting, removing,
    /// or reordering a keyed sibling doesn't misattribute the rest of the
    /// list's persistent state (focus, layer cache, scroll position, ...)
    /// to the wrong item. Default: `None` (positional matching, the
    /// existing behavior). Only takes effect when at least one child in a
    /// given parent has a key; unkeyed siblings under the same parent still
    /// match positionally against each other.
    fn key(&self) -> Option<WidgetKey> {
        None
    }

    /// Whether this widget can receive keyboard focus. A click inside a
    /// focusable widget's rect gives it focus; a click elsewhere clears
    /// focus. Default: `false`.
    fn focusable(&self) -> bool {
        false
    }

    /// Excludes previously painted controls from interaction and tab order.
    /// Modal layers must follow application content in paint order.
    fn is_modal(&self) -> bool {
        false
    }

    /// Optional keyboard handler, called with each [`KeyInput`] while this
    /// widget has focus (see [`Widget::focusable`]). Default: `None`.
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        None
    }

    /// Whether focusing this widget should enable the platform text input method.
    fn accepts_text_input(&self) -> bool {
        false
    }

    /// Optional drag handler for press-and-drag interactions (e.g. a
    /// slider). Called on the initial press and on every subsequent pointer
    /// move while the button stays held, with the pointer's position in
    /// this widget's own local coordinates (relative to its rect's
    /// top-left corner — negative or beyond the rect's width/height once
    /// the drag continues past the widget's own bounds) and the widget's
    /// current rect (so e.g. a slider can divide by its own resolved width
    /// without needing to know it ahead of time). Default: `None`.
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        None
    }

    /// Like [`Widget::on_drag`], with the content box resolved by layout.
    fn on_drag_with_content(&self, _content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.on_drag()
    }

    /// Optional handler for the initial pointer press of a drag gesture.
    /// Kept separate from [`Widget::on_drag`] so text editors can record a
    /// selection anchor before subsequent pointer moves extend the focus.
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        None
    }

    /// Like [`Widget::on_drag_start`], with the content box resolved by layout.
    fn on_drag_start_with_content(&self, _content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.on_drag_start()
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        None
    }

    /// Marks an app-selected region whose empty background moves the window.
    /// Unlike a normal drag handler, this never wins over interactive content.
    fn on_window_drag(&self) -> Option<Rc<dyn Fn()>> {
        None
    }

    /// Optional scroll-wheel handler, called with the vertical scroll delta
    /// (in logical pixels; positive scrolls content up, i.e. reveals
    /// content further down) when the pointer is over this widget's rect.
    /// The caller owns the actual scroll offset (see [`Widget::scroll_offset`])
    /// and is responsible for clamping it to whatever range makes sense for
    /// its content. Default: `None`.
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        None
    }

    /// Like [`Widget::on_scroll`], but receives the resolved maximum vertical
    /// offset for this widget's content. Returning this handler lets a
    /// controlled scroll view clamp itself without application-specific
    /// geometry code.
    fn on_scroll_bounded(&self) -> Option<Rc<dyn Fn(f32, f32)>> {
        None
    }

    fn scroll_requires_layout(&self) -> bool {
        false
    }

    /// Optional hook receiving this widget's resolved maximum vertical
    /// scroll offset every time it's painted (whether or not the pointer is
    /// involved) — the same value computed for [`Widget::on_scroll_bounded`],
    /// just published unconditionally rather than only wired into a
    /// deferred wheel-event closure. Lets a scrollbar rendered as a
    /// separate sibling widget (which has no access to this widget's
    /// children's resolved layout) size and position its thumb from a
    /// shared controller instead of duplicating the content-height
    /// calculation. Default: `None`.
    fn on_content_overflow(&self) -> Option<Rc<dyn Fn(f32)>> {
        None
    }

    /// Whether this widget clips its children to its own rect. Default:
    /// `false`. A scroll view returns `true`.
    fn clips_children(&self) -> bool {
        false
    }

    /// Corner radius used when clipping children. Defaults to a rectangular clip.
    fn clip_corner_radius(&self) -> f32 {
        0.0
    }

    /// Shifts every child's painted/hit-tested position by this offset
    /// (subtracted from the child's origin — a positive `y` here scrolls
    /// content up, revealing what's further down). Default: no offset.
    /// Meaningless unless [`Widget::clips_children`] is also `true`.
    fn scroll_offset(&self) -> Point {
        Point::default()
    }

    /// The system mouse cursor to show while the pointer hovers this
    /// widget's rect. Default: `None`, meaning this widget expresses no
    /// preference (an ancestor's or the platform default applies instead).
    fn cursor_icon(&self) -> Option<CursorIcon> {
        None
    }

    /// Called whenever the pointer enters or leaves this widget's visible
    /// rect. This is deliberately separate from clicks so controls can
    /// expose native hover affordances without owning renderer state.
    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        None
    }

    /// Paints an overlay on top of this widget's normal [`Widget::paint`]
    /// output, called only on the frame's currently focused widget (see
    /// [`Widget::focusable`]). Used for a text input's blinking caret.
    /// `caret_visible` is the current blink phase. Default: a no-op.
    fn paint_focused_overlay(&self, _painter: &mut dyn Painter, _rect: Rect, _caret_visible: bool) {
    }

    /// Paints the focused overlay using the content box resolved by layout.
    fn paint_focused_overlay_with_content(
        &self,
        painter: &mut dyn Painter,
        rect: Rect,
        _content: Rect,
        caret_visible: bool,
    ) {
        self.paint_focused_overlay(painter, rect, caret_visible);
    }
}

macro_rules! define_styled_trait_builders {
    ($( $variant:ident($value:ty) => $css_name:literal |$target:ident, $field:ident| $apply:block => $name:ident($( $argument:ident: $argument_type:ty ),*) |$style:ident| $body:block; )*) => {
        $(
            fn $name(mut self, $( $argument: $argument_type ),*) -> Self {
                let mut declaration = self.style();
                let $style = &mut declaration;
                $body
                self.set_style(declaration);
                self
            }
        )*
    };
}

/// Common style builders implemented once for every CreamUI component.
///
/// Custom widgets opt in by implementing only [`Styled::set_style`]. Builders
/// keep the concrete component type, so component-specific methods remain
/// available anywhere in the chain.
pub trait Styled: Widget + Sized {
    /// Replaces the declaration owned by this concrete component.
    fn set_style(&mut self, style: crate::Style);

    fn styled(self) -> Self {
        self
    }

    fn with_style(mut self, style: impl Into<crate::Style>) -> Self {
        self.set_style(style.into());
        self
    }

    fn property(mut self, property: crate::StyleProp) -> Self {
        let mut style = self.style();
        style.apply(property);
        self.set_style(style);
        self
    }

    fn properties(mut self, properties: impl IntoIterator<Item = crate::StyleProp>) -> Self {
        let mut style = self.style();
        for property in properties {
            style.apply(property);
        }
        self.set_style(style);
        self
    }

    fn layout(mut self, value: crate::layout::Style) -> Self {
        let mut style = self.style();
        style.layout = value;
        self.set_style(style);
        self
    }

    fn hover_style(mut self, patch: crate::StateStyle) -> Self {
        let mut style = self.style();
        style.states.hover = patch;
        self.set_style(style);
        self
    }

    fn pressed_style(mut self, patch: crate::StateStyle) -> Self {
        let mut style = self.style();
        style.states.pressed = patch;
        self.set_style(style);
        self
    }

    fn focus_style(mut self, patch: crate::StateStyle) -> Self {
        let mut style = self.style();
        style.states.focus = patch;
        self.set_style(style);
        self
    }

    fn disabled_style(mut self, patch: crate::StateStyle) -> Self {
        let mut style = self.style();
        style.states.disabled = patch;
        self.set_style(style);
        self
    }

    crate::creamui_style_property_schema!(define_styled_trait_builders);
}

pub type BoxedWidget = Box<dyn Widget>;

/// Assigns a stable sibling key to any widget without changing its layout.
/// Use this for controls in lists whose items can be inserted or removed.
pub fn keyed(widget: impl Widget + 'static, key: impl Into<WidgetKey>) -> BoxedWidget {
    Box::new(KeyedWidget {
        widget,
        key: key.into(),
    })
}

struct KeyedWidget<W> {
    widget: W,
    key: WidgetKey,
}

impl<W: Widget> Widget for KeyedWidget<W> {
    fn style(&self) -> crate::Style {
        self.widget.style()
    }
    fn style_state(&self) -> crate::StyleState {
        self.widget.style_state()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.widget.paint(painter, rect)
    }
    fn paint_content(&self, painter: &mut dyn Painter, rect: Rect, content: Rect) {
        self.widget.paint_content(painter, rect, content)
    }
    fn legacy_node_kind(&self) -> Option<crate::runtime::NodeKind> {
        self.widget.legacy_node_kind()
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.widget.children()
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.widget.on_layout(rect)
    }
    fn reports_layout(&self) -> bool {
        self.widget.reports_layout()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.widget.on_click()
    }
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.widget.on_click_at()
    }
    fn measure(&self) -> Option<MeasureFn> {
        self.widget.measure()
    }
    fn measure_fingerprint(&self) -> Option<u64> {
        self.widget.measure_fingerprint()
    }
    fn key(&self) -> Option<WidgetKey> {
        Some(self.key.clone())
    }
    fn focusable(&self) -> bool {
        self.widget.focusable()
    }
    fn is_modal(&self) -> bool {
        self.widget.is_modal()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.widget.on_key()
    }

    fn accepts_text_input(&self) -> bool {
        self.widget.accepts_text_input()
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.widget.on_drag()
    }
    fn on_drag_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.widget.on_drag_with_content(content)
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.widget.on_drag_start()
    }
    fn on_drag_start_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.widget.on_drag_start_with_content(content)
    }
    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        self.widget.on_drag_end()
    }
    fn on_window_drag(&self) -> Option<Rc<dyn Fn()>> {
        self.widget.on_window_drag()
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        self.widget.on_scroll()
    }
    fn on_scroll_bounded(&self) -> Option<Rc<dyn Fn(f32, f32)>> {
        self.widget.on_scroll_bounded()
    }
    fn scroll_requires_layout(&self) -> bool {
        self.widget.scroll_requires_layout()
    }
    fn on_content_overflow(&self) -> Option<Rc<dyn Fn(f32)>> {
        self.widget.on_content_overflow()
    }
    fn clips_children(&self) -> bool {
        self.widget.clips_children()
    }
    fn clip_corner_radius(&self) -> f32 {
        self.widget.clip_corner_radius()
    }
    fn scroll_offset(&self) -> Point {
        self.widget.scroll_offset()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.widget.cursor_icon()
    }
    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        self.widget.on_hover()
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, visible: bool) {
        self.widget.paint_focused_overlay(painter, rect, visible)
    }
    fn paint_focused_overlay_with_content(
        &self,
        painter: &mut dyn Painter,
        rect: Rect,
        content: Rect,
        visible: bool,
    ) {
        self.widget
            .paint_focused_overlay_with_content(painter, rect, content, visible)
    }
}
