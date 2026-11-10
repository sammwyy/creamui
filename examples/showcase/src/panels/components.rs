use crate::prelude::*;
use creamui_core::Styled;
use creamui_widgets::{
    Accordion, Alert, AlertStatus, AlertVariant, Collapsible, DescriptionList, EmptyState, Field,
    Form, Pagination, RawAlert, RawCollapsible, RawDescriptionList, RawEmptyState, RawField,
    RawSkeleton, RawStatusBar, RawStepper, Skeleton, StatusBar, StepState, Stepper,
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
            ])),
    )
}
