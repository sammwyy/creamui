use super::*;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

fn error(value: JsValue) -> RouterError {
    RouterError(format!("browser history: {value:?}"))
}
fn window() -> Result<web_sys::Window, RouterError> {
    web_sys::window().ok_or_else(|| RouterError("browser window unavailable".into()))
}
pub(super) fn current_location() -> Result<Location, RouterError> {
    let location = window()?.location();
    Location::resolve(
        &format!(
            "{}{}{}",
            location.pathname().map_err(error)?,
            location.search().map_err(error)?,
            location.hash().map_err(error)?
        ),
        "/",
    )
}
pub(super) struct BrowserHistory {
    window: web_sys::Window,
    listener: Closure<dyn FnMut(web_sys::Event)>,
    session: String,
}
impl BrowserHistory {
    pub(super) fn attach(router: &Router) -> Result<Self, RouterError> {
        let window = window()?;
        let session = format!("{}-{}", js_sys::Date::now(), js_sys::Math::random());
        let weak = Rc::downgrade(&router.0);
        let expected = session.clone();
        let listener = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let Ok(location) = current_location() else {
                return;
            };
            let index = window_state().and_then(|state| {
                let tag = js_sys::Reflect::get(&state, &"creamuiRouterSession".into())
                    .ok()?
                    .as_string()?;
                if tag != expected {
                    return None;
                }
                js_sys::Reflect::get(&state, &"creamuiRouterIndex".into())
                    .ok()?
                    .as_f64()
                    .map(|n| n as usize)
            });
            {
                let mut history = inner.history.borrow_mut();
                if let Some(index) = index.filter(|i| *i < history.entries.len()) {
                    history.index = index;
                    history.entries[index] = location.url();
                } else {
                    *history = History {
                        entries: vec![location.url()],
                        index: 0,
                    };
                }
            }
            inner.location.set(location);
        }) as Box<dyn FnMut(web_sys::Event)>);
        let state = tagged_state(&session, 0)?;
        window
            .history()
            .map_err(error)?
            .replace_state_with_url(&state, "", None)
            .map_err(error)?;
        window
            .add_event_listener_with_callback("popstate", listener.as_ref().unchecked_ref())
            .map_err(error)?;
        Ok(Self {
            window,
            listener,
            session,
        })
    }
}
impl Drop for BrowserHistory {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "popstate",
            self.listener.as_ref().unchecked_ref(),
        );
    }
}
fn window_state() -> Option<JsValue> {
    window().ok()?.history().ok()?.state().ok()
}
fn tagged_state(session: &str, index: usize) -> Result<JsValue, RouterError> {
    let state = js_sys::Object::new();
    // Preserve other integrations' state as a separate value.
    let previous = window_state().unwrap_or(JsValue::NULL);
    let previous =
        if js_sys::Reflect::has(&previous, &"creamuiRouterSession".into()).unwrap_or(false) {
            js_sys::Reflect::get(&previous, &"previousState".into()).unwrap_or(JsValue::NULL)
        } else {
            previous
        };
    js_sys::Reflect::set(&state, &"previousState".into(), &previous).map_err(error)?;
    js_sys::Reflect::set(&state, &"creamuiRouterSession".into(), &session.into()).map_err(error)?;
    js_sys::Reflect::set(
        &state,
        &"creamuiRouterIndex".into(),
        &JsValue::from_f64(index as f64),
    )
    .map_err(error)?;
    Ok(state.into())
}
pub(super) fn navigate(
    router: &Router,
    location: &Location,
    replace: bool,
) -> Result<(), RouterError> {
    let binding = router.0.browser.borrow();
    let binding = binding.as_ref().expect("browser attached");
    let index = router.0.history.borrow().index + usize::from(!replace);
    let state = tagged_state(&binding.session, index)?;
    let history = window()?.history().map_err(error)?;
    if replace {
        history.replace_state_with_url(&state, "", Some(&location.url()))
    } else {
        history.push_state_with_url(&state, "", Some(&location.url()))
    }
    .map_err(error)
}
pub(super) fn go(delta: i32) -> Result<bool, RouterError> {
    if delta == 0 {
        return Ok(false);
    }
    window()?
        .history()
        .map_err(error)?
        .go_with_delta(delta)
        .map_err(error)?;
    Ok(true)
}
