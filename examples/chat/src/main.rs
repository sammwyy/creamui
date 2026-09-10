use creamui_core::layout::{
    AlignItems, Dimension, JustifyContent, Position, Size as LayoutSize, Style,
};
use creamui_core::{BoxedWidget, Painter, Rect, Size, TextAlign, Widget};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::{component, jsx};
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, ColorScheme, Theme};
use creamui_widgets::layout::{column, fixed, full_width, padding, padding_xy, row};
use creamui_widgets::{
    AutoScrollController, Avatar, Badge, Icon, RawText, ScrollController, ScrollView, Symbol,
    TextController, TextInput, TextSize, TypingIndicator,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn apple_theme() -> Theme {
    let mut theme = Theme::light();
    theme.colors = ColorScheme {
        surface: Color::rgb(0xf5, 0xf5, 0xf7),
        surface_elevated: Color::rgb(0xff, 0xff, 0xff),
        surface_hover: Color::rgb(0xe9, 0xe9, 0xeb),
        accent: Color::rgb(0x0a, 0x84, 0xff),
        accent_hover: Color::rgb(0x35, 0x9c, 0xff),
        accent_pressed: Color::rgb(0x00, 0x6f, 0xe0),
        selection_background: Color::rgb(0x0a, 0x84, 0xff),
        selection_text: Color::rgb(0xff, 0xff, 0xff),
        text_primary: Color::rgb(0x1c, 0x1c, 0x1e),
        text_secondary: Color::rgb(0x8e, 0x8e, 0x93),
        text_disabled: Color::rgb(0xc7, 0xc7, 0xcc),
        border: Color::rgb(0xe5, 0xe5, 0xea),
        border_strong: Color::rgb(0xd1, 0xd1, 0xd6),
        danger: Color::rgb(0xff, 0x3b, 0x30),
        warning: Color::rgb(0xff, 0x95, 0x00),
        success: Color::rgb(0x34, 0xc7, 0x59),
    };
    theme.card_radius = 18.0;
    theme.input_radius = 20.0;
    theme.button_radius = 10.0;
    theme.radius_small = 10.0;
    theme.radius_medium = 14.0;
    theme.radius_large = 22.0;
    theme.sidebar_item_radius = 12.0;
    theme
}

fn tint(color: Color, alpha: u8) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

fn hairline() -> BoxedWidget {
    let theme = use_theme();
    let style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(1.0),
        },
        flex_shrink: 0.0,
        ..Default::default()
    };
    Box::new(jsx! { <RawView style={style} background={theme.border} /> })
}

#[derive(Clone)]
enum Attachment {
    Image(ImageData),
    File(String),
}

#[derive(Clone)]
struct ChatMessage {
    mine: bool,
    text: String,
    attachment: Option<Attachment>,
    time: String,
    read: bool,
}

#[derive(Clone)]
struct Contact {
    name: String,
    initials: String,
    color: Color,
    photo: Option<ImageData>,
    online: bool,
}

#[derive(Clone)]
struct Conversation {
    contact: Contact,
    messages: Signal<Vec<ChatMessage>>,
    unread: Signal<usize>,
    typing: Signal<bool>,
}

