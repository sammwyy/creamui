//! Declarative routes with reactive location, params and history.
//!
//! Create the router once, outside the window builder. Call
//! [`RouterProvider::render`] around your layout, and [`RouterOutlet::render`]
//! wherever the selected page belongs. Providers are scoped and add no node.

use creamui_core::{BoxedWidget, Painter, Rect, Style, Widget};
use creamui_reactive::{provide_context, try_use_context, use_context, with_context_scope, Signal};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_arch = "wasm32")]
mod browser;

/// A routing or platform error. Navigation leaves the old location intact on error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterError(pub String);
impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RouterError {}

/// Internal URL, including its query and fragment. Reads through hooks subscribe
/// to location changes; keep a Router handle in event callbacks instead of calling hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub pathname: String,
    /// Query without the leading `?`.
    pub search: String,
    /// Fragment without the leading `#`.
    pub hash: String,
}
impl Location {
    pub fn url(&self) -> String {
        format!(
            "{}{}{}",
            self.pathname,
            if self.search.is_empty() {
                String::new()
            } else {
                format!("?{}", self.search)
            },
            if self.hash.is_empty() {
                String::new()
            } else {
                format!("#{}", self.hash)
            }
        )
    }
    /// Decoded query pairs, preserving repeated keys and their order.
    pub fn query_params(&self) -> Vec<(String, String)> {
        url::form_urlencoded::parse(self.search.as_bytes())
            .into_owned()
            .collect()
    }
    pub fn query(&self, key: &str) -> Option<String> {
        self.query_params()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    fn resolve(to: &str, from: &str) -> Result<Self, RouterError> {
        if to.starts_with("//") || url::Url::parse(to).is_ok() {
            return Err(RouterError("navigation requires an internal URL".into()));
        }
        let base = url::Url::parse(&format!("https://creamui.invalid{from}"))
            .map_err(|e| RouterError(e.to_string()))?;
        let parsed = base.join(to).map_err(|e| RouterError(e.to_string()))?;
        if parsed.origin() != base.origin()
            || parsed.username() != ""
            || parsed.password().is_some()
        {
            return Err(RouterError(
                "navigation requires an internal URL; use apply_deep_link for external links"
                    .into(),
            ));
        }
        Ok(Self {
            pathname: parsed.path().into(),
            search: parsed.query().unwrap_or("").into(),
            hash: parsed.fragment().unwrap_or("").into(),
        })
    }
}

pub type Params = BTreeMap<String, String>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMatch {
    pub pattern: String,
    pub params: Params,
}

#[derive(Clone)]
enum Segment {
    Static(String),
    Param(String),
    Wildcard(String),
}
#[derive(Clone)]
struct Route {
    pattern: String,
    segments: Vec<Segment>,
    component: Rc<dyn Fn() -> BoxedWidget>,
}
impl Route {
    fn new(pattern: String, component: Rc<dyn Fn() -> BoxedWidget>) -> Result<Self, RouterError> {
        if !pattern.starts_with('/') || pattern.contains(['?', '#']) {
            return Err(RouterError(format!("invalid route pattern: {pattern}")));
        }
        let parts = split_path(&pattern);
        let mut names = BTreeSet::new();
        let mut segments = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let segment = if let Some(name) = part.strip_prefix(':') {
                if name.is_empty() || !names.insert(name.to_string()) {
                    return Err(RouterError(format!("invalid parameter in {pattern}")));
                }
                Segment::Param(name.into())
            } else if let Some(name) = part.strip_prefix('*') {
                let name = if name.is_empty() { "*" } else { name };
                if index + 1 != parts.len() || !names.insert(name.to_string()) {
                    return Err(RouterError(format!(
                        "wildcard must be unique and last in {pattern}"
                    )));
                }
                Segment::Wildcard(name.into())
            } else {
                Segment::Static(decode(part)?)
            };
            segments.push(segment);
        }
        Ok(Self {
            pattern,
            segments,
            component,
        })
    }
    fn matches(&self, path: &str) -> Option<(Params, Vec<u8>)> {
        let parts = split_path(path);
        let mut params = Params::new();
        let mut rank = Vec::new();
        for (index, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Wildcard(name) => {
                    let values = parts
                        .get(index..)
                        .unwrap_or_default()
                        .iter()
                        .map(|p| decode(p))
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;
                    params.insert(name.clone(), values.join("/"));
                    rank.push(0);
                    return Some((params, rank));
                }
                Segment::Static(value) => {
                    if decode(parts.get(index)?).ok()? != *value {
                        return None;
                    }
                    rank.push(3);
                }
                Segment::Param(name) => {
                    let part = parts.get(index)?;
                    if part.is_empty() {
                        return None;
                    }
                    params.insert(name.clone(), decode(part).ok()?);
                    rank.push(2);
                }
            }
        }
        if parts.len() != self.segments.len() {
            return None;
        }
        rank.push(4); // An exact endpoint wins over an empty wildcard.
        Some((params, rank))
    }
}
fn split_path(path: &str) -> Vec<&str> {
    let path = path.strip_prefix('/').unwrap_or(path).trim_end_matches('/');
    if path.is_empty() {
        Vec::new()
    } else {
        path.split('/').collect()
    }
}
fn decode(value: &str) -> Result<String, RouterError> {
    let mut bytes = Vec::new();
    let mut input = value.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = input.next().and_then(|b| (b as char).to_digit(16));
            let low = input.next().and_then(|b| (b as char).to_digit(16));
            bytes.push(match (high, low) {
                (Some(h), Some(l)) => (h * 16 + l) as u8,
                _ => return Err(RouterError("invalid percent encoding".into())),
            });
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|e| RouterError(e.to_string()))
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct History {
    entries: Vec<String>,
    index: usize,
}
struct Inner {
    #[cfg(target_os = "android")]
    dev_override: bool,
    routes: Vec<Route>,
    fallback: Option<Rc<dyn Fn() -> BoxedWidget>>,
    location: Signal<Location>,
    history: RefCell<History>,
    #[cfg(target_arch = "wasm32")]
    browser: RefCell<Option<browser::BrowserHistory>>,
    #[cfg(target_os = "android")]
    android: RefCell<Option<android::AndroidHistory>>,
}

