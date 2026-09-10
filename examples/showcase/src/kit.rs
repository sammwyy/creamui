//! Thin `#[component]` wrappers around widgets whose builder API isn't part
//! of `jsx!`'s fixed intrinsic set, so every panel can reach them as a plain
//! JSX tag instead of calling the widget's raw builder directly.

use crate::prelude::*;

#[component]
pub fn Card(gap: f32, children: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        creamui_widgets::Surface::new(
            SurfaceRole::Inset,
            padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    ..column(gap)
                },
                theme.spacing_large,
            ),
        )
        .with_children(children),
    )
}

#[component]
pub fn Surface(role: SurfaceRole, style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::Surface::new(role, style).with_children(children))
}

/// The themed, elevated `Card` widget (distinct from [`Card`] above, which
/// wraps the flatter `Surface`/`SurfaceRole::Inset` combination).
#[component]
pub fn PreviewCard(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::Card::new(style).with_children(children))
}

#[component]
pub fn FieldLabel(text: String) -> BoxedWidget {
    Box::new(
        Text::secondary(text)
            .align(TextAlign::Start)
            .style(label_style()),
    )
}

#[component]
pub fn StackedField(label: String, control: BoxedWidget) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <RawView style={column(theme.spacing_small)}>
            <FieldLabel text={label} />
            {control}
        </RawView>
    })
}

#[component]
pub fn FieldCard(label: String, control: BoxedWidget) -> BoxedWidget {
    let theme = use_theme();
    jsx! {
        <Card gap={theme.spacing_small}>
            <FieldLabel text={label} />
            {control}
        </Card>
    }
}

#[component]
pub fn CardRow(children: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(Style {
            align_items: Some(AlignItems::Stretch),
            ..row(theme.spacing_large)
        })
        .with_children(children),
    )
}

/// A bold [`RawText`] run — `bold` isn't one of `jsx!`'s `RawText` props.
#[component]
pub fn BoldText(text: String, color: Color, font_size: f32, align: TextAlign) -> BoxedWidget {
    Box::new(RawText::new(text, color, font_size).bold(true).align(align))
}

#[component]
pub fn Icon(symbol: Symbol, color: Color, size: f32) -> BoxedWidget {
    Box::new(creamui_widgets::Icon::new(symbol, color).size(size))
}

#[component]
pub fn Choice(label: String, active: bool, on_click: Box<dyn Fn()>) -> BoxedWidget {
    Box::new(creamui_widgets::Choice::new(label, active, on_click))
}

#[component]
pub fn Link(text: String, on_click: Box<dyn Fn()>) -> BoxedWidget {
    Box::new(creamui_widgets::Link::new(text, on_click))
}

#[component]
pub fn Quote(text: String) -> BoxedWidget {
    Box::new(creamui_widgets::Quote::new(text))
}

#[component]
pub fn Pre(code: String) -> BoxedWidget {
    Box::new(creamui_widgets::Pre::new(code))
}

#[component]
pub fn ProgressBar(value: Option<f32>) -> BoxedWidget {
    Box::new(match value {
        Some(value) => creamui_widgets::ProgressBar::new(value),
        None => creamui_widgets::ProgressBar::indeterminate(),
    })
}

#[component]
pub fn ProgressRing(value: Option<f32>, size: f32) -> BoxedWidget {
    let ring = match value {
        Some(value) => creamui_widgets::ProgressRing::new(value),
        None => creamui_widgets::ProgressRing::indeterminate(),
    };
    Box::new(ring.size(size))
}

#[component]
pub fn RadioGroup(
    selected: usize,
    on_change: Box<dyn Fn(usize)>,
    options: Vec<String>,
) -> BoxedWidget {
    let mut group = creamui_widgets::RadioGroup::new(selected, on_change);
    for option in options {
        group = group.option(option);
    }
    Box::new(group)
}

#[component]
pub fn SegmentedControl(
    selected: usize,
    on_change: Box<dyn Fn(usize)>,
    options: Vec<String>,
) -> BoxedWidget {
    let mut control = creamui_widgets::SegmentedControl::new(selected, on_change);
    for option in options {
        control = control.option(option);
    }
    Box::new(control)
}

#[component]
pub fn Select(options: Vec<String>, controller: SelectController) -> BoxedWidget {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    Box::new(creamui_widgets::Select::controlled(&refs, controller))
}

#[component]
pub fn ListBox(
    style: Style,
    scroll: ScrollController,
    selected: usize,
    on_change: Box<dyn Fn(usize)>,
    options: Vec<String>,
) -> BoxedWidget {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    Box::new(creamui_widgets::ListBox::new(style, scroll, selected, on_change).options(&refs))
}

#[component]
pub fn Sidebar(colors: TabColors, style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::Sidebar::new(colors, style).with_children(children))
}

#[component]
pub fn SidebarItem(
    colors: TabColors,
    style: Style,
    label: String,
    active: bool,
    on_click: Box<dyn Fn()>,
) -> BoxedWidget {
    Box::new(creamui_widgets::SidebarItem::new(
        colors, style, label, active, on_click,
    ))
}

#[component]
pub fn NavigationItem(
    symbol: Symbol,
    label: String,
    active: bool,
    on_click: Box<dyn Fn()>,
) -> BoxedWidget {
    Box::new(creamui_widgets::NavigationItem::new(
        symbol, label, active, on_click,
    ))
}

