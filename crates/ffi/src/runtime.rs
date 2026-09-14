//! ABI-v2: handle+mutation entry points over `creamui_core::runtime::Runtime`,
//! instead of ABI-v1's `CWidget` (build a whole subtree, mount it wholesale).
//! A [`CNode`] is an opaque packed index+generation handle — see
//! [`creamui_abi::CNode`].

use creamui_abi::{CColor, CNode, CStyle, CUI_NODE_KIND_TEXT, CUI_NODE_NONE};
use creamui_core::runtime::{Mutation, NodeKind, Runtime, RuntimeNodeId};
use creamui_core::PaintStyle;
use creamui_theme::Color;
use std::os::raw::{c_char, c_int};

pub struct CRuntime(Runtime);

fn decode(node: CNode) -> RuntimeNodeId {
    RuntimeNodeId::from_bits(node)
}

fn decode_optional(node: CNode) -> Option<RuntimeNodeId> {
    if node == CUI_NODE_NONE {
        None
    } else {
        Some(decode(node))
    }
}

#[no_mangle]
pub extern "C" fn cui_runtime_new() -> *mut CRuntime {
    Box::into_raw(Box::new(CRuntime(Runtime::new())))
}

/// # Safety
/// `rt` must be null or a pointer previously returned by
/// [`cui_runtime_new`] and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn cui_runtime_free(rt: *mut CRuntime) {
    if !rt.is_null() {
        drop(Box::from_raw(rt));
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_runtime_node_count(rt: *const CRuntime) -> usize {
    rt.as_ref().map_or(0, |rt| rt.0.len())
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_create_node(rt: *mut CRuntime, kind: c_int) -> CNode {
    let Some(rt) = rt.as_mut() else {
        return CUI_NODE_NONE;
    };
    let kind = if kind == CUI_NODE_KIND_TEXT {
        NodeKind::Text(Default::default())
    } else {
        NodeKind::Container
    };
    rt.0.transaction().create_node(kind).to_bits()
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_set_root(rt: *mut CRuntime, node: CNode) {
    if let Some(rt) = rt.as_mut() {
        rt.0.set_root(decode_optional(node));
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_insert_child(
    rt: *mut CRuntime,
    parent: CNode,
    child: CNode,
    before: CNode,
) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction()
            .insert_child(decode(parent), decode(child), decode_optional(before));
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_remove_subtree(rt: *mut CRuntime, node: CNode) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction().remove_subtree(decode(node));
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
/// `text` must be null or a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn cui_set_text(rt: *mut CRuntime, node: CNode, text: *const c_char) {
    let Some(rt) = rt.as_mut() else {
        return;
    };
    let text = crate::cstr_to_string(text);
    rt.0.transaction().apply(Mutation::SetText {
        node: decode(node),
        text: text.into(),
    });
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_set_layout_style(rt: *mut CRuntime, node: CNode, style: CStyle) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction().apply(Mutation::SetLayoutStyle {
            node: decode(node),
            style: crate::style_from_c(style),
        });
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_set_background(
    rt: *mut CRuntime,
    node: CNode,
    color: CColor,
    corner_radius: f32,
) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction().apply(Mutation::SetPaintStyle {
            node: decode(node),
            style: PaintStyle {
                background: Some(Color::rgba(color.r, color.g, color.b, color.a).into()),
                corner_radius: Some(corner_radius),
                ..Default::default()
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn create_insert_and_count_round_trip() {
        unsafe {
            let rt = cui_runtime_new();
            let root = cui_create_node(rt, 0);
            let child = cui_create_node(rt, 0);
            cui_set_root(rt, root);
            cui_insert_child(rt, root, child, CUI_NODE_NONE);

            assert_eq!(cui_runtime_node_count(rt), 2);

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn remove_subtree_drops_the_node() {
        unsafe {
            let rt = cui_runtime_new();
            let root = cui_create_node(rt, 0);
            let child = cui_create_node(rt, 0);
            cui_insert_child(rt, root, child, CUI_NODE_NONE);
            assert_eq!(cui_runtime_node_count(rt), 2);

            cui_remove_subtree(rt, child);
            assert_eq!(cui_runtime_node_count(rt), 1);

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn set_text_reaches_the_node() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, CUI_NODE_KIND_TEXT);
            let text = CString::new("hello").unwrap();
            cui_set_text(rt, node, text.as_ptr());

            let node_ref = (*rt).0.get(decode(node)).unwrap();
            match &node_ref.kind {
                NodeKind::Text(t) => assert_eq!(&*t.text, "hello"),
                _ => panic!("expected a text node"),
            }

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn a_stale_handle_is_a_no_op_not_a_crash() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, 0);
            cui_remove_subtree(rt, node);

            let text = CString::new("ignored").unwrap();
            cui_set_text(rt, node, text.as_ptr());
            assert_eq!(cui_runtime_node_count(rt), 0);

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn null_runtime_pointer_is_a_no_op_not_a_crash() {
        unsafe {
            assert_eq!(cui_runtime_node_count(std::ptr::null()), 0);
            cui_set_root(std::ptr::null_mut(), CUI_NODE_NONE);
            assert_eq!(cui_create_node(std::ptr::null_mut(), 0), CUI_NODE_NONE);
        }
    }
}
