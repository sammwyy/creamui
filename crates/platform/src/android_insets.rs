//! Occupied window bands reported by Android `WindowInsets`.
//!
//! The native content rectangle only describes where the content view was
//! laid out. On an edge-to-edge window that view fills the surface while the
//! status bar, navigation bar, display cutout, and IME still cover it, and
//! taps in those bands go to the system. `WindowInsets` is the report of
//! that overlap.

use crate::SafeArea;
use std::cell::Cell;

pub(crate) fn window_insets(app: &crate::AndroidApp, scale: f64) -> SafeArea {
    match query(app, scale) {
        Ok(area) => {
            note_insets(area);
            area
        }
        Err(err) => {
            thread_local! {
                static LOGGED: Cell<bool> = const { Cell::new(false) };
            }
            LOGGED.with(|logged| {
                if !logged.replace(true) {
                    let message = format!("creamui-platform: window insets unavailable: {err}");
                    log::warn!("{message}");
                    android_log(&message);
                }
            });
            SafeArea::ZERO
        }
    }
}

/// Makes the surface fullscreen and sets status/navigation icon contrast.
///
/// `light_background` asks for dark icons. Returns whether the update was
/// queued on the Activity's Java UI thread.
pub(crate) fn sync_system_bars(app: &crate::AndroidApp, light_background: bool) -> bool {
    let activity_app = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let vm = unsafe { jni::JavaVM::from_raw(activity_app.vm_as_ptr().cast()) };
        let result: jni::errors::Result<()> = vm.attach_current_thread(|env| {
            let raw = activity_app.activity_as_ptr() as jni::sys::jobject;
            let activity = activity_ref(env, raw)?;
            prepare_window(env, &activity)?;
            set_bar_appearance(env, &activity, light_background)
        });
        if let Err(err) = result {
            log::warn!("creamui-platform: system bar style unavailable: {err}");
        }
    }));
    true
}

fn note_insets(area: SafeArea) {
    thread_local! {
        static NOTED: Cell<bool> = const { Cell::new(false) };
    }
    NOTED.with(|noted| {
        if noted.replace(true) {
            return;
        }
        let message = format!(
            "creamui-platform: system insets top={} right={} bottom={} left={}",
            area.top, area.right, area.bottom, area.left
        );
        log::info!("{message}");
        android_log(&message);
    });
}

fn android_log(message: &str) {
    let Ok(tag) = std::ffi::CString::new("creamui") else {
        return;
    };
    let Ok(message) = std::ffi::CString::new(message) else {
        return;
    };
    unsafe {
        __android_log_write(4, tag.as_ptr(), message.as_ptr());
    }
}

unsafe extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}

fn query(app: &crate::AndroidApp, scale: f64) -> Result<SafeArea, String> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| {
        read_insets(env, app.activity_as_ptr() as jni::sys::jobject, scale)
    })
    .map_err(|err| err.to_string())
}

fn read_insets(
    env: &mut jni::Env<'_>,
    raw_activity: jni::sys::jobject,
    scale: f64,
) -> jni::errors::Result<SafeArea> {
    let activity = activity_ref(env, raw_activity)?;
    let sdk = sdk_int(env)?;
    let decor = decor_edges(env, &activity, sdk).unwrap_or((0, 0, 0, 0));
    let metrics = metrics_edges(env, &activity, sdk).unwrap_or((0, 0, 0, 0));
    let (top, right, bottom, left) = (
        decor.0.max(metrics.0),
        decor.1.max(metrics.1),
        decor.2.max(metrics.2),
        decor.3.max(metrics.3),
    );
    Ok(SafeArea::from_physical_px(top, right, bottom, left, scale))
}

fn activity_ref<'a>(
    env: &mut jni::Env<'a>,
    raw_activity: jni::sys::jobject,
) -> jni::errors::Result<jni::objects::JObject<'a>> {
    use jni::objects::{Global, JObject};
    let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
    env.new_local_ref(activity)
}

