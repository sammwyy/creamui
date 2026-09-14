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

#[derive(Debug, Clone, Default)]
pub struct ImageNode {
    pub source: Rc<str>,
}

/// Payload for a node not (yet) expressed as one of [`NodeKind`]'s other
/// primitives — e.g. one translated wholesale from a legacy `Widget` by
/// [`super::mount::mount_legacy_widget`].
#[derive(Debug, Clone, Default)]
pub struct CustomNode {
    pub label: &'static str,
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
    pub previous_rect: crate::Rect,
    /// Layout epoch as of the last time `rect` actually changed.
    pub last_layout_epoch: u64,
    /// This node's own [`RuntimeNode::transform`] plus every ancestor's, as
    /// of the last [`super::Runtime::rebuild_composite`].
    pub effective_transform: super::mutation::Transform2D,
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
    pub dirty: DirtyFlags,
    pub layout: LayoutState,
    pub events: EventState,
    pub paint: super::paint::PaintState,
}

impl RuntimeNode {
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
            dirty: DirtyFlags::STRUCTURE,
            layout: LayoutState {
                taffy_node,
                measure_fingerprint: None,
                rect: crate::Rect::default(),
                previous_rect: crate::Rect::default(),
                last_layout_epoch: 0,
                effective_transform: super::mutation::Transform2D::default(),
            },
            events: EventState::default(),
            paint: super::paint::PaintState::default(),
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
