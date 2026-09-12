use crate::geometry::{Point, Rect, Size};
use crate::widget::{BoxedWidget, CursorIcon, KeyInput, MeasureFn, Painter};
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

/// Consecutive full-paint observations of `animation_time()` being called
/// (or not) needed to promote a node to its own layer, or fully decay back
/// out of one. Promotion is quick (2 frames) since a false positive only
/// costs one extra offscreen buffer; demotion is slower (must decay to 0)
/// so a briefly-paused animation doesn't thrash the layer pool every frame.
const LAYER_PROMOTE_STREAK: u8 = 2;

struct Instance {
    widget: BoxedWidget,
    style: crate::Style,
    children: Vec<Instance>,
    node_id: taffy::NodeId,
    layer_id: u64,
    animating_streak: u8,
    is_layer: bool,
    /// `is_layer`, or true for any descendant — refreshed bottom-up on every
    /// full (non-`animated_only`) paint. Lets an animated-only tick prune a
    /// whole subtree with nothing promoted in it before even computing its
    /// layout, instead of walking every static node just to find nothing.
    has_animated_descendant: bool,
    /// The [`crate::Widget::paint_fingerprint`] the cached layer under
    /// `layer_id` was last painted against, if the widget opts in.
    content_fingerprint: Option<u64>,
    /// The resolved hover/press/focus/disabled state in effect when
    /// `content_fingerprint` was last refreshed. A fingerprint match alone
    /// isn't enough to reuse the cache — the widget's *resolved* appearance
    /// can depend on this live interaction state too (e.g. a hover
    /// highlight), so both must match.
    cached_states: Option<crate::StyleState>,
    /// The ambient clip rect in effect the last time this layer was freshly
    /// painted. A clipping ancestor (e.g. a `ScrollView`) restricts what
    /// actually gets rasterized into the cached layer — pixels outside that
    /// clip are never drawn, not merely hidden — so a later reuse under a
    /// *wider* clip must not composite a buffer that was never painted that
    /// far in the first place.
    cached_clip: Option<Rect>,
    /// Window-space bounds used to capture the cached layer. A cached layer
    /// includes its original backdrop, so it must be repainted when a
    /// scrolling ancestor translates it to a different position.
    cached_rect: Option<Rect>,
}

fn remove_instance(tree: &mut Tree, instance: Instance, painter: &mut dyn Painter) {
    for child in instance.children {
        remove_instance(tree, child, painter);
    }
    painter.forget_layer(instance.layer_id);
    let _ = tree.remove(instance.node_id);
}