fn sdk_int(env: &mut jni::Env<'_>) -> jni::errors::Result<i32> {
    env.get_static_field(
        jni::jni_str!("android/os/Build$VERSION"),
        jni::jni_str!("SDK_INT"),
        jni::jni_sig!("I"),
    )?
    .i()
}

/// The surface stays fullscreen in both layout modes. Inset mode pads inside
/// CreamUI; asking the decor view to shrink as well would pad twice.
fn prepare_window(
    env: &mut jni::Env<'_>,
    activity: &jni::objects::JObject<'_>,
) -> jni::errors::Result<()> {
    let window = window(env, activity)?;
    let sdk = sdk_int(env)?;
    if sdk >= 30 {
        env.call_method(
            &window,
            jni::jni_str!("setDecorFitsSystemWindows"),
            jni::jni_sig!("(Z)V"),
            &[jni::objects::JValue::Bool(false)],
        )?;
    } else {
        let decor = decor_view(env, &window)?;
        let mut flags = int_method(env, &decor, jni::jni_str!("getSystemUiVisibility"))?;
        // LAYOUT_STABLE | LAYOUT_HIDE_NAVIGATION | LAYOUT_FULLSCREEN
        flags |= 0x100 | 0x200 | 0x400;
        env.call_method(
            &decor,
            jni::jni_str!("setSystemUiVisibility"),
            jni::jni_sig!("(I)V"),
            &[jni::objects::JValue::Int(flags)],
        )?;
    }
    if sdk >= 29 {
        env.call_method(
            &window,
            jni::jni_str!("setStatusBarContrastEnforced"),
            jni::jni_sig!("(Z)V"),
            &[jni::objects::JValue::Bool(false)],
        )?;
        env.call_method(
            &window,
            jni::jni_str!("setNavigationBarContrastEnforced"),
            jni::jni_sig!("(Z)V"),
            &[jni::objects::JValue::Bool(false)],
        )?;
    }
    Ok(())
}

fn set_bar_appearance(
    env: &mut jni::Env<'_>,
    activity: &jni::objects::JObject<'_>,
    light_background: bool,
) -> jni::errors::Result<()> {
    let window = window(env, activity)?;
    let sdk = sdk_int(env)?;
    if sdk >= 30 {
        let controller = env
            .call_method(
                &window,
                jni::jni_str!("getInsetsController"),
                jni::jni_sig!("()Landroid/view/WindowInsetsController;"),
                &[],
            )?
            .l()?;
        if controller.is_null() {
            return Ok(());
        }
        // APPEARANCE_LIGHT_STATUS_BARS | APPEARANCE_LIGHT_NAVIGATION_BARS.
        // Set means dark icons on a light band.
        let mask = 8 | 16;
        let appearance = if light_background { mask } else { 0 };
        env.call_method(
            &controller,
            jni::jni_str!("setSystemBarsAppearance"),
            jni::jni_sig!("(II)V"),
            &[
                jni::objects::JValue::Int(appearance),
                jni::objects::JValue::Int(mask),
            ],
        )?;
        return Ok(());
    }
    let decor = decor_view(env, &window)?;
    let mut flags = int_method(env, &decor, jni::jni_str!("getSystemUiVisibility"))?;
    let light_status = 0x2000;
    let light_nav = 0x10;
    if light_background {
        flags |= light_status;
        if sdk >= 26 {
            flags |= light_nav;
        }
    } else {
        flags &= !light_status;
        flags &= !light_nav;
    }
    env.call_method(
        &decor,
        jni::jni_str!("setSystemUiVisibility"),
        jni::jni_sig!("(I)V"),
        &[jni::objects::JValue::Int(flags)],
    )?;
    Ok(())
}

fn decor_edges(
    env: &mut jni::Env<'_>,
    activity: &jni::objects::JObject<'_>,
    sdk: i32,
) -> jni::errors::Result<(i32, i32, i32, i32)> {
    let window = window(env, activity)?;
    let decor = decor_view(env, &window)?;
    let insets = env
        .call_method(
            &decor,
            jni::jni_str!("getRootWindowInsets"),
            jni::jni_sig!("()Landroid/view/WindowInsets;"),
            &[],
        )?
        .l()?;
    if insets.is_null() {
        return Ok((0, 0, 0, 0));
    }
    inset_edges(env, &insets, sdk)
}

