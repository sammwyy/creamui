use crate::geometry::{Point, Rect, Size};
use crate::widget::{BoxedWidget, CursorIcon, KeyInput, MeasureFn, Painter};
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, TaffyTree};

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
    style: crate::Style,
    children: Vec<Instance>,
    node_id: taffy::NodeId,
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
/// parent's children. That's enough to avoid recreating `taffy` nodes (and
/// their subtrees) on every reactive re-render for the common case where a
/// re-render only changes leaf styles/content, not the tree shape.
///
/// This does not (yet) support keyed reconciliation, so reordering a list
/// of children will be treated as every item after the reorder point
/// changing, rather than being matched up by identity — tracked on the
/// roadmap alongside a real virtual-list/keyed-diff widget.
fn reconcile(tree: &mut Tree, existing: Option<Instance>, mut widget: BoxedWidget) -> Instance {
    let new_child_widgets = widget.children();
    let new_style = widget.style();
    let new_measure = widget.measure();

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
            .new_with_children(new_style.layout.clone(), &child_ids)
            .expect("taffy node creation is infallible for well-formed styles");
        tree.set_node_context(node_id, new_measure)
            .expect("setting the context of a freshly created node should not fail");
        return Instance {
            widget,
            style: new_style,
            children,
            node_id,
        };
    };

    tree.set_style(old.node_id, new_style.layout.clone())
        .expect("updating the style of an existing node should not fail");
    tree.set_node_context(old.node_id, new_measure)
        .expect("updating the context of an existing node should not fail");

    let mut new_children = Vec::with_capacity(new_child_widgets.len());
    let mut old_children = old.children.drain(..);
    for child_widget in new_child_widgets {
        new_children.push(reconcile(tree, old_children.next(), child_widget));
    }
    for leftover in old_children {
        remove_instance(tree, leftover);
    }

    let child_ids: Vec<_> = new_children.iter().map(|c| c.node_id).collect();
    tree.set_children(old.node_id, &child_ids)
        .expect("setting children of an existing node should not fail");

    Instance {
        widget,
        style: new_style,
        children: new_children,
        node_id: old.node_id,
    }
}

/// Effectively "no clip": large enough that intersecting any on-screen rect
/// against it is a no-op. Used as the ambient clip at the root of the tree.
const UNCLIPPED: Rect = Rect {
    x: -1_000_000.0,
    y: -1_000_000.0,
    width: 2_000_000.0,
    height: 2_000_000.0,
};

#[derive(Default)]
struct PaintOutputs {
    hits: Vec<(Rect, Rc<dyn Fn()>)>,
    focusables: Vec<(Rect, Rc<dyn Fn(KeyInput)>)>,
    /// `(visible_rect, full_rect, handler)` — hit-testing uses the
    /// clip-visible portion, but the handler is called with the widget's
    /// full (unclipped) rect so e.g. a slider can divide by its own real
    /// width regardless of how much of it a scroll ancestor currently shows.
    draggables: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>)>,
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

fn paint_instance(
    tree: &Tree,
    instance: &Instance,
    painter: &mut dyn Painter,
    parent_origin: Point,
    clip: Rect,
    focus: &mut FocusContext,
    out: &mut PaintOutputs,
    mode: PaintMode,
) {
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
        UNCLIPPED
    } else {
        clip
    };
    if mode == PaintMode::Flow && absolute {
        // Absolute layers are rendered in a second pass, above all flow
        // siblings. Their subtree is skipped here as one complete layer.
        return;
    }

    // During the absolute pass, ordinary ancestors are traversal-only nodes;
    // an absolute node and its complete subtree are painted normally.
    let paint_self = mode == PaintMode::Flow || absolute;
    if paint_self {
        let focusable = instance.widget.focusable() && instance.widget.on_key().is_some();
        let states = instance
            .widget
            .style_state()
            .with_hovered(painter.hovered(rect))
            .with_pressed(painter.pressed(rect))
            .with_focused(focusable && focus.focused_index == Some(focus.counter));
        let resolved = instance.style.resolve(states);
        let colors = painter.color_scheme();
        let radius = resolved.paint.corner_radius.unwrap_or(0.0);
        if let Some(background) = resolved.paint.background {
            painter.fill_rect(rect, background.resolve(&colors), radius);
        }
        instance.widget.paint(painter, rect);
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
    }

    if paint_self {
        if let Some(visible) = rect.intersect(effective_clip) {
            if let Some(handler) = instance.widget.on_click() {
                out.hits.push((visible, handler));
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
                out.draggables.push((visible, rect, on_drag));
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
            None => return, // fully clipped away: nothing inside could be visible either
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
    for child in &instance.children {
        paint_instance(
            tree,
            child,
            painter,
            child_origin,
            child_clip,
            focus,
            out,
            child_mode,
        );
    }
    if clips {
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
    focusables: Vec<(Rect, Rc<dyn Fn(KeyInput)>)>,
    draggables: Vec<(Rect, Rect, Rc<dyn Fn(Point, Rect)>)>,
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
            .find(|(_, (visible, _, _))| visible.contains(point))
            .map(|(index, _)| index)
    }

    /// The drag handler and full (unclipped) rect at `index`, if it still exists this render.
    pub fn draggable_at(&self, index: usize) -> Option<(Rect, &Rc<dyn Fn(Point, Rect)>)> {
        self.draggables
            .get(index)
            .map(|(_, full, handler)| (*full, handler))
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
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            tree: TaffyTree::new(),
            root: None,
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
        let instance = reconcile(&mut self.tree, previous, root);

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

        let mut out = PaintOutputs::default();
        let mut focus = FocusContext {
            focused_index,
            caret_visible,
            counter: 0,
        };
        paint_instance(
            &self.tree,
            &instance,
            painter,
            Point::default(),
            UNCLIPPED,
            &mut focus,
            &mut out,
            PaintMode::Flow,
        );
        paint_instance(
            &self.tree,
            &instance,
            painter,
            Point::default(),
            UNCLIPPED,
            &mut focus,
            &mut out,
            PaintMode::Absolute,
        );
        self.root = Some(instance);
        Scene {
            hits: out.hits,
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
        let instance = self.root.as_ref()?;
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
            UNCLIPPED,
            &mut focus,
            &mut out,
            PaintMode::Flow,
        );
        paint_instance(
            &self.tree,
            instance,
            painter,
            Point::default(),
            UNCLIPPED,
            &mut focus,
            &mut out,
            PaintMode::Absolute,
        );
        Some(Scene {
            hits: out.hits,
            focusables: out.focusables,
            draggables: out.draggables,
            drag_starts: out.drag_starts,
            scrollables: out.scrollables,
            cursors: out.cursors,
            hovers: out.hovers,
        })
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
}