#[component]
pub fn Popover(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::Popover::new(style).with_children(children))
}

/// The themed `ScrollView`'s controller-bound constructor — `jsx!`'s
/// `ScrollView` intrinsic only covers the uncontrolled `scroll_y`/`on_scroll`
/// form.
#[component]
pub fn ControlledScrollView(
    style: Style,
    controller: ScrollController,
    children: Vec<BoxedWidget>,
) -> BoxedWidget {
    Box::new(ScrollView::controlled(style, controller).with_children(children))
}

#[component]
pub fn RawScrollView(
    style: Style,
    controller: ScrollController,
    content_gap: Option<f32>,
    scrollbar_gap: Option<f32>,
    background: Option<Color>,
    corner_radius: Option<f32>,
    scrollbar_width: Option<f32>,
    scrollbar_color: Option<Color>,
    scrollbar_hover_color: Option<Color>,
    children: Vec<BoxedWidget>,
) -> BoxedWidget {
    let mut view = creamui_widgets::RawScrollView::controlled(style, controller);
    if let Some(value) = content_gap {
        view = view.content_gap(value);
    }
    if let Some(value) = scrollbar_gap {
        view = view.scrollbar_gap(value);
    }
    if let Some(value) = background {
        view = view.background(value);
    }
    if let Some(value) = corner_radius {
        view = view.corner_radius(value);
    }
    if let Some(value) = scrollbar_width {
        view = view.scrollbar_width(value);
    }
    if let Some(value) = scrollbar_color {
        view = view.scrollbar_color(value);
    }
    if let Some(value) = scrollbar_hover_color {
        view = view.scrollbar_hover_color(value);
    }
    Box::new(view.with_children(children))
}

#[component]
pub fn Table(
    style: Style,
    scroll: ScrollController,
    columns: Vec<TableColumn>,
    rows: Vec<Vec<String>>,
    selected: Option<usize>,
    on_row_click: Box<dyn Fn(usize)>,
) -> BoxedWidget {
    Box::new(
        creamui_widgets::Table::new(style, scroll, columns)
            .rows(rows)
            .on_row_click(selected, on_row_click),
    )
}

#[component]
pub fn TreeView(
    style: Style,
    scroll: ScrollController,
    controller: TreeController,
    nodes: Vec<TreeNode>,
) -> BoxedWidget {
    Box::new(creamui_widgets::TreeView::new(
        style, scroll, controller, &nodes,
    ))
}

#[component]
pub fn AlertDialog(
    title: String,
    message: String,
    on_dismiss: Box<dyn Fn()>,
    dismiss_label: String,
    confirm_label: String,
    on_confirm: Box<dyn Fn()>,
) -> BoxedWidget {
    Box::new(
        creamui_widgets::AlertDialog::new(title, message, on_dismiss)
            .dismiss_button(dismiss_label)
            .confirm(confirm_label, on_confirm),
    )
}

#[component]
pub fn FilePicker(
    value: String,
    on_change: Box<dyn Fn(std::path::PathBuf)>,
    title: String,
    filter_label: String,
    filter_extensions: Vec<String>,
) -> BoxedWidget {
    Box::new(
        creamui_widgets::FilePicker::new(value, on_change)
            .title(title)
            .filter(filter_label, filter_extensions),
    )
}

#[component]
pub fn Tabs(colors: TabColors, style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::Tabs::new(colors, style).with_children(children))
}

#[component]
pub fn Tab(
    colors: TabColors,
    style: Style,
    label: String,
    active: bool,
    on_click: Box<dyn Fn()>,
) -> BoxedWidget {
    Box::new(creamui_widgets::Tab::new(
        colors, style, label, active, on_click,
    ))
}

#[component]
pub fn Switch(checked: bool, on_click: Box<dyn Fn()>) -> BoxedWidget {
    Box::new(creamui_widgets::Switch::new(checked, on_click))
}

/// A `TextInput` whose border color is overridden — `border` isn't one of
/// `jsx!`'s `TextInput` props.
#[component]
pub fn BorderedInput(border: Color) -> BoxedWidget {
    Box::new(TextInput::new("", |_| {}).border(border))
}

/// One `Text` weight/decoration sample — `bold`/`italic`/`underline`/
/// `strikethrough` aren't among `jsx!`'s `Text` props.
#[component]
pub fn StyledText(
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
) -> BoxedWidget {
    let mut widget = Text::new(text).align(TextAlign::Start);
    if bold {
        widget = widget.bold(true);
    }
    if italic {
        widget = widget.italic(true);
    }
    if underline {
        widget = widget.underline(true);
    }
    if strikethrough {
        widget = widget.strikethrough(true);
    }
    Box::new(widget)
}

/// Every themed `Button` shape the showcase needs: `Button::new`,
/// `::secondary`, `::state`, and `::styled` are all just `::styled` with
/// different defaults, so one wrapper covers them all.
#[component]
pub fn StyledButton(
    variant: ButtonVariant,
    size: ButtonSize,
    label: String,
    state: ButtonState,
    on_click: Box<dyn Fn()>,
    disabled: bool,
) -> BoxedWidget {
    Box::new(Button::styled(variant, size, label, state, on_click).disabled(disabled))
}