// Zero-size widget mounted at the root so the window keeps repainting at
// ~30fps; that's what drives the `Instant` reply delays and the settle-scroll
// catch-up below, since neither is backed by a `Signal`.
struct Heartbeat;
impl Widget for Heartbeat {
    fn style(&self) -> creamui_core::Style {
        Style {
            position: Position::Absolute,
            size: fixed(0.0, 0.0),
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, _rect: Rect) {
        let _ = painter.animation_time();
    }
}

type PendingReplies = Rc<RefCell<Vec<(usize, Instant)>>>;

fn format_clock(minute_of_day: u32) -> String {
    let hour24 = (minute_of_day / 60) % 24;
    let minute = minute_of_day % 60;
    let period = if hour24 >= 12 { "PM" } else { "AM" };
    let hour12 = match hour24 % 12 {
        0 => 12,
        other => other,
    };
    format!("{hour12}:{minute:02} {period}")
}

fn alice_photo_placeholder() -> ImageData {
    ImageData::from_bytes(include_bytes!("../assets/botanical.webp"))
        .expect("bundled WebP should decode")
}
fn camila_photo_placeholder() -> ImageData {
    ImageData::from_bytes(include_bytes!("../assets/iridescent.png"))
        .expect("bundled PNG should decode")
}

fn seed_conversations(clock: &Rc<Cell<u32>>) -> Vec<Conversation> {
    fn tick(clock: &Rc<Cell<u32>>, minutes: u32) -> String {
        let value = clock.get() + minutes;
        clock.set(value);
        format_clock(value)
    }
    fn msg(mine: bool, text: &str, time: String, read: bool) -> ChatMessage {
        ChatMessage {
            mine,
            text: text.to_owned(),
            attachment: None,
            time,
            read,
        }
    }

    let iridescent = ImageData::from_bytes(include_bytes!("../assets/iridescent.png"))
        .expect("bundled PNG should decode");
    let still_life = ImageData::from_bytes(include_bytes!("../assets/still-life.jpg"))
        .expect("bundled JPEG should decode");
    let botanical = ImageData::from_bytes(include_bytes!("../assets/botanical.webp"))
        .expect("bundled WebP should decode");

    clock.set(9 * 60 + 20);

    let alice = Contact {
        name: "Alice Rivera".into(),
        initials: "AR".into(),
        color: Color::rgb(181, 139, 255),
        photo: Some(iridescent),
        online: true,
    };
    let alice_messages = vec![
        msg(
            false,
            "Morning! Did the export finish overnight?",
            tick(clock, 1),
            false,
        ),
        msg(true, "Yep, just checked — all green.", tick(clock, 2), true),
        msg(
            false,
            "Amazing, sending it upstream then.",
            tick(clock, 1),
            false,
        ),
        ChatMessage {
            mine: false,
            text: "Here's the palette I mentioned".into(),
            attachment: Some(Attachment::Image(alice_photo_placeholder())),
            time: tick(clock, 3),
            read: false,
        },
        msg(
            true,
            "Oh that's gorgeous, using it for the header.",
            tick(clock, 1),
            true,
        ),
    ];

    let ben = Contact {
        name: "Ben Okafor".into(),
        initials: "BO".into(),
        color: Color::rgb(105, 218, 166),
        photo: None,
        online: false,
    };
    let ben_messages = vec![
        msg(
            false,
            "Can you review PR #482 when you get a chance?",
            tick(clock, 4),
            false,
        ),
        msg(
            false,
            "No rush, just don't want it going stale.",
            tick(clock, 1),
            false,
        ),
    ];

    let camila = Contact {
        name: "Camila Sol".into(),
        initials: "CS".into(),
        color: Color::rgb(248, 135, 181),
        photo: Some(still_life),
        online: true,
    };
    let camila_messages = vec![
        msg(true, "Lunch spot from last week?", tick(clock, 2), true),
        msg(
            false,
            "The one with the tiled counter, sending a pic",
            tick(clock, 1),
            false,
        ),
        ChatMessage {
            mine: false,
            text: String::new(),
            attachment: Some(Attachment::Image(camila_photo_placeholder())),
            time: tick(clock, 1),
            read: false,
        },
        msg(true, "That's the one, thank you!", tick(clock, 1), false),
    ];

    let design_team = Contact {
        name: "Design Team".into(),
        initials: "DT".into(),
        color: Color::rgb(255, 177, 109),
        photo: None,
        online: false,
    };
    let design_messages = vec![
        msg(
            false,
            "New icon set is up in the shared drive.",
            tick(clock, 5),
            false,
        ),
        msg(
            false,
            "Cream and berry accent variants included.",
            tick(clock, 1),
            false,
        ),
        msg(
            false,
            "Let us know if anything reads wrong at 16px.",
            tick(clock, 1),
            false,
        ),
        msg(
            false,
            "Also — renamed the spacing tokens, heads up.",
            tick(clock, 2),
            false,
        ),
        msg(false, "Docs are updated to match.", tick(clock, 1), false),
    ];

    let diego = Contact {
        name: "Diego Muram".into(),
        initials: "DM".into(),
        color: Color::rgb(118, 192, 255),
        photo: Some(botanical),
        online: true,
    };
    let diego_messages = vec![
        msg(true, "Studio still open till 8?", tick(clock, 3), true),
        msg(false, "Till 9 tonight actually.", tick(clock, 1), false),
    ];

    let nadia = Contact {
        name: "Nadia Petrov".into(),
        initials: "NP".into(),
        color: Color::rgb(0xa7, 0x7b, 0xff),
        photo: None,
        online: false,
    };
    let nadia_messages = vec![
        msg(
            false,
            "Quarterly numbers are in, looking solid.",
            tick(clock, 6),
            false,
        ),
        msg(
            false,
            "Deck's in the folder, feel free to poke holes.",
            tick(clock, 1),
            false,
        ),
        msg(
            false,
            "Standup moved to 10:30 tomorrow, by the way.",
            tick(clock, 1),
            false,
        ),
        msg(false, "And happy Friday!", tick(clock, 1), false),
    ];

    vec![
        Conversation {
            contact: alice,
            messages: Signal::new(alice_messages),
            unread: Signal::new(0),
            typing: Signal::new(false),
        },
        Conversation {
            contact: ben,
            messages: Signal::new(ben_messages),
            unread: Signal::new(2),
            typing: Signal::new(false),
        },
        Conversation {
            contact: camila,
            messages: Signal::new(camila_messages),
            unread: Signal::new(0),
            typing: Signal::new(false),
        },
        Conversation {
            contact: design_team,
            messages: Signal::new(design_messages),
            unread: Signal::new(4),
            typing: Signal::new(false),
        },
        Conversation {
            contact: diego,
            messages: Signal::new(diego_messages),
            unread: Signal::new(0),
            typing: Signal::new(false),
        },
        Conversation {
            contact: nadia,
            messages: Signal::new(nadia_messages),
            unread: Signal::new(9),
            typing: Signal::new(false),
        },
    ]
}

fn box_style(width: f32, height: f32) -> Style {
    Style {
        size: fixed(width, height),
        flex_shrink: 0.0,
        ..Default::default()
    }
}

fn contact_avatar(contact: &Contact, size: f32) -> Avatar {
    let theme = use_theme();
    let mut avatar = Avatar::new(size)
        .fallback_color(contact.color)
        .initials(contact.initials.clone())
        .initials_color(Color::rgb(0x1c, 0x1b, 0x1d));
    if let Some(photo) = &contact.photo {
        avatar = avatar.image(Box::new(
            Image::new(photo.clone())
                .layout(box_style(size, size))
                .fit(ImageFit::Cover),
        ));
    }
    if contact.online {
        avatar = avatar.status(theme.success);
    }
    avatar
}

fn icon_button(
    symbol: Symbol,
    size: f32,
    background: Color,
    icon_color: Color,
    on_click: impl Fn() + 'static,
) -> BoxedWidget {
    let style = Style {
        size: fixed(size, size),
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        flex_shrink: 0.0,
        ..Default::default()
    };
    Box::new(jsx! {
        <RawButton style={style} background={background} corner_radius={size / 2.0} on_click={on_click}>
            {Box::new(Icon::new(symbol, icon_color).size(size * 0.46)) as BoxedWidget}
        </RawButton>
    })
}

fn line_style(height: f32, grow: bool) -> Style {
    Style {
        size: LayoutSize {
            width: Dimension::Auto,
            height: Dimension::Length(height),
        },
        flex_grow: if grow { 1.0 } else { 0.0 },
        flex_shrink: if grow { 1.0 } else { 0.0 },
        min_size: LayoutSize {
            width: Dimension::Length(0.0),
            height: Dimension::Auto,
        },
        ..Default::default()
    }
}

fn attach_dialog() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rfd::FileDialog::new()
            .set_title("Attach a file")
            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
            .add_filter("All files", &["*"])
            .pick_file()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn attachment_from_path(path: &std::path::Path) -> Attachment {
    match ImageData::from_path(path) {
        Ok(data) => Attachment::Image(data),
        Err(_) => Attachment::File(
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_owned()),
        ),
    }
}

