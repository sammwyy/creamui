//! A calm desktop editor built entirely with static CreamUI JSX.
//!
//! `TextArea` is also available through `abi_jsx!` / `creamui_dynamic`, so
//! this screen is a useful native reference for applications shipped over
//! the dynamic ABI.

use creamui_core::layout::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentageAuto, Position, Style,
};
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_macros::{component, jsx};
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{fixed, row};

/// App-level semantic tokens. Numeric layout decisions live here rather than
/// being scattered through JSX, just as a CSS design system centralizes
/// custom properties and component rules.
#[derive(Clone, Copy)]
struct EditorTokens {
    theme: Theme,
    active_line: Color,
    menu_height: f32,
    menu_trigger_width: f32,
    menu_item_height: f32,
    gutter_width: f32,
}

impl EditorTokens {
    fn from_theme(theme: Theme) -> Self {
        Self {
            active_line: theme.surface_hover,
            theme,
            menu_height: 28.0,
            menu_trigger_width: 42.0,
            menu_item_height: 25.0,
            gutter_width: 58.0,
        }
    }

    fn menu_colors(self) -> creamui_widgets::MenuColors {
        creamui_widgets::MenuColors::dark()
    }
}

/// Only callable during `build_ui` (uses `use_theme()`) — see `main`'s
/// `Theme::dark()` for the one call site before the window exists.
fn tokens() -> EditorTokens {
    EditorTokens::from_theme(use_theme())
}

fn size(width: f32, height: f32) -> Style {
    Style {
        size: fixed(width, height),
        ..Default::default()
    }
}

#[component]
fn ToolbarMenu(label: String, id: i32, active: Signal<i32>) -> BoxedWidget {
    let tokens = tokens();
    let is_active = active.get() == id;
    let click_active = active.clone();
    let text_color = if is_active {
        tokens.theme.accent
    } else {
        tokens.theme.text_secondary
    };
    // Top-level menus are deliberately text-only. A menu bar is navigation,
    // not a row of contained buttons; the popup supplies the active affordance.
    let trigger_style = size(
        tokens.menu_trigger_width,
        tokens.menu_height - tokens.theme.spacing_small,
    );
    Box::new(jsx! {
        <RawButton style={trigger_style.clone()} on_click={move || { click_active.set(if click_active.get() == id { 0 } else { id }); }}>
            <RawText color={text_color} font_size={13.0} align={TextAlign::Start} style={trigger_style}>{label}</RawText>
        </RawButton>
    })
}

#[component]
fn EditorToolbar(
    menus: Vec<String>,
    title: String,
    active: Signal<i32>,
    children: Vec<BoxedWidget>,
) -> BoxedWidget {
    let tokens = tokens();
    let toolbar = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(tokens.menu_height),
        },
        padding: creamui_core::layout::Rect {
            left: creamui_core::layout::LengthPercentage::Length(tokens.theme.spacing_medium),
            right: creamui_core::layout::LengthPercentage::Length(tokens.theme.spacing_medium),
            top: creamui_core::layout::LengthPercentage::Length(0.0),
            bottom: creamui_core::layout::LengthPercentage::Length(0.0),
        },
        ..row(tokens.theme.spacing_medium)
    };
    let mut view = creamui_widgets::MenuBar::new(tokens.menu_colors(), toolbar);
    for (index, label) in menus.into_iter().enumerate() {
        view = view.child(ToolbarMenu(ToolbarMenuProps {
            label,
            id: index as i32 + 1,
            active: active.clone(),
        }));
    }
    let title_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(tokens.menu_height - tokens.theme.spacing_small),
        },
        flex_grow: 1.0,
        ..Default::default()
    };
    view = view.child(Box::new(
        creamui_widgets::RawText::new(title, tokens.menu_colors().muted_text, 12.0)
            .layout(title_style),
    ));
    for child in children {
        view = view.child(child);
    }
    Box::new(view)
}

