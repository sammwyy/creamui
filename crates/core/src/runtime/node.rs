use super::arena::Id;
use super::dirty::DirtyFlags;
use std::rc::Rc;

pub type RuntimeNodeId = Id<RuntimeNode>;

/// A node's child list, sized for the common cases (no children, one
/// child) without a `Vec` allocation.
#[derive(Debug, Default)]
pub enum Children {
    #[default]
    None,
    One(RuntimeNodeId),
    Many(Vec<RuntimeNodeId>),
}

impl Children {
    pub fn as_slice(&self) -> &[RuntimeNodeId] {
        match self {
            Children::None => &[],
            Children::One(id) => std::slice::from_ref(id),
            Children::Many(ids) => ids,
        }
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Children::None)
    }

    pub(super) fn append(&mut self, id: RuntimeNodeId) {
        match self {
            Children::None => *self = Children::One(id),
            Children::One(existing) => {
                *self = Children::Many(vec![*existing, id]);
            }
            Children::Many(ids) => ids.push(id),
        }
    }

    /// Inserts `id` before `before`, or at the end when `before` is `None`
    /// or not found among the current children. Idempotent: if `id` is
    /// already present, it's moved rather than duplicated.
    pub fn insert(&mut self, id: RuntimeNodeId, before: Option<RuntimeNodeId>) {
        let mut ids = match std::mem::take(self) {
            Children::None => Vec::new(),
            Children::One(existing) => vec![existing],
            Children::Many(ids) => ids,
        };
        ids.retain(|&existing| existing != id);
        match before.and_then(|b| ids.iter().position(|&existing| existing == b)) {
            Some(position) => ids.insert(position, id),
            None => ids.push(id),
        }
        *self = Children::from(ids);
    }

    pub fn remove(&mut self, id: RuntimeNodeId) {
        let mut ids = match std::mem::take(self) {
            Children::None => Vec::new(),
            Children::One(existing) => vec![existing],
            Children::Many(ids) => ids,
        };
        ids.retain(|&existing| existing != id);
        *self = Children::from(ids);
    }
}

impl From<Vec<RuntimeNodeId>> for Children {
    fn from(mut ids: Vec<RuntimeNodeId>) -> Self {
        match ids.len() {
            0 => Children::None,
            1 => Children::One(ids.pop().expect("len checked above")),
            _ => Children::Many(ids),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TextNode {
    pub text: Rc<str>,
}

#[derive(Debug, Clone)]
pub enum ImageContent {
    Source(Rc<str>),
    Decoded(crate::RgbaImage),
}

impl PartialEq for ImageContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Source(a), Self::Source(b)) => a == b,
            (Self::Decoded(a), Self::Decoded(b)) => a.id() == b.id(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    pub content: ImageContent,
    pub fit: ImageFit,
}

/// How decoded pixels map into an image node's layout box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    /// Stretch to the layout box.
    Fill,
    /// Preserve aspect ratio; keep the entire image visible.
    Contain,
    /// Preserve aspect ratio; crop to fill the layout box.
    #[default]
    Cover,
    /// Keep source dimensions at the box's top-left corner.
    None,
}

impl Default for ImageNode {
    fn default() -> Self {
        Self::source("")
    }
}

impl ImageNode {
    pub fn source(source: impl Into<Rc<str>>) -> Self {
        Self {
            content: ImageContent::Source(source.into()),
            fit: ImageFit::Cover,
        }
    }

    pub fn decoded(image: crate::RgbaImage) -> Self {
        Self {
            content: ImageContent::Decoded(image),
            fit: ImageFit::Cover,
        }
    }