#[component]
fn ContactRow(conversation: Conversation, active: bool, on_select: Rc<dyn Fn()>) -> BoxedWidget {
    let theme = use_theme();
    let messages = conversation.messages.get();
    let unread = conversation.unread.get();
    let is_typing = conversation.typing.get();
    let preview = if is_typing {
        "Typing…".to_owned()
    } else {
        match messages.last() {
            Some(last) => {
                let body = match (&last.attachment, last.text.is_empty()) {
                    (Some(Attachment::Image(_)), true) => "[photo]".to_owned(),
                    (Some(Attachment::Image(_)), false) => format!("[photo] {}", last.text),
                    (Some(Attachment::File(name)), _) => format!("[file] {name}"),
                    (None, _) => last.text.clone(),
                };
                if last.mine {
                    format!("You: {body}")
                } else {
                    body
                }
            }
            None => "No messages yet".to_owned(),
        }
    };
    let time_label = messages.last().map(|m| m.time.clone()).unwrap_or_default();
    let background = if active {
        tint(theme.accent, 24)
    } else {
        theme.surface
    };
    let preview_color = if is_typing {
        theme.accent
    } else {
        theme.text_secondary
    };

    let row_style = padding(
        Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(68.0),
            },
            align_items: Some(AlignItems::Center),
            flex_shrink: 0.0,
            ..row(10.0)
        },
        10.0,
    );
    let text_column_style = Style {
        flex_grow: 1.0,
        min_size: LayoutSize {
            width: Dimension::Length(0.0),
            height: Dimension::Auto,
        },
        ..column(3.0)
    };
    let name_row_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(16.0),
        },
        align_items: Some(AlignItems::Center),
        ..row(6.0)
    };
    let preview_row_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(16.0),
        },
        align_items: Some(AlignItems::Center),
        ..row(6.0)
    };

    let name_text: BoxedWidget = Box::new(
        RawText::new(conversation.contact.name.clone(), theme.text_primary, 13.5)
            .bold(true)
            .text_align(TextAlign::Start)
            .layout(line_style(16.0, true)),
    );
    let avatar: BoxedWidget = Box::new(contact_avatar(&conversation.contact, 44.0));
    let badge: BoxedWidget = Box::new(Badge::count(unread));

    Box::new(jsx! {
        <RawButton style={row_style} background={background} corner_radius={theme.radius_medium} on_click={move || on_select()}>
            {avatar}
            <RawView style={text_column_style}>
                <RawView style={name_row_style}>
                    {name_text}
                    <RawText color={theme.text_disabled} font_size={10.5} style={line_style(16.0, false)}>{time_label}</RawText>
                </RawView>
                <RawView style={preview_row_style}>
                    <RawText color={preview_color} font_size={12.0} align={TextAlign::Start} style={line_style(16.0, true)}>{preview}</RawText>
                    {badge}
                </RawView>
            </RawView>
        </RawButton>
    })
}

