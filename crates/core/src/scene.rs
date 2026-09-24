use crate::geometry::{Point, Rect, Size};
use crate::widget::{BoxedWidget, CursorIcon, KeyInput, MeasureFn, Painter, WidgetKey};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, Dimension, TaffyTree};
use taffy::style::Position;

type Tree = TaffyTree<MeasureFn>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaintMode {
    /// Paint ordinary flow nodes, deferring every absolutely positioned node.
    Flow,
    /// Walk the tree looking for deferred absolute layers and paint them last.
    Absolute,
}

struct Instance {
    widget: BoxedWidget,
    /// Compared field-by-field (specifically `style.layout`) against next
    /// frame's declared style to decide whether `node_id`'s `taffy::Style`
    /// needs rewriting. `constrain_inflow` is a pure function, so comparing
    /// its inputs here is equivalent to comparing its outputs, without
    /// needing to also store the constrained form (a full extra
    /// `taffy::style::Style` per node adds up fast down a deep chain).
    style: crate::Style,
    /// Whether the measure context last written to `node_id` was `Some`. A
    /// widget with no [`crate::Widget::measure`] at all (the common case
    /// for containers) has nothing to compare fingerprints against, but
    /// `None -> None` is trivially unchanged, so this alone is enough to
    /// skip rewriting it — no fingerprint required.
    has_measure: bool,
    /// [`crate::Widget::measure_fingerprint`]'s value the last time
    /// `node_id`'s measure context was written.
    measure_fingerprint: Option<u64>,
    /// [`crate::Widget::key`]'s value, cached so keyed reconciliation can
    /// match without re-invoking a partially-consumed widget.
    key: Option<WidgetKey>,
    children: Vec<Instance>,
    node_id: taffy::NodeId,
    /// The largest border/outline overflow across every interaction state,
    /// cached here because clipping ancestors need it on every paint.
    paint_overflow: f32,
}

fn remove_instance(tree: &mut Tree, instance: Instance) {
    for child in instance.children {
        remove_instance(tree, child);
    }
    let _ = tree.remove(instance.node_id);
}

/// Reconciles `widget` against a previous frame's `Instance` at the same
/// tree position, if any.
///
/// Widgets carry no persistent identity of their own (all long-lived state
/// lives in `Signal`s, not in widget structs — see `creamui_reactive`), so
/// this reconciles purely structurally: a widget is considered "the same
/// node" as whatever widget previously occupied the same position among its
/// parent's children, unless it or a sibling opts into [`WidgetKey`]
/// matching (see [`reconcile_children`]). That's enough to avoid recreating
/// `taffy` nodes (and their subtrees) on every reactive re-render for the
/// common case where a re-render only changes leaf styles/content, not the
/// tree shape — and, for a matched node, every `taffy` write is skipped
/// unless the value being written actually changed.
fn reconcile(tree: &mut Tree, existing: Option<Instance>, mut widget: BoxedWidget) -> Instance {
    #[cfg(feature = "perf-metrics")]
    crate::metrics::record(|m| {
        m.reconcile_visits += 1;
        m.widget_objects_built += 1;
    });

    let new_child_widgets = widget.children();
    let new_style = widget.style();
    let new_measure = widget.measure();
    let new_has_measure = new_measure.is_some();
    let new_measure_fingerprint = widget.measure_fingerprint();
    let new_key = widget.key();

    let Some(mut old) = existing else {
        // No previous node at this position: build a fresh subtree.
        let mut child_ids = Vec::with_capacity(new_child_widgets.len());
        let mut children = Vec::with_capacity(new_child_widgets.len());
        for child_widget in new_child_widgets {
            let child = reconcile(tree, None, child_widget);
            child_ids.push(child.node_id);
            children.push(child);
        }
        let node_id = tree
            .new_with_children(constrain_inflow(new_style.layout.clone()), &child_ids)
            .expect("taffy node creation is infallible for well-formed styles");
        tree.set_node_context(node_id, new_measure)
            .expect("setting the context of a freshly created node should not fail");
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| {
            m.taffy_style_writes += 1;
            m.taffy_children_writes += 1;
            m.taffy_context_writes += 1;
        });
        return Instance {
            widget,
            paint_overflow: max_paint_overflow(&new_style),
            style: new_style,
            has_measure: new_has_measure,
            measure_fingerprint: new_measure_fingerprint,
            key: new_key,
            children,
            node_id,
        };
    };

    let paint_overflow =
        if old.style.paint == new_style.paint && old.style.states == new_style.states {
            old.paint_overflow
        } else {
            max_paint_overflow(&new_style)
        };
    if old.style.layout != new_style.layout {
        tree.set_style(old.node_id, constrain_inflow(new_style.layout.clone()))
            .expect("updating the style of an existing node should not fail");
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.taffy_style_writes += 1);
    }

    let context_unchanged = if new_has_measure {
        old.has_measure
            && new_measure_fingerprint.is_some()
            && new_measure_fingerprint == old.measure_fingerprint
    } else {
        !old.has_measure
    };
    if !context_unchanged {
        tree.set_node_context(old.node_id, new_measure)
            .expect("updating the context of an existing node should not fail");
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.taffy_context_writes += 1);
    }

    let old_child_ids: Vec<taffy::NodeId> = old.children.iter().map(|c| c.node_id).collect();
    let new_children = reconcile_children(tree, &mut old.children, new_child_widgets);

    let child_ids: Vec<_> = new_children.iter().map(|c| c.node_id).collect();
    if child_ids != old_child_ids {
        tree.set_children(old.node_id, &child_ids)
            .expect("setting children of an existing node should not fail");
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.taffy_children_writes += 1);
    }

    Instance {
        widget,
        style: new_style,
        has_measure: new_has_measure,
        measure_fingerprint: new_measure_fingerprint,
        key: new_key,
        children: new_children,
        node_id: old.node_id,
        paint_overflow,
    }
}