fn command(label: &str, action: impl Fn() + 'static) -> BoxedWidget {
    let tokens = tokens();
    Box::new(creamui_widgets::MenuItem::new(
        tokens.menu_colors(),
        size(134.0, tokens.menu_item_height),
        label,
        false,
        action,
    ))
}

fn open_document(document: Signal<String>, status: Signal<String>) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Text", &["txt", "md", "rs"])
        .pick_file()
    {
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                document.set(contents);
                status.set(format!("Opened {}", path.display()));
            }
            Err(error) => status.set(format!("Could not open file: {error}")),
        }
    }
}

#[component]
fn MenuPanel(
    active: Signal<i32>,
    document: Signal<String>,
    saved: Signal<bool>,
    status: Signal<String>,
) -> BoxedWidget {
    let tokens = tokens();
    let open = active.get();
    // Do not paint a zero-height popup: some raster backends turn a
    // zero-height rounded rect into a one-pixel hairline below the menu bar.
    if open == 0 {
        return Box::new(creamui_widgets::RawView::new(Style::default()));
    }
    let item_count = if open == 1 { 3 } else { 0 };
    let left = tokens.theme.spacing_medium
        + (open.saturating_sub(1) as f32
            * (tokens.menu_trigger_width + tokens.theme.spacing_medium));
    let panel_style = Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(left),
            right: LengthPercentageAuto::Auto,
            top: LengthPercentageAuto::Length(tokens.menu_height),
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Length(136.0),
            height: Dimension::Length(if item_count == 0 {
                0.0
            } else {
                item_count as f32 * tokens.menu_item_height + 2.0
            }),
        },
        flex_direction: FlexDirection::Column,
        padding: creamui_core::layout::Rect {
            left: creamui_core::layout::LengthPercentage::Length(1.0),
            right: creamui_core::layout::LengthPercentage::Length(1.0),
            top: creamui_core::layout::LengthPercentage::Length(1.0),
            bottom: creamui_core::layout::LengthPercentage::Length(1.0),
        },
        ..Default::default()
    };
    let close = active.clone();
    let mut panel = creamui_widgets::MenuPopup::new(tokens.menu_colors(), panel_style);
    match open {
        1 => {
            let doc = document.clone();
            let state = status.clone();
            let close_new = close.clone();
            panel = panel.child(command("New", move || {
                doc.set(String::new());
                state.set("New document".into());
                close_new.set(0);
            }));
            let doc = document.clone();
            let state = status.clone();
            let close_open = close.clone();
            panel = panel.child(command("Open File…", move || {
                open_document(doc.clone(), state.clone());
                close_open.set(0);
            }));
            let doc = document.clone();
            let saved = saved.clone();
            let state = status.clone();
            let close_save = close.clone();
            panel = panel.child(command("Save File…", move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text", &["txt", "md"])
                    .set_file_name("Untitled.md")
                    .save_file()
                {
                    match std::fs::write(&path, doc.get()) {
                        Ok(()) => {
                            saved.set(true);
                            state.set(format!("Saved {}", path.display()));
                        }
                        Err(error) => state.set(format!("Could not save file: {error}")),
                    }
                }
                close_save.set(0);
            }));
        }
        _ => {}
    }
    Box::new(panel)
}

#[component]
fn LineNumbers(value: String) -> BoxedWidget {
    let tokens = tokens();
    let lines = value.matches('\n').count() + 1;
    let labels = (1..=lines)
        .map(|number| {
            Box::new(
                creamui_widgets::RawText::new(
                    number.to_string(),
                    tokens.theme.text_secondary,
                    14.0,
                )
                .text_align(TextAlign::End)
                .layout(size(42.0, 20.0)),
            ) as BoxedWidget
        })
        .collect();
    Box::new(
        jsx! { <RawView style={Style { size: creamui_core::layout::Size { width: Dimension::Length(tokens.gutter_width), height: Dimension::Percent(1.0) }, flex_shrink: 0.0, flex_direction: FlexDirection::Column, padding: creamui_core::layout::Rect { left: creamui_core::layout::LengthPercentage::Length(0.0), right: creamui_core::layout::LengthPercentage::Length(tokens.theme.spacing_medium + 2.0), top: creamui_core::layout::LengthPercentage::Length(14.0), bottom: creamui_core::layout::LengthPercentage::Length(0.0) }, ..Default::default() }} background={tokens.theme.surface} children={labels} /> },
    )
}

