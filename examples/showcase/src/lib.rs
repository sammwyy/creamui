//! A component showcase: a sidebar switches between a live theme editor
//! ("Appearance") and a gallery view for every themed control CreamUI ships
//! with — inputs, selection controls, feedback, navigation, and overlays.
//!
//! Each sidebar section lives in its own file under `panels/`; `common.rs`
//! holds the shared design tokens/layout helpers, `nav.rs` the sidebar rail
//! itself, and `prelude.rs` the one import every one of those files starts
//! with.
//!
//! The whole window is driven by a handful of small signals — `theme_mode`,
//! `accent_index`, `active_section`, plus one signal per interactive control
//! — so picking a new accent color or theme mode re-renders
//! every panel with the new `Theme` immediately, the same reactive path any
//! other `Signal` change takes.
//!
//! The derived theme itself flows through `use_theme()`: an effect set up
//! in `on_window_ready` watches `theme_mode`/`accent_index` and pushes the
//! recomputed `Theme` via `WindowHandle::set_theme`, so every panel below
//! reads it with `use_theme()` instead of recomputing it locally.

mod common;
mod kit;
mod nav;
mod panels;
mod prelude;

use panels::*;
use prelude::*;

/// Starts the native showcase, or the browser canvas when built for WASM.
pub fn launch() {
    let image_png = ImageData::from_bytes(include_bytes!("../assets/images/iridescent.png"))
        .expect("bundled PNG should decode");
    let image_jpeg = ImageData::from_bytes(include_bytes!("../assets/images/still-life.jpg"))
        .expect("bundled JPEG should decode");
    let image_webp = ImageData::from_bytes(include_bytes!("../assets/images/botanical.webp"))
        .expect("bundled WebP should decode");
    let theme_mode = Signal::new(ThemeMode::Dark);
    let accent_index = Signal::new(0usize);
    let active_section = Signal::new(0usize);

    let plain = TextController::default();
    let with_placeholder = TextController::default();
    let notes =
        TextController::new("Every control on this page reads its colors from the current Theme.");
    let notes_wrapped = TextController::new(
        "This one sets wrap={true}: long lines break onto a new row instead of scrolling past the edge.",
    );
    let picker_date_time = DateTimeController::new(DateTime::new(2026, 9, 8, 14, 30));
    let picker_color = Signal::new(Color::rgb(181, 139, 255));
    let picker_color_popup = ColorPickerController::default();
    let picker_file = Signal::new(String::new());

    let clicks = Signal::new(0i32);
    let typography_link_clicks = Signal::new(0i32);

    let volume = Signal::new(0.6f32);
    let brightness = Signal::new(0.8f32);
    let zoom = Signal::new(0.3f32);

    let notifications = Signal::new(true);
    let auto_save = Signal::new(false);
    let beta_features = Signal::new(false);

    let select = SelectController::default();
    let radio = Signal::new(0usize);
    let segment = Signal::new(1usize);
    let progress = Signal::new(0.62f32);
    let show_popover = Signal::new(false);
    let show_alert = Signal::new(false);

    let sidebar_demo_active = Signal::new(0usize);
    let tabs_filled = TabController::default();
    let tabs_pill = TabController::new(1);
    let tabs_indicator = TabController::new(2);
    let tabs_content = TabController::default();
    let content_scroll = ScrollController::default();
    let nav_scroll = ScrollController::default();
    let themed_scroll_demo = ScrollController::default();
    let custom_scroll_demo = ScrollController::default();
    let list_scroll_demo = ScrollController::default();
    let list_selected = Signal::new(0usize);
    let tree_scroll_demo = ScrollController::default();
    let tree_demo = TreeController::default();
    let table_scroll_demo = ScrollController::default();
    let table_selected = Signal::new(0usize);

    // Kept alive for the window's whole lifetime (`launch` doesn't return
    // until `run` does) so the effect it holds keeps reacting; dropping an
    // `Effect` unsubscribes it.
    let theme_sync: Rc<RefCell<Option<Effect>>> = Rc::new(RefCell::new(None));

    run(
        WindowOptions {
            title: "CreamUI — Showcase".into(),
            width: 1080,
            height: 740,
            theme: build_theme(theme_mode.peek(), ACCENTS[accent_index.peek()].1),
            ..Default::default()
        },
        Theme::dark().surface,
        {
            let theme_mode = theme_mode.clone();
            let accent_index = accent_index.clone();
            let theme_sync = theme_sync.clone();
            move |handle: WindowHandle| {
                let theme_mode = theme_mode.clone();
                let accent_index = accent_index.clone();
                *theme_sync.borrow_mut() = Some(create_effect(move || {
                    handle.set_theme(build_theme(theme_mode.get(), ACCENTS[accent_index.get()].1));
                }));
            }
        },
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();

            let root_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                align_items: Some(AlignItems::Stretch),
                ..row(0.0)
            };

            let content_outer_style = padding(
                Style {
                    flex_grow: 1.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Percent(1.0),
                    },
                    ..column(0.0)
                },
                24.,
            );
            let content_style = padding(
                Style {
                    flex_grow: 0.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    ..column(0.0)
                },
                28.,
            );

            // Only build the visible page. The reactive runtime removes
            // dependencies from the prior execution before collecting the
            // active branch's signals, so a hidden editor or progress demo
            // cannot invalidate this window or make us lay it out again.
            let panel = match active_section.get() {
                0 => AppearancePanel(AppearancePanelProps {
                    mode: theme_mode.clone(),
                    accent_index: accent_index.clone(),
                }),
                1 => TypographyPanel(TypographyPanelProps {
                    link_clicks: typography_link_clicks.clone(),
                }),
                2 => InputPanel(InputPanelProps {
                    plain: plain.clone(),
                    with_placeholder: with_placeholder.clone(),
                    notes: notes.clone(),
                    notes_wrapped: notes_wrapped.clone(),
                }),
                3 => PickersPanel(PickersPanelProps {
                    date_time: picker_date_time.clone(),
                    color: picker_color.clone(),
                    color_picker: picker_color_popup.clone(),
                    file: picker_file.clone(),
                }),
                4 => ImagesPanel(ImagesPanelProps {
                    png: image_png.clone(),
                    jpeg: image_jpeg.clone(),
                    webp: image_webp.clone(),
                }),
                5 => ButtonPanel(ButtonPanelProps {
                    clicks: clicks.clone(),
                }),
                6 => SliderPanel(SliderPanelProps {
                    volume: volume.clone(),
                    brightness: brightness.clone(),
                    zoom: zoom.clone(),
                }),
                7 => CheckboxPanel(CheckboxPanelProps {
                    notifications: notifications.clone(),
                    auto_save: auto_save.clone(),
                    beta_features: beta_features.clone(),
                }),
                8 => SelectionPanel(SelectionPanelProps {
                    select: select.clone(),
                    radio: radio.clone(),
                    segment: segment.clone(),
                    list_scroll: list_scroll_demo.clone(),
                    list_selected: list_selected.clone(),
                }),
                9 => FeedbackPanel(FeedbackPanelProps {
                    progress: progress.clone(),
                    show_popover: show_popover.clone(),
                    show_alert: show_alert.clone(),
                }),
                10 => SidebarPanel(SidebarPanelProps {
                    active: sidebar_demo_active.clone(),
                }),
                11 => TabsPanel(TabsPanelProps {
                    filled: tabs_filled.clone(),
                    pill: tabs_pill.clone(),
                    indicator: tabs_indicator.clone(),
                    content: tabs_content.clone(),
                }),
                12 => ScrollPanel(ScrollPanelProps {
                    themed_scroll: themed_scroll_demo.clone(),
                    custom_scroll: custom_scroll_demo.clone(),
                }),
                13 => TreePanel(TreePanelProps {
                    tree_scroll: tree_scroll_demo.clone(),
                    tree: tree_demo.clone(),
                }),
                14 => TablePanel(TablePanelProps {
                    table_scroll: table_scroll_demo.clone(),
                    table_selected: table_selected.clone(),
                }),
                15 => GridPanel(),
                _ => FlexPanel(),
            };

            let scroll_style = Style {
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                ..Default::default()
            };
            let panel_surface: BoxedWidget = jsx! {
                <Surface role={SurfaceRole::Panel} style={content_style} children={vec![panel]} />
            };
            let content: BoxedWidget = jsx! {
                <RawScrollView
                    style={scroll_style}
                    controller={content_scroll.clone()}
                    content_gap={None}
                    scrollbar_gap={None}
                    background={None}
                    corner_radius={None}
                    scrollbar_width={None}
                    scrollbar_color={None}
                    scrollbar_hover_color={None}
                    children={vec![panel_surface]}
                />
            };
            let dialog: BoxedWidget = if show_alert.get() {
                let dismiss = show_alert.clone();
                let confirm_dismiss = show_alert.clone();
                jsx! {
                    <AlertDialog
                        title={"Delete this draft?".to_owned()}
                        message={"This example uses an Overlay backdrop. Clicking outside or Cancel closes it.".to_owned()}
                        on_dismiss={Box::new(move || dismiss.set(false)) as Box<dyn Fn()>}
                        dismiss_label={"Cancel".to_owned()}
                        confirm_label={"Delete".to_owned()}
                        on_confirm={Box::new(move || confirm_dismiss.set(false)) as Box<dyn Fn()>}
                    />
                }
            } else {
                Box::new(jsx! { <RawView style={Style::default()} /> })
            };

            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    <Nav active={active_section.clone()} content_scroll={content_scroll.clone()} nav_scroll={nav_scroll.clone()} />
                    <RawView style={content_outer_style} background={theme.surface}>
                        {content}
                    </RawView>
                    {dialog}
                </RawView>
            })
        },
    );
}
