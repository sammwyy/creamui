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
    Actions,
    NumberInput,
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

pub const NAV_PAGES: [NavPage; 41] = [
    NavPage {
        page: ShowcasePage::Appearance,
        label: "Appearance",
        symbol: Symbol::Appearance,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Theming,
        label: "Theming",
        symbol: Symbol::Layers,
        group: NavGroup::Features,
    },
    NavPage {
        page: ShowcasePage::Hooks,
        label: "Hooks & state",
        symbol: Symbol::Bolt,
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
        symbol: Symbol::Text,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Input,
        label: "Input",
        symbol: Symbol::Text,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Pickers,
        label: "Pickers",
        symbol: Symbol::Calendar,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Images,
        label: "Images",
        symbol: Symbol::Image,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Button,
        label: "Button",
        symbol: Symbol::Button,
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
        symbol: Symbol::Toggle,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Feedback,
        label: "Feedback",
        symbol: Symbol::Bell,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Sidebar,
        label: "Sidebar",
        symbol: Symbol::Sidebar,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tabs,
        label: "Tabs",
        symbol: Symbol::Display,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Scroll,
        label: "Scroll",
        symbol: Symbol::List,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tree,
        label: "Tree",
        symbol: Symbol::Tree,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Table,
        label: "Table",
        symbol: Symbol::Table,
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
        symbol: Symbol::Layers,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Disclosure,
        label: "Disclosure",
        symbol: Symbol::ChevronDown,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::States,
        label: "States & alerts",
        symbol: Symbol::Bell,
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
        symbol: Symbol::ChevronRight,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Workspace,
        label: "Workspace",
        symbol: Symbol::Sidebar,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tags,
        label: "Tags & shortcuts",
        symbol: Symbol::Tag,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::InputGroups,
        label: "Input groups",
        symbol: Symbol::Button,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Actions,
        label: "Actions",
        symbol: Symbol::Bolt,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::NumberInput,
        label: "Number input",
        symbol: Symbol::Sliders,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Calendar,
        label: "Calendar",
        symbol: Symbol::Calendar,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Rating,
        label: "Rating",
        symbol: Symbol::Star,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Carousel,
        label: "Carousel",
        symbol: Symbol::Image,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Verification,
        label: "Verification",
        symbol: Symbol::Lock,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Notifications,
        label: "Notifications",
        symbol: Symbol::Bell,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Tooltip,
        label: "Tooltip",
        symbol: Symbol::Message,
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
        symbol: Symbol::Table,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Sheets,
        label: "Sheets",
        symbol: Symbol::Layers,
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
        symbol: Symbol::Chart,
        group: NavGroup::Components,
    },
    NavPage {
        page: ShowcasePage::Diff,
        label: "Diff",
        symbol: Symbol::Code,
        group: NavGroup::Components,
    },
];

impl ShowcasePage {
    /// One line describing what a component gallery page demonstrates.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Disclosure => {
                "Progressive disclosure that keeps secondary detail one click away."
            }
            Self::States => "Inline status messages, loading placeholders and empty results.",
            Self::Forms => "Labelled fields with hints, validation and a single clear submit.",
            Self::Pagination => "Step through long collections without losing your place.",
            Self::Workspace => "Status bars and steppers that frame multi-step work.",
            Self::Tags => "Compact labels, filters and keyboard shortcut hints.",
            Self::InputGroups => {
                "Fields with fixed prefixes and suffixes such as units or currency."
            }
            Self::Actions => "Toggle and split buttons for primary actions with alternatives.",
            Self::NumberInput => "Bounded numeric entry with stepper controls.",
            Self::Calendar => "A month grid for picking a single day.",
            Self::Rating => "Fractional star ratings for quick feedback.",
            Self::Carousel => "Paged content with arrow and indicator navigation.",
            Self::Verification => "One-time code entry split into individual cells.",
            Self::Notifications => "Transient toasts that confirm actions without interrupting.",
            Self::Tooltip => "Short contextual hints on hover and keyboard focus.",
            Self::Panels => "Resizable panes and draggable dashboard grids.",
            Self::DataTable => "Sortable, filterable rows with multi-selection.",
            Self::Sheets => "Edge-anchored panels for details and secondary tasks.",
            Self::Toolbar => "Grouped commands that overflow gracefully into a menu.",
            Self::Chart => "Line, area and bar series on a shared scale.",
            Self::Diff => "Unified and side-by-side patch review.",
            _ => "",
        }
    }

    pub fn label(self) -> &'static str {
        NAV_PAGES[self as usize].label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_entries_match_page_indices_and_group_order() {
        let mut labels = std::collections::HashSet::new();
        for (index, entry) in NAV_PAGES.iter().enumerate() {
            assert_eq!(entry.page as usize, index);
            assert!(labels.insert(entry.label));
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
