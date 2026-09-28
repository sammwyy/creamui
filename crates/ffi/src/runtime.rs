//! ABI-v2: handle+mutation entry points over `creamui_core::runtime::Runtime`,
//! instead of ABI-v1's `CWidget` (build a whole subtree, mount it wholesale).
//! A [`CNode`] is an opaque packed index+generation handle — see
//! [`creamui_abi::CNode`].

use creamui_abi::{
    CColor, CColorScheme, CNode, CPaintOp, CRect, CStyle, CTypographyStyle, CUI_NODE_KIND_TEXT,
    CUI_NODE_NONE, CUI_PAINT_BORDER, CUI_PAINT_IMAGE, CUI_PAINT_LINEAR_GRADIENT,
    CUI_PAINT_POP_CLIP, CUI_PAINT_POP_TRANSFORM, CUI_PAINT_PUSH_CLIP, CUI_PAINT_PUSH_ROUNDED_CLIP,
    CUI_PAINT_PUSH_TRANSFORM, CUI_PAINT_QUAD, CUI_PAINT_RADIAL_GRADIENT, CUI_PAINT_TEXT,
};
use creamui_core::runtime::{
    Mutation, NodeKind, PaintOp, PaintPrimitive, Runtime, RuntimeNodeId, Transform2D,
};
use creamui_core::{PaintStyle, Size};
use creamui_theme::Color;
use std::os::raw::{c_char, c_int, c_void};
use std::rc::Rc;

pub struct CRuntime(Runtime);

pub type CEventCallback = Option<extern "C" fn(CNode, *mut c_void)>;

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

fn color_scheme_from_c(colors: CColorScheme) -> creamui_theme::ColorScheme {
    creamui_theme::ColorScheme {
        surface: Color::rgba(
            colors.surface.r,
            colors.surface.g,
            colors.surface.b,
            colors.surface.a,
        ),
        surface_elevated: Color::rgba(
            colors.surface_elevated.r,
            colors.surface_elevated.g,
            colors.surface_elevated.b,
            colors.surface_elevated.a,
        ),
        surface_hover: Color::rgba(
            colors.surface_hover.r,
            colors.surface_hover.g,
            colors.surface_hover.b,
            colors.surface_hover.a,
        ),
        accent: Color::rgba(
            colors.accent.r,
            colors.accent.g,
            colors.accent.b,
            colors.accent.a,
        ),
        accent_hover: Color::rgba(
            colors.accent_hover.r,
            colors.accent_hover.g,
            colors.accent_hover.b,
            colors.accent_hover.a,
        ),
        accent_pressed: Color::rgba(
            colors.accent_pressed.r,
            colors.accent_pressed.g,
            colors.accent_pressed.b,
            colors.accent_pressed.a,
        ),
        selection_background: Color::rgba(
            colors.selection_background.r,
            colors.selection_background.g,
            colors.selection_background.b,
            colors.selection_background.a,
        ),
        selection_text: Color::rgba(
            colors.selection_text.r,
            colors.selection_text.g,
            colors.selection_text.b,
            colors.selection_text.a,
        ),
        text_primary: Color::rgba(
            colors.text_primary.r,
            colors.text_primary.g,
            colors.text_primary.b,
            colors.text_primary.a,
        ),
        text_secondary: Color::rgba(
            colors.text_secondary.r,
            colors.text_secondary.g,
            colors.text_secondary.b,
            colors.text_secondary.a,
        ),
        text_disabled: Color::rgba(
            colors.text_disabled.r,
            colors.text_disabled.g,
            colors.text_disabled.b,
            colors.text_disabled.a,
        ),
        border: Color::rgba(
            colors.border.r,
            colors.border.g,
            colors.border.b,
            colors.border.a,
        ),
        border_strong: Color::rgba(
            colors.border_strong.r,
            colors.border_strong.g,
            colors.border_strong.b,
            colors.border_strong.a,
        ),
        danger: Color::rgba(
            colors.danger.r,
            colors.danger.g,
            colors.danger.b,
            colors.danger.a,
        ),
        warning: Color::rgba(
            colors.warning.r,
            colors.warning.g,
            colors.warning.b,
            colors.warning.a,
        ),
        success: Color::rgba(
            colors.success.r,
            colors.success.g,
            colors.success.b,
            colors.success.a,
        ),
    }
}