/// Matches `new_child_widgets` against `old_children` (draining it) and
/// reconciles each pair. Purely positional, unless at least one new child
/// carries a [`WidgetKey`] — then every child in this list is matched by
/// key where present, falling back to positional matching, in original
/// relative order, for the rest. Unmatched leftovers are removed.
fn reconcile_children(
    tree: &mut Tree,
    old_children: &mut Vec<Instance>,
    new_child_widgets: Vec<BoxedWidget>,
) -> Vec<Instance> {
    let mut new_children = Vec::with_capacity(new_child_widgets.len());

    if new_child_widgets.iter().any(|w| w.key().is_some()) {
        let mut by_key: HashMap<WidgetKey, Instance> = HashMap::new();
        let mut unkeyed: VecDeque<Instance> = VecDeque::new();
        for child in old_children.drain(..) {
            match &child.key {
                Some(key) => {
                    by_key.insert(key.clone(), child);
                }
                None => unkeyed.push_back(child),
            }
        }
        for child_widget in new_child_widgets {
            let matched = match child_widget.key() {
                Some(key) => by_key.remove(&key),
                None => unkeyed.pop_front(),
            };
            new_children.push(reconcile(tree, matched, child_widget));
        }
        for leftover in by_key.into_values() {
            remove_instance(tree, leftover);
        }
        for leftover in unkeyed {
            remove_instance(tree, leftover);
        }
    } else {
        let mut old = old_children.drain(..);
        for child_widget in new_child_widgets {
            new_children.push(reconcile(tree, old.next(), child_widget));
        }
        for leftover in old {
            remove_instance(tree, leftover);
        }
    }

    new_children
}

fn viewport_rect(viewport: Size) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: viewport.width.max(0.0),
        height: viewport.height.max(0.0),
    }
}

fn constrain_inflow(mut style: taffy::style::Style) -> taffy::style::Style {
    if style.min_size.width == Dimension::Auto {
        style.min_size.width = Dimension::Length(0.0);
    }
    if style.min_size.height == Dimension::Auto {
        style.min_size.height = Dimension::Length(0.0);
    }
    if style.position != Position::Absolute && style.flex_shrink > 0.0 {
        if style.max_size.width == Dimension::Auto {
            style.max_size.width = Dimension::Percent(1.0);
        }
        if style.max_size.height == Dimension::Auto {
            style.max_size.height = Dimension::Percent(1.0);
        }
    }
    style
}

#[derive(Default)]
struct PaintOutputs {
    hits: Vec<(Rect, Rc<dyn Fn()>)>,
    hits_at: Vec<(Rect, Rc<dyn Fn(Point)>)>,
    /// Every focusable widget in tab order, with its visible rect if any
    /// part of it is on screen, so indices stay stable while scrolling.
    focusables: Vec<(Option<Rect>, Rc<dyn Fn(KeyInput)>)>,
    /// `(visible_rect, full_rect, handler)` — hit-testing uses the
    /// clip-visible portion, but the handler is called with the widget's
    /// full (unclipped) rect so e.g. a slider can divide by its own real
    /// width regardless of how much of it a scroll ancestor currently shows.
    draggables: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>, Option<Rc<dyn Fn()>>)>,
    drag_starts: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>)>,
    scrollables: Vec<(Rect, Rc<dyn Fn(f32)>, bool)>,
    cursors: Vec<(Rect, CursorIcon)>,
    hovers: Vec<(Rect, Rc<dyn Fn(bool)>)>,
}

/// Context threaded through [`paint_instance`] to identify and paint the
/// frame's currently focused widget (see [`Renderer::render_focused`]).
/// `focused_index` refers to the same ordinal space as [`Scene::focusables`]
/// (assigned in paint order); `counter` tracks that ordinal as it walks the
/// tree so it can tell when it's standing on the focused widget itself.
struct FocusContext {
    focused_index: Option<usize>,
    caret_visible: bool,
    counter: usize,
}

#[cfg(test)]
thread_local! {
    /// Counts `paint_instance` calls, reset per-test — the only way to
    /// observe from outside that a subtree was skipped entirely rather
    /// than walked-but-not-painted (see the `_prunes_` test below).
    static PAINT_INSTANCE_VISITS: std::cell::Cell<usize> = std::cell::Cell::new(0);
}

/// How far a border or outline paints outside its own rect.
fn paint_overflow(paint: &crate::PaintStyle) -> f32 {
    let decoration = paint
        .border
        .map(|b| b.width / 2.0)
        .unwrap_or(0.0)
        .max(paint.outline.map(|o| o.width * 1.5).unwrap_or(0.0));
    let shadow = paint
        .box_shadow
        .map(|shadow| {
            shadow.blur + shadow.spread.max(0.0) + shadow.offset_x.abs().max(shadow.offset_y.abs())
        })
        .unwrap_or(0.0);
    decoration.max(shadow)
}

/// The largest [`paint_overflow`] across any single interaction state —
/// checking each flag alone covers every combination too, since `resolve`'s
/// cascade only ever picks one state's patch per field.
fn max_paint_overflow(style: &crate::Style) -> f32 {
    [
        crate::StyleState::NORMAL,
        crate::StyleState::NORMAL.with_hovered(true),
        crate::StyleState::NORMAL.with_pressed(true),
        crate::StyleState::NORMAL.with_focused(true),
        crate::StyleState::NORMAL.with_disabled(true),
    ]
    .iter()
    .map(|&state| paint_overflow(&style.resolve(state).paint))
    .fold(0.0f32, f32::max)
}

