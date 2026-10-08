//! NativeActivity integration. `AppBuilder::run_android` installs the app and
//! the top-level provider attaches automatically. Explicit attachment supports
//! standalone users and a custom persistence key. New Intents must be exposed
//! by the host Activity (`setIntent` in `onNewIntent`), or forwarded via
//! `Router::apply_deep_link`. See the crate README.
use super::*;
use android_activity::AndroidApp;
use jni::objects::{Global, JObject, JString, JValue};

thread_local! {
    // The initializer is const; Android's generated TLS fallback triggers this lint.
    #[allow(clippy::missing_const_for_thread_local)]
    static APP: RefCell<Option<AndroidApp>> = const { RefCell::new(None) };
}

/// Called by the renderer before starting its Android loop.
pub fn install(app: AndroidApp) {
    APP.with(|slot| *slot.borrow_mut() = Some(app));
}

pub(super) struct AndroidHistory {
    app: AndroidApp,
    key: String,
    last_intent: RefCell<Option<String>>,
}
fn with_activity<R>(
    app: &AndroidApp,
    f: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> jni::errors::Result<R>,
) -> Result<R, RouterError> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| {
        let raw = app.activity_as_ptr() as jni::sys::jobject;
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
        let activity = env.new_local_ref(activity)?;
        let result = f(env, &activity);
        if result.is_err() {
            env.exception_clear();
        }
        result
    })
    .map_err(|e| RouterError(format!("Android router: {e}")))
}

/// Leave the root route by moving the existing Activity task to the background.
/// Keeping its native loop alive also permits a warm resume with winit, which
/// cannot recreate an event loop in the same process. Queued on the Java UI thread.
pub fn background_task(router: &Router) -> Result<(), RouterError> {
    let app = router
        .0
        .android
        .borrow()
        .as_ref()
        .map(|binding| binding.app.clone())
        .ok_or_else(|| RouterError("Android router is not attached".into()))?;
    let activity_app = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let result = with_activity(&activity_app, |env, activity| {
            env.call_method(
                activity,
                jni::jni_str!("moveTaskToBack"),
                jni::jni_sig!("(Z)Z"),
                &[JValue::Bool(true)],
            )?
            .z()
        });
        if !matches!(result, Ok(true)) {
            log::warn!("Android router could not background its task: {result:?}");
        }
    }));
    Ok(())
}

impl AndroidHistory {
    fn access<R>(
        &self,
        f: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> jni::errors::Result<R>,
    ) -> Result<R, RouterError> {
        with_activity(&self.app, f)
    }
    fn preferences<'a>(
        &self,
        env: &mut jni::Env<'a>,
        activity: &JObject<'_>,
    ) -> jni::errors::Result<JObject<'a>> {
        let name = env.new_string("creamui-router")?;
        env.call_method(
            activity,
            jni::jni_str!("getSharedPreferences"),
            jni::jni_sig!("(Ljava/lang/String;I)Landroid/content/SharedPreferences;"),
            &[JValue::Object(&name), JValue::Int(0)],
        )?
        .l()
    }
    fn load(&self) -> Result<Option<String>, RouterError> {
        self.access(|env, activity| {
            let prefs = self.preferences(env, activity)?;
            let key = env.new_string(&self.key)?;
            let value = env
                .call_method(
                    &prefs,
                    jni::jni_str!("getString"),
                    jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&key), JValue::Object(&JObject::null())],
                )?
                .l()?;
            if value.is_null() {
                Ok(None)
            } else {
                let value = env.cast_local::<JString>(value)?;
                Ok(Some(value.try_to_string(env)?))
            }
        })
    }
    pub(super) fn save(&self, json: &str) {
        let result = self.access(|env, activity| {
            let prefs = self.preferences(env, activity)?;
            let editor = env.call_method(&prefs, jni::jni_str!("edit"), jni::jni_sig!("()Landroid/content/SharedPreferences$Editor;"), &[])?.l()?;
            let key = env.new_string(&self.key)?;
            let value = env.new_string(json)?;
            env.call_method(&editor, jni::jni_str!("putString"), jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;"), &[JValue::Object(&key), JValue::Object(&value)])?;
            env.call_method(&editor, jni::jni_str!("apply"), jni::jni_sig!("()V"), &[])?;
            Ok(())
        });
        if let Err(error) = result {
            log::warn!("{error}");
        }
    }
    fn intent(&self) -> Result<Option<(String, String)>, RouterError> {
        self.access(|env, activity| {
            let intent = env
                .call_method(
                    activity,
                    jni::jni_str!("getIntent"),
                    jni::jni_sig!("()Landroid/content/Intent;"),
                    &[],
                )?
                .l()?;
            if intent.is_null() {
                return Ok(None);
            }
            let data = env
                .call_method(
                    &intent,
                    jni::jni_str!("getDataString"),
                    jni::jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            if data.is_null() {
                return Ok(None);
            }
            let data = env.cast_local::<JString>(data)?;
            let uri = data.try_to_string(env)?;
            let identity = env
                .call_method(
                    &intent,
                    jni::jni_str!("hashCode"),
                    jni::jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            Ok(Some((format!("{identity}:{uri}"), uri)))
        })
    }
}
/// Restore persisted memory history and process the current launch Intent.
/// A launch deep link wins over restoration; the debug env override skips restoration.
pub fn attach(
    router: &Router,
    app: AndroidApp,
    storage_key: impl Into<String>,
) -> Result<(), RouterError> {
    if router.0.android.borrow().is_some() {
        return Ok(());
    }
    let binding = AndroidHistory {
        app,
        key: storage_key.into(),
        last_intent: RefCell::new(None),
    };
    if !(router.0.dev_override
        && cfg!(debug_assertions)
        && std::env::var_os("CUI_ROUTER_DEFAULT_PATH").is_some())
    {
        if let Some(json) = binding.load()? {
            if let Err(error) = router.restore_history(&json) {
                log::warn!("discarding invalid router history: {error}");
            }
        }
    }
    *router.0.android.borrow_mut() = Some(binding);
    poll(router);
    router.persist();
    Ok(())
}
pub(super) fn attach_default(router: &Router) {
    let app = APP.with(|slot| slot.borrow().clone());
    if let Some(app) = app {
        if let Err(error) = attach(router, app, "main") {
            log::warn!("{error}");
        }
    }
}
pub(super) fn poll(router: &Router) {
    // Release all backend borrows before navigating (effects execute synchronously).
    let uri = {
        let binding = router.0.android.borrow();
        let Some(binding) = binding.as_ref() else {
            return;
        };
        match binding.intent() {
            Ok(Some((identity, uri)))
                if binding.last_intent.borrow().as_ref() != Some(&identity) =>
            {
                *binding.last_intent.borrow_mut() = Some(identity);
                Some(uri)
            }
            Ok(_) => None,
            Err(error) => {
                log::warn!("{error}");
                None
            }
        }
    };
    if let Some(uri) = uri {
        if let Err(error) = router.apply_deep_link(&uri) {
            log::warn!("invalid router Intent: {error}");
        }
    }
}
