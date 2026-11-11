//! Shared design tokens and layout helpers used across every panel.

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ShowcasePage {
    Appearance,
    Theming,
    Hooks,
    Events,
    Typography,
    Input,
    Pickers,
    Images,
    Button,
    Slider,
    Checkbox,
    Selection,
    Feedback,
    Sidebar,
    Tabs,
    Scroll,
    Tree,
    Table,
    Grid,
    Flex,
    Disclosure,
    States,
    Forms,
    Pagination,
    Workspace,
    Tags,
    InputGroups,
    Calendar,
    Rating,
    Carousel,
    Verification,
    Notifications,
    Tooltip,
    Panels,
    DataTable,
    Sheets,
    Toolbar,
    Chart,
    Diff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavGroup {
    Features,
    Components,
}

pub struct NavPage {
    pub page: ShowcasePage,
    pub label: &'static str,
    pub symbol: Symbol,
    pub group: NavGroup,
}

pub const NAV_PAGES: [NavPage; 39] = [
    NavPage {
        page: ShowcasePage::Appearance,
        label: "Appearance",
        symbol: Symbol::Appearance,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Theming,
        label: "Theming",
        symbol: Symbol::Display,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Hooks,
        label: "Hooks & state",
        symbol: Symbol::Controls,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Events,
        label: "Events",
        symbol: Symbol::Keyboard,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Typography,
        label: "Typography",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Input,
        label: "Input",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Pickers,
        label: "Pickers",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Images,
        label: "Images",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Button,
        label: "Button",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Slider,
        label: "Slider",
        symbol: Symbol::Sliders,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Checkbox,
        label: "Checkbox",
        symbol: Symbol::Check,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Selection,
        label: "Selection",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Feedback,
        label: "Feedback",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Sidebar,
        label: "Sidebar",
        symbol: Symbol::Folder,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tabs,
        label: "Tabs",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Scroll,
        label: "Scroll",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tree,
        label: "Tree",
        symbol: Symbol::Folder,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Table,
        label: "Table",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Grid,
        label: "Grid",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Flex,
        label: "Flex",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Disclosure,
        label: "Disclosure",
        symbol: Symbol::Folder,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::States,
        label: "States & alerts",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Forms,
        label: "Forms",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Pagination,
        label: "Pagination",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Workspace,
        label: "Workspace",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tags,
        label: "Tags & shortcuts",
        symbol: Symbol::Keyboard,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::InputGroups,
        label: "Input groups",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Calendar,
        label: "Calendar",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Rating,
        label: "Rating",
        symbol: Symbol::Check,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Carousel,
        label: "Carousel",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Verification,
        label: "Verification",
        symbol: Symbol::Keyboard,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Notifications,
        label: "Notifications",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tooltip,
        label: "Tooltip",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Panels,
        label: "Panels & layouts",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::DataTable,
        label: "Data table",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Sheets,
        label: "Sheets",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Toolbar,
        label: "Toolbar",
        symbol: Symbol::Controls,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Chart,
        label: "Chart",
        symbol: Symbol::Grid,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Diff,
        label: "Diff",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
];

impl ShowcasePage {
    pub fn label(self) -> &'static str {
        NAV_PAGES[self as usize].label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_entries_match_page_indices_and_group_order() {
        for (index, entry) in NAV_PAGES.iter().enumerate() {
            assert_eq!(entry.page as usize, index);
            assert_eq!(
                entry.group,
                if index < 4 {
                    NavGroup::Features
                } else {
                    NavGroup::Components
                }
            );
        }
    }
}
pub const ACCENTS: [(&str, Color); 5] = [
    ("Lilac", Color::rgb(181, 139, 255)),
    ("Sky", Color::rgb(118, 192, 255)),
    ("Mint", Color::rgb(105, 218, 166)),
    ("Berry", Color::rgb(248, 135, 181)),
    ("Apricot", Color::rgb(255, 177, 109)),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    Midnight,
}

/// Shifts each color channel by `delta`, clamping at the `u8` bounds. Used to
/// derive hover/pressed accent shades from whichever swatch is selected.
pub fn shade(color: Color, delta: i32) -> Color {
    let shift = |c: u8| (c as i32 + delta).clamp(0, 255) as u8;
    Color::rgb(shift(color.r), shift(color.g), shift(color.b))
}

pub fn accent_foreground(color: Color) -> Color {
    let luminance =
        (color.r as f32 * 0.2126 + color.g as f32 * 0.7152 + color.b as f32 * 0.0722) / 255.;
    if luminance > 0.56 {
        Color::rgb(0x2d, 0x29, 0x2b)
    } else {
        Color::rgb(0xff, 0xff, 0xff)
    }
}

pub fn build_theme(mode: ThemeMode, accent: Color) -> Theme {
    let mut theme = match mode {
        ThemeMode::Light => Theme::light(),
        ThemeMode::Dark => Theme::dark(),
        ThemeMode::Midnight => Theme::midnight(),
    };
    theme.colors.accent = accent;
    theme.colors.accent_hover = shade(accent, if mode == ThemeMode::Light { -12 } else { 20 });
    theme.colors.accent_pressed = shade(accent, -26);
    theme.colors.selection_background = accent;
    theme.colors.selection_text = accent_foreground(accent);
    theme
}

pub fn label_style() -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            // Auto so a wrapped second line grows the box instead of overflowing it.
            height: Dimension::Auto,
        },
        ..Default::default()
    }
}

/// Vertical rhythm between the cards inside a panel — wider than the
/// theme's own `spacing_large` so each topic reads as a separate block
/// instead of one continuous, undifferentiated column.
pub fn section_gap() -> f32 {
    let theme = use_theme();
    theme.spacing_large * 1.75
}

/// A section heading: a bold title plus a muted one-line description.
#[component]
pub fn SectionHeader(title: String, subtitle: String) -> BoxedWidget {
    let theme = use_theme();
    // Auto height so a wrapped title doesn't overflow into the subtitle.
    let heading_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={column(4.0)}>
            <Heading size={if use_screen_class() == ScreenClass::Compact { TextSize::Lg } else { TextSize::Xl }} style={heading_style}>{title}</Heading>
            <Text color={theme.text_secondary} align={TextAlign::Start} style={label_style()}>{subtitle}</Text>
        </RawView>
    })
}