#[allow(clippy::too_many_arguments)]
fn paint_instance(
    tree: &Tree,
    instance: &Instance,
    painter: &mut dyn Painter,
    parent_origin: Point,
    clip: Rect,
    viewport: Rect,
    focus: &mut FocusContext,
    out: &mut PaintOutputs,
    mode: PaintMode,
) {
    #[cfg(test)]
    PAINT_INSTANCE_VISITS.with(|c| c.set(c.get() + 1));
    #[cfg(feature = "perf-metrics")]
    crate::metrics::record(|m| m.paint_nodes_visited += 1);

    let layout = tree
        .layout(instance.node_id)
        .expect("layout was computed for every instantiated node");
    let rect = Rect {
        x: parent_origin.x + layout.location.x,
        y: parent_origin.y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };

    let absolute = instance.style.layout.position == Position::Absolute;
    if mode == PaintMode::Flow && absolute {
        return;
    }
    let effective_clip = if mode == PaintMode::Absolute && absolute {
        viewport
    } else {
        clip
    };

    let paint_self = mode == PaintMode::Flow || absolute;
    if paint_self
        && rect
            .inflate(instance.paint_overflow)
            .overlaps(effective_clip)
    {
        let focusable = instance.widget.focusable() && instance.widget.on_key().is_some();
        let states = instance
            .widget
            .style_state()
            .with_hovered(painter.hovered(rect))
            .with_pressed(painter.pressed(rect))
            .with_focused(focusable && focus.focused_index == Some(focus.counter));
        let colors = painter.color_scheme();
        let resolved = instance.style.resolve(states);
        let radius = resolved.paint.corner_radius.unwrap_or(0.0);
        if let Some(shadow) = resolved.paint.box_shadow {
            painter.draw_box_shadow(
                rect,
                shadow.color.resolve(&colors),
                shadow.offset_x,
                shadow.offset_y,
                shadow.blur,
                shadow.spread,
                radius,
            );
        }
        if let Some(background) = resolved.paint.background {
            match background {
                crate::Background::Solid(color) => {
                    painter.fill_rect(rect, color.resolve(&colors), radius)
                }
                crate::Background::LinearGradient(gradient) => painter.fill_linear_gradient(
                    rect,
                    gradient.start.resolve(&colors),
                    gradient.end.resolve(&colors),
                    gradient.angle_degrees,
                    radius,
                ),
            }
        }
        instance.widget.paint(painter, rect);
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.paint_nodes_recorded += 1);
        // Borders and outlines sit over component-specific content, matching
        // CSS box painting and preventing edge-to-edge content from hiding
        // the common decoration.
        if let Some(border) = resolved.paint.border {
            painter.stroke_rect(rect, border.color.resolve(&colors), border.width, radius);
        }
        if let Some(outline) = resolved.paint.outline {
            painter.stroke_rect(
                rect.inflate(outline.width),
                outline.color.resolve(&colors),
                outline.width,
                radius + outline.width,
            );
        }
    }

    if paint_self && instance.widget.focusable() {
        if let Some(on_key) = instance.widget.on_key() {
            let visible = rect.intersect(effective_clip);
            if visible.is_some() && focus.focused_index == Some(focus.counter) {
                instance
                    .widget
                    .paint_focused_overlay(painter, rect, focus.caret_visible);
            }
            focus.counter += 1;
            out.focusables.push((visible, on_key));
        }
    }

    if paint_self {
        if let Some(visible) = rect.intersect(effective_clip) {
            #[cfg(feature = "perf-metrics")]
            crate::metrics::record(|m| m.hit_nodes_updated += 1);
            if let Some(handler) = instance.widget.on_click() {
                out.hits.push((visible, handler));
            }
            if let Some(handler) = instance.widget.on_click_at() {
                out.hits_at.push((visible, handler));
            }
            if let Some(on_drag) = instance.widget.on_drag() {
                out.draggables
                    .push((visible, rect, on_drag, instance.widget.on_drag_end()));
            }
            if let Some(on_drag_start) = instance.widget.on_drag_start() {
                out.drag_starts.push((visible, rect, on_drag_start));
            }
            let on_scroll_bounded = instance.widget.on_scroll_bounded();
            let on_content_overflow = instance.widget.on_content_overflow();
            if on_scroll_bounded.is_some() || on_content_overflow.is_some() {
                let content_bottom = instance
                    .children
                    .iter()
                    .filter_map(|child| tree.layout(child.node_id).ok())
                    .map(|layout| layout.location.y + layout.size.height)
                    .fold(0.0_f32, f32::max);
                let max_offset = (content_bottom - rect.height).max(0.0);
                if let Some(report) = on_content_overflow {
                    report(max_offset);
                }
                if let Some(on_scroll) = on_scroll_bounded {
                    if max_offset > 0.5 {
                        out.scrollables.push((
                            visible,
                            Rc::new(move |delta| on_scroll(delta, max_offset)),
                            true,
                        ));
                    }
                }
            } else if let Some(on_scroll) = instance.widget.on_scroll() {
                out.scrollables.push((visible, on_scroll, false));
            }
            if let Some(cursor) = instance.widget.cursor_icon() {
                out.cursors.push((visible, cursor));
            }
            if let Some(handler) = instance.widget.on_hover() {
                out.hovers.push((visible, handler));
            }
        }
    }

    let offset = instance.widget.scroll_offset();
    let child_origin = Point {
        x: rect.x - offset.x,
        y: rect.y - offset.y,
    };

    // Portal layers escape ancestor clips.
    let clips = instance.widget.clips_children() && mode != PaintMode::Absolute;
    // Give a child's own border/outline overflow (e.g. a focus ring)
    // headroom so this container's own tight-fit edge doesn't clip it.
    let margin = instance
        .children
        .iter()
        .map(|child| child.paint_overflow)
        .fold(0.0f32, f32::max);
    let clip_rect = rect.inflate(margin);
    let child_clip = if clips {
        match clip_rect.intersect(effective_clip) {
            Some(c) => c,
            None => return,
        }
    } else {
        effective_clip
    };

    let scrolls = clips
        && (instance.widget.on_scroll().is_some() || instance.widget.on_scroll_bounded().is_some());
    // Painters intersect with the clip already in effect themselves; the
    // container's own rect keeps the pushed clip independent of how far an
    // enclosing scroll layer is scrolled.
    if scrolls {
        painter.push_scroll_layer(clip_rect, instance.widget.clip_corner_radius(), offset);
    } else if clips {
        painter.push_clip_rounded(clip_rect, instance.widget.clip_corner_radius());
    }
    let child_mode = if mode == PaintMode::Absolute && absolute {
        PaintMode::Flow
    } else {
        mode
    };
    for child in &instance.children {
        paint_instance(
            tree,
            child,
            painter,
            child_origin,
            child_clip,
            viewport,
            focus,
            out,
            child_mode,
        );
    }
    if scrolls {
        painter.pop_scroll_layer();
    } else if clips {
        painter.pop_clip();
    }
}