fn color_to_c(color: Color) -> CColor {
    CColor::rgba(color.r, color.g, color.b, color.a)
}

fn rect_to_c(rect: creamui_core::Rect) -> CRect {
    CRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn paint_op_to_c(op: &PaintOp) -> CPaintOp {
    let mut out = CPaintOp::default();
    match op {
        PaintOp::PushClip(rect) => {
            out.kind = CUI_PAINT_PUSH_CLIP;
            out.rect = rect_to_c(*rect);
        }
        PaintOp::PushRoundedClip(rect, radius) => {
            out.kind = CUI_PAINT_PUSH_ROUNDED_CLIP;
            out.rect = rect_to_c(*rect);
            out.radius = *radius;
        }
        PaintOp::PopClip => out.kind = CUI_PAINT_POP_CLIP,
        PaintOp::PushTransform(transform) => {
            out.kind = CUI_PAINT_PUSH_TRANSFORM;
            out.x = transform.x;
            out.y = transform.y;
        }
        PaintOp::PopTransform => out.kind = CUI_PAINT_POP_TRANSFORM,
        PaintOp::Primitive(primitive) => match primitive {
            PaintPrimitive::Quad(quad) => {
                out.kind = CUI_PAINT_QUAD;
                out.rect = rect_to_c(quad.rect);
                out.color = color_to_c(quad.color);
                out.radius = quad.corner_radius;
            }
            PaintPrimitive::Gradient(gradient) => {
                out.kind = CUI_PAINT_LINEAR_GRADIENT;
                out.rect = rect_to_c(gradient.rect);
                out.color = color_to_c(gradient.start);
                out.color2 = color_to_c(gradient.end);
                out.radius = gradient.corner_radius;
                out.angle_degrees = gradient.angle_degrees;
            }
            PaintPrimitive::RadialGradient(gradient) => {
                out.kind = CUI_PAINT_RADIAL_GRADIENT;
                out.rect = rect_to_c(gradient.rect);
                out.color = color_to_c(gradient.start);
                out.color2 = color_to_c(gradient.end);
                out.x = gradient.center.x;
                out.y = gradient.center.y;
                out.radius = gradient.radius;
            }
            PaintPrimitive::Border(border) => {
                out.kind = CUI_PAINT_BORDER;
                out.rect = rect_to_c(border.rect);
                out.color = color_to_c(border.color);
                out.radius = border.corner_radius;
                out.x = border.width;
            }
            PaintPrimitive::Text(text) => {
                out.kind = CUI_PAINT_TEXT;
                out.rect = rect_to_c(text.rect);
                out.color = color_to_c(text.color);
                out.radius = text.font_size;
                out.text = text.text.as_ptr().cast();
                out.text_len = text.text.len();
            }
            PaintPrimitive::Image(image) => {
                out.kind = CUI_PAINT_IMAGE;
                out.rect = rect_to_c(image.rect);
            }
        },
    }
    out
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

/// Rebuilds retained paint fragments after layout and style mutations. The
/// supplied color tokens are copied into the generated operations.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_rebuild_paint(rt: *mut CRuntime, colors: CColorScheme) {
    if let Some(rt) = rt.as_mut() {
        rt.0.rebuild_paint(&color_scheme_from_c(colors));
    }
}

/// Returns the number of retained operations in `node`'s last paint
/// fragment. Call [`cui_rebuild_paint`] after mutations before reading it.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_paint_op_count(rt: *const CRuntime, node: CNode) -> usize {
    rt.as_ref()
        .and_then(|rt| rt.0.get(decode(node)))
        .and_then(|node| node.paint.fragment.as_ref())
        .map_or(0, |fragment| fragment.ops.len())
}

