//! Headless widgets: fully unstyled building blocks with no opinion on
//! color, radius, or spacing. Themed widgets (see [`crate::themed`]) wrap
//! these and fill in appearance from a [`creamui_theme::Theme`]; apps that
//! want a completely custom look can use these directly instead.

use creamui_core::layout::Style;
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Painter, Point, Rect, Styled, TextAlign, Widget,
};
use creamui_theme::Color;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
thread_local! {
    // On X11/Wayland the clipboard owner must remain alive after the write;
    // creating and dropping `arboard::Clipboard` inside a key callback makes
    // clipboard managers lose the contents immediately.
    static SYSTEM_CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

fn activate_on_key(click: Rc<dyn Fn()>) -> Rc<dyn Fn(KeyInput)> {
    Rc::new(move |input| {
        if !input.modifiers.ctrl && matches!(input.key, Key::Enter | Key::Char(' ')) {
            click();
        }
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn clipboard_write(text: String) {
    SYSTEM_CLIPBOARD.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = arboard::Clipboard::new().ok();
        }
        if let Some(clipboard) = slot.as_mut() {
            let _ = clipboard.set_text(text);
        }
    });
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn clipboard_read() -> Option<String> {
    SYSTEM_CLIPBOARD.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = arboard::Clipboard::new().ok();
        }
        slot.as_mut()
            .and_then(|clipboard| clipboard.get_text().ok())
    })
}

#[cfg(target_os = "android")]
fn android_clipboard<T>(
    operation: impl for<'local> FnOnce(
        &mut jni::Env<'local>,
        jni::objects::JObject<'local>,
    ) -> jni::errors::Result<T>,
) -> Option<T> {
    use jni::objects::{Global, JObject};

    let context = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread(|env| {
        let raw_activity = context.context() as jni::sys::jobject;
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
        let activity = env.new_local_ref(activity)?;
        operation(env, activity)
    })
    .ok()
}

#[cfg(target_os = "android")]
fn clipboard_manager<'local>(
    env: &mut jni::Env<'local>,
    activity: &jni::objects::JObject<'local>,
) -> jni::errors::Result<jni::objects::JObject<'local>> {
    let service = env.new_string("clipboard")?;
    env.call_method(
        activity,
        jni::jni_str!("getSystemService"),
        jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
        &[jni::objects::JValue::Object(&*service)],
    )?
    .l()
}

#[cfg(target_os = "android")]
fn clipboard_write(text: String) {
    let _ = android_clipboard(|env, activity| {
        let manager = clipboard_manager(env, &activity)?;
        let label = env.new_string("CreamUI")?;
        let value = env.new_string(text)?;
        let clip = env
            .call_static_method(
                jni::jni_str!("android/content/ClipData"),
                jni::jni_str!("newPlainText"),
                jni::jni_sig!(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                ),
                &[
                    jni::objects::JValue::Object(&*label),
                    jni::objects::JValue::Object(&*value),
                ],
            )?
            .l()?;
        env.call_method(
            manager,
            jni::jni_str!("setPrimaryClip"),
            jni::jni_sig!("(Landroid/content/ClipData;)V"),
            &[jni::objects::JValue::Object(&clip)],
        )?;
        Ok(())
    });
}