/// The result of rendering one frame: nothing but interactive hit-regions
/// (click, focus/keyboard, drag, scroll), since painting has already
/// happened by the time this is returned.
///
/// Regions are ordered parent-before-child, so hit-testing walks them in
/// reverse to prefer the most specific (topmost) match, and are already
/// clipped to whatever a scrollable ancestor actually shows. Indices into
/// [`Scene::focusables`]/[`Scene::draggables`]/[`Scene`]'s scrollables are
/// only stable across renders while the widget tree's shape doesn't change
/// — see `scene::reconcile`'s docs on structural (not keyed) reconciliation.
pub struct Scene {
    hits: Vec<(Rect, Rc<dyn Fn()>)>,
    hits_at: Vec<(Rect, Rc<dyn Fn(Point)>)>,
    /// Every focusable widget in tab order, with its visible rect if any
    /// part of it is on screen, so indices stay stable while scrolling.
    focusables: Vec<(Option<Rect>, Rc<dyn Fn(KeyInput)>)>,
    draggables: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>, Option<Rc<dyn Fn()>>)>,
    drag_starts: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>)>,
    scrollables: Vec<(Rect, Rc<dyn Fn(f32)>, bool)>,
    cursors: Vec<(Rect, CursorIcon)>,
    hovers: Vec<(Rect, Rc<dyn Fn(bool)>)>,
}

impl Scene {
    /// Cycle through visible keyboard controls in layout order.
    pub fn next_focus(&self, current: Option<usize>, backwards: bool) -> Option<usize> {
        let count = self.focusables.len();
        if count == 0 {
            return None;
        }
        Some(match current.filter(|i| *i < count) {
            Some(i) if backwards => (i + count - 1) % count,
            Some(i) => (i + 1) % count,
            None if backwards => count - 1,
            None => 0,
        })
    }
    /// Returns the click handler for the topmost widget containing `point`, if any.
    pub fn hit_test(&self, point: Point) -> Option<&Rc<dyn Fn()>> {
        self.hits
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, handler)| handler)
    }

    /// Returns the click handler with the pointer position for the topmost
    /// widget containing `point`, if any.
    pub fn hit_test_at(&self, point: Point) -> Option<&Rc<dyn Fn(Point)>> {
        self.hits_at
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, handler)| handler)
    }

    /// Returns the index (into this scene's focusables) of the topmost
    /// focusable widget containing `point`, if any.
    pub fn focus_hit_test(&self, point: Point) -> Option<usize> {
        self.focusables
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (rect, _))| rect.is_some_and(|rect| rect.contains(point)))
            .map(|(index, _)| index)
    }

    /// The keyboard handler at `index`, if it still exists this render.
    pub fn on_key_at(&self, index: usize) -> Option<&Rc<dyn Fn(KeyInput)>> {
        self.focusables.get(index).map(|(_, handler)| handler)
    }

    /// Returns the index (into this scene's draggables) of the topmost
    /// draggable widget containing `point`, if any.
    pub fn drag_hit_test(&self, point: Point) -> Option<usize> {
        self.draggables
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (visible, _, _, _))| visible.contains(point))
            .map(|(index, _)| index)
    }

    /// The drag handler and full (unclipped) rect at `index`, if it still exists this render.
    pub fn draggable_at(&self, index: usize) -> Option<(Rect, &Rc<dyn Fn(Point, Rect)>)> {
        self.draggables
            .get(index)
            .map(|(_, full, handler, _)| (*full, handler))
    }

    pub fn drag_end_at(&self, index: usize) -> Option<Rc<dyn Fn()>> {
        self.draggables
            .get(index)
            .and_then(|(_, _, _, handler)| handler.clone())
    }

    pub fn drag_start_at(&self, point: Point) -> Option<(Rect, Rc<dyn Fn(Point, Rect)>)> {
        self.drag_starts
            .iter()
            .rev()
            .find(|(visible, _, _)| visible.contains(point))
            .map(|(_, rect, handler)| (*rect, handler.clone()))
    }

    /// Returns the index (into this scene's scrollables) of the topmost
    /// scrollable widget containing `point`, if any.
    pub fn scroll_hit_test(&self, point: Point) -> Option<usize> {
        self.scrollables
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (rect, _, _))| rect.contains(point))
            .map(|(index, _)| index)
    }

    /// The scroll-wheel handler at `index`, if it still exists this render.
    pub fn on_scroll_at(&self, index: usize) -> Option<&Rc<dyn Fn(f32)>> {
        self.scrollables.get(index).map(|(_, handler, _)| handler)
    }

    pub fn scroll_is_local_at(&self, index: usize) -> bool {
        self.scrollables
            .get(index)
            .is_some_and(|(_, _, local)| *local)
    }

    /// The visible rect at `index`, if it still exists this render.
    pub fn scroll_rect_at(&self, index: usize) -> Option<Rect> {
        self.scrollables.get(index).map(|(rect, _, _)| *rect)
    }

    /// Returns the cursor icon of the topmost widget with a cursor
    /// preference containing `point`, if any.
    pub fn cursor_hit_test(&self, point: Point) -> Option<CursorIcon> {
        self.cursors
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, icon)| *icon)
    }

    /// The hover callback belonging to the topmost hovered widget.
    pub fn hover_hit_test(&self, point: Point) -> Option<(Rect, Rc<dyn Fn(bool)>)> {
        self.hovers
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(point))
            .map(|(rect, handler)| (*rect, handler.clone()))
    }
}