#[component]
fn ContactSidebar(
    conversations: Vec<Conversation>,
    active_index: usize,
    filter: TextController,
    on_select: Rc<dyn Fn(usize)>,
) -> BoxedWidget {
    let theme = use_theme();
    let query = filter.value().to_lowercase();
    let header_style = padding(
        Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            flex_shrink: 0.0,
            ..column(10.0)
        },
        14.0,
    );
    let list_style = Style {
        flex_grow: 1.0,
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };

    let mut rows: Vec<BoxedWidget> = Vec::new();
    for (index, conversation) in conversations.iter().enumerate() {
        if !query.is_empty() && !conversation.contact.name.to_lowercase().contains(&query) {
            continue;
        }
        let select = on_select.clone();
        rows.push(ContactRow(ContactRowProps {
            conversation: conversation.clone(),
            active: active_index == index,
            on_select: Rc::new(move || select(index)),
        }));
    }
    if rows.is_empty() {
        rows.push(Box::new(jsx! {
            <RawView style={padding(Style { size: LayoutSize { width: Dimension::Percent(1.0), height: Dimension::Length(60.0) }, ..Default::default() }, 14.0)}>
                <Text secondary={true} align={TextAlign::Start}>"No conversations match."</Text>
            </RawView>
        }));
    }

    let sidebar_style = Style {
        size: LayoutSize {
            width: Dimension::Length(300.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        ..column(0.0)
    };

    let search_style = full_width(Style {
        size: LayoutSize {
            width: Dimension::Auto,
            height: Dimension::Length(36.0),
        },
        ..Default::default()
    });
    let header: BoxedWidget = Box::new(jsx! {
        <RawView style={header_style}>
            <Heading size={TextSize::Lg}>"Chats"</Heading>
            <TextInput controller={&filter} style={search_style} placeholder={"Search people…"} />
        </RawView>
    });
    let list: BoxedWidget = Box::new(ScrollView::new(list_style, 0.0, |_| {}).with_children(rows));

    Box::new(jsx! {
        <RawView style={sidebar_style} background={theme.surface}>
            {header}
            {list}
        </RawView>
    })
}

#[component]
fn ConversationHeader(conversation: Conversation) -> BoxedWidget {
    let theme = use_theme();
    let status = if conversation.typing.get() {
        "Typing…".to_owned()
    } else if conversation.contact.online {
        "Online".to_owned()
    } else {
        "Offline".to_owned()
    };
    let status_color = if conversation.typing.get() {
        theme.accent
    } else if conversation.contact.online {
        theme.success
    } else {
        theme.text_secondary
    };
    let header_style = padding_xy(
        Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(72.0),
            },
            flex_shrink: 0.0,
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::SpaceBetween),
            ..row(12.0)
        },
        18.0,
        0.0,
    );
    let identity_style = Style {
        align_items: Some(AlignItems::Center),
        ..row(12.0)
    };

    let name_text: BoxedWidget = Box::new(
        RawText::new(conversation.contact.name.clone(), theme.text_primary, 14.0)
            .bold(true)
            .text_align(TextAlign::Start),
    );
    let avatar: BoxedWidget = Box::new(contact_avatar(&conversation.contact, 40.0));

    let wrapper_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(73.0),
        },
        flex_shrink: 0.0,
        ..column(0.0)
    };
    Box::new(jsx! {
        <RawView style={wrapper_style}>
            <RawView style={header_style} background={theme.surface_elevated}>
                <RawView style={identity_style}>
                    {avatar}
                    <RawView style={column(2.0)}>
                        {name_text}
                        <RawText color={status_color} font_size={11.5} align={TextAlign::Start}>{status}</RawText>
                    </RawView>
                </RawView>
                <RawView style={row(8.0)}>
                    {icon_button(Symbol::Search, 30.0, theme.surface_hover, theme.text_secondary, || {})}
                    {icon_button(Symbol::Controls, 30.0, theme.surface_hover, theme.text_secondary, || {})}
                </RawView>
            </RawView>
            {hairline()}
        </RawView>
    })
}