#[cfg(target_os = "android")]
fn clipboard_read() -> Option<String> {
    android_clipboard(|env, activity| {
        let manager = clipboard_manager(env, &activity)?;
        if !env
            .call_method(
                &manager,
                jni::jni_str!("hasPrimaryClip"),
                jni::jni_sig!("()Z"),
                &[],
            )?
            .z()?
        {
            return Ok(None);
        }
        let clip = env
            .call_method(
                &manager,
                jni::jni_str!("getPrimaryClip"),
                jni::jni_sig!("()Landroid/content/ClipData;"),
                &[],
            )?
            .l()?;
        if clip.is_null() {
            return Ok(None);
        }
        let item = env
            .call_method(
                clip,
                jni::jni_str!("getItemAt"),
                jni::jni_sig!("(I)Landroid/content/ClipData$Item;"),
                &[jni::objects::JValue::Int(0)],
            )?
            .l()?;
        let text = env
            .call_method(
                item,
                jni::jni_str!("coerceToText"),
                jni::jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                &[jni::objects::JValue::Object(&activity)],
            )?
            .l()?;
        if text.is_null() {
            return Ok(None);
        }
        let string = env
            .call_method(
                text,
                jni::jni_str!("toString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        if string.is_null() {
            return Ok(None);
        }
        let string = env.cast_local::<jni::objects::JString>(string)?;
        let text = string.mutf8_chars(env)?.to_str().into_owned();
        Ok(Some(text))
    })
    .flatten()
}

#[cfg(target_arch = "wasm32")]
fn clipboard_write(_: String) {}

#[cfg(target_arch = "wasm32")]
fn clipboard_read() -> Option<String> {
    None
}

/// Draws an underline and/or strikethrough rule under/through a run of text
/// painted with [`Painter::fill_text_font`], shared by [`RawText`] and
/// [`RawLink`] so both decorate exactly the same way. `rect`, `font_size`,
/// `bold` and `align` must match the values the text itself was painted
/// with — the rule's width and horizontal position are derived from
/// [`crate::text_metrics::measure_family`] using them, not from re-measuring
/// glyphs the painter already laid out.
fn draw_text_decorations(
    painter: &mut dyn Painter,
    rect: Rect,
    text: &str,
    font_size: f32,
    family: Option<&str>,
    bold: bool,
    align: TextAlign,
    color: Color,
    underline: bool,
    strikethrough: bool,
) {
    if !underline && !strikethrough {
        return;
    }
    let (width, _) = crate::text_metrics::measure_family(text, font_size, rect.width, family, bold);
    let x = match align {
        TextAlign::Start => rect.x,
        TextAlign::Center => rect.x + (rect.width - width) / 2.0,
        TextAlign::End => rect.x + rect.width - width,
    };
    let center_y = rect.y + rect.height / 2.0;
    let thickness = (font_size * 0.06).max(1.0);
    if underline {
        let y = center_y + font_size * 0.32;
        painter.fill_rect(
            Rect {
                x,
                y: y - thickness / 2.0,
                width,
                height: thickness,
            },
            color,
            0.0,
        );
    }
    if strikethrough {
        let y = center_y - font_size * 0.02;
        painter.fill_rect(
            Rect {
                x,
                y: y - thickness / 2.0,
                width,
                height: thickness,
            },
            color,
            0.0,
        );
    }
}

mod button;
mod controls;
mod dataview;
mod foundation;
mod marquee;
mod navigation;
mod pickers;
mod scroll;
mod spinner;
mod text_input;
mod typography;
mod virtualize;

pub use button::*;
pub use controls::*;
pub use dataview::*;
pub use foundation::*;
pub use marquee::*;
pub use navigation::*;
pub use pickers::*;
pub use scroll::*;
pub use spinner::*;
pub use text_input::*;
pub use typography::*;
pub use virtualize::*;

macro_rules! impl_direct_styled {
    ($($widget:ty),+ $(,)?) => {
        $(
            impl creamui_core::Styled for $widget {
                fn set_style(&mut self, style: creamui_core::Style) {
                    self.style = style;
                }
            }
        )+
    };
}

impl_direct_styled!(
    RawButton,
    RawCheckbox,
    RawColorPicker,
    RawDateTimePicker,
    RawFilePicker,
    RawLink,
    RawListView,
    RawMarquee,
    RawPre,
    RawQuote,
    RawScrollView,
    RawScrollbar,
    RawSidebar,
    RawSlider,
    RawSpinner,
    RawSwitch,
    RawTab,
    RawTable,
    RawTabs,
    RawText,
    RawTextArea,
    RawTextInput,
    RawView,
    RawVirtualList,
);
