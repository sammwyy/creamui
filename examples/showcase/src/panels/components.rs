use crate::prelude::*;
use creamui_core::Styled;
use creamui_widgets::{
    Accordion, Alert, AlertStatus, AlertVariant, Attachment, Calendar, CalendarColors, Carousel,
    ChatMessage, Collapsible, DescriptionList, DropdownButton, EmptyState, Field, Form, InputGroup,
    Kbd, NumberInput, OtpColors, OtpController, OtpInput, Pagination, Rating, RawAlert,
    RawAttachment, RawCalendar, RawCarousel, RawChatMessage, RawCollapsible, RawDescriptionList,
    RawDropdownButton, RawEmptyState, RawField, RawInputGroup, RawKbd, RawNumberInput, RawOtpInput,
    RawRating, RawSkeleton, RawStatusBar, RawStepper, RawTag, RawTextInput, Skeleton, StatusBar,
    StepState, Stepper, Tag, ToggleButton,
};

#[component]
pub fn ComponentsPanel(
    expanded: Signal<bool>,
    accordion: Signal<Vec<bool>>,
    page: Signal<usize>,
    visible: Signal<bool>,
    submissions: Signal<usize>,
    input: TextController,
    step: Signal<usize>,
    tag: Signal<bool>,
    tag_visible: Signal<bool>,
    group_input: TextController,
    raw_group: Signal<String>,
    pressed: Signal<bool>,
    menu_open: Signal<bool>,
    action_count: Signal<usize>,
    number: Signal<f64>,
    number_text: TextController,
    raw_number: Signal<f64>,
    raw_number_text: TextController,
    month: Signal<DateTime>,
    date: Signal<Option<DateTime>>,
    rating: Signal<f32>,
    carousel: Signal<usize>,
    otp: OtpController,
    raw_otp: OtpController,
    otp_updates: Signal<usize>,
) -> BoxedWidget {
    let theme = use_theme();
    let card = |children: Vec<BoxedWidget>| -> BoxedWidget {
        Box::new(
            creamui_widgets::Surface::new(
                SurfaceRole::Inset,
                padding(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        ..column(theme.spacing_medium)
                    },
                    theme.spacing_medium,
                ),
            )
            .with_children(children),
        )
    };
    let label = |text: &str| -> BoxedWidget {
        Box::new(RawText::new(
            text,
            theme.text_secondary,
            theme.typography.caption,
        ))
    };
    let sections = accordion.get();
    let set_sections = accordion.clone();
    let themed_accordion = Accordion::new(move |index, open| {
        set_sections.update(|items| items[index] = open);
    })
    .item(
        "Account settings",
        Box::new(RawText::new(
            "Update the details associated with this account.",
            theme.text_secondary,
            theme.typography.body,
        )),
        sections[0],
    )
    .item(
        "Security",
        Box::new(RawText::new(
            "Review devices and active sessions.",
            theme.text_secondary,
            theme.typography.body,
        )),
        sections[1],
    );
    let set_expanded = expanded.clone();
    let disclosure = Collapsible::new("Advanced options", expanded.get(), move |next| {
        set_expanded.set(next)
    })
    .content(Box::new(RawText::new(
        "The content follows the trigger when it opens.",
        theme.text_secondary,
        theme.typography.body,
    )));
    let set_visible = visible.clone();
    let alert = if visible.get() {
        Box::new(
            Alert::new(
                AlertStatus::Info,
                "Changes saved",
                "Your settings are available across devices.",
            )
            .dismiss(move || set_visible.set(false)),
        ) as BoxedWidget
    } else {
        let show = visible.clone();
        Box::new(Button::secondary(ButtonSize::Sm, "Show alert", move || {
            show.set(true)
        }))
    };
    let field = Field::new(
        "Display name",
        Box::new(TextInput::controlled(&input).placeholder("Enter a name")),
        true,
    )
    .hint("Visible to other members of your workspace.");
    let set_submissions = submissions.clone();
    let form = Form::new()
        .field(Box::new(field))
        .submit("Save changes", move || {
            set_submissions.update(|count| *count += 1)
        });
    let set_page = page.clone();
    let mut pagination = Pagination::new(page.get(), 12, move |next| set_page.set(next));
    if use_screen_class() == ScreenClass::Compact {
        pagination = pagination.compact();
    }
    let raw_alert = RawAlert::new(
        Style::default()
            .flex_row()
            .gap(theme.spacing_small)
            .padding_all(theme.spacing_medium),
        AlertStatus::Success,
    )
    .background(theme.surface_hover)
    .corner_radius(theme.radius_medium)
    .title(Box::new(RawText::new(
        "Raw alert",
        theme.success,
        theme.typography.body,
    )))
    .message(Box::new(RawText::new(
        "Custom colors and layout.",
        theme.text_primary,
        theme.typography.body,
    )));
    let raw_disclosure = RawCollapsible::new(Style::default().flex_column(), expanded.get(), {
        let set_expanded = expanded.clone();
        move |next| set_expanded.set(next)
    })
    .trigger(Box::new(RawText::new(
        "Raw trigger: toggle details",
        theme.accent,
        theme.typography.body,
    )))
    .content(Box::new(RawText::new(
        "A custom-styled content slot.",
        theme.text_secondary,
        theme.typography.body,
    )));
    let raw_field = RawField::new(
        Style::default().flex_column().gap(theme.spacing_small),
        Box::new(RawText::new(
            "Raw field label",
            theme.text_primary,
            theme.typography.body,
        )),
        Box::new(RawText::new(
            "Your own control goes here",
            theme.text_secondary,
            theme.typography.body,
        )),
    )
    .hint(Box::new(RawText::new(
        "Custom hint slot",
        theme.text_secondary,
        theme.typography.caption,
    )));
    let raw_empty = RawEmptyState::new(
        Style::default()
            .flex_column()
            .gap(theme.spacing_small)
            .padding_all(theme.spacing_small),
        Box::new(RawText::new(
            "Nothing selected",
            theme.text_primary,
            theme.typography.body,
        )),
    )
    .description(Box::new(RawText::new(
        "The raw layout accepts any slots.",
        theme.text_secondary,
        theme.typography.caption,
    )));
    let set_step = step.clone();
    let mut stepper = Stepper::new(move |next| set_step.set(next))
        .step(
            "Draft",
            if step.get() == 0 {
                StepState::Active
            } else {
                StepState::Completed
            },
        )
        .step(
            "Review",
            if step.get() == 1 {
                StepState::Active
            } else {
                StepState::Upcoming
            },
        )
        .step(
            "Publish",
            if step.get() == 2 {
                StepState::Active
            } else {
                StepState::Upcoming
            },
        );
    if use_screen_class() == ScreenClass::Compact {
        stepper = stepper.vertical();
    }
    let detail = DescriptionList::new()
        .horizontal(100.0)
        .entry("Owner", "Design team")
        .entry("Status", "In progress")
        .entry("Updated", "Today");
    let status = StatusBar::new()
        .start(Box::new(RawText::new(
            "Ready",
            theme.text_primary,
            theme.typography.caption,
        )))
        .center(Box::new(RawText::new(
            "UTF-8",
            theme.text_secondary,
            theme.typography.caption,
        )))
        .end(Box::new(RawText::new(
            "Online",
            theme.success,
            theme.typography.caption,
        )));
    let raw_details =
        RawDescriptionList::new(Style::default().flex_column().gap(theme.spacing_small))
            .term_width(96.0)
            .entry(
                Box::new(RawText::new(
                    "Raw term",
                    theme.text_secondary,
                    theme.typography.body,
                )),
                Box::new(RawText::new(
                    "Custom value",
                    theme.text_primary,
                    theme.typography.body,
                )),
            );
    let raw_status =
        RawStatusBar::new(Style::default().flex_row().padding_all(theme.spacing_small))
            .background(theme.surface_hover)
            .start(Box::new(RawText::new(
                "Raw status",
                theme.text_primary,
                theme.typography.caption,
            )));
    let raw_stepper = RawStepper::new(Style::default().flex_row().gap(theme.spacing_small), {
        let set_step = step.clone();
        move |next| set_step.set(next)
    })
    .step(
        Box::new(RawText::new(
            "Raw step",
            theme.accent,
            theme.typography.body,
        )),
        StepState::Active,
    );
    let set_tag = tag.clone();
    let set_tag_visible = tag_visible.clone();
    let themed_tag: BoxedWidget = if tag_visible.get() {
        Box::new(
            Tag::new("Selectable tag")
                .selected(tag.get(), move |next| set_tag.set(next))
                .removable(move || set_tag_visible.set(false)),
        )
    } else {
        let show_tag = tag_visible.clone();
        Box::new(Button::secondary(
            ButtonSize::Sm,
            "Restore tag",
            move || show_tag.set(true),
        ))
    };
    let content = ChatMessage::new(
        "Dana",
        Box::new(RawText::new(
            "The design is ready for review.",
            theme.text_primary,
            theme.typography.body,
        )),
        false,
    )
    .timestamp("10:42");
    let attachment = Attachment::new("wireframes.png")
        .preview(Box::new(
            RawView::new(Style::default().size(36.0, 36.0))
                .background(theme.accent)
                .corner_radius(theme.radius_small),
        ))
        .progress(0.65);
    let raw_tag = RawTag::new(
        Style::default()
            .flex_row()
            .padding_xy(theme.spacing_small, 4.0),
        Box::new(RawText::new(
            "Raw tag",
            theme.text_primary,
            theme.typography.caption,
        )),
    )
    .background(theme.surface_hover);
    let set_month = month.clone();
    let select_date = date.clone();
    let on_month = month.clone();
    let themed_calendar = Calendar::new(
        month.get(),
        date.get(),
        move |next| {
            set_month.set(DateTime::new(next.year, next.month, 1, 0, 0));
            select_date.set(Some(next));
        },
        move |next| on_month.set(next),
    )
    .today(DateTime::new(2026, 3, 15, 0, 0))
    .week_start(1);
    let raw_calendar = RawCalendar::new(
        Style::default().flex_column(),
        month.get(),
        date.get(),
        {
            let set_date = date.clone();
            move |next| set_date.set(Some(next))
        },
        {
            let set_month = month.clone();
            move |next| set_month.set(next)
        },
    )
    .colors(CalendarColors {
        text: theme.text_primary,
        muted: theme.text_secondary,
        selected: theme.accent,
        selected_text: theme.selection_text,
        today: theme.accent,
    })
    .background(theme.surface_hover);
    let set_rating = rating.clone();
    let themed_rating = Rating::new(rating.get(), 5, move |next| set_rating.set(next)).half_steps();
    let raw_rating = RawRating::new(Style::default().size(140.0, 28.0), rating.get(), 5, {
        let set_rating = rating.clone();
        move |next| set_rating.set(next)
    })
    .step(0.5)
    .colors(theme.success, theme.border);
    let select_slide = carousel.clone();
    let mut themed_carousel = Carousel::new(carousel.get(), move |index| select_slide.set(index));
    let raw_select_slide = carousel.clone();
    let mut raw_carousel = RawCarousel::new(
        Style::default().flex_column().gap(theme.spacing_small),
        carousel.get(),
        move |index| raw_select_slide.set(index),
    )
    .indicators(theme.border, theme.success)
    .looping(true);
    for (index, name) in ["Overview", "Details", "Activity"].iter().enumerate() {
        themed_carousel = themed_carousel.slide(Box::new(
            RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..Style::default().padding_all(theme.spacing_medium)
            })
            .background(theme.surface_hover)
            .corner_radius(theme.radius_small)
            .child(Box::new(RawText::new(
                format!("Slide {}: {}", index + 1, name),
                theme.text_primary,
                theme.typography.body,
            ))),
        ));
        raw_carousel = raw_carousel.slide(Box::new(
            RawText::new(
                format!("Raw slide {}", index + 1),
                theme.text_primary,
                theme.typography.body,
            )
            .padding(theme.spacing_small),
        ));
    }
    let record_otp_update = otp_updates.clone();
    let themed_otp = OtpInput::new(&otp, move |_| record_otp_update.update(|count| *count += 1));
    let record_raw_update = otp_updates.clone();
    let raw_otp_input = RawOtpInput::new(Style::default().size(160.0, 42.0), &raw_otp, move |_| {
        record_raw_update.update(|count| *count += 1)
    })
    .colors(OtpColors {
        text: theme.text_primary,
        surface: theme.surface_hover,
        border: theme.border,
        focus: theme.accent,
        error: theme.danger,
    });
    let raw_attachment = RawAttachment::new(
        Style::default().flex_row().gap(theme.spacing_medium),
        Box::new(RawText::new(
            "raw-file.zip",
            theme.text_primary,
            theme.typography.body,
        )),
    )
    .error(Box::new(RawText::new(
        "Upload failed",
        theme.danger,
        theme.typography.caption,
    )));
    let raw_chat = RawChatMessage::new(
        Style::default().flex_column().gap(theme.spacing_small),
        Box::new(RawText::new(
            "Raw sender",
            theme.text_primary,
            theme.typography.caption,
        )),
        Box::new(RawText::new(
            "Content remains caller-controlled.",
            theme.text_secondary,
            theme.typography.body,
        )),
    );
    let raw_shortcut = RawKbd::new(
        Style::default().padding_all(4.0),
        ["Mod".to_owned(), "P".to_owned()],
    )
    .text_style(theme.text_primary, theme.typography.caption)
    .background(theme.surface_hover);
    let set_pressed = pressed.clone();
    let toggle = ToggleButton::new("Pin selection", pressed.get(), move |next| {
        set_pressed.set(next)
    });
    let set_action_count = action_count.clone();
    let toggle_menu = menu_open.clone();
    let dropdown = DropdownButton::new(
        "Save",
        move || set_action_count.update(|count| *count += 1),
        move || toggle_menu.update(|open| *open = !*open),
    );
    let menu_content: BoxedWidget = if menu_open.get() {
        Box::new(
            RawView::new(Style::default().padding_all(theme.spacing_medium))
                .background(theme.surface_hover)
                .child(Box::new(RawText::new(
                    "Save as…     Export…",
                    theme.text_primary,
                    theme.typography.body,
                ))),
        )
    } else {
        Box::new(RawView::new(Style::default()))
    };
    let raw_group_value = raw_group.get();
    let set_raw_group = raw_group.clone();
    let raw_input = RawTextInput::new(
        Style::default().size(140.0, 34.0),
        raw_group_value,
        theme.typography.body,
        theme.text_primary,
        move |next| set_raw_group.set(next),
    )
    .background(theme.surface_elevated);
    let raw_input_group = RawInputGroup::new(
        Style::default()
            .flex_row()
            .align(Align::Center)
            .padding_all(2.0),
        Box::new(raw_input),
    )
    .leading(Box::new(RawText::new(
        "€",
        theme.text_secondary,
        theme.typography.body,
    )))
    .trailing(Box::new(RawText::new(
        "EUR",
        theme.text_secondary,
        theme.typography.body,
    )))
    .border(theme.border, 1.0)
    .corner_radius(theme.input_radius);
    let set_pressed_raw = pressed.clone();
    let raw_toggle = creamui_widgets::RawButton::new(
        Style::default().padding_all(theme.spacing_small),
        move || set_pressed_raw.update(|active| *active = !*active),
    )
    .pressed_state(pressed.get())
    .background(if pressed.get() {
        theme.accent
    } else {
        theme.surface_hover
    })
    .child(Box::new(RawText::new(
        "Raw toggle",
        if pressed.get() {
            theme.selection_text
        } else {
            theme.text_primary
        },
        theme.typography.body,
    )));
    let raw_split = RawDropdownButton::new(
        Style::default().flex_row().gap(2.0),
        Box::new(RawText::new(
            "Raw action",
            theme.text_primary,
            theme.typography.body,
        )),
        Box::new(RawText::new("…", theme.text_primary, theme.typography.body)),
        {
            let count = action_count.clone();
            move || count.update(|value| *value += 1)
        },
        {
            let open = menu_open.clone();
            move || open.update(|value| *value = !*value)
        },
    )
    .button_styles(
        creamui_core::Style::new()
            .background(theme.surface_hover)
            .padding(theme.spacing_small),
        creamui_core::Style::new()
            .background(theme.surface_hover)
            .padding(theme.spacing_small),
    );
    let set_number = number.clone();
    let themed_number =
        NumberInput::new(&number_text, number.get(), move |next| set_number.set(next))
            .bounds(Some(0.0), Some(10.0))
            .step(0.5)
            .precision(1);
    let set_raw_number = raw_number.clone();
    let raw_numeric = RawNumberInput::new(
        Style::default()
            .flex_row()
            .align(Align::Center)
            .size(190.0, 38.0)
            .padding_all(4.0),
        &raw_number_text,
        raw_number.get(),
        move |next| set_raw_number.set(next),
    )
    .bounds(Some(0.0), Some(20.0))
    .step(1.0)
    .colors(
        theme.text_primary,
        theme.text_primary,
        theme.typography.body,
    )
    .background(theme.surface_hover);
    Box::new(
        RawView::new(column(section_gap()))
            .child(jsx! {
                <SectionHeader title={"Application components".to_owned()} subtitle={"Interactive disclosure, status, forms, and pagination, shown in themed and raw styles.".to_owned()} />
            })
            .child(card(vec![
                label("Accordion & collapsible"),
                Box::new(themed_accordion),
                Box::new(disclosure),
            ]))
            .child(card(vec![
                label("Inline alerts"),
                alert,
                Box::new(Alert::styled(
                    AlertStatus::Warning,
                    AlertVariant::Solid,
                    "Heads up",
                    "Review the details before continuing.",
                )),
            ]))
            .child(card(vec![
                label("Loading & empty states"),
                Box::new(
                    RawView::new(row(theme.spacing_medium))
                        .child(Box::new(Skeleton::circle(36.0)))
                        .child(Box::new(Skeleton::block(180.0, 36.0)))
                        .child(Box::new(
                            RawSkeleton::new(Style::default().size(36.0, 36.0))
                                .background(theme.border)
                                .corner_radius(4.0),
                        )),
                ),
                Box::new(
                    EmptyState::new("No results", "Try a different search term.").compact(),
                ),
            ]))
            .child(card(vec![
                label("Fields & form"),
                Box::new(form),
                Box::new(RawText::new(
                    format!("Submitted {} times", submissions.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
            ]))
            .child(card(vec![
                label("Pagination"),
                Box::new(pagination),
                Box::new(RawText::new(
                    format!("Page {} of 12", page.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
            ]))
            .child(card(vec![
                label("Headless building blocks"),
                Box::new(raw_alert),
                Box::new(raw_disclosure),
                Box::new(raw_field),
                Box::new(raw_empty),
            ]))
            .child(card(vec![
                label("Workspace & data"),
                Box::new(stepper),
                Box::new(detail),
                Box::new(status),
            ]))
            .child(card(vec![
                label("Raw workspace building blocks"),
                Box::new(raw_stepper),
                Box::new(raw_details),
                Box::new(raw_status),
            ]))
            .child(card(vec![
                label("Shortcuts, tags & content"),
                Box::new(
                    RawView::new(row(theme.spacing_medium))
                        .child(Box::new(Kbd::new(["Mod".to_owned(), "K".to_owned()])))
                        .child(themed_tag),
                ),
                Box::new(attachment),
                Box::new(content),
            ]))
            .child(card(vec![
                label("Raw content building blocks"),
                Box::new(
                    RawView::new(row(theme.spacing_medium))
                        .child(Box::new(raw_shortcut))
                        .child(Box::new(raw_tag)),
                ),
                Box::new(raw_attachment),
                Box::new(raw_chat),
            ]))
            .child(card(vec![
                label("Grouped inputs & split actions"),
                Box::new(InputGroup::controlled(&group_input).prefix("$").suffix("USD")),
                Box::new(RawView::new(row(theme.spacing_medium)).child(Box::new(toggle)).child(Box::new(dropdown))),
                menu_content,
                Box::new(RawText::new(
                    format!("Primary action: {} activations", action_count.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
            ]))
            .child(card(vec![
                label("Raw input & action building blocks"),
                Box::new(raw_input_group),
                Box::new(RawView::new(row(theme.spacing_small)).child(Box::new(raw_toggle)).child(Box::new(raw_split))),
            ]))
            .child(card(vec![
                label("Bounded number input"),
                Box::new(themed_number),
                Box::new(RawText::new(
                    format!("Value: {:.1}", number.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
                Box::new(raw_numeric),
            ]))
            .child(card(vec![
                label("Calendar"),
                Box::new(themed_calendar),
                Box::new(RawText::new(
                    date.get().map(|selected| format!("{:04}-{:02}-{:02}", selected.year, selected.month, selected.day)).unwrap_or_else(|| "None".to_owned()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
                label("Raw calendar"),
                Box::new(raw_calendar),
            ]))
            .child(card(vec![
                label("Ratings"),
                Box::new(themed_rating),
                Box::new(RawText::new(
                    format!("{:.1} / 5", rating.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
                Box::new(raw_rating),
            ]))
            .child(card(vec![
                label("Carousel"),
                Box::new(themed_carousel),
                label("Raw carousel"),
                Box::new(raw_carousel),
            ]))
            .child(card(vec![
                label("Verification code"),
                Box::new(themed_otp),
                Box::new(RawText::new(
                    format!("Entered: {}  |  edits: {}", otp.value(), otp_updates.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                )),
                label("Raw code cells"),
                Box::new(raw_otp_input),
            ])),
    )
}