fn attachment_widget(attachment: &Attachment) -> BoxedWidget {
    let theme = use_theme();
    match attachment {
        Attachment::Image(data) => {
            let aspect = data.width() as f32 / data.height() as f32;
            let width = 220.0f32;
            let height = (width / aspect).clamp(120.0, 260.0);
            Box::new(jsx! {
                <Image data={data.clone()} style={box_style(width, height)} fit={ImageFit::Cover} corner_radius={theme.radius_medium} />
            })
        }
        Attachment::File(name) => {
            let style = padding(
                Style {
                    align_items: Some(AlignItems::Center),
                    ..row(8.0)
                },
                8.0,
            );
            let icon: BoxedWidget =
                Box::new(Icon::new(Symbol::Attachment, theme.text_secondary).size(16.0));
            Box::new(jsx! {
                <RawView style={style} background={theme.surface_elevated} corner_radius={theme.radius_small}>
                    {icon}
                    <RawText color={theme.text_primary} font_size={12.0}>{name.clone()}</RawText>
                </RawView>
            })
        }
    }
}

struct ReadReceipt {
    color: Color,
    double: bool,
}
impl Widget for ReadReceipt {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: fixed(if self.double { 18.0 } else { 11.0 }, 11.0),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        Icon::draw(
            Symbol::Check,
            painter,
            Rect {
                x: rect.x,
                y: rect.y,
                width: 11.0,
                height: 11.0,
            },
            self.color,
        );
        if self.double {
            Icon::draw(
                Symbol::Check,
                painter,
                Rect {
                    x: rect.x + 7.0,
                    y: rect.y,
                    width: 11.0,
                    height: 11.0,
                },
                self.color,
            );
        }
    }
}

