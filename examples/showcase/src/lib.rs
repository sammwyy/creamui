//! A showcase with feature demos above a gallery of raw and themed
//! components — inputs, selection controls, feedback, navigation, and overlays.
//!
//! Related sidebar pages live under `panels/`; `common.rs`
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

use creamui_chart::PlotController;
use creamui_core::{Styled, Widget};
use creamui_data_table::DataTableController;
use creamui_diff_viewer::DiffController;
use creamui_grid_layout::{GridCell, GridController};
use creamui_otp_input::OtpController;
use creamui_widgets::{
    RawSheet, Sheet, SheetEdge, SplitState, ToastController, ToastHost, ToolbarController,
    TooltipController, VirtualListState,
};
use panels::*;
use prelude::*;

type ReadyCallback = Box<dyn FnOnce(WindowHandle)>;
type ViewBuilder = Box<dyn Fn(Size) -> BoxedWidget>;

/// Builds the showcase callbacks shared by the desktop, browser, and Android hosts.
fn build_showcase() -> (WindowOptions, Color, ReadyCallback, ViewBuilder) {
    let image_png = ImageData::from_bytes(include_bytes!("../assets/images/iridescent.png"))
        .expect("bundled PNG should decode");
    let image_jpeg = ImageData::from_bytes(include_bytes!("../assets/images/still-life.jpg"))
        .expect("bundled JPEG should decode");
    let image_webp = ImageData::from_bytes(include_bytes!("../assets/images/botanical.webp"))
        .expect("bundled WebP should decode");
    let theme_mode = Signal::new(ThemeMode::Dark);
    let accent_index = Signal::new(0usize);
    let active_section = Signal::new(ShowcasePage::Appearance);
    let hooks_counter = Signal::new(0i32);
    let hooks_enabled = Signal::new(true);
    let theme_raw_clicks = Signal::new(0usize);
    let event_clicks = Signal::new(0usize);
    let event_last = Signal::new("No events yet".to_owned());
    let event_input = TextController::default();
    let sidebar_open = Signal::new(false);

    let plain = TextController::default();
    let with_placeholder = TextController::default();
    let notes =
        TextController::new("Every control on this page reads its colors from the current Theme.");
    let notes_wrapped = TextController::new(
        "This one sets wrap={true}: long lines break onto a new row instead of scrolling past the edge.",
    );
    notes.set_cursor(0);
    notes_wrapped.set_cursor(0);
    let picker_date_time = DateTimeController::new(DateTime::new(2026, 9, 8, 14, 30));
    let picker_color = Signal::new(Color::rgb(181, 139, 255));
    let picker_color_popup = ColorPickerController::default();
    let picker_file = Signal::new(String::new());
    let picker_file_prompt = creamui_widgets::FilePickerController::default();

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
    let components_expanded = Signal::new(false);
    let components_accordion = Signal::new(vec![true, false]);
    let components_page = Signal::new(6usize);
    let components_alert = Signal::new(true);
    let components_submissions = Signal::new(0usize);
    let components_input = TextController::default();
    let components_step = Signal::new(1usize);
    let components_tag = Signal::new(false);
    let components_tag_visible = Signal::new(true);
    let components_group_input = TextController::default();
    let components_raw_group = Signal::new(String::new());
    let components_pressed = Signal::new(false);
    let components_menu_open = Signal::new(false);
    let components_action_count = Signal::new(0usize);
    let components_number = Signal::new(2.5f64);
    let components_number_text = TextController::new("2.5");
    let components_raw_number_text = TextController::new("3");
    let components_raw_number = Signal::new(3.0f64);
    let components_month = Signal::new(DateTime::new(2026, 3, 1, 0, 0));
    let components_date = Signal::new(Some(DateTime::new(2026, 3, 15, 0, 0)));
    let components_rating = Signal::new(3.5f32);
    let components_carousel = Signal::new(0usize);
    let components_otp = OtpController::new(6, "12");
    let components_raw_otp = OtpController::new(4, "");
    let components_otp_updates = Signal::new(0usize);
    let components_toasts = ToastController::new();
    let components_raw_toast_visible = Signal::new(true);
    let components_tooltip = TooltipController::default();
    let components_raw_tooltip = TooltipController::default();
    let components_panels = vec![SplitState::new(0.32), SplitState::new(0.55)];
    let components_raw_panels = SplitState::new(0.4);
    let components_grid = GridController::new(vec![
        GridCell::new("summary", 0, 0),
        GridCell::new("activity", 1, 0),
        GridCell::new("metrics", 0, 1).span(2, 1),
    ]);
    let components_raw_grid = GridController::new(vec![
        GridCell::new("left", 0, 0),
        GridCell::new("right", 1, 0),
    ]);
    let components_data_table = DataTableController::new(3);
    let components_table_scroll = ScrollController::new(0.0);
    let components_table_virtual = VirtualListState::new(8, 32.0);
    let components_raw_table_scroll = ScrollController::new(0.0);
    let components_raw_table_virtual = VirtualListState::new(8, 32.0);
    let components_sheet_open = Signal::new(false);
    let components_raw_sheet_open = Signal::new(false);
    let components_sheet_scroll = ScrollController::new(0.0);
    let components_raw_sheet_scroll = ScrollController::new(0.0);
    let components_toolbar = ToolbarController::new();
    let components_raw_toolbar = ToolbarController::new();
    let components_toolbar_actions = Signal::new(0usize);
    let components_chart = PlotController::new();
    let components_raw_plot = PlotController::new();
    let components_diff = DiffController::new();
    let components_diff_state = VirtualListState::new(8, 26.0);
    let components_diff_scroll = ScrollController::new(0.0);
    let components_raw_diff = DiffController::new();
    let components_raw_diff_state = VirtualListState::new(8, 26.0);
    let components_raw_diff_scroll = ScrollController::new(0.0);

    // Kept alive for the window's whole lifetime (`launch` doesn't return
    // until `run` does) so the effect it holds keeps reacting; dropping an
    // `Effect` unsubscribes it.
    let theme_sync: Rc<RefCell<Option<Effect>>> = Rc::new(RefCell::new(None));

    (
        WindowOptions {
            title: "CreamUI — Showcase".into(),
            width: 1080,
            height: 740,
            theme: build_theme(theme_mode.peek(), ACCENTS[accent_index.peek()].1),
            ..Default::default()
        },
        Theme::dark().surface,
        Box::new({
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
        }),
        Box::new(move |viewport: Size| -> BoxedWidget {
            creamui_reactive::provide_context(picker_file_prompt.clone());
            components_tooltip.set_viewport(viewport);
            components_raw_tooltip.set_viewport(viewport);
            // Browser event loops return from `run`, so the build closure owns
            // the theme effect for the lifetime of the window.
            let _theme_sync = &theme_sync;
            let theme = use_theme();
            let screen = use_screen_class();
            let safe = use_safe_area().clamp(viewport.width, viewport.height);
            let collapsed_navigation = screen != ScreenClass::Expanded;
            let compact = screen == ScreenClass::Compact;
            let outer_padding = if compact { 12. } else { 20. };
            let panel_padding = if compact { 18. } else { 28. };
            // The left rail owns the left band. The top bar owns the top band
            // only while it is on screen; the persistent rail has to clear
            // the status bar itself.
            let rail_insets = SafeArea {
                top: safe.top,
                bottom: safe.bottom,
                left: safe.left,
                right: 0.0,
            };

            let root_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                align_items: Some(AlignItems::Stretch),
                ..if collapsed_navigation {
                    column(0.0)
                } else {
                    row(0.0)
                }
            };

            let content_pad_top = outer_padding + if collapsed_navigation { 0.0 } else { safe.top };
            let content_pad_bottom = outer_padding + safe.bottom;
            let content_pad_left =
                outer_padding + if collapsed_navigation { safe.left } else { 0.0 };
            let content_pad_right = outer_padding + safe.right;
            let content_outer_style = Style {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Percent(1.0),
                },
                align_items: Some(AlignItems::Center),
                padding: creamui_core::layout::Rect {
                    left: creamui_core::layout::LengthPercentage::Length(content_pad_left),
                    right: creamui_core::layout::LengthPercentage::Length(content_pad_right),
                    top: creamui_core::layout::LengthPercentage::Length(content_pad_top),
                    bottom: creamui_core::layout::LengthPercentage::Length(content_pad_bottom),
                },
                ..column(0.0)
            };
            let content_style = padding(
                Style {
                    flex_grow: 0.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    max_size: creamui_core::layout::Size {
                        width: Dimension::Length(1120.),
                        height: Dimension::Auto,
                    },
                    ..column(0.0)
                },
                panel_padding,
            );

            // Only build the visible page. The reactive runtime removes
            // dependencies from the prior execution before collecting the
            // active branch's signals, so a hidden editor or progress demo
            // cannot invalidate this window or make us lay it out again.
            let panel = match active_section.get() {
                ShowcasePage::Appearance => AppearancePanel(AppearancePanelProps {
                    mode: theme_mode.clone(),
                    accent_index: accent_index.clone(),
                }),
                ShowcasePage::Theming => ThemingPanel(ThemingPanelProps {
                    mode: theme_mode.clone(),
                    accent_index: accent_index.clone(),
                    raw_clicks: theme_raw_clicks.clone(),
                }),
                ShowcasePage::Hooks => HooksPanel(HooksPanelProps {
                    counter: hooks_counter.clone(),
                    enabled: hooks_enabled.clone(),
                }),
                ShowcasePage::Events => EventsPanel(EventsPanelProps {
                    clicks: event_clicks.clone(),
                    last_event: event_last.clone(),
                    input: event_input.clone(),
                }),
                ShowcasePage::Typography => TypographyPanel(TypographyPanelProps {
                    link_clicks: typography_link_clicks.clone(),
                }),
                ShowcasePage::Input => InputPanel(InputPanelProps {
                    plain: plain.clone(),
                    with_placeholder: with_placeholder.clone(),
                    notes: notes.clone(),
                    notes_wrapped: notes_wrapped.clone(),
                }),
                ShowcasePage::Pickers => PickersPanel(PickersPanelProps {
                    date_time: picker_date_time.clone(),
                    color: picker_color.clone(),
                    color_picker: picker_color_popup.clone(),
                    file: picker_file.clone(),
                }),
                ShowcasePage::Images => ImagesPanel(ImagesPanelProps {
                    png: image_png.clone(),
                    jpeg: image_jpeg.clone(),
                    webp: image_webp.clone(),
                }),
                ShowcasePage::Button => ButtonPanel(ButtonPanelProps {
                    clicks: clicks.clone(),
                }),
                ShowcasePage::Slider => SliderPanel(SliderPanelProps {
                    volume: volume.clone(),
                    brightness: brightness.clone(),
                    zoom: zoom.clone(),
                }),
                ShowcasePage::Checkbox => CheckboxPanel(CheckboxPanelProps {
                    notifications: notifications.clone(),
                    auto_save: auto_save.clone(),
                    beta_features: beta_features.clone(),
                }),
                ShowcasePage::Selection => SelectionPanel(SelectionPanelProps {
                    select: select.clone(),
                    radio: radio.clone(),
                    segment: segment.clone(),
                    list_scroll: list_scroll_demo.clone(),
                    list_selected: list_selected.clone(),
                }),
                ShowcasePage::Feedback => FeedbackPanel(FeedbackPanelProps {
                    progress: progress.clone(),
                    show_popover: show_popover.clone(),
                    show_alert: show_alert.clone(),
                }),
                ShowcasePage::Sidebar => SidebarPanel(SidebarPanelProps {
                    active: sidebar_demo_active.clone(),
                }),
                ShowcasePage::Tabs => TabsPanel(TabsPanelProps {
                    filled: tabs_filled.clone(),
                    pill: tabs_pill.clone(),
                    indicator: tabs_indicator.clone(),
                    content: tabs_content.clone(),
                }),
                ShowcasePage::Scroll => ScrollPanel(ScrollPanelProps {
                    themed_scroll: themed_scroll_demo.clone(),
                    custom_scroll: custom_scroll_demo.clone(),
                }),
                ShowcasePage::Tree => TreePanel(TreePanelProps {
                    tree_scroll: tree_scroll_demo.clone(),
                    tree: tree_demo.clone(),
                }),
                ShowcasePage::Table => TablePanel(TablePanelProps {
                    table_scroll: table_scroll_demo.clone(),
                    table_selected: table_selected.clone(),
                }),
                ShowcasePage::Grid => GridPanel(),
                ShowcasePage::Flex => FlexPanel(),
                ShowcasePage::Disclosure
                | ShowcasePage::States
                | ShowcasePage::Forms
                | ShowcasePage::Pagination
                | ShowcasePage::Workspace
                | ShowcasePage::Tags
                | ShowcasePage::InputGroups
                | ShowcasePage::Actions
                | ShowcasePage::NumberInput
                | ShowcasePage::Calendar
                | ShowcasePage::Rating
                | ShowcasePage::Carousel
                | ShowcasePage::Verification
                | ShowcasePage::Notifications
                | ShowcasePage::Tooltip
                | ShowcasePage::Panels
                | ShowcasePage::DataTable
                | ShowcasePage::Sheets
                | ShowcasePage::Toolbar
                | ShowcasePage::Chart
                | ShowcasePage::Diff => ComponentsPanel(ComponentsPanelProps {
                    section: active_section.get(),
                    expanded: components_expanded.clone(),
                    accordion: components_accordion.clone(),
                    page: components_page.clone(),
                    visible: components_alert.clone(),
                    submissions: components_submissions.clone(),
                    input: components_input.clone(),
                    step: components_step.clone(),
                    tag: components_tag.clone(),
                    tag_visible: components_tag_visible.clone(),
                    group_input: components_group_input.clone(),
                    raw_group: components_raw_group.clone(),
                    pressed: components_pressed.clone(),
                    menu_open: components_menu_open.clone(),
                    action_count: components_action_count.clone(),
                    number: components_number.clone(),
                    number_text: components_number_text.clone(),
                    raw_number: components_raw_number.clone(),
                    raw_number_text: components_raw_number_text.clone(),
                    month: components_month.clone(),
                    date: components_date.clone(),
                    rating: components_rating.clone(),
                    carousel: components_carousel.clone(),
                    otp: components_otp.clone(),
                    raw_otp: components_raw_otp.clone(),
                    otp_updates: components_otp_updates.clone(),
                    toasts: components_toasts.clone(),
                    raw_toast_visible: components_raw_toast_visible.clone(),
                    tooltip: components_tooltip.clone(),
                    raw_tooltip: components_raw_tooltip.clone(),
                    panels: components_panels.clone(),
                    raw_panels: components_raw_panels.clone(),
                    grid: components_grid.clone(),
                    raw_grid: components_raw_grid.clone(),
                    data_table: components_data_table.clone(),
                    table_scroll: components_table_scroll.clone(),
                    table_virtual: components_table_virtual.clone(),
                    raw_table_scroll: components_raw_table_scroll.clone(),
                    raw_table_virtual: components_raw_table_virtual.clone(),
                    sheet_open: components_sheet_open.clone(),
                    raw_sheet_open: components_raw_sheet_open.clone(),
                    toolbar: components_toolbar.clone(),
                    raw_toolbar: components_raw_toolbar.clone(),
                    toolbar_actions: components_toolbar_actions.clone(),
                    chart: components_chart.clone(),
                    raw_plot: components_raw_plot.clone(),
                    diff: components_diff.clone(),
                    diff_state: components_diff_state.clone(),
                    diff_scroll: components_diff_scroll.clone(),
                    raw_diff: components_raw_diff.clone(),
                    raw_diff_state: components_raw_diff_state.clone(),
                    raw_diff_scroll: components_raw_diff_scroll.clone(),
                }),
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
            let panel_host_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                align_items: Some(AlignItems::Center),
                ..column(0.0)
            };
            let panel_host: BoxedWidget = Box::new(jsx! {
                <RawView style={panel_host_style} children={vec![panel_surface]} />
            });
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
                    children={vec![panel_host]}
                />
            };
            let titlebar: BoxedWidget = if collapsed_navigation {
                let open_sidebar = sidebar_open.clone();
                let title_style = Style {
                    flex_shrink: 0.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(56. + safe.top),
                    },
                    align_items: Some(AlignItems::Center),
                    padding: creamui_core::layout::Rect {
                        left: creamui_core::layout::LengthPercentage::Length(12. + safe.left),
                        right: creamui_core::layout::LengthPercentage::Length(12. + safe.right),
                        top: creamui_core::layout::LengthPercentage::Length(8. + safe.top),
                        bottom: creamui_core::layout::LengthPercentage::Length(8.),
                    },
                    ..row(10.)
                };
                let title_label = active_section.get().label().to_owned();
                let title_text_style = Style {
                    flex_grow: 1.0,
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..Default::default()
                };
                Box::new(jsx! {
                    <RawView style={title_style} background={theme.surface_elevated}>
                        <StyledButton variant={ButtonVariant::Secondary} size={ButtonSize::Sm} label={"Menu".to_owned()} state={ButtonState::Normal} on_click={Box::new(move || open_sidebar.set(true)) as Box<dyn Fn()>} disabled={false} />
                        <RawView style={title_text_style}>
                            <BoldText text={title_label} color={theme.text_primary} font_size={16.0} align={TextAlign::Start} />
                        </RawView>
                    </RawView>
                })
            } else {
                Box::new(RawView::new(Style::default()))
            };
            let desktop_nav: BoxedWidget = if collapsed_navigation {
                Box::new(RawView::new(Style::default()))
            } else {
                jsx! {
                    <Nav active={active_section.clone()} content_scroll={content_scroll.clone()} nav_scroll={nav_scroll.clone()} sidebar_open={sidebar_open.clone()} width={232.} insets={rail_insets} />
                }
            };
            let sidebar_drawer: BoxedWidget = if collapsed_navigation && sidebar_open.get() {
                let dismiss = sidebar_open.clone();
                let preferred: f32 = if compact { 288. } else { 320. };
                let available = (viewport.width - safe.left - safe.right).max(0.0);
                // Leave a strip of the dimmed page visible so the drawer can
                // be dismissed without covering the whole window.
                let drawer_width = if available <= 48.0 {
                    available
                } else {
                    preferred.min(available - 36.0)
                };
                let backdrop_style = Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Length(0.),
                        right: LengthPercentageAuto::Length(0.),
                        top: LengthPercentageAuto::Length(0.),
                        bottom: LengthPercentageAuto::Length(0.),
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Percent(1.0),
                    },
                    ..column(0.)
                };
                let drawer_style = Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Length(0.),
                        right: LengthPercentageAuto::Auto,
                        top: LengthPercentageAuto::Length(0.),
                        bottom: LengthPercentageAuto::Length(0.),
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(drawer_width),
                        height: Dimension::Percent(1.0),
                    },
                    ..column(0.)
                };
                let drawer_nav = jsx! {
                    <Nav active={active_section.clone()} content_scroll={content_scroll.clone()} nav_scroll={nav_scroll.clone()} sidebar_open={sidebar_open.clone()} width={drawer_width} insets={rail_insets} />
                };
                Box::new(
                    Overlay::new(backdrop_style, move || dismiss.set(false))
                        .child(Box::new(Popover::new(drawer_style).child(drawer_nav))),
                )
            } else {
                Box::new(RawView::new(Style::default()))
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
            let file_prompt: BoxedWidget =
                Box::new(creamui_widgets::FilePrompt::new(&picker_file_prompt));
            let toast_host: BoxedWidget = Box::new(ToastHost::new(&components_toasts));
            let sheet_layer: BoxedWidget = if components_sheet_open.get() {
                let dismiss = components_sheet_open.clone();
                let content = RawView::new(
                    Style::default()
                        .flex_column()
                        .gap(theme.spacing_medium)
                        .padding_all(theme.spacing_medium),
                )
                .child(Box::new(RawText::new(
                    "Workspace details remain scrollable inside the sheet.",
                    theme.text_primary,
                    theme.typography.body,
                )))
                .child(Box::new(RawText::new(
                    "The backdrop or Escape dismisses this modal panel.",
                    theme.text_secondary,
                    theme.typography.body,
                )));
                let content = (0..60).fold(content, |content, index| {
                    content.child(Box::new(RawText::new(
                        format!("Detail item {}", index + 1),
                        theme.text_secondary,
                        theme.typography.body,
                    )))
                });
                Box::new(
                    Sheet::new(
                        SheetEdge::Right,
                        components_sheet_scroll.clone(),
                        Box::new(content),
                        move || dismiss.set(false),
                    )
                    .title("Sheet details")
                    .extent(320.0),
                )
            } else if components_raw_sheet_open.get() {
                let dismiss = components_raw_sheet_open.clone();
                let content = RawView::new(
                    Style::default()
                        .flex_column()
                        .gap(theme.spacing_medium)
                        .padding_all(theme.spacing_medium),
                )
                .child(Box::new(RawText::new(
                    "Raw bottom sheet",
                    theme.text_primary,
                    theme.typography.body,
                )))
                .child(Box::new(
                    creamui_widgets::RawButton::new(Style::default(), move || dismiss.set(false))
                        .child(Box::new(RawText::new(
                            "Close raw sheet",
                            theme.accent,
                            theme.typography.body,
                        ))),
                ));
                let dismiss = components_raw_sheet_open.clone();
                Box::new(
                    RawSheet::new(
                        Style::default(),
                        components_raw_sheet_scroll.clone(),
                        SheetEdge::Bottom,
                        Box::new(content),
                        move || dismiss.set(false),
                    )
                    .panel_style(
                        RawView::new(Style::default())
                            .background(theme.surface_elevated)
                            .style(),
                    )
                    .extent(180.0)
                    .modal(false),
                )
            } else {
                Box::new(RawView::new(Style::default()))
            };

            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    {titlebar}
                    {desktop_nav}
                    <RawView style={content_outer_style} background={theme.surface}>
                        {content}
                    </RawView>
                    {sidebar_drawer}
                    {dialog}
                    {file_prompt}
                    {toast_host}
                    {sheet_layer}
                </RawView>
            })
        }),
    )
}

/// Starts the native showcase, or the browser canvas when built for WASM.
pub fn launch() {
    let (options, clear_color, on_window_ready, build_ui) = build_showcase();
    run(options, clear_color, on_window_ready, build_ui);
}

/// Starts the showcase in an Android NativeActivity.
#[cfg(target_os = "android")]
pub fn launch_android(app: android_activity::AndroidApp) {
    let (options, clear_color, on_window_ready, build_ui) = build_showcase();
    creamui_render::run_android(app, options, clear_color, on_window_ready, build_ui);
}
