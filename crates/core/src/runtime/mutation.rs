use super::node::{EventState, RuntimeNodeId};
use std::rc::Rc;

/// 2D translation only; no rotation/scale.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Transform2D {
    pub x: f32,
    pub y: f32,
}

/// Field-level updates to an existing node, each mapped to a specific
/// [`super::DirtyFlags`] subset by [`super::RuntimeTransaction::apply`] —
/// splitting these (rather than one `SetStyle`) is what keeps a paint-only
/// change from marking layout dirty and vice versa.
pub enum Mutation {
    SetLayoutStyle {
        node: RuntimeNodeId,
        style: taffy::style::Style,
    },
    SetPaintStyle {
        node: RuntimeNodeId,
        style: crate::PaintStyle,
    },
    SetTypographyStyle {
        node: RuntimeNodeId,
        style: crate::TypographyStyle,
    },
    SetText {
        node: RuntimeNodeId,
        text: Rc<str>,
    },
    SetTransform {
        node: RuntimeNodeId,
        transform: Transform2D,
    },
    /// Clamped to `[0.0, 1.0]`; see [`super::node::RuntimeNode::opacity`].
    SetOpacity {
        node: RuntimeNodeId,
        opacity: f32,
    },
    /// Registers (or clears) `node`'s intrinsic-size function. `fingerprint`
    /// is a hash of whatever `measure` captures; a match against the
    /// node's stored fingerprint skips the `taffy` write. `None` always
    /// writes.
    SetMeasure {
        node: RuntimeNodeId,
        measure: Option<crate::MeasureFn>,
        fingerprint: Option<u64>,
    },
    /// Replaces `node`'s event handlers wholesale.
    SetEventHandlers {
        node: RuntimeNodeId,
        handlers: EventState,
    },
}
