#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use std::cell::RefCell;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
thread_local! {
    // X11 and Wayland require the clipboard owner to outlive a write.
    static SYSTEM_CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn with_clipboard<T>(
    operation: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    SYSTEM_CLIPBOARD.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            match arboard::Clipboard::new() {
                Ok(clipboard) => *slot = Some(clipboard),
                Err(error) => {
                    log::warn!("creamui-widgets: cannot access clipboard: {error}");
                    return None;
                }
            }
        }
        match operation(slot.as_mut().expect("clipboard initialized")) {
            Ok(value) => Some(value),
            Err(error) => {
                log::warn!("creamui-widgets: clipboard operation failed: {error}");
                None
            }
        }
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) fn write_text(text: String) {
    with_clipboard(|clipboard| clipboard.set_text(text));
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn read_text() -> Option<String> {
    with_clipboard(|clipboard| clipboard.get_text())
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
    let result = vm.attach_current_thread(|env| {
        let raw_activity = context.context() as jni::sys::jobject;
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
        let activity = env.new_local_ref(activity)?;
        operation(env, activity)
    });
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            log::warn!("creamui-widgets: Android clipboard operation failed: {error}");
            None
        }
    }
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
pub(crate) fn write_text(text: String) {
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
fn read_text() -> Option<String> {
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read(callback: impl FnOnce(String) + 'static) {
    if let Some(text) = read_text() {
        callback(text);
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_clipboard() -> Option<web_sys::Clipboard> {
    let Some(window) = web_sys::window() else {
        log::warn!("creamui-widgets: browser clipboard requires a window");
        return None;
    };
    let navigator = window.navigator();
    let available = match js_sys::Reflect::get(
        navigator.as_ref(),
        &wasm_bindgen::JsValue::from_str("clipboard"),
    ) {
        Ok(value) => value,
        Err(error) => {
            log::warn!("creamui-widgets: cannot access browser clipboard: {error:?}");
            return None;
        }
    };
    if available.is_undefined() || available.is_null() {
        log::warn!("creamui-widgets: browser clipboard is unavailable");
        return None;
    }
    Some(navigator.clipboard())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn write_text(text: String) {
    let Some(clipboard) = browser_clipboard() else {
        return;
    };
    let promise = clipboard.write_text(&text);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = wasm_bindgen_futures::JsFuture::from(promise).await {
            log::warn!("creamui-widgets: browser clipboard write failed: {error:?}");
        }
    });
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn read(callback: impl FnOnce(String) + 'static) {
    let Some(clipboard) = browser_clipboard() else {
        return;
    };
    let promise = clipboard.read_text();
    wasm_bindgen_futures::spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(text) => match text.as_string() {
                Some(text) => {
                    if !text.is_empty() {
                        callback(text);
                    }
                }
                None => log::warn!("creamui-widgets: browser clipboard returned non-text data"),
            },
            Err(error) => log::warn!("creamui-widgets: browser clipboard read failed: {error:?}"),
        }
    });
}