/// Cloneable router handle. Construct once, then share it through a provider.
#[derive(Clone)]
pub struct Router(Rc<Inner>);

/// Builds immutable route registration. Static paths outrank params, which
/// outrank wildcards; ties use registration order. Matching ignores trailing `/`.
pub struct RouterBuilder {
    initial: String,
    memory: bool,
    dev_override: bool,
    routes: Vec<(String, Rc<dyn Fn() -> BoxedWidget>)>,
    fallback: Option<Rc<dyn Fn() -> BoxedWidget>>,
}
impl RouterBuilder {
    pub fn route(
        mut self,
        path: impl Into<String>,
        component: impl Fn() -> BoxedWidget + 'static,
    ) -> Self {
        self.routes.push((path.into(), Rc::new(component)));
        self
    }
    pub fn not_found(mut self, component: impl Fn() -> BoxedWidget + 'static) -> Self {
        self.fallback = Some(Rc::new(component));
        self
    }
    /// Disable the debug-build `CUI_ROUTER_DEFAULT_PATH` override.
    pub fn dev_override(mut self, enabled: bool) -> Self {
        self.dev_override = enabled;
        self
    }
    pub fn build(self) -> Result<Router, RouterError> {
        let mut patterns = BTreeSet::new();
        let mut routes = Vec::new();
        for (pattern, component) in self.routes {
            if !patterns.insert(pattern.clone()) {
                return Err(RouterError(format!("duplicate route: {pattern}")));
            }
            routes.push(Route::new(pattern, component)?);
        }
        let mut initial = self.initial;
        if self.dev_override && cfg!(debug_assertions) {
            if let Ok(path) = std::env::var("CUI_ROUTER_DEFAULT_PATH") {
                initial = path;
            }
            #[cfg(target_arch = "wasm32")]
            if let Some(path) = option_env!("CUI_ROUTER_DEFAULT_PATH") {
                initial = path.into();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let location = Location::resolve(&initial, "/")?;
        #[cfg(target_arch = "wasm32")]
        let location = if !self.memory {
            browser::current_location()?
        } else {
            Location::resolve(&initial, "/")?
        };
        let router = Router(Rc::new(Inner {
            #[cfg(target_os = "android")]
            dev_override: self.dev_override,
            history: RefCell::new(History {
                entries: vec![location.url()],
                index: 0,
            }),
            routes,
            fallback: self.fallback,
            location: Signal::new(location),
            #[cfg(target_arch = "wasm32")]
            browser: RefCell::new(None),
            #[cfg(target_os = "android")]
            android: RefCell::new(None),
        }));
        #[cfg(target_arch = "wasm32")]
        if !self.memory {
            *router.0.browser.borrow_mut() = Some(browser::BrowserHistory::attach(&router)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.memory;
        Ok(router)
    }
}
impl Router {
    /// Browser history on web, memory history elsewhere. The browser URL takes
    /// precedence over `initial` and the dev override.
    pub fn builder(initial: impl Into<String>) -> RouterBuilder {
        RouterBuilder {
            initial: initial.into(),
            memory: false,
            dev_override: true,
            routes: Vec::new(),
            fallback: None,
        }
    }
    /// Explicit in-memory routing, including on web.
    pub fn memory(initial: impl Into<String>) -> RouterBuilder {
        RouterBuilder {
            memory: true,
            ..Self::builder(initial)
        }
    }
    pub fn location(&self) -> Location {
        self.0.location.get()
    }
    pub fn url(&self) -> String {
        self.location().url()
    }
    pub fn current_match(&self) -> Option<RouteMatch> {
        self.resolve(&self.location().pathname)
            .map(|(route, params)| RouteMatch {
                pattern: route.pattern.clone(),
                params,
            })
    }
    pub fn params(&self) -> Params {
        self.current_match().map(|m| m.params).unwrap_or_default()
    }
    fn resolve(&self, path: &str) -> Option<(&Route, Params)> {
        let mut best: Option<(&Route, Params, Vec<u8>)> = None;
        for route in &self.0.routes {
            if let Some((params, rank)) = route.matches(path) {
                if best
                    .as_ref()
                    .is_none_or(|(_, _, previous)| rank > *previous)
                {
                    best = Some((route, params, rank));
                }
            }
        }
        best.map(|(route, params, _)| (route, params))
    }
    pub fn navigate(&self, to: &str) -> Result<(), RouterError> {
        self.commit(to, false)
    }
    pub fn replace(&self, to: &str) -> Result<(), RouterError> {
        self.commit(to, true)
    }
    fn commit(&self, to: &str, replace: bool) -> Result<(), RouterError> {
        let location = Location::resolve(to, &self.0.location.peek().url())?;
        if replace && location == self.0.location.peek() {
            return Ok(());
        }
        #[cfg(target_arch = "wasm32")]
        if self.0.browser.borrow().is_some() {
            browser::navigate(self, &location, replace)?;
        }
        {
            let mut history = self.0.history.borrow_mut();
            let index = history.index;
            if replace {
                history.entries[index] = location.url();
            } else {
                history.entries.truncate(index + 1);
                history.entries.push(location.url());
                history.index += 1;
            }
        }
        self.persist();
        self.0.location.set(location);
        Ok(())
    }
    /// Apply an Android Intent/App Link URL, keeping only path/query/fragment.
    pub fn apply_deep_link(&self, uri: &str) -> Result<(), RouterError> {
        let internal = if uri.starts_with('/') {
            uri.to_string()
        } else {
            let parsed = url::Url::parse(uri).map_err(|e| RouterError(e.to_string()))?;
            format!(
                "{}{}{}",
                parsed.path(),
                parsed.query().map(|q| format!("?{q}")).unwrap_or_default(),
                parsed
                    .fragment()
                    .map(|h| format!("#{h}"))
                    .unwrap_or_default()
            )
        };
        self.navigate(&internal)
    }
    /// Memory navigation is immediate. Browser traversal finishes on `popstate`.
    pub fn back(&self) -> Result<bool, RouterError> {
        self.go(-1)
    }
    pub fn forward(&self) -> Result<bool, RouterError> {
        self.go(1)
    }
    pub fn go(&self, delta: i32) -> Result<bool, RouterError> {
        #[cfg(target_arch = "wasm32")]
        if self.0.browser.borrow().is_some() {
            return browser::go(delta);
        }
        let location = {
            let mut history = self.0.history.borrow_mut();
            let next = history.index as i64 + delta as i64;
            if next < 0 || next >= history.entries.len() as i64 || delta == 0 {
                return Ok(false);
            }
            history.index = next as usize;
            Location::resolve(&history.entries[history.index], "/")?
        };
        self.persist();
        self.0.location.set(location);
        Ok(true)
    }
    pub fn can_go_back(&self) -> bool {
        self.0.location.get();
        self.0.history.borrow().index > 0
    }
    pub fn can_go_forward(&self) -> bool {
        self.0.location.get();
        let h = self.0.history.borrow();
        h.index + 1 < h.entries.len()
    }
    /// Portable history serialization for process recreation. Browser history
    /// belongs to the browser and cannot be restored with this method.
    pub fn save_history(&self) -> String {
        serde_json::to_string(&*self.0.history.borrow()).expect("serializable router history")
    }
    pub fn restore_history(&self, json: &str) -> Result<(), RouterError> {
        #[cfg(target_arch = "wasm32")]
        if self.0.browser.borrow().is_some() {
            return Err(RouterError("cannot restore browser history".into()));
        }
        let mut history: History =
            serde_json::from_str(json).map_err(|e| RouterError(e.to_string()))?;
        if history.entries.is_empty() || history.index >= history.entries.len() {
            return Err(RouterError("invalid history index".into()));
        }
        for entry in &mut history.entries {
            *entry = Location::resolve(entry, "/")?.url();
        }
        let location = Location::resolve(&history.entries[history.index], "/")?;
        *self.0.history.borrow_mut() = history;
        self.persist();
        self.0.location.set(location);
        Ok(())
    }
    fn persist(&self) {
        #[cfg(target_os = "android")]
        if let Some(android) = self.0.android.borrow().as_ref() {
            android.save(&self.save_history());
        }
    }
}

#[derive(Clone)]
pub struct RouterProvider {
    router: Router,
}
impl RouterProvider {
    pub fn new(router: Router) -> Self {
        Self { router }
    }
    /// Children must be built INSIDE the closure so hooks see this provider.
    pub fn render<R>(&self, children: impl FnOnce() -> R) -> R {
        if try_use_context::<Router>().is_none() {
            if let Some(host) = try_use_context::<RouterHost>() {
                *host.0.borrow_mut() = Rc::downgrade(&self.router.0);
            }
            #[cfg(target_os = "android")]
            android::attach_default(&self.router);
        }
        with_context_scope(|| {
            provide_context(self.router.clone());
            children()
        })
    }
}
pub fn use_router() -> Router {
    use_context::<Router>()
}
pub fn use_location() -> Location {
    use_router().location()
}
pub fn use_params() -> Params {
    use_router().params()
}
pub fn use_query_params() -> Vec<(String, String)> {
    use_location().query_params()
}

/// Render the current page under the nearest provider, inheriting all contexts
/// between provider and outlet. A missing route uses `not_found`, or an empty node.
pub struct RouterOutlet;
impl RouterOutlet {
    pub fn render() -> BoxedWidget {
        let router = use_router();
        let location = router.location();
        if let Some((route, _)) = router.resolve(&location.pathname) {
            (route.component)()
        } else if let Some(fallback) = &router.0.fallback {
            fallback()
        } else {
            Box::new(Empty)
        }
    }
}
struct Empty;
impl Widget for Empty {
    fn style(&self) -> Style {
        Style::default()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}

/// Window integration: provided by the renderer, automatically bound by the
/// outer provider. Uses a weak reference, so it does not keep routers alive.
#[derive(Clone, Default)]
pub struct RouterHost(Rc<RefCell<Weak<Inner>>>);
impl RouterHost {
    pub fn router(&self) -> Option<Router> {
        self.0.borrow().upgrade().map(Router)
    }
    pub fn back(&self) -> Result<bool, RouterError> {
        self.router().map(|r| r.back()).unwrap_or(Ok(false))
    }
    pub fn poll(&self) {
        #[cfg(target_os = "android")]
        if let Some(router) = self.router() {
            android::poll(&router);
        }
    }
}