fn message_bubble(message: &ChatMessage) -> BoxedWidget {
    let theme = use_theme();
    let bubble_background = if message.mine {
        theme.accent
    } else {
        theme.surface_hover
    };
    let text_color = if message.mine {
        theme.selection_text
    } else {
        theme.text_primary
    };
    let footer_color = if message.mine {
        Color::rgba(
            theme.selection_text.r,
            theme.selection_text.g,
            theme.selection_text.b,
            170,
        )
    } else {
        theme.text_disabled
    };

    let row_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        justify_content: Some(if message.mine {
            JustifyContent::End
        } else {
            JustifyContent::Start
        }),
        ..row(0.0)
    };
    let bubble_style = padding(
        Style {
            max_size: LayoutSize {
                width: Dimension::Length(320.0),
                height: Dimension::Auto,
            },
            size: LayoutSize {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            ..column(6.0)
        },
        11.0,
    );
    let text_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        ..Default::default()
    };
    let footer_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(13.0),
        },
        justify_content: Some(JustifyContent::End),
        align_items: Some(AlignItems::Center),
        ..row(4.0)
    };

    let mut bubble_children: Vec<BoxedWidget> = Vec::new();
    if let Some(attachment) = &message.attachment {
        bubble_children.push(attachment_widget(attachment));
    }
    if !message.text.is_empty() {
        bubble_children.push(Box::new(jsx! {
            <RawText color={text_color} font_size={13.5} align={TextAlign::Start} style={text_style}>{message.text.clone()}</RawText>
        }));
    }

    let mut footer_children: Vec<BoxedWidget> = vec![Box::new(jsx! {
        <RawText color={footer_color} font_size={10.0}>{message.time.clone()}</RawText>
    })];
    if message.mine {
        footer_children.push(Box::new(ReadReceipt {
            color: if message.read {
                theme.selection_text
            } else {
                footer_color
            },
            double: message.read,
        }));
    }
    bubble_children.push(Box::new(
        jsx! { <RawView style={footer_style} children={footer_children} /> },
    ));

    Box::new(jsx! {
        <RawView style={row_style}>
            <RawView style={bubble_style} background={bubble_background} corner_radius={theme.card_radius} children={bubble_children} />
        </RawView>
    })
}

