//! ABI-v2: handle+mutation entry points over `creamui_core::runtime::Runtime`,
//! instead of ABI-v1's `CWidget` (build a whole subtree, mount it wholesale).
//! A [`CNode`] is an opaque packed index+generation handle — see
//! [`creamui_abi::CNode`].

use creamui_abi::{
    CColor, CNode, CRect, CStyle, CTypographyStyle, CUI_NODE_KIND_TEXT, CUI_NODE_NONE,
};
use creamui_core::runtime::{Mutation, NodeKind, Runtime, RuntimeNodeId, Transform2D};
use creamui_core::{PaintStyle, Size};
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

/// Overrides `node`'s typography. Each field of `style` independently
/// falls back to whatever `node`'s style already resolves (theme default,
/// or an ancestor's) when left unset — see [`CTypographyStyle::unset`].
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
/// `style.font_family` must be null or a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn cui_set_typography_style(
    rt: *mut CRuntime,
    node: CNode,
    style: CTypographyStyle,
) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction().apply(Mutation::SetTypographyStyle {
            node: decode(node),
            style: crate::typography_style_from_c(style),
        });
    }
}

/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_set_transform(rt: *mut CRuntime, node: CNode, x: f32, y: f32) {
    if let Some(rt) = rt.as_mut() {
        rt.0.transaction().apply(Mutation::SetTransform {
            node: decode(node),
            transform: Transform2D { x, y },
        });
    }
}

/// Recomputes layout for every mutation applied since the last call, using
/// `width`/`height` as the viewport. No-op if `rt` has no root.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_compute_layout(rt: *mut CRuntime, width: f32, height: f32) {
    if let Some(rt) = rt.as_mut() {
        rt.0.compute_layout(Size { width, height });
    }
}

/// `node`'s window-space rect as of the last [`cui_compute_layout`] call,
/// or a zeroed rect for a null/unknown/stale handle.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_get_rect(rt: *const CRuntime, node: CNode) -> CRect {
    let zero = CRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    let Some(rt) = rt.as_ref() else {
        return zero;
    };
    let Some(node) = rt.0.get(decode(node)) else {
        return zero;
    };
    CRect {
        x: node.layout.rect.x,
        y: node.layout.rect.y,
        width: node.layout.rect.width,
        height: node.layout.rect.height,
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

    #[test]
    fn set_transform_reaches_the_node() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, 0);
            cui_set_transform(rt, node, 5.0, 10.0);

            let transform = (*rt).0.get(decode(node)).unwrap().transform;
            assert_eq!(transform, Transform2D { x: 5.0, y: 10.0 });

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn compute_layout_and_get_rect_round_trip() {
        unsafe {
            let rt = cui_runtime_new();
            let root = cui_create_node(rt, 0);
            cui_set_root(rt, root);
            cui_set_layout_style(
                rt,
                root,
                creamui_abi::CStyle {
                    width: creamui_abi::CDimension::length(80.0),
                    height: creamui_abi::CDimension::length(40.0),
                    ..CStyle::default_style()
                },
            );

            cui_compute_layout(rt, 200.0, 200.0);
            let rect = cui_get_rect(rt, root);

            assert_eq!(
                rect,
                CRect {
                    x: 0.0,
                    y: 0.0,
                    width: 80.0,
                    height: 40.0,
                }
            );

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn get_rect_for_an_unknown_or_null_handle_is_zeroed_not_a_crash() {
        unsafe {
            let rt = cui_runtime_new();
            let zero = CRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            };
            assert_eq!(cui_get_rect(rt, CUI_NODE_NONE), zero);
            assert_eq!(cui_get_rect(std::ptr::null(), CUI_NODE_NONE), zero);

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn set_typography_style_reaches_the_node() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, CUI_NODE_KIND_TEXT);
            let family = CString::new("Inter").unwrap();
            cui_set_typography_style(
                rt,
                node,
                CTypographyStyle {
                    has_color: 1,
                    color: CColor::rgb(1, 2, 3),
                    font_size: 18.0,
                    font_family: family.as_ptr(),
                    align: creamui_abi::TEXT_ALIGN_START,
                    bold: creamui_abi::TRISTATE_TRUE,
                    ..CTypographyStyle::unset()
                },
            );

            let typography = &(*rt).0.get(decode(node)).unwrap().typography_style;
            assert_eq!(
                typography.color,
                Some(creamui_theme::Color::rgb(1, 2, 3).into())
            );
            assert_eq!(typography.font_size, Some(18.0));
            assert_eq!(typography.font_family.as_deref(), Some("Inter"));
            assert_eq!(typography.align, Some(creamui_core::TextAlign::Start));
            assert_eq!(typography.bold, Some(true));
            assert_eq!(typography.italic, None);

            cui_runtime_free(rt);
        }
    }

    #[test]
    fn set_typography_style_unset_leaves_every_field_none() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, CUI_NODE_KIND_TEXT);
            cui_set_typography_style(rt, node, CTypographyStyle::unset());

            let typography = &(*rt).0.get(decode(node)).unwrap().typography_style;
            assert_eq!(typography.color, None);
            assert_eq!(typography.font_size, None);
            assert_eq!(typography.font_family, None);
            assert_eq!(typography.align, None);
            assert_eq!(typography.bold, None);
            assert_eq!(typography.italic, None);
            assert_eq!(typography.underline, None);
            assert_eq!(typography.strikethrough, None);

            cui_runtime_free(rt);
        }
    }
}
