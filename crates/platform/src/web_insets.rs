//! `env(safe-area-inset-*)` in CSS pixels.
//!
//! Those values are non-zero when the page opts into
//! `viewport-fit=cover` and the browser actually overlays system UI on the
//! canvas (a notch, or the home indicator in a standalone web app). A normal
//! desktop browser reports zeros.

use crate::SafeArea;
use std::cell::RefCell;
use wasm_bindgen::JsCast;

thread_local! {
    static PROBE: RefCell<Option<web_sys::HtmlElement>> = RefCell::new(None);
}

pub(crate) fn safe_area() -> SafeArea {
    let Some(probe) = probe() else {
        return SafeArea::ZERO;
    };
    let Some(window) = web_sys::window() else {
        return SafeArea::ZERO;
    };
    let Ok(Some(computed)) = window.get_computed_style(&probe) else {
        return SafeArea::ZERO;
    };
    let edge = |name: &str| match computed.get_property_value(name) {
        Ok(value) => css_px(&value),
        Err(_) => 0.0,
    };
    SafeArea {
        top: edge("padding-top"),
        right: edge("padding-right"),
        bottom: edge("padding-bottom"),
        left: edge("padding-left"),
    }
}

fn css_px(value: &str) -> f32 {
    value
        .trim()
        .trim_end_matches("px")
        .parse::<f32>()
        .unwrap_or(0.0)
        .max(0.0)
}

fn probe() -> Option<web_sys::HtmlElement> {
    PROBE.with(|slot| {
        if let Some(existing) = slot.borrow().clone() {
            let node: &web_sys::Node = existing.as_ref();
            if node.parent_node().is_some() {
                return Some(existing);
            }
        }
        let window = web_sys::window()?;
        let document = window.document()?;
        let created = document.create_element("div").ok()?;
        let element = created.dyn_into::<web_sys::HtmlElement>().ok()?;
        let style = element.style();
        for (property, value) in [
            ("position", "fixed"),
            ("top", "0"),
            ("left", "0"),
            ("width", "0"),
            ("height", "0"),
            ("visibility", "hidden"),
            ("pointer-events", "none"),
            ("padding-top", "env(safe-area-inset-top)"),
            ("padding-right", "env(safe-area-inset-right)"),
            ("padding-bottom", "env(safe-area-inset-bottom)"),
            ("padding-left", "env(safe-area-inset-left)"),
        ] {
            let _ = style.set_property(property, value);
        }
        let parent = document
            .body()
            .map(|body| body.unchecked_into::<web_sys::Node>())
            .or_else(|| {
                document
                    .document_element()
                    .map(|element| element.unchecked_into::<web_sys::Node>())
            })?;
        let node: &web_sys::Node = element.as_ref();
        parent.append_child(node).ok()?;
        *slot.borrow_mut() = Some(element.clone());
        Some(element)
    })
}