/// Owns a persistent `taffy` layout tree across frames, reconciling each new
/// widget tree against the previous one instead of rebuilding from scratch.
///
/// This is CreamUI's render loop entry point: construct one `Renderer` per
/// window/surface and call [`Renderer::render`] once per reactive re-render.
pub struct Renderer {
    tree: Tree,
    root: Option<Instance>,
    viewport: Size,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            tree: TaffyTree::new(),
            root: None,
            viewport: Size::default(),
        }
    }

    /// Reconciles `root` against the previously rendered tree (if any),
    /// computes layout for `viewport`, paints via `painter`, and returns the
    /// resulting click hit-regions. Equivalent to [`Renderer::render_focused`]
    /// with no widget focused.
    pub fn render(
        &mut self,
        root: BoxedWidget,
        viewport: Size,
        painter: &mut dyn Painter,
    ) -> Scene {
        self.render_focused(root, viewport, painter, None, true)
    }

    /// Like [`Renderer::render`], but also paints a focus overlay (e.g. a
    /// text input's caret) on the widget at `focused_index` into the
    /// previous frame's [`Scene::focusables`] ordinal space, if any — see
    /// [`Widget::paint_focused_overlay`](crate::Widget::paint_focused_overlay).
    /// `caret_visible` is that overlay's current blink phase.
    pub fn render_focused(
        &mut self,
        root: BoxedWidget,
        viewport: Size,
        painter: &mut dyn Painter,
        focused_index: Option<usize>,
        caret_visible: bool,
    ) -> Scene {
        self.update(root, viewport);
        self.paint(painter, focused_index, caret_visible)
            .expect("update always leaves a root to paint")
    }

    /// Reconciles `root` against the retained tree and recomputes layout,
    /// without painting.
    pub fn update(&mut self, root: BoxedWidget, viewport: Size) {
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.root_builds += 1);

        let previous = self.root.take();
        let instance = {
            #[cfg(feature = "perf-metrics")]
            let _span = tracing::info_span!("reconcile").entered();
            reconcile(&mut self.tree, previous, root)
        };

        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.layout_runs += 1);
        #[cfg(feature = "perf-metrics")]
        let _layout_span = tracing::info_span!("taffy_compute_layout").entered();
        self.tree
            .compute_layout_with_measure(
                instance.node_id,
                taffy::geometry::Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
                |known_dimensions, available_space, _node_id, measure, _style| match measure {
                    Some(measure) => {
                        #[cfg(feature = "perf-metrics")]
                        crate::metrics::record(|m| m.measure_calls += 1);
                        measure(known_dimensions, available_space)
                    }
                    None => taffy::geometry::Size::ZERO,
                },
            )
            .expect("layout computation should not fail for a well-formed tree");
        self.viewport = viewport;
        self.root = Some(instance);
    }

    /// Paints the retained tree without rebuilding widgets or recomputing
    /// layout, returning its hit regions. `None` before the first
    /// [`Renderer::update`].
    pub fn paint(
        &self,
        painter: &mut dyn Painter,
        focused_index: Option<usize>,
        caret_visible: bool,
    ) -> Option<Scene> {
        let instance = self.root.as_ref()?;
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("paint_traversal").entered();
        let clip = viewport_rect(self.viewport);
        painter.push_clip(clip);
        let mut out = PaintOutputs::default();
        let mut focus = FocusContext {
            focused_index,
            caret_visible,
            counter: 0,
        };
        for mode in [PaintMode::Flow, PaintMode::Absolute] {
            paint_instance(
                &self.tree,
                instance,
                painter,
                Point::default(),
                clip,
                clip,
                &mut focus,
                &mut out,
                mode,
            );
        }
        painter.pop_clip();
        Some(Scene {
            hits: out.hits,
            hits_at: out.hits_at,
            focusables: out.focusables,
            draggables: out.draggables,
            drag_starts: out.drag_starts,
            scrollables: out.scrollables,
            cursors: out.cursors,
            hovers: out.hovers,
        })
    }

    /// The viewport passed to the last [`Renderer::update`].
    pub fn viewport(&self) -> Size {
        self.viewport
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer::new()
    }
}