    pub fn with_fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
}

/// Payload for a node not (yet) expressed as one of [`NodeKind`]'s other
/// primitives — e.g. one translated wholesale from a legacy `Widget` by
/// [`super::mount::mount_legacy_widget`].
#[derive(Clone, Default)]
pub struct CustomNode {
    pub label: &'static str,
    pub(super) widget: Option<Rc<dyn crate::Widget>>,
}

impl std::fmt::Debug for CustomNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomNode")
            .field("label", &self.label)
            .field("has_widget", &self.widget.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Container,
    Text(TextNode),
    Image(ImageNode),
    Custom(CustomNode),
}

/// A node's derived `taffy` layout node and its last computed geometry.
pub struct LayoutState {
    pub taffy_node: taffy::NodeId,
    /// Fingerprint behind the last `taffy` measure-context write; see
    /// [`super::mutation::Mutation::SetMeasure`].
    pub measure_fingerprint: Option<u64>,
    /// Window-space rect as of the last [`super::Runtime::compute_layout`].
    pub rect: crate::Rect,
    pub content_rect: crate::Rect,
    pub previous_rect: crate::Rect,
    /// Layout epoch as of the last time `rect` actually changed.
    pub last_layout_epoch: u64,
    /// This node's own [`RuntimeNode::transform`] plus every ancestor's, as
    /// of the last [`super::Runtime::rebuild_composite`].
    pub effective_transform: super::mutation::Transform2D,
    /// This node's own [`RuntimeNode::opacity`] multiplied by every
    /// ancestor's, as of the last [`super::Runtime::rebuild_composite`].
    pub effective_opacity: f32,
    /// The visible region this node is clipped to by the nearest ancestor
    /// (or ancestors) with [`RuntimeNode::clips_children`] set, as of the
    /// last [`super::Runtime::rebuild_composite`]. `None` means unclipped
    /// (the common case: no clipping ancestor). A clipping ancestor whose
    /// rect doesn't overlap the ambient clip at all yields a zero-area
    /// rect here, not `None` — "clipped to nothing" is still clipped.
    pub effective_clip: Option<crate::Rect>,
}

/// Retained event handlers and interaction metadata for one node, set
/// wholesale via `Mutation::SetEventHandlers` rather than extracted from a
/// widget during paint.
#[derive(Default, Clone)]
pub struct EventState {
    pub on_click: Option<Rc<dyn Fn()>>,
    pub on_click_at: Option<Rc<dyn Fn(crate::Point)>>,
    pub on_hover: Option<Rc<dyn Fn(bool)>>,
    pub on_key: Option<Rc<dyn Fn(crate::KeyInput)>>,
    pub on_drag: Option<Rc<dyn Fn(crate::Point, crate::Rect)>>,
    pub on_drag_start: Option<Rc<dyn Fn(crate::Point, crate::Rect)>>,
    pub on_drag_end: Option<Rc<dyn Fn()>>,
    pub on_scroll: Option<Rc<dyn Fn(f32)>>,
    pub cursor: Option<crate::CursorIcon>,
    pub focusable: bool,
    pub text_input: bool,
}

impl EventState {
    pub fn is_interactive(&self) -> bool {
        self.on_click.is_some()
            || self.on_click_at.is_some()
            || self.on_hover.is_some()
            || self.on_drag.is_some()
            || self.on_drag_start.is_some()
            || self.on_scroll.is_some()
            || self.cursor.is_some()
            || self.focusable
    }
}

pub struct RuntimeNode {
    pub id: RuntimeNodeId,
    pub parent: Option<RuntimeNodeId>,
    pub children: Children,
    pub kind: NodeKind,
    pub layout_style: taffy::style::Style,
    pub paint_style: crate::PaintStyle,
    pub typography_style: crate::TypographyStyle,
    pub transform: super::mutation::Transform2D,
    pub z_index: i32,
    /// This node's own opacity, in `[0.0, 1.0]`, independent of its
    /// ancestors' — see [`LayoutState::effective_opacity`] for the
    /// cascaded value a renderer actually composites with.
    pub opacity: f32,
    /// Whether this node clips content painted by its children to its own
    /// laid-out rect — see [`LayoutState::effective_clip`] for the
    /// cascaded clip region a renderer actually composites with.
    pub clips_children: bool,
    pub dirty: DirtyFlags,
    pub layout: LayoutState,
    pub events: EventState,
    pub paint: super::paint::PaintState,
    /// The [`super::Runtime`]-wide transaction stamp as of the last time
    /// this node was added to that transaction's touched list — lets
    /// [`super::transaction::RuntimeTransaction::touch`] dedup in `O(1)`
    /// instead of scanning the touched list. `0` never matches a real
    /// stamp (stamps start at `1`), so a freshly created node is correctly
    /// "not yet touched".
    pub(super) touched_stamp: u64,
    /// Set on ancestor paths with changed layout inputs or outputs.
    pub(super) on_layout_path: bool,
    /// This node's index in [`super::Runtime`]'s hit-test list, if listed.
    pub(super) hit_slot: Option<u32>,
}

impl RuntimeNode {
    pub(super) fn is_portal(&self) -> bool {
        self.layout_style.position == taffy::style::Position::Absolute
            && match &self.kind {
                NodeKind::Custom(custom) => custom
                    .widget
                    .as_ref()
                    .is_none_or(|widget| widget.is_portal()),
                _ => true,
            }
    }

    pub(super) fn new(id: RuntimeNodeId, kind: NodeKind, taffy_node: taffy::NodeId) -> Self {
        RuntimeNode {
            id,
            parent: None,
            children: Children::None,
            kind,
            layout_style: taffy::style::Style::default(),
            paint_style: crate::PaintStyle::default(),
            typography_style: crate::TypographyStyle::default(),
            transform: super::mutation::Transform2D::default(),
            z_index: 0,
            opacity: 1.0,
            clips_children: false,
            dirty: DirtyFlags::STRUCTURE,
            layout: LayoutState {
                taffy_node,
                measure_fingerprint: None,
                rect: crate::Rect::default(),
                content_rect: crate::Rect::default(),
                previous_rect: crate::Rect::default(),
                last_layout_epoch: 0,
                effective_transform: super::mutation::Transform2D::default(),
                effective_opacity: 1.0,
                effective_clip: None,
            },
            events: EventState::default(),
            paint: super::paint::PaintState::default(),
            touched_stamp: 0,
            on_layout_path: false,
            hit_slot: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(index: u32) -> RuntimeNodeId {
        Id::from_raw(index, 0)
    }

    #[test]
    fn insert_grows_none_to_one_to_many() {
        let mut children = Children::None;
        children.insert(id(1), None);
        assert!(matches!(children, Children::One(_)));
        children.insert(id(2), None);
        assert!(matches!(children, Children::Many(_)));
        assert_eq!(children.as_slice(), &[id(1), id(2)]);
    }

    #[test]
    fn insert_before_places_at_the_right_position() {
        let mut children = Children::from(vec![id(1), id(3)]);
        children.insert(id(2), Some(id(3)));
        assert_eq!(children.as_slice(), &[id(1), id(2), id(3)]);
    }

    #[test]
    fn remove_shrinks_many_back_to_one_and_none() {
        let mut children = Children::from(vec![id(1), id(2)]);
        children.remove(id(1));
        assert_eq!(children.as_slice(), &[id(2)]);
        children.remove(id(2));
        assert!(children.is_empty());
    }
}