/// Copies one retained paint operation into a C value. Returns a default
/// invalid operation (`kind == -1`) for a null runtime, stale node, or index
/// outside the fragment.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_get_paint_op(
    rt: *const CRuntime,
    node: CNode,
    index: usize,
) -> CPaintOp {
    rt.as_ref()
        .and_then(|rt| rt.0.get(decode(node)))
        .and_then(|node| node.paint.fragment.as_ref())
        .and_then(|fragment| fragment.ops.get(index))
        .map_or_else(CPaintOp::default, paint_op_to_c)
}

/// Installs or clears a C click callback. `userdata` is passed back unchanged
/// and must remain valid until the callback is cleared or the runtime is
/// freed. Installing a callback replaces the node's existing event handlers.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
/// When `callback` is non-null, `userdata` must remain valid for its use.
#[no_mangle]
pub unsafe extern "C" fn cui_set_click_callback(
    rt: *mut CRuntime,
    node: CNode,
    callback: CEventCallback,
    userdata: *mut c_void,
) {
    let Some(rt) = rt.as_mut() else {
        return;
    };
    let on_click =
        callback.map(|callback| Rc::new(move || callback(node, userdata)) as Rc<dyn Fn()>);
    rt.0.transaction().apply(Mutation::SetEventHandlers {
        node: decode(node),
        handlers: creamui_core::runtime::EventState {
            on_click,
            ..Default::default()
        },
    });
}

/// Synchronously invokes the click callback currently installed on `node`.
/// Returns `false` when no callback is present.
///
/// # Safety
/// `rt` must be null or a pointer previously returned by [`cui_runtime_new`].
#[no_mangle]
pub unsafe extern "C" fn cui_dispatch_click(rt: *const CRuntime, node: CNode) -> bool {
    let Some(callback) = rt
        .as_ref()
        .and_then(|rt| rt.0.get(decode(node)))
        .and_then(|node| node.events.on_click.clone())
    else {
        return false;
    };
    callback();
    true
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
    fn rebuild_and_read_back_a_retained_background_fragment() {
        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, creamui_abi::CUI_NODE_KIND_CONTAINER);
            cui_set_root(rt, node);
            cui_set_layout_style(
                rt,
                node,
                CStyle {
                    width: creamui_abi::CDimension::length(40.0),
                    height: creamui_abi::CDimension::length(20.0),
                    ..CStyle::default_style()
                },
            );
            cui_set_background(rt, node, CColor::rgb(10, 20, 30), 3.0);
            cui_compute_layout(rt, 100.0, 100.0);
            cui_rebuild_paint(rt, CColorScheme::from_theme(crate::creamui_theme_dark()));

            assert_eq!(cui_paint_op_count(rt, node), 1);
            let op = cui_get_paint_op(rt, node, 0);
            assert_eq!(op.kind, creamui_abi::CUI_PAINT_QUAD);
            assert_eq!(op.color, CColor::rgb(10, 20, 30));
            assert_eq!(op.radius, 3.0);
            assert_eq!(cui_get_paint_op(rt, node, 1).kind, -1);
            cui_runtime_free(rt);
        }
    }

    #[test]
    fn click_callback_round_trip_preserves_userdata() {
        extern "C" fn callback(_node: CNode, userdata: *mut c_void) {
            unsafe { *(userdata as *mut i32) += 1 };
        }

        unsafe {
            let rt = cui_runtime_new();
            let node = cui_create_node(rt, 0);
            let mut count = 0;
            cui_set_click_callback(
                rt,
                node,
                Some(callback),
                (&mut count) as *mut i32 as *mut c_void,
            );
            assert!(cui_dispatch_click(rt, node));
            assert_eq!(count, 1);
            cui_set_click_callback(rt, node, None, std::ptr::null_mut());
            assert!(!cui_dispatch_click(rt, node));
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