/// Renders a widget tree from scratch with no retained state across calls.
///
/// Prefer [`Renderer`] for anything rendered more than once (it reconciles
/// against the previous frame instead of rebuilding every `taffy` node);
/// this is a convenience for one-shot rendering (e.g. tests, or a single
/// static frame).
pub fn render_frame(root: BoxedWidget, viewport: Size, painter: &mut dyn Painter) -> Scene {
    Renderer::new().render(root, viewport, painter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::TextAlign;
    use creamui_theme::Color;

    struct NoopPainter;
    impl Painter for NoopPainter {
        fn fill_rect(&mut self, _rect: Rect, _color: Color, _corner_radius: f32) {}
        fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32, _corner_radius: f32) {}
        fn fill_text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _color: Color,
            _font_size: f32,
            _align: TextAlign,
        ) {
        }
    }

    struct Branch {
        child_count: usize,
    }
    impl crate::widget::Widget for Branch {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            (0..self.child_count)
                .map(|_| Box::new(Branch { child_count: 0 }) as BoxedWidget)
                .collect()
        }
        fn measure_fingerprint(&self) -> Option<u64> {
            // `Branch` never has a `measure()`, so its (empty) measure
            // context is always the same — a fixed fingerprint lets tests
            // prove the context write itself gets skipped when unchanged.
            Some(0)
        }
    }

    const VIEWPORT: Size = Size {
        width: 100.0,
        height: 100.0,
    };

    #[test]
    fn reconcile_reuses_taffy_nodes_when_tree_shape_is_unchanged() {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;

        renderer.render(Box::new(Branch { child_count: 2 }), VIEWPORT, &mut painter);
        let root1 = &renderer.root.as_ref().unwrap();
        let root_id_1 = root1.node_id;
        let child_ids_1: Vec<_> = root1.children.iter().map(|c| c.node_id).collect();

        renderer.render(Box::new(Branch { child_count: 2 }), VIEWPORT, &mut painter);
        let root2 = &renderer.root.as_ref().unwrap();
        let root_id_2 = root2.node_id;
        let child_ids_2: Vec<_> = root2.children.iter().map(|c| c.node_id).collect();

        assert_eq!(
            root_id_1, root_id_2,
            "root node identity should be preserved across renders"
        );
        assert_eq!(
            child_ids_1, child_ids_2,
            "child node identities should be preserved across renders"
        );
    }

    #[test]
    fn reconcile_adjusts_taffy_children_when_child_count_shrinks() {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;

        renderer.render(Box::new(Branch { child_count: 3 }), VIEWPORT, &mut painter);
        assert_eq!(renderer.root.as_ref().unwrap().children.len(), 3);

        renderer.render(Box::new(Branch { child_count: 1 }), VIEWPORT, &mut painter);
        let root = renderer.root.as_ref().unwrap();
        assert_eq!(root.children.len(), 1);
        assert_eq!(
            renderer.tree.children(root.node_id).unwrap().len(),
            1,
            "the underlying taffy tree's child list should also shrink, not just our Instance tree"
        );
    }

    #[test]
    fn reconcile_grows_taffy_children_when_child_count_increases() {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;

        renderer.render(Box::new(Branch { child_count: 1 }), VIEWPORT, &mut painter);
        renderer.render(Box::new(Branch { child_count: 4 }), VIEWPORT, &mut painter);

        let root = renderer.root.as_ref().unwrap();
        assert_eq!(root.children.len(), 4);
        assert_eq!(renderer.tree.children(root.node_id).unwrap().len(), 4);
    }

    #[cfg(feature = "perf-metrics")]
    #[test]
    fn unchanged_rerender_performs_zero_taffy_writes() {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;

        renderer.render(Box::new(Branch { child_count: 4 }), VIEWPORT, &mut painter);

        crate::metrics::reset_frame_metrics();
        renderer.render(Box::new(Branch { child_count: 4 }), VIEWPORT, &mut painter);
        let metrics = crate::metrics::frame_metrics();

        assert_eq!(metrics.taffy_style_writes, 0);
        assert_eq!(metrics.taffy_context_writes, 0);
        assert_eq!(metrics.taffy_children_writes, 0);
    }

    struct KeyedLeaf {
        id: u64,
    }
    impl crate::widget::Widget for KeyedLeaf {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn key(&self) -> Option<crate::widget::WidgetKey> {
            Some(crate::widget::WidgetKey::U64(self.id))
        }
    }

    #[test]
    fn keyed_children_preserve_taffy_node_identity_across_a_reorder() {
        let build = |order: [u64; 3]| -> BoxedWidget {
            Box::new(Root {
                children: order
                    .into_iter()
                    .map(|id| Box::new(KeyedLeaf { id }) as BoxedWidget)
                    .collect(),
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;

        renderer.render(build([1, 2, 3]), VIEWPORT, &mut painter);
        let ids_before: std::collections::HashMap<u64, taffy::NodeId> = renderer
            .root
            .as_ref()
            .unwrap()
            .children
            .iter()
            .zip([1, 2, 3])
            .map(|(instance, key)| (key, instance.node_id))
            .collect();

        renderer.render(build([3, 1, 2]), VIEWPORT, &mut painter);
        let root = renderer.root.as_ref().unwrap();
        let reordered_keys = [3u64, 1, 2];
        for (instance, key) in root.children.iter().zip(reordered_keys) {
            assert_eq!(
                instance.node_id, ids_before[&key],
                "key {key} must keep its taffy node identity across the reorder"
            );
        }
    }

    #[derive(Default)]
    struct ClipRecorder {
        last_push_clip_rect: Option<Rect>,
        filled: Vec<Rect>,
    }
    impl Painter for ClipRecorder {
        fn fill_rect(&mut self, rect: Rect, _color: Color, _corner_radius: f32) {
            self.filled.push(rect);
        }
        fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32, _corner_radius: f32) {}
        fn fill_text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _color: Color,
            _font_size: f32,
            _align: TextAlign,
        ) {
        }
        fn push_clip_rounded(&mut self, rect: Rect, _corner_radius: f32) {
            self.last_push_clip_rect = Some(rect);
        }
    }

    struct CountingWidget {
        count: Rc<std::cell::Cell<usize>>,
    }
    impl crate::widget::Widget for CountingWidget {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {
            self.count.set(self.count.get() + 1);
        }
    }

    struct Root {
        children: Vec<BoxedWidget>,
    }
    impl crate::widget::Widget for Root {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            std::mem::take(&mut self.children)
        }
    }

    struct BorderedWidget {
        border_width: f32,
    }
    impl crate::widget::Widget for BorderedWidget {
        fn style(&self) -> crate::Style {
            crate::Style {
                layout: taffy::style::Style {
                    size: taffy::geometry::Size {
                        width: Dimension::Length(10.0),
                        height: Dimension::Length(10.0),
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
            .border(Color::rgb(0, 0, 0), self.border_width)
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
    }

    struct ClippingRoot {
        child: Option<BoxedWidget>,
    }
    impl crate::widget::Widget for ClippingRoot {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            self.child.take().into_iter().collect()
        }
        fn clips_children(&self) -> bool {
            true
        }
    }

    #[test]
    fn a_clipping_containers_child_clip_covers_the_childs_border_overflow() {
        let mut renderer = Renderer::new();
        let mut painter = ClipRecorder::default();
        renderer.render(
            Box::new(Root {
                children: vec![Box::new(ClippingRoot {
                    child: Some(Box::new(BorderedWidget { border_width: 8.0 })),
                })],
            }),
            VIEWPORT,
            &mut painter,
        );

        // The container shrinks to fit its 10x10 child exactly, so its own
        // edge sits flush against the child's — a naive clip there would cut
        // off the border's overflow past that edge.
        let clip = painter
            .last_push_clip_rect
            .expect("a clipping container pushes a clip");
        assert!(
            clip.width > 10.0 && clip.height > 10.0,
            "child clip {clip:?} must have headroom for the child's border overflow"
        );
    }

    struct ScrollWrapper {
        offset: f32,
        children: Vec<BoxedWidget>,
    }
    impl crate::widget::Widget for ScrollWrapper {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: Dimension::Length(20.0),
                    height: Dimension::Length(20.0),
                },
                flex_direction: taffy::style::FlexDirection::Column,
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            std::mem::take(&mut self.children)
        }
        fn clips_children(&self) -> bool {
            true
        }
        fn scroll_offset(&self) -> Point {
            Point {
                x: 0.0,
                y: self.offset,
            }
        }
    }

    #[test]
    fn nodes_scrolled_outside_their_clip_are_not_painted() {
        let visible = Rc::new(std::cell::Cell::new(0usize));
        let hidden = Rc::new(std::cell::Cell::new(0usize));
        let row = |count: &Rc<std::cell::Cell<usize>>| -> BoxedWidget {
            Box::new(SizedRow {
                count: count.clone(),
            })
        };
        let mut renderer = Renderer::new();
        renderer.render(
            Box::new(ScrollWrapper {
                offset: 0.0,
                children: vec![row(&visible), row(&hidden), row(&hidden), row(&hidden)],
            }),
            VIEWPORT,
            &mut NoopPainter,
        );
        assert_eq!(visible.get(), 1);
        assert_eq!(
            hidden.get(),
            1,
            "only the row touching the clip edge paints"
        );
    }

    struct SizedRow {
        count: Rc<std::cell::Cell<usize>>,
    }
    impl crate::widget::Widget for SizedRow {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: Dimension::Length(20.0),
                    height: Dimension::Length(20.0),
                },
                flex_shrink: 0.0,
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {
            self.count.set(self.count.get() + 1);
        }
    }

    #[test]
    fn paint_without_update_reuses_layout() {
        let count = Rc::new(std::cell::Cell::new(0usize));
        let mut renderer = Renderer::new();
        assert!(renderer.paint(&mut NoopPainter, None, false).is_none());
        renderer.update(
            Box::new(Root {
                children: vec![Box::new(CountingWidget {
                    count: count.clone(),
                })],
            }),
            VIEWPORT,
        );
        assert_eq!(count.get(), 0);
        for _ in 0..3 {
            renderer.paint(&mut NoopPainter, None, false).unwrap();
        }
        assert_eq!(count.get(), 3);
        PAINT_INSTANCE_VISITS.with(|c| c.set(0));
        renderer.paint(&mut NoopPainter, None, false).unwrap();
        assert_eq!(PAINT_INSTANCE_VISITS.with(|c| c.get()), 4);
    }

    struct AbsoluteWrapper {
        child: Option<BoxedWidget>,
    }
    impl crate::widget::Widget for AbsoluteWrapper {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                position: taffy::style::Position::Absolute,
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            self.child.take().into_iter().collect()
        }
    }

    struct Filled {
        size: f32,
    }
    impl crate::widget::Widget for Filled {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: Dimension::Length(self.size),
                    height: Dimension::Length(self.size),
                },
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.fill_rect(rect, Color::rgb(0, 0, 0), 0.0);
        }
    }

    #[test]
    fn absolute_subtrees_paint_after_flow_siblings() {
        let mut painter = ClipRecorder::default();
        let colored = |size: f32| -> BoxedWidget { Box::new(Filled { size }) };
        render_frame(
            Box::new(Root {
                children: vec![
                    Box::new(AbsoluteWrapper {
                        child: Some(colored(5.0)),
                    }),
                    colored(7.0),
                ],
            }),
            VIEWPORT,
            &mut painter,
        );
        let widths: Vec<f32> = painter.filled.iter().map(|r| r.width).collect();
        assert_eq!(widths, vec![7.0, 5.0]);
    }

    struct SizedClick {
        width: f32,
        height: f32,
        flex_shrink: f32,
        clicked: Rc<std::cell::Cell<bool>>,
    }
    impl crate::widget::Widget for SizedClick {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: Dimension::Length(self.width),
                    height: Dimension::Length(self.height),
                },
                flex_shrink: self.flex_shrink,
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn on_click(&self) -> Option<Rc<dyn Fn()>> {
            let clicked = self.clicked.clone();
            Some(Rc::new(move || clicked.set(true)))
        }
    }

    struct ColumnRoot {
        height: f32,
        children: Vec<BoxedWidget>,
    }
    impl crate::widget::Widget for ColumnRoot {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                display: taffy::style::Display::Flex,
                flex_direction: taffy::style::FlexDirection::Column,
                size: taffy::geometry::Size {
                    width: Dimension::Length(100.0),
                    height: Dimension::Length(self.height),
                },
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            std::mem::take(&mut self.children)
        }
    }

    struct FocusRow {
        keys: Rc<std::cell::Cell<usize>>,
        overlays: Rc<std::cell::Cell<usize>>,
    }
    impl crate::widget::Widget for FocusRow {
        fn style(&self) -> crate::Style {
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: Dimension::Length(20.0),
                    height: Dimension::Length(20.0),
                },
                flex_shrink: 0.0,
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn focusable(&self) -> bool {
            true
        }
        fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
            let keys = self.keys.clone();
            Some(Rc::new(move |_| keys.set(keys.get() + 1)))
        }
        fn paint_focused_overlay(&self, _painter: &mut dyn Painter, _rect: Rect, _caret: bool) {
            self.overlays.set(self.overlays.get() + 1);
        }
    }

    #[test]
    fn focus_indices_survive_scrolling_a_focusable_out_of_view() {
        let keys: Vec<_> = (0..3).map(|_| Rc::new(std::cell::Cell::new(0))).collect();
        let overlays: Vec<_> = (0..3).map(|_| Rc::new(std::cell::Cell::new(0))).collect();
        let build = |offset: f32| -> BoxedWidget {
            Box::new(ScrollWrapper {
                offset,
                children: (0..3)
                    .map(|i| {
                        Box::new(FocusRow {
                            keys: keys[i].clone(),
                            overlays: overlays[i].clone(),
                        }) as BoxedWidget
                    })
                    .collect(),
            })
        };
        let mut renderer = Renderer::new();
        let scene = renderer.render_focused(build(0.0), VIEWPORT, &mut NoopPainter, Some(1), true);
        scene.on_key_at(1).unwrap()(KeyInput {
            key: crate::widget::Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(keys[1].get(), 1);

        let scene = renderer.render_focused(build(30.0), VIEWPORT, &mut NoopPainter, Some(1), true);
        scene.on_key_at(1).unwrap()(KeyInput {
            key: crate::widget::Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(keys[1].get(), 2, "index 1 still targets the second row");
        assert_eq!(keys[2].get(), 0);
        assert_eq!(scene.focus_hit_test(Point { x: 5.0, y: 5.0 }), Some(1));
        assert_eq!(scene.focus_hit_test(Point { x: 5.0, y: 15.0 }), Some(2));
        assert_eq!(
            overlays[1].get(),
            1,
            "painted only once it scrolled into view"
        );
        assert_eq!(overlays[2].get(), 0);

        let scene = renderer.render_focused(build(30.0), VIEWPORT, &mut NoopPainter, Some(0), true);
        assert!(
            scene.on_key_at(0).is_some(),
            "the hidden first row keeps its slot"
        );
        assert_eq!(
            overlays[0].get(),
            0,
            "a hidden widget paints no focus overlay"
        );
    }

    #[test]
    fn inflow_child_cannot_exceed_parent() {
        let clicked = Rc::new(std::cell::Cell::new(false));
        let scene = render_frame(
            Box::new(ColumnRoot {
                height: 100.0,
                children: vec![Box::new(SizedClick {
                    width: 100.0,
                    height: 400.0,
                    flex_shrink: 1.0,
                    clicked: clicked.clone(),
                })],
            }),
            Size {
                width: 100.0,
                height: 100.0,
            },
            &mut NoopPainter,
        );
        assert!(scene.hit_test(Point { x: 50.0, y: 150.0 }).is_none());
        scene
            .hit_test(Point { x: 50.0, y: 50.0 })
            .expect("capped child still fills the parent")();
        assert!(clicked.get());
    }

    #[test]
    fn viewport_clips_hit_testing_of_nonshrinking_overflow() {
        let clicked = Rc::new(std::cell::Cell::new(false));
        let scene = render_frame(
            Box::new(ColumnRoot {
                height: 100.0,
                children: vec![Box::new(SizedClick {
                    width: 100.0,
                    height: 400.0,
                    flex_shrink: 0.0,
                    clicked: clicked.clone(),
                })],
            }),
            Size {
                width: 100.0,
                height: 100.0,
            },
            &mut NoopPainter,
        );
        assert!(scene.hit_test(Point { x: 50.0, y: 150.0 }).is_none());
        scene
            .hit_test(Point { x: 50.0, y: 50.0 })
            .expect("visible slice inside the viewport stays hittable")();
        assert!(clicked.get());
    }
}