#[component]
fn MessageList(conversation: Conversation, scroll: ScrollController) -> BoxedWidget {
    let theme = use_theme();
    let messages = conversation.messages.get();
    let is_typing = conversation.typing.get();
    let list_style = Style {
        flex_grow: 1.0,
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    let mut view = ScrollView::controlled(list_style, scroll)
        .background(theme.surface_elevated)
        .content_gap(10.0)
        .with_children(messages.iter().map(|m| message_bubble(m)).collect());
    if is_typing {
        let typing_row_style = Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            justify_content: Some(JustifyContent::Start),
            ..row(0.0)
        };
        let bubble_style = padding(
            Style {
                size: LayoutSize {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                ..Default::default()
            },
            12.0,
        );
        view = view.child(Box::new(jsx! {
            <RawView style={typing_row_style}>
                <RawView style={bubble_style} background={theme.surface_hover} corner_radius={theme.card_radius}>
                    {Box::new(TypingIndicator::new()) as BoxedWidget}
                </RawView>
            </RawView>
        }));
    }
    // Explicit min_size:0 keeps this wrapper shrinkable once messages overflow —
    // otherwise its automatic min-height floors at the unclipped content height
    // and pushes the composer below it.
    let outer_style = padding(
        Style {
            flex_grow: 1.0,
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            min_size: LayoutSize {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..column(10.0)
        },
        16.0,
    );
    let view: BoxedWidget = Box::new(view);
    Box::new(jsx! {
        <RawView style={outer_style} background={theme.surface_elevated}>
            {view}
        </RawView>
    })
}

// Sized to sit inside the composer's one-row bar rather than a second row
// above it, so an attached/cleared file never changes the bar's height.
fn inline_attachment_chip(attachment: &Attachment, on_remove: impl Fn() + 'static) -> BoxedWidget {
    let theme = use_theme();
    let chip_style = padding_xy(
        Style {
            align_items: Some(AlignItems::Center),
            size: LayoutSize {
                width: Dimension::Auto,
                height: Dimension::Length(40.0),
            },
            flex_shrink: 0.0,
            ..row(4.0)
        },
        4.0,
        4.0,
    );
    let mut chip_children: Vec<BoxedWidget> = Vec::new();
    match attachment {
        Attachment::Image(data) => {
            chip_children.push(Box::new(jsx! {
                <Image data={data.clone()} style={box_style(32.0, 32.0)} fit={ImageFit::Cover} corner_radius={theme.radius_large} />
            }));
        }
        Attachment::File(name) => {
            let short: String = if name.chars().count() > 12 {
                name.chars().take(11).chain(['…']).collect()
            } else {
                name.clone()
            };
            chip_children.push(Box::new(
                Icon::new(Symbol::Attachment, theme.text_secondary).size(14.0),
            ));
            chip_children.push(Box::new(jsx! {
                <RawText color={theme.text_primary} font_size={11.0}>{short}</RawText>
            }));
        }
    }
    chip_children.push(icon_button(
        Symbol::Close,
        18.0,
        Color::rgba(0, 0, 0, 0),
        theme.text_disabled,
        on_remove,
    ));
    Box::new(jsx! {
        <RawView style={chip_style} background={theme.surface_hover} corner_radius={theme.radius_large} children={chip_children} />
    })
}

#[component]
fn Composer(
    composer: TextController,
    pending_attachment: Signal<Option<Attachment>>,
    on_attach: Rc<dyn Fn()>,
    on_send: Rc<dyn Fn()>,
) -> BoxedWidget {
    let theme = use_theme();
    let bar_style = padding_xy(
        Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(64.0),
            },
            align_items: Some(AlignItems::Center),
            flex_shrink: 0.0,
            ..row(8.0)
        },
        16.0,
        12.0,
    );
    let input_style = Style {
        flex_grow: 1.0,
        size: LayoutSize {
            width: Dimension::Auto,
            height: Dimension::Length(40.0),
        },
        ..Default::default()
    };

    let mut bar_children: Vec<BoxedWidget> = vec![icon_button(
        Symbol::Attachment,
        36.0,
        theme.surface_hover,
        theme.text_secondary,
        move || on_attach(),
    )];

    if let Some(attachment) = pending_attachment.get() {
        let clear = pending_attachment.clone();
        bar_children.push(inline_attachment_chip(&attachment, move || clear.set(None)));
    }

    let submit = on_send.clone();
    let input: BoxedWidget = Box::new(
        TextInput::controlled_with_style(input_style, &composer)
            .placeholder("Message")
            .background(theme.surface_hover)
            .on_submit(move || submit()),
    );
    bar_children.push(input);
    bar_children.push(icon_button(
        Symbol::Send,
        36.0,
        theme.accent,
        theme.selection_text,
        move || on_send(),
    ));

    let wrapper_style = Style {
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(65.0),
        },
        flex_shrink: 0.0,
        ..column(0.0)
    };
    Box::new(jsx! {
        <RawView style={wrapper_style}>
            {hairline()}
            <RawView style={bar_style} background={theme.surface_elevated} children={bar_children} />
        </RawView>
    })
}