fn metrics_edges(
    env: &mut jni::Env<'_>,
    activity: &jni::objects::JObject<'_>,
    sdk: i32,
) -> jni::errors::Result<(i32, i32, i32, i32)> {
    if sdk < 30 {
        return Ok((0, 0, 0, 0));
    }
    let name = env.new_string("window")?;
    let manager = env
        .call_method(
            activity,
            jni::jni_str!("getSystemService"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
            &[jni::objects::JValue::Object(&name)],
        )?
        .l()?;
    if manager.is_null() {
        return Ok((0, 0, 0, 0));
    }
    let metrics = env
        .call_method(
            &manager,
            jni::jni_str!("getCurrentWindowMetrics"),
            jni::jni_sig!("()Landroid/view/WindowMetrics;"),
            &[],
        )?
        .l()?;
    let insets = env
        .call_method(
            &metrics,
            jni::jni_str!("getWindowInsets"),
            jni::jni_sig!("()Landroid/view/WindowInsets;"),
            &[],
        )?
        .l()?;
    if insets.is_null() {
        return Ok((0, 0, 0, 0));
    }
    inset_edges(env, &insets, sdk)
}

fn inset_edges(
    env: &mut jni::Env<'_>,
    insets: &jni::objects::JObject<'_>,
    sdk: i32,
) -> jni::errors::Result<(i32, i32, i32, i32)> {
    let modern = if sdk >= 30 {
        let mask = inset_type(env, jni::jni_str!("systemBars"))?
            | inset_type(env, jni::jni_str!("displayCutout"))?
            | inset_type(env, jni::jni_str!("ime"))?;
        let combined = env
            .call_method(
                insets,
                jni::jni_str!("getInsets"),
                jni::jni_sig!("(I)Landroid/graphics/Insets;"),
                &[jni::objects::JValue::Int(mask)],
            )?
            .l()?;
        (
            int_field(env, &combined, jni::jni_str!("top"))?,
            int_field(env, &combined, jni::jni_str!("right"))?,
            int_field(env, &combined, jni::jni_str!("bottom"))?,
            int_field(env, &combined, jni::jni_str!("left"))?,
        )
    } else {
        legacy_edges(env, insets, sdk)?
    };
    if modern.0 == 0 && modern.1 == 0 && modern.2 == 0 && modern.3 == 0 && sdk >= 30 {
        return legacy_edges(env, insets, sdk);
    }
    Ok(modern)
}

fn legacy_edges(
    env: &mut jni::Env<'_>,
    insets: &jni::objects::JObject<'_>,
    sdk: i32,
) -> jni::errors::Result<(i32, i32, i32, i32)> {
    let mut top = int_method(env, insets, jni::jni_str!("getSystemWindowInsetTop"))?;
    let mut right = int_method(env, insets, jni::jni_str!("getSystemWindowInsetRight"))?;
    let mut bottom = int_method(env, insets, jni::jni_str!("getSystemWindowInsetBottom"))?;
    let mut left = int_method(env, insets, jni::jni_str!("getSystemWindowInsetLeft"))?;
    if sdk >= 28 {
        let cutout = env
            .call_method(
                insets,
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
    Ok((top, right, bottom, left))
}

fn window<'a>(
    env: &mut jni::Env<'a>,
    activity: &jni::objects::JObject<'_>,
) -> jni::errors::Result<jni::objects::JObject<'a>> {
    env.call_method(
        activity,
        jni::jni_str!("getWindow"),
        jni::jni_sig!("()Landroid/view/Window;"),
        &[],
    )?
    .l()
}

fn decor_view<'a>(
    env: &mut jni::Env<'a>,
    window: &jni::objects::JObject<'_>,
) -> jni::errors::Result<jni::objects::JObject<'a>> {
    env.call_method(
        window,
        jni::jni_str!("getDecorView"),
        jni::jni_sig!("()Landroid/view/View;"),
        &[],
    )?
    .l()
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
