use creamui_core::{BoxedWidget, Painter, Rect, Style, Widget};
use creamui_reactive::{create_effect, provide_context, use_context, with_context_scope};
use creamui_router::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Empty;
impl Widget for Empty {
    fn style(&self) -> Style {
        Style::default()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}
fn empty() -> BoxedWidget {
    Box::new(Empty)
}
fn router() -> Router {
    Router::memory("/")
        .dev_override(false)
        .route("/*rest", empty)
        .route("/users/:id", empty)
        .route("/users/new", empty)
        .route("/", empty)
        .build()
        .unwrap()
}

#[test]
fn ranked_routes_decode_params_without_splitting_encoded_slashes() {
    let router = router();
    router.navigate("/users/new/").unwrap();
    assert_eq!(router.current_match().unwrap().pattern, "/users/new");
    router
        .navigate("/users/a%2Fb%20c?tag=a+b&tag=c%2Bd#details")
        .unwrap();
    assert_eq!(router.params()["id"], "a/b c");
    assert_eq!(
        router.location().query_params(),
        vec![("tag".into(), "a b".into()), ("tag".into(), "c+d".into())]
    );
    assert_eq!(router.location().hash, "details");
    router.navigate("/some/deep/path").unwrap();
    assert_eq!(router.params()["rest"], "some/deep/path");
    router.navigate("/").unwrap();
    assert_eq!(router.current_match().unwrap().pattern, "/");
}

#[test]
fn memory_history_replace_traversal_and_branching() {
    let router = router();
    assert!(!router.back().unwrap());
    router.navigate("/one").unwrap();
    router.navigate("/two").unwrap();
    router.replace("/three").unwrap();
    assert!(router.back().unwrap());
    assert_eq!(router.url(), "/one");
    assert!(router.forward().unwrap());
    assert_eq!(router.url(), "/three");
    router.back().unwrap();
    router.navigate("/branch").unwrap();
    assert!(!router.forward().unwrap());
    assert!(router.go(-2).unwrap());
    assert_eq!(router.url(), "/");
    assert!(!router.go(-100).unwrap());
}

#[test]
fn hooks_remain_reactive_after_provider_scope_exits() {
    let router = router();
    let values = Rc::new(RefCell::new(Vec::new()));
    let effect = RouterProvider::new(router.clone()).render(|| {
        create_effect({
            let values = values.clone();
            move || {
                values.borrow_mut().push((
                    use_location().url(),
                    use_params(),
                    use_router().can_go_back(),
                ))
            }
        })
    });
    router.navigate("/users/42").unwrap();
    router.navigate("/users/42").unwrap(); // Same URL, changed history.
    router.go(-2).unwrap();
    let values = values.borrow();
    assert_eq!(values.len(), 4);
    assert_eq!(values[1].1["id"], "42");
    assert!(values[2].2);
    assert!(!values[3].2);
    drop(effect);
}

#[test]
fn outlet_inherits_layout_context_and_nested_providers_are_isolated() {
    let seen = Rc::new(Cell::new(0));
    let router = Router::memory("/")
        .dev_override(false)
        .route("/", {
            let seen = seen.clone();
            move || {
                seen.set(use_context::<u32>());
                empty()
            }
        })
        .build()
        .unwrap();
    let outer = RouterProvider::new(router.clone());
    let nested = RouterProvider::new(
        Router::memory("/nested")
            .dev_override(false)
            .build()
            .unwrap(),
    );
    with_context_scope(|| {
        let host = RouterHost::default();
        provide_context(host.clone());
        outer.render(|| {
            with_context_scope(|| {
                provide_context(42u32);
                RouterOutlet::render()
            });
            nested.render(|| assert_eq!(use_location().pathname, "/nested"));
            assert_eq!(use_location().pathname, "/");
        });
        assert_eq!(host.router().unwrap().url(), router.url());
    });
    assert_eq!(seen.get(), 42);
    assert!(creamui_reactive::try_use_context::<Router>().is_none());
}

#[test]
fn history_roundtrip_and_invalid_restoration_is_atomic() {
    let original = router();
    original.navigate("/one").unwrap();
    original.navigate("/two?foo=bar").unwrap();
    original.back().unwrap();
    let restored = router();
    restored.restore_history(&original.save_history()).unwrap();
    assert_eq!(restored.url(), "/one");
    assert!(restored.forward().unwrap());
    assert_eq!(restored.url(), "/two?foo=bar");
    for json in [
        "{}",
        r#"{"entries":[],"index":0}"#,
        r#"{"entries":["/","https://evil.test/"],"index":0}"#,
    ] {
        assert!(restored.restore_history(json).is_err());
        assert_eq!(restored.url(), "/two?foo=bar");
    }
}

#[test]
fn navigation_resolves_relative_urls_and_rejects_external_urls() {
    let router = router();
    router.navigate("/users/42").unwrap();
    router.navigate("?tab=info").unwrap();
    assert_eq!(router.url(), "/users/42?tab=info");
    router.navigate("../settings#general").unwrap();
    assert_eq!(router.url(), "/settings#general");
    for url in [
        "https://example.com/",
        "//example.com/path",
        "javascript:alert(1)",
    ] {
        assert!(router.navigate(url).is_err());
        assert_eq!(router.url(), "/settings#general");
    }
    router
        .apply_deep_link("https://app.example/users/7?tab=settings#bio")
        .unwrap();
    assert_eq!(router.url(), "/users/7?tab=settings#bio");
}

#[test]
fn invalid_patterns_fail_at_registration_and_fallback_renders() {
    for pattern in [
        "relative",
        "/users/:",
        "/:id/:id",
        "/files/*rest/edit",
        "/a?b",
        "/%zz",
    ] {
        assert!(Router::memory("/").route(pattern, empty).build().is_err());
    }
    assert!(Router::memory("/")
        .route("/", empty)
        .route("/", empty)
        .build()
        .is_err());
    let seen = Rc::new(Cell::new(false));
    let router = Router::memory("/missing")
        .dev_override(false)
        .not_found({
            let seen = seen.clone();
            move || {
                seen.set(true);
                assert!(use_router().current_match().is_none());
                empty()
            }
        })
        .build()
        .unwrap();
    RouterProvider::new(router).render(RouterOutlet::render);
    assert!(seen.get());
}