fn main() {
    let clock = Rc::new(Cell::new(0u32));
    let conversations = seed_conversations(&clock);
    let active = Signal::new(0usize);
    let last_active = Rc::new(Cell::new(0usize));
    let composer = TextController::default();
    let filter = TextController::default();
    let pending_attachment: Signal<Option<Attachment>> = Signal::new(None);
    let content_scroll = AutoScrollController::new();
    let pending_replies: PendingReplies = Rc::new(RefCell::new(Vec::new()));

    const REPLIES: [&str; 5] = [
        "Got it, thanks!",
        "Sounds good to me.",
        "Let me check and get back to you.",
        "Ha, fair point.",
        "On it.",
    ];

    run(
        WindowOptions {
            title: "CreamUI — Chat".into(),
            width: 1180,
            height: 760,
            theme: apple_theme(),
            ..Default::default()
        },
        apple_theme().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();

            {
                let now = Instant::now();
                let mut due = Vec::new();
                pending_replies.borrow_mut().retain(|(index, deadline)| {
                    if now >= *deadline {
                        due.push(*index);
                        false
                    } else {
                        true
                    }
                });
                for index in due {
                    let conversation = &conversations[index];
                    conversation.typing.set(false);
                    let reply_text = REPLIES[clock.get() as usize % REPLIES.len()];
                    let time = format_clock(clock.get());
                    clock.set(clock.get() + 1);
                    conversation.messages.update(|list| {
                        for message in list.iter_mut() {
                            if message.mine {
                                message.read = true;
                            }
                        }
                        list.push(ChatMessage {
                            mine: false,
                            text: reply_text.to_owned(),
                            attachment: None,
                            time,
                            read: false,
                        });
                    });
                    if active.peek() != index {
                        conversation.unread.update(|count| *count += 1);
                    }
                }
            }

            let active_index = active.get();
            if last_active.get() != active_index {
                last_active.set(active_index);
                content_scroll.reset();
            }
            content_scroll.tick();

            let select_active = active.clone();
            let select_conversations = conversations.clone();
            let on_select: Rc<dyn Fn(usize)> = Rc::new(move |index| {
                select_active.set(index);
                select_conversations[index].unread.set(0);
            });

            let sidebar = ContactSidebar(ContactSidebarProps {
                conversations: conversations.clone(),
                active_index,
                filter: filter.clone(),
                on_select,
            });

            let header = ConversationHeader(ConversationHeaderProps {
                conversation: conversations[active_index].clone(),
            });

            let message_list = MessageList(MessageListProps {
                conversation: conversations[active_index].clone(),
                scroll: content_scroll.scroll(),
            });

            let attach_pending = pending_attachment.clone();
            let on_attach: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(path) = attach_dialog() {
                    attach_pending.set(Some(attachment_from_path(&path)));
                }
            });

            let send_conversations = conversations.clone();
            let send_composer = composer.clone();
            let send_pending = pending_attachment.clone();
            let send_scroll = content_scroll.clone();
            let send_replies = pending_replies.clone();
            let send_clock = clock.clone();
            let send_active = active.clone();
            let on_send: Rc<dyn Fn()> = Rc::new(move || {
                let text = send_composer.value();
                let attachment = send_pending.get();
                if text.trim().is_empty() && attachment.is_none() {
                    return;
                }
                let index = send_active.peek();
                let time = format_clock(send_clock.get());
                send_clock.set(send_clock.get() + 1);
                send_conversations[index].messages.update(|list| {
                    list.push(ChatMessage {
                        mine: true,
                        text,
                        attachment,
                        time,
                        read: false,
                    });
                });
                send_composer.set_value(String::new());
                send_pending.set(None);
                send_scroll.snap_to_bottom();
                send_conversations[index].typing.set(true);
                let delay = 900 + (index as u64 * 263) % 1200;
                send_replies
                    .borrow_mut()
                    .push((index, Instant::now() + Duration::from_millis(delay)));
            });

            let composer_bar = Composer(ComposerProps {
                composer: composer.clone(),
                pending_attachment: pending_attachment.clone(),
                on_attach,
                on_send,
            });

            let root_style = Style {
                size: LayoutSize {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                align_items: Some(AlignItems::Stretch),
                ..row(0.0)
            };
            let main_column_style = Style {
                flex_grow: 1.0,
                size: LayoutSize {
                    width: Dimension::Auto,
                    height: Dimension::Percent(1.0),
                },
                ..column(0.0)
            };

            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    {sidebar}
                    <RawView style={main_column_style}>
                        {header}
                        {message_list}
                        {composer_bar}
                    </RawView>
                    {Box::new(Heartbeat) as BoxedWidget}
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