/// Reconciles `widget` against a previous frame's `Instance` at the same
/// tree position, if any.
///
/// Widgets carry no persistent identity of their own (all long-lived state
/// lives in `Signal`s, not in widget structs — see `creamui_reactive`), so
/// this reconciles purely structurally: a widget is considered "the same
/// node" as whatever widget previously occupied the same position among its
/// parent's children. That's enough to avoid recreating `taffy` nodes (and
/// their subtrees) on every reactive re-render for the common case where a
/// re-render only changes leaf styles/content, not the tree shape.
///
/// This does not (yet) support keyed reconciliation, so reordering a list
/// of children will be treated as every item after the reorder point
/// changing, rather than being matched up by identity — tracked on the
/// roadmap alongside a real virtual-list/keyed-diff widget.
fn reconcile(
    tree: &mut Tree,
    existing: Option<Instance>,
    mut widget: BoxedWidget,
    next_layer_id: &mut u64,
    painter: &mut dyn Painter,
) -> Instance {
    let new_child_widgets = widget.children();
    let new_style = widget.style();
    let new_measure = widget.measure();

    let Some(mut old) = existing else {
        // No previous node at this position: build a fresh subtree.
        let mut child_ids = Vec::with_capacity(new_child_widgets.len());
        let mut children = Vec::with_capacity(new_child_widgets.len());
        for child_widget in new_child_widgets {
            let child = reconcile(tree, None, child_widget, next_layer_id, painter);
            child_ids.push(child.node_id);
            children.push(child);
        }
        let node_id = tree
            .new_with_children(constrain_inflow(new_style.layout.clone()), &child_ids)
            .expect("taffy node creation is infallible for well-formed styles");
        tree.set_node_context(node_id, new_measure)
            .expect("setting the context of a freshly created node should not fail");
        let layer_id = *next_layer_id;
        *next_layer_id += 1;
        return Instance {
            widget,
            style: new_style,
            children,
            node_id,
            layer_id,
            animating_streak: 0,
            is_layer: false,
            has_animated_descendant: false,
            content_fingerprint: None,
            cached_states: None,
            cached_clip: None,
            cached_rect: None,
        };
    };

    tree.set_style(old.node_id, constrain_inflow(new_style.layout.clone()))
        .expect("updating the style of an existing node should not fail");
    tree.set_node_context(old.node_id, new_measure)
        .expect("updating the context of an existing node should not fail");

    let mut new_children = Vec::with_capacity(new_child_widgets.len());
    let mut old_children = old.children.drain(..);
    for child_widget in new_child_widgets {
        new_children.push(reconcile(
            tree,
            old_children.next(),
            child_widget,
            next_layer_id,
            painter,
        ));
    }
    for leftover in old_children {
        remove_instance(tree, leftover, painter);
    }

    let child_ids: Vec<_> = new_children.iter().map(|c| c.node_id).collect();
    tree.set_children(old.node_id, &child_ids)
        .expect("setting children of an existing node should not fail");

    Instance {
        widget,
        style: new_style,
        children: new_children,
        node_id: old.node_id,
        layer_id: old.layer_id,
        animating_streak: old.animating_streak,
        is_layer: old.is_layer,
        has_animated_descendant: old.has_animated_descendant,
        content_fingerprint: old.content_fingerprint,
        cached_states: old.cached_states,
        cached_clip: old.cached_clip,
        cached_rect: old.cached_rect,
    }
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
    focusables: Vec<(Rect, Rc<dyn Fn(KeyInput)>)>,
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

#[allow(clippy::too_many_arguments)]
fn paint_instance(
    tree: &Tree,
    instance: &mut Instance,
    painter: &mut dyn Painter,
    parent_origin: Point,
    clip: Rect,
    viewport: Rect,
    focus: &mut FocusContext,
    out: &mut PaintOutputs,
    mode: PaintMode,
    animated_only: bool,
) {
    #[cfg(test)]
    PAINT_INSTANCE_VISITS.with(|c| c.set(c.get() + 1));

    // Nothing here or below is promoted, so an animated-only tick has no
    // work in this subtree — skip it before even computing layout.
    if animated_only && !instance.is_layer && !instance.has_animated_descendant {
        return;
    }
    let layout = tree
        .layout(instance.node_id)
        .expect("layout was computed for every instantiated node");
    let rect = Rect {
        x: parent_origin.x + layout.location.x,
        y: parent_origin.y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };

    let absolute = tree
        .style(instance.node_id)
        .map(|style| style.position == taffy::style::Position::Absolute)
        .unwrap_or(false);
    // Absolute layers are portals.
    let effective_clip = if mode == PaintMode::Absolute && absolute {
        viewport
    } else {
        clip
    };
    if mode == PaintMode::Flow && absolute {
        // Absolute layers are rendered in a second pass, above all flow
        // siblings. Their subtree is skipped here as one complete layer.
        return;
    }

    // During the absolute pass, ordinary ancestors are traversal-only nodes;
    // an absolute node and its complete subtree are painted normally. In an
    // animated-only pass, non-layer nodes contribute nothing (their pixels
    // are already sitting in the target buffer from the last full paint),
    // so only a promoted node still runs its paint-self block.
    let paint_self = (mode == PaintMode::Flow || absolute) && (!animated_only || instance.is_layer);
    let mut layer_active = false;
    if paint_self {
        let focusable = instance.widget.focusable() && instance.widget.on_key().is_some();
        let states = instance
            .widget
            .style_state()
            .with_hovered(painter.hovered(rect))
            .with_pressed(painter.pressed(rect))
            .with_focused(focusable && focus.focused_index == Some(focus.counter));

        let fingerprint = instance.widget.paint_fingerprint();
        // A fingerprint match alone doesn't prove the cached pixels are
        // still correct — the widget's *resolved* appearance can also
        // depend on live hover/press/focus state that has nothing to do
        // with its own fingerprint (see `cached_states`'s doc comment).
        // A clipping ancestor restricts what actually gets rasterized into
        // the cached layer, not just what's visible when compositing it —
        // pixels outside that clip were never painted at all. A fingerprint
        // and state match alone can't tell a layer cached under a narrower
        // clip from one cached with nothing cut off, so the ambient clip in
        // effect at capture time must match too.
        let cached_clip_covers = instance.cached_clip.is_some_and(|cached| {
            cached.x <= effective_clip.x
                && cached.y <= effective_clip.y
                && cached.x + cached.width >= effective_clip.x + effective_clip.width
                && cached.y + cached.height >= effective_clip.y + effective_clip.height
        });
        let cache_hit = fingerprint.is_some()
            && fingerprint == instance.content_fingerprint
            && instance.cached_states == Some(states)
            && cached_clip_covers
            && instance.cached_rect == Some(rect)
            && painter.composite_cached_layer(instance.layer_id, rect);

        if !cache_hit {
            let resolved = instance.style.resolve(states);
            let colors = painter.color_scheme();
            let radius = resolved.paint.corner_radius.unwrap_or(0.0);
            // A not-yet-promoted node is only ever visited on a full (non-
            // `animated_only`) pass — see the pruning check above — so one call
            // early is always a full pass too, with `clear()` already behind
            // it. Waiting until `is_layer` itself flips would mean the *actual*
            // first `push_layer` could land on a later animated-only tick
            // instead, capturing a backdrop still contaminated by this widget's
            // own last direct paint rather than the clean ambient background.
            // A fingerprinted widget is promoted unconditionally, on the same
            // reasoning — its first paint must seed the cache a fresh pass
            // reads back on the next unrelated rebuild.
            layer_active = instance.is_layer
                || instance.animating_streak.saturating_add(1) >= LAYER_PROMOTE_STREAK
                || fingerprint.is_some();
            if layer_active {
                painter.push_layer(instance.layer_id, rect, !animated_only);
            }
            if let Some(background) = resolved.paint.background {
                painter.fill_rect(rect, background.resolve(&colors), radius);
            }
            instance.widget.paint(painter, rect);
            if painter.take_animated() {
                instance.animating_streak = instance.animating_streak.saturating_add(1);
                if instance.animating_streak >= LAYER_PROMOTE_STREAK {
                    instance.is_layer = true;
                }
            } else {
                instance.animating_streak = instance.animating_streak.saturating_sub(1);
                if instance.animating_streak == 0 {
                    instance.is_layer = false;
                    // A fingerprinted widget never calls `animation_time()`,
                    // so this branch runs on every one of its paints —
                    // forgetting its layer here would evict the cache this
                    // same paint just seeded.
                    if fingerprint.is_none() {
                        painter.forget_layer(instance.layer_id);
                    }
                }
            }
            // Borders and outlines sit over component-specific content, matching
            // CSS box painting and preventing edge-to-edge content from hiding
            // the common decoration.
            if let Some(border) = resolved.paint.border {
                painter.stroke_rect(rect, border.color.resolve(&colors), border.width, radius);
            }
            if let Some(outline) = resolved.paint.outline {
                painter.stroke_rect(
                    Rect {
                        x: rect.x - outline.width,
                        y: rect.y - outline.width,
                        width: rect.width + outline.width * 2.0,
                        height: rect.height + outline.width * 2.0,
                    },
                    outline.color.resolve(&colors),
                    outline.width,
                    radius + outline.width,
                );
            }
            instance.content_fingerprint = fingerprint;
            instance.cached_states = fingerprint.map(|_| states);
            instance.cached_clip = fingerprint.map(|_| effective_clip);
            instance.cached_rect = fingerprint.map(|_| rect);
            if layer_active && fingerprint.is_some() {
                painter.pop_layer();
                layer_active = false;
            }
        }
    }

    if paint_self && !animated_only {
        if let Some(visible) = rect.intersect(effective_clip) {
            if let Some(handler) = instance.widget.on_click() {
                out.hits.push((visible, handler));
            }
            if let Some(handler) = instance.widget.on_click_at() {
                out.hits_at.push((visible, handler));
            }
            if instance.widget.focusable() {
                if let Some(on_key) = instance.widget.on_key() {
                    if focus.focused_index == Some(focus.counter) {
                        instance
                            .widget
                            .paint_focused_overlay(painter, rect, focus.caret_visible);
                    }
                    focus.counter += 1;
                    out.focusables.push((visible, on_key));
                }
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
    let child_clip = if clips {
        match rect.intersect(effective_clip) {
            Some(c) => c,
            None => {
                // fully clipped away: nothing inside could be visible either
                if layer_active {
                    painter.pop_layer();
                }
                return;
            }
        }
    } else {
        effective_clip
    };

    if clips {
        painter.push_clip_rounded(child_clip, instance.widget.clip_corner_radius());
    }
    let child_mode = if mode == PaintMode::Absolute && absolute {
        PaintMode::Flow
    } else {
        mode
    };
    for child in instance.children.iter_mut() {
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
            animated_only,
        );
    }
    if clips {
        painter.pop_clip();
    }
    if !animated_only {
        instance.has_animated_descendant =
            instance.is_layer || instance.children.iter().any(|c| c.has_animated_descendant);
    }
    if layer_active {
        painter.pop_layer();
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
    focusables: Vec<(Rect, Rc<dyn Fn(KeyInput)>)>,
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
            .find(|(_, (rect, _))| rect.contains(point))
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
    next_layer_id: u64,
    viewport: Size,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            tree: TaffyTree::new(),
            root: None,
            next_layer_id: 0,
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
        let previous = self.root.take();
        let mut instance = reconcile(
            &mut self.tree,
            previous,
            root,
            &mut self.next_layer_id,
            painter,
        );

        self.tree
            .compute_layout_with_measure(
                instance.node_id,
                taffy::geometry::Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
                |known_dimensions, available_space, _node_id, measure, _style| match measure {
                    Some(measure) => measure(known_dimensions, available_space),
                    None => taffy::geometry::Size::ZERO,
                },
            )
            .expect("layout computation should not fail for a well-formed tree");

        self.viewport = viewport;
        let clip = viewport_rect(viewport);
        painter.push_clip(clip);
        let mut out = PaintOutputs::default();
        let mut focus = FocusContext {
            focused_index,
            caret_visible,
            counter: 0,
        };
        paint_instance(
            &self.tree,
            &mut instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Flow,
            false,
        );
        paint_instance(
            &self.tree,
            &mut instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Absolute,
            false,
        );
        painter.pop_clip();
        self.root = Some(instance);
        Scene {
            hits: out.hits,
            hits_at: out.hits_at,
            focusables: out.focusables,
            draggables: out.draggables,
            drag_starts: out.drag_starts,
            scrollables: out.scrollables,
            cursors: out.cursors,
            hovers: out.hovers,
        }
    }

    /// Repaints the retained tree without rebuilding widgets or recomputing
    /// layout. Used for local interaction state such as controlled scrolling.
    pub fn repaint_focused(
        &mut self,
        painter: &mut dyn Painter,
        focused_index: Option<usize>,
        caret_visible: bool,
    ) -> Option<Scene> {
        let instance = self.root.as_mut()?;
        let clip = viewport_rect(self.viewport);
        painter.push_clip(clip);
        let mut out = PaintOutputs::default();
        let mut focus = FocusContext {
            focused_index,
            caret_visible,
            counter: 0,
        };
        paint_instance(
            &self.tree,
            instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Flow,
            false,
        );
        paint_instance(
            &self.tree,
            instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Absolute,
            false,
        );
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

    /// Repaints only the subtrees currently promoted to their own layer
    /// (see [`Painter::push_layer`]) — no rebuild, no layout, and no work
    /// for any other node, whose pixels already sit in `painter`'s target
    /// from the last full [`Renderer::render_focused`]/[`Renderer::repaint_focused`].
    /// Returns the window-space rects that were repainted, or an empty
    /// `Vec` if nothing is currently promoted (e.g. hysteresis just demoted
    /// the last animating widget). The stale [`Scene`] from the last full
    /// paint remains valid, since a pure animation tick changes no
    /// interactive geometry.
    pub fn repaint_animated(
        &mut self,
        painter: &mut dyn Painter,
        focused_index: Option<usize>,
        caret_visible: bool,
    ) -> Vec<Rect> {
        let Some(instance) = self.root.as_mut() else {
            return Vec::new();
        };
        painter.begin_animated_frame();
        let clip = viewport_rect(self.viewport);
        painter.push_clip(clip);
        let mut out = PaintOutputs::default();
        let mut focus = FocusContext {
            focused_index,
            caret_visible,
            counter: 0,
        };
        paint_instance(
            &self.tree,
            instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Flow,
            true,
        );
        paint_instance(
            &self.tree,
            instance,
            painter,
            Point::default(),
            clip,
            clip,
            &mut focus,
            &mut out,
            PaintMode::Absolute,
            true,
        );
        painter.pop_clip();
        painter.take_damage()
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

    struct AnimPainter {
        node_animated: bool,
        push_layer_calls: usize,
        cached_layers: std::collections::HashSet<u64>,
    }
    impl AnimPainter {
        fn new() -> Self {
            AnimPainter {
                node_animated: false,
                push_layer_calls: 0,
                cached_layers: std::collections::HashSet::new(),
            }
        }
    }
    impl Painter for AnimPainter {
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
        fn animation_time(&mut self) -> f32 {
            self.node_animated = true;
            0.0
        }
        fn take_animated(&mut self) -> bool {
            std::mem::take(&mut self.node_animated)
        }
        fn push_layer(&mut self, id: u64, _rect: Rect, _fresh: bool) {
            self.push_layer_calls += 1;
            self.cached_layers.insert(id);
        }
        fn composite_cached_layer(&mut self, id: u64, _rect: Rect) -> bool {
            self.cached_layers.contains(&id)
        }
        fn forget_layer(&mut self, id: u64) {
            self.cached_layers.remove(&id);
        }
    }

    struct CountingWidget {
        count: Rc<std::cell::Cell<usize>>,
        animate: bool,
    }
    impl crate::widget::Widget for CountingWidget {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, painter: &mut dyn Painter, _rect: Rect) {
            self.count.set(self.count.get() + 1);
            if self.animate {
                painter.animation_time();
            }
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

    struct FingerprintWidget {
        count: Rc<std::cell::Cell<usize>>,
        fingerprint: u64,
    }
    impl crate::widget::Widget for FingerprintWidget {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {
            self.count.set(self.count.get() + 1);
        }
        fn paint_fingerprint(&self) -> Option<u64> {
            Some(self.fingerprint)
        }
    }

    struct ScrollWrapper {
        offset: Rc<std::cell::Cell<f32>>,
        child: Option<BoxedWidget>,
    }
    impl crate::widget::Widget for ScrollWrapper {
        fn style(&self) -> crate::Style {
            taffy::style::Style::default().into()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            self.child.take().into_iter().collect()
        }
        fn scroll_offset(&self) -> Point {
            Point {
                x: 0.0,
                y: self.offset.get(),
            }
        }
    }

    #[test]
    fn fingerprint_cache_skips_repaint_when_content_is_unchanged() {
        let count = Rc::new(std::cell::Cell::new(0usize));
        let build = |count: Rc<std::cell::Cell<usize>>, fingerprint: u64| -> BoxedWidget {
            Box::new(Root {
                children: vec![Box::new(FingerprintWidget { count, fingerprint })],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(build(count.clone(), 1), VIEWPORT, &mut painter);
        assert_eq!(count.get(), 1);

        // A second render with an identical fingerprint simulates an
        // unrelated sibling triggering a rebuild — this widget's own
        // content never changed, so it should reuse its cached layer.
        renderer.render(build(count.clone(), 1), VIEWPORT, &mut painter);
        assert_eq!(
            count.get(),
            1,
            "an unrelated rebuild with an unchanged fingerprint should not repaint"
        );
    }

    #[test]
    fn fingerprint_cache_repaints_when_content_changes() {
        let count = Rc::new(std::cell::Cell::new(0usize));
        let build = |count: Rc<std::cell::Cell<usize>>, fingerprint: u64| -> BoxedWidget {
            Box::new(Root {
                children: vec![Box::new(FingerprintWidget { count, fingerprint })],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(build(count.clone(), 1), VIEWPORT, &mut painter);
        renderer.render(build(count.clone(), 2), VIEWPORT, &mut painter);
        assert_eq!(count.get(), 2, "a changed fingerprint must repaint");
    }

    #[test]
    fn fingerprint_cache_repaints_when_a_scroll_ancestor_moves_the_widget() {
        let count = Rc::new(std::cell::Cell::new(0usize));
        let offset = Rc::new(std::cell::Cell::new(0.0));
        let root = Box::new(ScrollWrapper {
            offset: offset.clone(),
            child: Some(Box::new(FingerprintWidget {
                count: count.clone(),
                fingerprint: 1,
            })),
        });
        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(root, VIEWPORT, &mut painter);
        offset.set(20.0);
        renderer.repaint_focused(&mut painter, None, false);

        assert_eq!(
            count.get(),
            2,
            "a cached layer must repaint at its new scroll position"
        );
    }

    #[test]
    fn removing_a_promoted_instance_forgets_its_cached_layer() {
        let count = Rc::new(std::cell::Cell::new(0usize));
        let with_child = |count: Rc<std::cell::Cell<usize>>| -> BoxedWidget {
            Box::new(Root {
                children: vec![Box::new(FingerprintWidget { count, fingerprint: 1 })],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(with_child(count.clone()), VIEWPORT, &mut painter);
        assert_eq!(
            painter.cached_layers.len(),
            1,
            "a fingerprinted widget is promoted unconditionally"
        );

        renderer.render(Box::new(Root { children: vec![] }), VIEWPORT, &mut painter);
        assert!(
            painter.cached_layers.is_empty(),
            "removing a promoted instance must release its cached layer"
        );
    }

    #[test]
    fn repaint_animated_only_repaints_promoted_layers() {
        let animated_count = Rc::new(std::cell::Cell::new(0usize));
        let static_count = Rc::new(std::cell::Cell::new(0usize));
        let build = |animated_count: Rc<std::cell::Cell<usize>>,
                     static_count: Rc<std::cell::Cell<usize>>|
         -> BoxedWidget {
            Box::new(Root {
                children: vec![
                    Box::new(CountingWidget {
                        count: animated_count,
                        animate: true,
                    }),
                    Box::new(CountingWidget {
                        count: static_count,
                        animate: false,
                    }),
                ],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        // Two full renders: the animated child's streak crosses
        // `LAYER_PROMOTE_STREAK` and it gets promoted to a layer.
        renderer.render(
            build(animated_count.clone(), static_count.clone()),
            VIEWPORT,
            &mut painter,
        );
        renderer.render(
            build(animated_count.clone(), static_count.clone()),
            VIEWPORT,
            &mut painter,
        );
        assert_eq!(animated_count.get(), 2);
        assert_eq!(static_count.get(), 2);

        for _ in 0..3 {
            renderer.repaint_animated(&mut painter, None, false);
        }

        assert_eq!(
            animated_count.get(),
            5,
            "the promoted widget should keep repainting on every animated-only pass"
        );
        assert_eq!(
            static_count.get(),
            2,
            "a non-animating sibling should not repaint outside a full render"
        );
        assert!(painter.push_layer_calls > 0);
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

    #[test]
    fn repaint_animated_prunes_subtrees_without_any_layer() {
        let animated_count = Rc::new(std::cell::Cell::new(0usize));
        let build = |animated_count: Rc<std::cell::Cell<usize>>| -> BoxedWidget {
            Box::new(Root {
                children: vec![
                    Box::new(CountingWidget {
                        count: animated_count,
                        animate: true,
                    }),
                    // A large, entirely static subtree with nothing promoted
                    // anywhere inside it.
                    Box::new(Branch { child_count: 200 }),
                ],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(build(animated_count.clone()), VIEWPORT, &mut painter);
        renderer.render(build(animated_count.clone()), VIEWPORT, &mut painter);
        assert_eq!(animated_count.get(), 2);

        PAINT_INSTANCE_VISITS.with(|c| c.set(0));
        renderer.repaint_animated(&mut painter, None, false);

        let visits = PAINT_INSTANCE_VISITS.with(|c| c.get());
        assert!(
            visits < 20,
            "an animated-only pass should prune the 200-node static subtree \
             entirely instead of walking it looking for nothing; visited {visits} nodes"
        );
        assert_eq!(
            animated_count.get(),
            3,
            "the promoted widget must still repaint despite the pruning"
        );
    }

    #[test]
    fn repaint_animated_reaches_promoted_layer_under_absolute_ancestor() {
        let animated_count = Rc::new(std::cell::Cell::new(0usize));
        let build = |animated_count: Rc<std::cell::Cell<usize>>| -> BoxedWidget {
            Box::new(Root {
                children: vec![Box::new(AbsoluteWrapper {
                    child: Some(Box::new(CountingWidget {
                        count: animated_count,
                        animate: true,
                    })),
                })],
            })
        };

        let mut renderer = Renderer::new();
        let mut painter = AnimPainter::new();

        renderer.render(build(animated_count.clone()), VIEWPORT, &mut painter);
        renderer.render(build(animated_count.clone()), VIEWPORT, &mut painter);
        assert_eq!(animated_count.get(), 2);

        renderer.repaint_animated(&mut painter, None, false);
        assert_eq!(
            animated_count.get(),
            3,
            "a promoted layer nested under an absolute ancestor — skipped by \
             the Flow pass — must still be reached and repainted by the Absolute pass"
        );
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