fn main() {
    let document = Signal::new("# A small thought\n\nCreamUI makes desktop interfaces feel calm.\n\nStart writing here — this is a real multiline editor.\nThe line count, word count, and character count react to each change.\n\n## Notes\n\n- Press Return for a new line\n- Backspace edits normally\n- The UI tree is declarative JSX".to_owned());
    let saved = Signal::new(false);
    let cursor = Signal::new(document.get().len());
    let selection = Signal::new(creamui_widgets::TextSelection {
        anchor: cursor.get(),
        focus: cursor.get(),
    });
    let active_menu = Signal::new(0_i32);
    let status_message = Signal::new("Markdown · UTF-8".to_owned());
    run(
        WindowOptions {
            title: "CreamUI — Text Editor".into(),
            width: 980,
            height: 680,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let tokens = tokens();
            let value = document.get();
            let lines = value.matches('\n').count() + 1;
            let words = value.split_whitespace().count();
            let chars = value.chars().count();
            let status_text = status_message.get();
            let on_change = document.clone();
            let saved_for_change = saved.clone();
            let cursor_for_change = cursor.clone();
            let selection_for_change = selection.clone();
            let open_document_value = document.clone();
            let open_document_status = status_message.clone();
            let root = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                flex_direction: FlexDirection::Column,
                ..Default::default()
            };
            let editor_row = Style {
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..row(0.0)
            };
            let area = Style {
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Percent(1.0),
                },
                ..Default::default()
            };
            let status = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(30.0),
                },
                flex_shrink: 0.0,
                padding: creamui_core::layout::Rect {
                    left: creamui_core::layout::LengthPercentage::Length(16.0),
                    right: creamui_core::layout::LengthPercentage::Length(16.0),
                    top: creamui_core::layout::LengthPercentage::Length(0.0),
                    bottom: creamui_core::layout::LengthPercentage::Length(0.0),
                },
                justify_content: Some(JustifyContent::SpaceBetween),
                align_items: Some(AlignItems::Center),
                ..Default::default()
            };
            Box::new(jsx! {
                <RawView style={root} background={tokens.theme.surface}>
                    <EditorToolbar menus={vec!["File".into()]} title={"Untitled.md".into()} active={active_menu.clone()} children={Vec::<BoxedWidget>::new()} />
                    <RawView style={editor_row}>
                        <LineNumbers value={value.clone()} />
                        <TextArea style={area} value={value.clone()} cursor={cursor.get()} on_cursor_change={move |next| cursor_for_change.set(next)} selection={selection.get()} on_selection_change={move |next| selection_for_change.set(next)} on_ctrl_o={move || open_document(open_document_value.clone(), open_document_status.clone())} on_change={move |next| { saved_for_change.set(false); on_change.set(next) }} placeholder={"Start writing…"} corner_radius={0.0} border={(tokens.theme.border, 0.0)} active_line_background={tokens.active_line} />
                    </RawView>
                    <RawView style={status} background={tokens.theme.surface}>
                        <RawText color={tokens.theme.accent} font_size={12.0} style={Style { flex_grow: 1.0, ..Default::default()}}>{status_text}</RawText>
                        <RawText color={tokens.theme.text_secondary} font_size={12.0} style={size(250.0, 24.0)} align={TextAlign::End}>{format!("{lines} lines · {words} words · {chars} characters")}</RawText>
                    </RawView>
                    <MenuPanel active={active_menu.clone()} document={document.clone()} saved={saved.clone()} status={status_message.clone()} />
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
