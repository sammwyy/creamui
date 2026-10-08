use creamui::core::layout::{Dimension, Style};
use creamui::reactive::{provide_context, use_context, with_context_scope};
use creamui::widgets::layout::{column, padding, row};
use creamui::{
    use_location, use_params, use_router, AppBuilder, BoxedWidget, Button, RawView, Router,
    RouterOutlet, RouterProvider, Sidebar, SidebarItem, Size, TabColors, Text, Theme,
    WindowOptions,
};

#[derive(Clone)]
struct WorkspaceName(&'static str);

fn page(title: &str, description: &str) -> BoxedWidget {
    let workspace = use_context::<WorkspaceName>();
    Box::new(
        RawView::new(padding(column(16.0), 24.0))
            .child(Box::new(Text::new(title)))
            .child(Box::new(Text::secondary(description)))
            .child(Box::new(Text::secondary(format!(
                "Inherited layout context: {}",
                workspace.0
            )))),
    )
}

fn home() -> BoxedWidget {
    page(
        "Home",
        "Choose a page in the sidebar. The global layout stays in place.",
    )
}
fn profile() -> BoxedWidget {
    let params = use_params();
    let location = use_location();
    page(
        &format!("Profile {}", params["id"]),
        &format!(
            "tab = {:?}; hash = {}",
            location.query("tab"),
            location.hash
        ),
    )
}
fn files() -> BoxedWidget {
    page("Files", &format!("Wildcard: {}", use_params()["rest"]))
}
fn not_found() -> BoxedWidget {
    page("Page not found", &use_location().pathname)
}

fn layout(size: Size) -> BoxedWidget {
    let router = use_router();
    let location = use_location();
    let colors = TabColors::sidebar();
    let mut sidebar = Sidebar::new(
        colors,
        Style {
            size: creamui::core::layout::Size {
                width: Dimension::Length(190.0),
                height: Dimension::Percent(1.0),
            },
            flex_shrink: 0.0,
            ..padding(column(6.0), 12.0)
        },
    );
    for (label, path) in [
        ("Home", "/"),
        ("Profile 42", "/users/42?tab=overview#details"),
        ("Profile 7", "/users/7?tab=settings"),
        ("Files", "/files/docs/router.md"),
        ("Missing page", "/missing"),
    ] {
        let click = router.clone();
        sidebar = sidebar.child(Box::new(SidebarItem::new(
            colors,
            padding(
                Style {
                    size: creamui::core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(42.0),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                12.0,
            ),
            label,
            location.pathname == path.split(['?', '#']).next().unwrap(),
            move || {
                click.navigate(path).expect("internal route");
            },
        )));
    }
    let back = router.clone();
    let forward = router.clone();
    let replace = router.clone();
    sidebar = sidebar
        .child(Box::new(Button::new("Back", move || {
            back.back().expect("back");
        })))
        .child(Box::new(Button::new("Forward", move || {
            forward.forward().expect("forward");
        })))
        .child(Box::new(Button::new("Replace with home", move || {
            replace.replace("/").expect("replace");
        })));
    let content = with_context_scope(|| {
        provide_context(WorkspaceName("CreamUI workspace"));
        RawView::new(Style {
            flex_grow: 1.0,
            ..column(12.0)
        })
        .child(Box::new(Text::secondary(format!(
            "URL: {}",
            location.url()
        ))))
        .child(RouterOutlet::render())
    });
    Box::new(
        RawView::new(Style {
            size: creamui::core::layout::Size {
                width: Dimension::Length(size.width),
                height: Dimension::Length(size.height),
            },
            ..row(16.0)
        })
        .child(Box::new(sidebar))
        .child(Box::new(content)),
    )
}

pub fn app() -> AppBuilder {
    let router = Router::builder("/")
        .route("/", home)
        .route("/users/:id", profile)
        .route("/files/*rest", files)
        .not_found(not_found)
        .build()
        .expect("router initialization");
    let provider = RouterProvider::new(router);
    let theme = Theme::dark();
    AppBuilder::new().window(
        WindowOptions {
            title: "CreamUI — Router".into(),
            width: 960,
            height: 600,
            theme,
            ..Default::default()
        },
        theme.surface,
        |_| {},
        move |size| provider.render(|| layout(size)),
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    creamui::fonts::register_bytes(
        creamui::fonts::DEFAULT_FAMILY,
        creamui::FontWeight::Regular,
        include_bytes!("../../../demo/showcase/assets/LiberationSans-Regular.ttf"),
    )
    .expect("bundled demo font");
    app().run();
}

#[cfg(all(feature = "platform-android", target_os = "android"))]
#[no_mangle]
fn android_main(android_app: creamui::AndroidApp) {
    app().run_android(android_app);
}
