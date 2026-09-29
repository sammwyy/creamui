//! Occupied window bands reported by Android `WindowInsets`.
//!
//! The native content rectangle only describes where the content view was
//! laid out. On an edge-to-edge window that view fills the surface while the
//! status bar, navigation bar, display cutout, and IME still cover it, and
//! taps in those bands go to the system. `WindowInsets` is the report of
//! that overlap.

use crate::SafeArea;
use std::cell::Cell;

pub(crate) fn window_insets(scale: f64) -> SafeArea {
    match query(scale) {
        Ok(area) => area,
        Err(err) => {
            thread_local! {
                static LOGGED: Cell<bool> = const { Cell::new(false) };
            }
            LOGGED.with(|logged| {
                if !logged.replace(true) {
                    log::debug!("creamui-platform: window insets unavailable: {err}");
                }
            });
            SafeArea::ZERO
        }
    }
}

fn query(scale: f64) -> Result<SafeArea, String> {
    let context = ndk_context::android_context();
    let vm =
        unsafe { jni::JavaVM::from_raw(context.vm().cast()) }.map_err(|err| err.to_string())?;
    vm.attach_current_thread(|env| read_insets(env, context.context() as jni::sys::jobject, scale))
        .map_err(|err| err.to_string())
}

fn read_insets(
    env: &mut jni::Env<'_>,
    raw_activity: jni::sys::jobject,
    scale: f64,
) -> jni::errors::Result<SafeArea> {
    use jni::objects::{Global, JObject, JValue};

    let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
    let activity = env.new_local_ref(activity)?;
    let window = env
        .call_method(
            &activity,
            jni::jni_str!("getWindow"),
            jni::jni_sig!("()Landroid/view/Window;"),
            &[],
        )?
        .l()?;
    let decor = env
        .call_method(
            &window,
            jni::jni_str!("getDecorView"),
            jni::jni_sig!("()Landroid/view/View;"),
            &[],
        )?
        .l()?;
    let insets = env
        .call_method(
            &decor,
            jni::jni_str!("getRootWindowInsets"),
            jni::jni_sig!("()Landroid/view/WindowInsets;"),
            &[],
        )?
        .l()?;
    if insets.is_null() {
        return Ok(SafeArea::ZERO);
    }

    let sdk = env
        .get_static_field(
            jni::jni_str!("android/os/Build$VERSION"),
            jni::jni_str!("SDK_INT"),
            jni::jni_sig!("I"),
        )?
        .i()?;
    let (top, right, bottom, left) = if sdk >= 30 {
        let mask = inset_type(env, jni::jni_str!("systemBars"))?
            | inset_type(env, jni::jni_str!("displayCutout"))?
            | inset_type(env, jni::jni_str!("ime"))?;
        let combined = env
            .call_method(
                &insets,
                jni::jni_str!("getInsets"),
                jni::jni_sig!("(I)Landroid/graphics/Insets;"),
                &[JValue::Int(mask)],
            )?
            .l()?;
        (
            int_field(env, &combined, jni::jni_str!("top"))?,
            int_field(env, &combined, jni::jni_str!("right"))?,
            int_field(env, &combined, jni::jni_str!("bottom"))?,
            int_field(env, &combined, jni::jni_str!("left"))?,
        )
    } else {
        let mut top = int_method(env, &insets, jni::jni_str!("getSystemWindowInsetTop"))?;
        let mut right = int_method(env, &insets, jni::jni_str!("getSystemWindowInsetRight"))?;
        let mut bottom = int_method(env, &insets, jni::jni_str!("getSystemWindowInsetBottom"))?;
        let mut left = int_method(env, &insets, jni::jni_str!("getSystemWindowInsetLeft"))?;
        if sdk >= 28 {
            let cutout = env
                .call_method(
                    &insets,
                    jni::jni_str!("getDisplayCutout"),
                    jni::jni_sig!("()Landroid/view/DisplayCutout;"),
                    &[],
                )?
                .l()?;
            if !cutout.is_null() {
                top = top.max(int_method(env, &cutout, jni::jni_str!("getSafeInsetTop"))?);
                right = right.max(int_method(
                    env,
                    &cutout,
                    jni::jni_str!("getSafeInsetRight"),
                )?);
                bottom = bottom.max(int_method(
                    env,
                    &cutout,
                    jni::jni_str!("getSafeInsetBottom"),
                )?);
                left = left.max(int_method(env, &cutout, jni::jni_str!("getSafeInsetLeft"))?);
            }
        }
        (top, right, bottom, left)
    };
    Ok(SafeArea::from_physical_px(top, right, bottom, left, scale))
}

fn inset_type(
    env: &mut jni::Env<'_>,
    name: impl AsRef<jni::strings::JNIStr>,
) -> jni::errors::Result<i32> {
    env.call_static_method(
        jni::jni_str!("android/view/WindowInsets$Type"),
        name,
        jni::jni_sig!("()I"),
        &[],
    )?
    .i()
}

fn int_method(
    env: &mut jni::Env<'_>,
    object: &jni::objects::JObject<'_>,
    name: impl AsRef<jni::strings::JNIStr>,
) -> jni::errors::Result<i32> {
    env.call_method(object, name, jni::jni_sig!("()I"), &[])?
        .i()
}

fn int_field(
    env: &mut jni::Env<'_>,
    object: &jni::objects::JObject<'_>,
    name: impl AsRef<jni::strings::JNIStr>,
) -> jni::errors::Result<i32> {
    env.get_field(object, name, jni::jni_sig!("I"))?.i()
}
