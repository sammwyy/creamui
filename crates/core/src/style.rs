//! CreamUI's component-independent style model.
//!
//! Layout remains powered by Taffy, but it is only one part of a style.  A
//! component is free to consume the paint and typography properties it
//! understands and ignore the rest.

use crate::TextAlign;
use creamui_theme::{Color, ColorScheme};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

/// Semantic colors resolved against the active [`ColorScheme`] at paint time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorToken {
    Surface,
    SurfaceElevated,
    SurfaceHover,
    Accent,
    AccentHover,
    AccentPressed,
    SelectionBackground,
    SelectionText,
    TextPrimary,
    TextSecondary,
    TextDisabled,
    Border,
    BorderStrong,
    Danger,
    Warning,
    Success,
}

impl ColorToken {
    pub const fn resolve(self, colors: &ColorScheme) -> Color {
        match self {
            Self::Surface => colors.surface,
            Self::SurfaceElevated => colors.surface_elevated,
            Self::SurfaceHover => colors.surface_hover,
            Self::Accent => colors.accent,
            Self::AccentHover => colors.accent_hover,
            Self::AccentPressed => colors.accent_pressed,
            Self::SelectionBackground => colors.selection_background,
            Self::SelectionText => colors.selection_text,
            Self::TextPrimary => colors.text_primary,
            Self::TextSecondary => colors.text_secondary,
            Self::TextDisabled => colors.text_disabled,
            Self::Border => colors.border,
            Self::BorderStrong => colors.border_strong,
            Self::Danger => colors.danger,
            Self::Warning => colors.warning,
            Self::Success => colors.success,
        }
    }
}

/// A concrete color or a semantic theme token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorValue {
    Literal(Color),
    Token(ColorToken),
}

impl ColorValue {
    pub const fn resolve(self, colors: &ColorScheme) -> Color {
        match self {
            Self::Literal(color) => color,
            Self::Token(token) => token.resolve(colors),
        }
    }
}

impl From<Color> for ColorValue {
    fn from(color: Color) -> Self {
        Self::Literal(color)
    }
}

impl From<ColorToken> for ColorValue {
    fn from(token: ColorToken) -> Self {
        Self::Token(token)
    }
}

impl From<&str> for ColorValue {
    fn from(value: &str) -> Self {
        value
            .parse()
            .unwrap_or_else(|error| panic!("invalid CreamUI color `{value}`: {error}"))
    }
}

impl From<String> for ColorValue {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl PartialEq<Color> for ColorValue {
    fn eq(&self, other: &Color) -> bool {
        matches!(self, Self::Literal(color) if color == other)
    }
}

impl PartialEq<ColorValue> for Color {
    fn eq(&self, other: &ColorValue) -> bool {
        other == self
    }
}

/// CSS-like lengths accepted by the declaration layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthValue {
    Auto,
    Px(f32),
    Percent(f32),
}

impl LengthValue {
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    pub(crate) fn dimension(self) -> crate::layout::Dimension {
        match self {
            Self::Auto => crate::layout::Dimension::Auto,
            Self::Px(value) => crate::layout::Dimension::Length(value),
            Self::Percent(value) => crate::layout::Dimension::Percent(value / 100.0),
        }
    }

    pub(crate) fn length_percentage(self) -> crate::layout::LengthPercentage {
        match self {
            Self::Px(value) => crate::layout::LengthPercentage::Length(value),
            Self::Percent(value) => crate::layout::LengthPercentage::Percent(value / 100.0),
            Self::Auto => panic!("`auto` is not valid for padding or gap"),
        }
    }

    pub(crate) fn length_percentage_auto(self) -> crate::layout::LengthPercentageAuto {
        match self {
            Self::Auto => crate::layout::LengthPercentageAuto::Auto,
            Self::Px(value) => crate::layout::LengthPercentageAuto::Length(value),
            Self::Percent(value) => crate::layout::LengthPercentageAuto::Percent(value / 100.0),
        }
    }
}

impl From<f32> for LengthValue {
    fn from(value: f32) -> Self {
        Self::Px(value)
    }
}

impl From<&str> for LengthValue {
    fn from(value: &str) -> Self {
        value
            .parse()
            .unwrap_or_else(|error| panic!("invalid CreamUI length `{value}`: {error}"))
    }
}

impl From<String> for LengthValue {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

/// Error returned by CSS-value and property parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleParseError(pub String);

impl fmt::Display for StyleParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StyleParseError {}

impl FromStr for LengthValue {
    type Err = StyleParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if input.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        if let Some(value) = input.strip_suffix("px") {
            return value
                .trim()
                .parse()
                .map(Self::Px)
                .map_err(|_| StyleParseError(format!("invalid pixel length `{input}`")));
        }
        if let Some(value) = input.strip_suffix('%') {
            return value
                .trim()
                .parse()
                .map(Self::Percent)
                .map_err(|_| StyleParseError(format!("invalid percentage `{input}`")));
        }
        Err(StyleParseError(format!("unsupported length `{input}`")))
    }
}

impl FromStr for ColorValue {
    type Err = StyleParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if let Some(hex) = input.strip_prefix('#') {
            let bytes = match hex.len() {
                6 | 8 => u32::from_str_radix(hex, 16)
                    .map_err(|_| StyleParseError(format!("invalid color `{input}`")))?,
                _ => return Err(StyleParseError(format!("invalid color `{input}`"))),
            };
            let (r, g, b, a) = if hex.len() == 6 {
                ((bytes >> 16) as u8, (bytes >> 8) as u8, bytes as u8, 255)
            } else {
                (
                    (bytes >> 24) as u8,
                    (bytes >> 16) as u8,
                    (bytes >> 8) as u8,
                    bytes as u8,
                )
            };
            return Ok(Self::Literal(Color::rgba(r, g, b, a)));
        }
        let token = match input
            .strip_prefix("var(--")
            .and_then(|value| value.strip_suffix(')'))
            .unwrap_or(input)
        {
            "surface" => ColorToken::Surface,
            "surface-elevated" => ColorToken::SurfaceElevated,
            "surface-hover" => ColorToken::SurfaceHover,
            "accent" | "primary" => ColorToken::Accent,
            "accent-hover" => ColorToken::AccentHover,
            "accent-pressed" => ColorToken::AccentPressed,
            "selection-background" => ColorToken::SelectionBackground,
            "selection-text" => ColorToken::SelectionText,
            "text-primary" => ColorToken::TextPrimary,
            "text-secondary" => ColorToken::TextSecondary,
            "text-disabled" => ColorToken::TextDisabled,
            "border" => ColorToken::Border,
            "border-strong" => ColorToken::BorderStrong,
            "danger" => ColorToken::Danger,
            "warning" => ColorToken::Warning,
            "success" => ColorToken::Success,
            _ => return Err(StyleParseError(format!("unsupported color `{input}`"))),
        };
        Ok(Self::Token(token))
    }
}

/// A border or outline declaration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub color: ColorValue,
    pub width: f32,
}

impl Border {
    pub fn new(color: impl Into<ColorValue>, width: f32) -> Self {
        Self {
            color: color.into(),
            width,
        }
    }
}

/// Properties painted behind and around a component's content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintStyle {
    pub background: Option<ColorValue>,
    pub border: Option<Border>,
    pub corner_radius: Option<f32>,
    /// An outline is painted outside the layout box and therefore does not
    /// participate in layout. It is commonly used by the focus state.
    pub outline: Option<Border>,
}

impl PaintStyle {
    fn patched(self, patch: Self) -> Self {
        Self {
            background: patch.background.or(self.background),
            border: patch.border.or(self.border),
            corner_radius: patch.corner_radius.or(self.corner_radius),
            outline: patch.outline.or(self.outline),
        }
    }
}

/// Text properties shared by text-producing components.
///
/// Every field is optional because a style is also used as a state patch.
/// A text component keeps its own semantic defaults for unspecified values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypographyStyle {
    pub color: Option<ColorValue>,
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    pub align: Option<TextAlign>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
}

impl TypographyStyle {
    fn patched(mut self, patch: &Self) -> Self {
        self.color = patch.color.or(self.color);
        self.font_size = patch.font_size.or(self.font_size);
        if patch.font_family.is_some() {
            self.font_family.clone_from(&patch.font_family);
        }
        self.align = patch.align.or(self.align);
        self.bold = patch.bold.or(self.bold);
        self.italic = patch.italic.or(self.italic);
        self.underline = patch.underline.or(self.underline);
        self.strikethrough = patch.strikethrough.or(self.strikethrough);
        self
    }
}

/// A non-layout style patch used by interaction states.
///
/// Keeping layout out of state patches makes hover/press transitions stable:
/// a hit target cannot move while the pointer is interacting with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateStyle {
    pub paint: PaintStyle,
    pub typography: TypographyStyle,
}

/// Shared metadata for CreamUI's typed style declarations.
#[doc(hidden)]
#[macro_export]
macro_rules! creamui_style_property_schema {
    ($consumer:ident) => {
        $consumer! {
            Background(crate::ColorValue) => "background" |target, value| { target.paint.background = Some(value); } => background(color: impl Into<crate::ColorValue>) |style| { style.paint.background = Some(color.into()); };
            Border(crate::Border) => "border" |target, value| { target.paint.border = Some(value); } => border(color: impl Into<crate::ColorValue>, width: f32) |style| { style.paint.border = Some(crate::Border::new(color, width)); };
            CornerRadius(f32) => "border-radius" |target, value| { target.paint.corner_radius = Some(value); } => corner_radius(radius: f32) |style| { style.paint.corner_radius = Some(radius); };
            Outline(crate::Border) => "outline" |target, value| { target.paint.outline = Some(value); } => outline(color: impl Into<crate::ColorValue>, width: f32) |style| { style.paint.outline = Some(crate::Border::new(color, width)); };
            Color(crate::ColorValue) => "color" |target, value| { target.typography.color = Some(value); } => color(color: impl Into<crate::ColorValue>) |style| { style.typography.color = Some(color.into()); };
            FontSize(f32) => "font-size" |target, value| { target.typography.font_size = Some(value); } => font_size(size: f32) |style| { style.typography.font_size = Some(size); };
            FontFamily(String) => "font-family" |target, value| { target.typography.font_family = Some(value); } => font_family(family: impl Into<String>) |style| { style.typography.font_family = Some(family.into()); };
            TextAlign(crate::TextAlign) => "text-align" |target, value| { target.typography.align = Some(value); } => text_align(align: crate::TextAlign) |style| { style.typography.align = Some(align); };
            Bold(bool) => "font-weight" |target, value| { target.typography.bold = Some(value); } => bold(active: bool) |style| { style.typography.bold = Some(active); };
            Italic(bool) => "font-style" |target, value| { target.typography.italic = Some(value); } => italic(active: bool) |style| { style.typography.italic = Some(active); };
            Underline(bool) => "text-decoration-underline" |target, value| { target.typography.underline = Some(value); } => underline(active: bool) |style| { style.typography.underline = Some(active); };
            Strikethrough(bool) => "text-decoration-line-through" |target, value| { target.typography.strikethrough = Some(value); } => strikethrough(active: bool) |style| { style.typography.strikethrough = Some(active); };
            Width(crate::LengthValue) => "width" |target, value| { target.layout.size.width = value.dimension(); } => width(value: impl Into<crate::LengthValue>) |style| { style.layout.size.width = value.into().dimension(); };
            Height(crate::LengthValue) => "height" |target, value| { target.layout.size.height = value.dimension(); } => height(value: impl Into<crate::LengthValue>) |style| { style.layout.size.height = value.into().dimension(); };
            MinWidth(crate::LengthValue) => "min-width" |target, value| { target.layout.min_size.width = value.dimension(); } => min_width(value: impl Into<crate::LengthValue>) |style| { style.layout.min_size.width = value.into().dimension(); };
            MinHeight(crate::LengthValue) => "min-height" |target, value| { target.layout.min_size.height = value.dimension(); } => min_height(value: impl Into<crate::LengthValue>) |style| { style.layout.min_size.height = value.into().dimension(); };
            MaxWidth(crate::LengthValue) => "max-width" |target, value| { target.layout.max_size.width = value.dimension(); } => max_width(value: impl Into<crate::LengthValue>) |style| { style.layout.max_size.width = value.into().dimension(); };
            MaxHeight(crate::LengthValue) => "max-height" |target, value| { target.layout.max_size.height = value.dimension(); } => max_height(value: impl Into<crate::LengthValue>) |style| { style.layout.max_size.height = value.into().dimension(); };
            Display(crate::layout::Display) => "display" |target, value| { target.layout.display = value; } => display(value: crate::layout::Display) |style| { style.layout.display = value; };
            FlexDirection(crate::layout::FlexDirection) => "flex-direction" |target, value| { target.layout.flex_direction = value; } => flex_direction(value: crate::layout::FlexDirection) |style| { style.layout.flex_direction = value; };
            FlexWrap(crate::layout::FlexWrap) => "flex-wrap" |target, value| { target.layout.flex_wrap = value; } => flex_wrap(value: crate::layout::FlexWrap) |style| { style.layout.flex_wrap = value; };
            FlexGrow(f32) => "flex-grow" |target, value| { target.layout.flex_grow = value; } => flex_grow(value: f32) |style| { style.layout.flex_grow = value; };
            FlexShrink(f32) => "flex-shrink" |target, value| { target.layout.flex_shrink = value; } => flex_shrink(value: f32) |style| { style.layout.flex_shrink = value; };
            FlexBasis(crate::LengthValue) => "flex-basis" |target, value| { target.layout.flex_basis = value.dimension(); } => flex_basis(value: impl Into<crate::LengthValue>) |style| { style.layout.flex_basis = value.into().dimension(); };
            Gap(crate::LengthValue) => "gap" |target, value| { let value = value.length_percentage(); target.layout.gap = crate::layout::Size { width: value, height: value }; } => gap(value: impl Into<crate::LengthValue>) |style| { let value = value.into().length_percentage(); style.layout.gap = crate::layout::Size { width: value, height: value }; };
            RowGap(crate::LengthValue) => "row-gap" |target, value| { target.layout.gap.height = value.length_percentage(); } => row_gap(value: impl Into<crate::LengthValue>) |style| { style.layout.gap.height = value.into().length_percentage(); };
            ColumnGap(crate::LengthValue) => "column-gap" |target, value| { target.layout.gap.width = value.length_percentage(); } => column_gap(value: impl Into<crate::LengthValue>) |style| { style.layout.gap.width = value.into().length_percentage(); };
            Padding(crate::LengthValue) => "padding" |target, value| { let value = value.length_percentage(); target.layout.padding = crate::layout::Rect { left: value, right: value, top: value, bottom: value }; } => padding(value: impl Into<crate::LengthValue>) |style| { let value = value.into().length_percentage(); style.layout.padding = crate::layout::Rect { left: value, right: value, top: value, bottom: value }; };
            PaddingTop(crate::LengthValue) => "padding-top" |target, value| { target.layout.padding.top = value.length_percentage(); } => padding_top(value: impl Into<crate::LengthValue>) |style| { style.layout.padding.top = value.into().length_percentage(); };
            PaddingRight(crate::LengthValue) => "padding-right" |target, value| { target.layout.padding.right = value.length_percentage(); } => padding_right(value: impl Into<crate::LengthValue>) |style| { style.layout.padding.right = value.into().length_percentage(); };
            PaddingBottom(crate::LengthValue) => "padding-bottom" |target, value| { target.layout.padding.bottom = value.length_percentage(); } => padding_bottom(value: impl Into<crate::LengthValue>) |style| { style.layout.padding.bottom = value.into().length_percentage(); };
            PaddingLeft(crate::LengthValue) => "padding-left" |target, value| { target.layout.padding.left = value.length_percentage(); } => padding_left(value: impl Into<crate::LengthValue>) |style| { style.layout.padding.left = value.into().length_percentage(); };
            Margin(crate::LengthValue) => "margin" |target, value| { let value = value.length_percentage_auto(); target.layout.margin = crate::layout::Rect { left: value, right: value, top: value, bottom: value }; } => margin(value: impl Into<crate::LengthValue>) |style| { let value = value.into().length_percentage_auto(); style.layout.margin = crate::layout::Rect { left: value, right: value, top: value, bottom: value }; };
            MarginTop(crate::LengthValue) => "margin-top" |target, value| { target.layout.margin.top = value.length_percentage_auto(); } => margin_top(value: impl Into<crate::LengthValue>) |style| { style.layout.margin.top = value.into().length_percentage_auto(); };
            MarginRight(crate::LengthValue) => "margin-right" |target, value| { target.layout.margin.right = value.length_percentage_auto(); } => margin_right(value: impl Into<crate::LengthValue>) |style| { style.layout.margin.right = value.into().length_percentage_auto(); };
            MarginBottom(crate::LengthValue) => "margin-bottom" |target, value| { target.layout.margin.bottom = value.length_percentage_auto(); } => margin_bottom(value: impl Into<crate::LengthValue>) |style| { style.layout.margin.bottom = value.into().length_percentage_auto(); };
            MarginLeft(crate::LengthValue) => "margin-left" |target, value| { target.layout.margin.left = value.length_percentage_auto(); } => margin_left(value: impl Into<crate::LengthValue>) |style| { style.layout.margin.left = value.into().length_percentage_auto(); };
            AlignItems(crate::layout::AlignItems) => "align-items" |target, value| { target.layout.align_items = Some(value); } => align_items(value: crate::layout::AlignItems) |style| { style.layout.align_items = Some(value); };
            AlignSelf(crate::layout::AlignSelf) => "align-self" |target, value| { target.layout.align_self = Some(value); } => self_alignment(value: crate::layout::AlignSelf) |style| { style.layout.align_self = Some(value); };
            JustifyContent(crate::layout::JustifyContent) => "justify-content" |target, value| { target.layout.justify_content = Some(value); } => justify_content(value: crate::layout::JustifyContent) |style| { style.layout.justify_content = Some(value); };
            AlignContent(crate::layout::AlignContent) => "align-content" |target, value| { target.layout.align_content = Some(value); } => content_alignment(value: crate::layout::AlignContent) |style| { style.layout.align_content = Some(value); };
            Position(crate::layout::Position) => "position" |target, value| { target.layout.position = value; } => position(value: crate::layout::Position) |style| { style.layout.position = value; };
            Top(crate::LengthValue) => "top" |target, value| { target.layout.inset.top = value.length_percentage_auto(); } => top(value: impl Into<crate::LengthValue>) |style| { style.layout.inset.top = value.into().length_percentage_auto(); };
            Right(crate::LengthValue) => "right" |target, value| { target.layout.inset.right = value.length_percentage_auto(); } => right(value: impl Into<crate::LengthValue>) |style| { style.layout.inset.right = value.into().length_percentage_auto(); };
            Bottom(crate::LengthValue) => "bottom" |target, value| { target.layout.inset.bottom = value.length_percentage_auto(); } => bottom(value: impl Into<crate::LengthValue>) |style| { style.layout.inset.bottom = value.into().length_percentage_auto(); };
            Left(crate::LengthValue) => "left" |target, value| { target.layout.inset.left = value.length_percentage_auto(); } => left(value: impl Into<crate::LengthValue>) |style| { style.layout.inset.left = value.into().length_percentage_auto(); };
            GridTemplateColumns(Vec<crate::layout::TrackSizingFunction>) => "grid-template-columns" |target, value| { target.layout.grid_template_columns = value.into(); } => grid_template_columns(tracks: impl IntoIterator<Item = crate::layout::TrackSizingFunction>) |style| { style.layout.grid_template_columns = tracks.into_iter().collect(); };
            GridTemplateRows(Vec<crate::layout::TrackSizingFunction>) => "grid-template-rows" |target, value| { target.layout.grid_template_rows = value.into(); } => grid_template_rows(tracks: impl IntoIterator<Item = crate::layout::TrackSizingFunction>) |style| { style.layout.grid_template_rows = tracks.into_iter().collect(); };
            GridAutoFlow(crate::layout::GridAutoFlow) => "grid-auto-flow" |target, value| { target.layout.grid_auto_flow = value; } => grid_auto_flow(value: crate::layout::GridAutoFlow) |style| { style.layout.grid_auto_flow = value; };
        }
    };
}

macro_rules! common_value_builders {
    () => {
        pub fn background(mut self, color: impl Into<ColorValue>) -> Self {
            self.paint.background = Some(color.into());
            self
        }

        pub fn border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
            self.paint.border = Some(Border::new(color, width));
            self
        }

        pub const fn corner_radius(mut self, radius: f32) -> Self {
            self.paint.corner_radius = Some(radius);
            self
        }

        pub fn outline(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
            self.paint.outline = Some(Border::new(color, width));
            self
        }

        pub fn color(mut self, color: impl Into<ColorValue>) -> Self {
            self.typography.color = Some(color.into());
            self
        }

        pub const fn font_size(mut self, size: f32) -> Self {
            self.typography.font_size = Some(size);
            self
        }

        pub fn font_family(mut self, family: impl Into<String>) -> Self {
            self.typography.font_family = Some(family.into());
            self
        }

        pub const fn text_align(mut self, align: TextAlign) -> Self {
            self.typography.align = Some(align);
            self
        }

        pub const fn bold(mut self, bold: bool) -> Self {
            self.typography.bold = Some(bold);
            self
        }

        pub const fn italic(mut self, italic: bool) -> Self {
            self.typography.italic = Some(italic);
            self
        }

        pub const fn underline(mut self, underline: bool) -> Self {
            self.typography.underline = Some(underline);
            self
        }

        pub const fn strikethrough(mut self, strikethrough: bool) -> Self {
            self.typography.strikethrough = Some(strikethrough);
            self
        }
    };
}

impl StateStyle {
    pub const fn new() -> Self {
        Self {
            paint: PaintStyle {
                background: None,
                border: None,
                corner_radius: None,
                outline: None,
            },
            typography: TypographyStyle {
                color: None,
                font_size: None,
                font_family: None,
                align: None,
                bold: None,
                italic: None,
                underline: None,
                strikethrough: None,
            },
        }
    }

    common_value_builders!();
}

/// Interaction-state patches associated with a common [`Style`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractionStyles {
    pub hover: StateStyle,
    pub pressed: StateStyle,
    pub focus: StateStyle,
    pub disabled: StateStyle,
}

/// The common style accepted by CreamUI components.
///
/// Existing `creamui_core::layout::Style` values convert into this type, so
/// applications can migrate without rewriting their Taffy layout literals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub layout: crate::layout::Style,
    pub paint: PaintStyle,
    pub typography: TypographyStyle,
    pub states: InteractionStyles,
}

macro_rules! define_style_builders {
    ($( $variant:ident($value:ty) => $name:literal |$target:ident, $field:ident| $apply:block => $builder:ident($( $argument:ident: $argument_type:ty ),*) |$style:ident| $body:block; )*) => {
        $(
            pub fn $builder(mut self, $( $argument: $argument_type ),*) -> Self {
                let $style = &mut self;
                $body
                self
            }
        )*
    };
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn layout(mut self, layout: crate::layout::Style) -> Self {
        self.layout = layout;
        self
    }

    crate::creamui_style_property_schema!(define_style_builders);

    pub fn hover(mut self, style: StateStyle) -> Self {
        self.states.hover = style;
        self
    }

    pub fn pressed(mut self, style: StateStyle) -> Self {
        self.states.pressed = style;
        self
    }

    pub fn focus(mut self, style: StateStyle) -> Self {
        self.states.focus = style;
        self
    }

    pub fn disabled(mut self, style: StateStyle) -> Self {
        self.states.disabled = style;
        self
    }

    /// Resolves a state patch over the base declaration. Pressed inherits
    /// hover first, matching the usual pointer-state cascade.
    pub fn resolve(&self, state: impl Into<StyleState>) -> ResolvedStyle {
        let state = state.into();
        let mut paint = self.paint;
        let mut typography = self.typography.clone();
        let mut apply = |patch: &StateStyle| {
            paint = paint.patched(patch.paint);
            typography = typography.clone().patched(&patch.typography);
        };
        if state.hovered() {
            apply(&self.states.hover);
        }
        if state.focused() {
            apply(&self.states.focus);
        }
        if state.pressed() {
            apply(&self.states.pressed);
        }
        if state.disabled() {
            apply(&self.states.disabled);
        }
        ResolvedStyle { paint, typography }
    }
}

impl From<crate::layout::Style> for Style {
    fn from(layout: crate::layout::Style) -> Self {
        Self {
            layout,
            ..Self::default()
        }
    }
}

impl From<Style> for crate::layout::Style {
    fn from(style: Style) -> Self {
        style.layout
    }
}

impl AsRef<crate::layout::Style> for Style {
    fn as_ref(&self) -> &crate::layout::Style {
        &self.layout
    }
}

impl Deref for Style {
    type Target = crate::layout::Style;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl DerefMut for Style {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.layout
    }
}

/// Composable pseudo-class state. Multiple flags can be active together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleState(u8);

impl StyleState {
    const HOVERED: u8 = 1 << 0;
    const FOCUSED: u8 = 1 << 1;
    const PRESSED: u8 = 1 << 2;
    const DISABLED: u8 = 1 << 3;

    pub const NORMAL: Self = Self(0);

    pub const fn hovered(self) -> bool {
        self.0 & Self::HOVERED != 0
    }

    pub const fn focused(self) -> bool {
        self.0 & Self::FOCUSED != 0
    }

    pub const fn pressed(self) -> bool {
        self.0 & Self::PRESSED != 0
    }

    pub const fn disabled(self) -> bool {
        self.0 & Self::DISABLED != 0
    }

    pub const fn with_hovered(mut self, active: bool) -> Self {
        self.0 = if active {
            self.0 | Self::HOVERED
        } else {
            self.0 & !Self::HOVERED
        };
        self
    }

    pub const fn with_focused(mut self, active: bool) -> Self {
        self.0 = if active {
            self.0 | Self::FOCUSED
        } else {
            self.0 & !Self::FOCUSED
        };
        self
    }

    pub const fn with_pressed(mut self, active: bool) -> Self {
        self.0 = if active {
            self.0 | Self::PRESSED
        } else {
            self.0 & !Self::PRESSED
        };
        self
    }

    pub const fn with_disabled(mut self, active: bool) -> Self {
        self.0 = if active {
            self.0 | Self::DISABLED
        } else {
            self.0 & !Self::DISABLED
        };
        self
    }
}

/// Compatibility adapter for code that previously selected one state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InteractionState {
    #[default]
    Normal,
    Hover,
    Pressed,
    Focus,
    Disabled,
}

impl From<InteractionState> for StyleState {
    fn from(state: InteractionState) -> Self {
        match state {
            InteractionState::Normal => Self::NORMAL,
            InteractionState::Hover => Self::NORMAL.with_hovered(true),
            InteractionState::Pressed => Self::NORMAL.with_hovered(true).with_pressed(true),
            InteractionState::Focus => Self::NORMAL.with_focused(true),
            InteractionState::Disabled => Self::NORMAL.with_disabled(true),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedStyle {
    pub paint: PaintStyle,
    pub typography: TypographyStyle,
}

macro_rules! style_properties {
    ($( $variant:ident($value:ty) => $name:literal |$target:ident, $field:ident| $apply:block => $builder:ident($( $argument:ident: $argument_type:ty ),*) |$style:ident| $body:block; )*) => {
        /// A parsed, typed declaration. This is the dynamic/CSS boundary;
        /// rendering uses the compiled [`Style`] instead of matching a bag
        /// of properties every frame.
        #[derive(Clone, Debug, PartialEq)]
        pub enum StyleProp {
            $( $variant($value), )+
        }

        impl StyleProp {
            pub const fn name(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => $name, )+
                }
            }

            fn apply_to(self, style: &mut Style) {
                match self {
                    $( Self::$variant(field) => {
                        let $target = style;
                        let $field = field;
                        $apply
                    }, )+
                }
            }
        }
    };
}

crate::creamui_style_property_schema!(style_properties);

impl StyleProp {
    /// Parses one CSS-like name/value pair into a typed declaration.
    pub fn parse(name: &str, value: &str) -> Result<Self, StyleParseError> {
        let number = |value: &str| {
            value
                .trim()
                .strip_suffix("px")
                .unwrap_or(value.trim())
                .parse::<f32>()
                .map_err(|_| StyleParseError(format!("invalid number `{value}`")))
        };
        let boolean = |value: &str| match value.trim() {
            "true" | "yes" | "1" => Ok(true),
            "false" | "no" | "0" => Ok(false),
            other => Err(StyleParseError(format!("invalid boolean `{other}`"))),
        };
        let border = |value: &str| {
            let mut parts = value.split_whitespace();
            let width = parts
                .next()
                .ok_or_else(|| StyleParseError("border width is missing".into()))?;
            let color = parts
                .next()
                .ok_or_else(|| StyleParseError("border color is missing".into()))?;
            if parts.next().is_some() {
                return Err(StyleParseError(format!("invalid border `{value}`")));
            }
            Ok(Border::new(color.parse::<ColorValue>()?, number(width)?))
        };
        let align_items = |value: &str| match value.trim() {
            "start" => Ok(crate::layout::AlignItems::Start),
            "end" => Ok(crate::layout::AlignItems::End),
            "flex-start" => Ok(crate::layout::AlignItems::FlexStart),
            "flex-end" => Ok(crate::layout::AlignItems::FlexEnd),
            "center" => Ok(crate::layout::AlignItems::Center),
            "baseline" => Ok(crate::layout::AlignItems::Baseline),
            "stretch" => Ok(crate::layout::AlignItems::Stretch),
            other => Err(StyleParseError(format!("invalid alignment `{other}`"))),
        };
        let align_content = |value: &str| match value.trim() {
            "start" => Ok(crate::layout::AlignContent::Start),
            "end" => Ok(crate::layout::AlignContent::End),
            "flex-start" => Ok(crate::layout::AlignContent::FlexStart),
            "flex-end" => Ok(crate::layout::AlignContent::FlexEnd),
            "center" => Ok(crate::layout::AlignContent::Center),
            "stretch" => Ok(crate::layout::AlignContent::Stretch),
            "space-between" => Ok(crate::layout::AlignContent::SpaceBetween),
            "space-evenly" => Ok(crate::layout::AlignContent::SpaceEvenly),
            "space-around" => Ok(crate::layout::AlignContent::SpaceAround),
            other => Err(StyleParseError(format!(
                "invalid content alignment `{other}`"
            ))),
        };
        let spacing = |value: &str| match value.parse::<LengthValue>()? {
            LengthValue::Auto => Err(StyleParseError(format!(
                "`auto` is not valid for this spacing property: `{value}`"
            ))),
            value => Ok(value),
        };

        match name.trim() {
            "background" | "background-color" => Ok(Self::Background(value.parse()?)),
            "border" => Ok(Self::Border(border(value)?)),
            "border-radius" => Ok(Self::CornerRadius(number(value)?)),
            "outline" => Ok(Self::Outline(border(value)?)),
            "color" => Ok(Self::Color(value.parse()?)),
            "font-size" => Ok(Self::FontSize(number(value)?)),
            "font-family" => Ok(Self::FontFamily(value.trim().to_owned())),
            "text-align" => Ok(Self::TextAlign(match value.trim() {
                "left" | "start" => TextAlign::Start,
                "center" => TextAlign::Center,
                "right" | "end" => TextAlign::End,
                other => return Err(StyleParseError(format!("invalid text-align `{other}`"))),
            })),
            "font-weight" => Ok(Self::Bold(matches!(
                value.trim(),
                "bold" | "700" | "800" | "900"
            ))),
            "font-style" => Ok(Self::Italic(matches!(value.trim(), "italic" | "oblique"))),
            "text-decoration-underline" => Ok(Self::Underline(boolean(value)?)),
            "text-decoration-line-through" => Ok(Self::Strikethrough(boolean(value)?)),
            "width" => Ok(Self::Width(value.parse()?)),
            "height" => Ok(Self::Height(value.parse()?)),
            "min-width" => Ok(Self::MinWidth(value.parse()?)),
            "min-height" => Ok(Self::MinHeight(value.parse()?)),
            "max-width" => Ok(Self::MaxWidth(value.parse()?)),
            "max-height" => Ok(Self::MaxHeight(value.parse()?)),
            "display" => Ok(Self::Display(match value.trim() {
                "block" => crate::layout::Display::Block,
                "flex" => crate::layout::Display::Flex,
                "grid" => crate::layout::Display::Grid,
                "none" => crate::layout::Display::None,
                other => return Err(StyleParseError(format!("invalid display `{other}`"))),
            })),
            "flex-direction" => Ok(Self::FlexDirection(match value.trim() {
                "row" => crate::layout::FlexDirection::Row,
                "row-reverse" => crate::layout::FlexDirection::RowReverse,
                "column" => crate::layout::FlexDirection::Column,
                "column-reverse" => crate::layout::FlexDirection::ColumnReverse,
                other => return Err(StyleParseError(format!("invalid flex-direction `{other}`"))),
            })),
            "flex-wrap" => Ok(Self::FlexWrap(match value.trim() {
                "nowrap" => crate::layout::FlexWrap::NoWrap,
                "wrap" => crate::layout::FlexWrap::Wrap,
                "wrap-reverse" => crate::layout::FlexWrap::WrapReverse,
                other => return Err(StyleParseError(format!("invalid flex-wrap `{other}`"))),
            })),
            "flex-grow" => Ok(Self::FlexGrow(number(value)?)),
            "flex-shrink" => Ok(Self::FlexShrink(number(value)?)),
            "flex-basis" => Ok(Self::FlexBasis(value.parse()?)),
            "gap" => Ok(Self::Gap(spacing(value)?)),
            "row-gap" => Ok(Self::RowGap(spacing(value)?)),
            "column-gap" => Ok(Self::ColumnGap(spacing(value)?)),
            "padding" => Ok(Self::Padding(spacing(value)?)),
            "padding-top" => Ok(Self::PaddingTop(spacing(value)?)),
            "padding-right" => Ok(Self::PaddingRight(spacing(value)?)),
            "padding-bottom" => Ok(Self::PaddingBottom(spacing(value)?)),
            "padding-left" => Ok(Self::PaddingLeft(spacing(value)?)),
            "margin" => Ok(Self::Margin(value.parse()?)),
            "margin-top" => Ok(Self::MarginTop(value.parse()?)),
            "margin-right" => Ok(Self::MarginRight(value.parse()?)),
            "margin-bottom" => Ok(Self::MarginBottom(value.parse()?)),
            "margin-left" => Ok(Self::MarginLeft(value.parse()?)),
            "align-items" => Ok(Self::AlignItems(align_items(value)?)),
            "align-self" => Ok(Self::AlignSelf(align_items(value)?)),
            "justify-content" => Ok(Self::JustifyContent(align_content(value)?)),
            "align-content" => Ok(Self::AlignContent(align_content(value)?)),
            "position" => Ok(Self::Position(match value.trim() {
                "relative" => crate::layout::Position::Relative,
                "absolute" => crate::layout::Position::Absolute,
                other => return Err(StyleParseError(format!("invalid position `{other}`"))),
            })),
            "top" => Ok(Self::Top(value.parse()?)),
            "right" => Ok(Self::Right(value.parse()?)),
            "bottom" => Ok(Self::Bottom(value.parse()?)),
            "left" => Ok(Self::Left(value.parse()?)),
            other => Err(StyleParseError(format!("unknown style property `{other}`"))),
        }
    }
}

impl Style {
    /// Compiles one typed declaration into this style.
    pub fn apply(&mut self, property: StyleProp) {
        property.apply_to(self);
    }

    /// Fluent equivalent of [`Style::apply`].
    pub fn property(mut self, property: StyleProp) -> Self {
        self.apply(property);
        self
    }

    pub fn properties(mut self, properties: impl IntoIterator<Item = StyleProp>) -> Self {
        for property in properties {
            self.apply(property);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_cascades_over_hover_and_base_without_touching_layout() {
        let base = Color::rgb(1, 2, 3);
        let hover_border = Color::rgb(4, 5, 6);
        let pressed = Color::rgb(7, 8, 9);
        let style = Style::new()
            .background(base)
            .corner_radius(2.0)
            .hover(
                StateStyle::new()
                    .border(hover_border, 3.0)
                    .corner_radius(6.0),
            )
            .pressed(StateStyle::new().background(pressed));

        let resolved = style.resolve(InteractionState::Pressed);
        assert_eq!(resolved.paint.background, Some(pressed.into()));
        assert_eq!(resolved.paint.border, Some(Border::new(hover_border, 3.0)));
        assert_eq!(resolved.paint.corner_radius, Some(6.0));
    }

    #[test]
    fn pseudo_states_compose_in_a_deterministic_cascade() {
        let style = Style::new()
            .background(Color::rgb(1, 1, 1))
            .hover(StateStyle::new().background(Color::rgb(2, 2, 2)))
            .focus(StateStyle::new().border(ColorToken::Accent, 2.0))
            .pressed(StateStyle::new().background(Color::rgb(3, 3, 3)))
            .disabled(StateStyle::new().background(ColorToken::TextDisabled));
        let states = StyleState::NORMAL
            .with_hovered(true)
            .with_focused(true)
            .with_pressed(true)
            .with_disabled(true);
        let resolved = style.resolve(states);

        assert_eq!(
            resolved.paint.background,
            Some(ColorValue::Token(ColorToken::TextDisabled))
        );
        assert_eq!(
            resolved.paint.border,
            Some(Border::new(ColorToken::Accent, 2.0))
        );
    }

    #[test]
    fn parsed_properties_compile_to_the_typed_style() {
        let style = Style::new().properties([
            StyleProp::parse("background", "var(--accent)").unwrap(),
            StyleProp::parse("width", "75%").unwrap(),
            StyleProp::parse("height", "32px").unwrap(),
            StyleProp::parse("border", "2px #102030").unwrap(),
        ]);

        assert_eq!(style.paint.background, Some(ColorToken::Accent.into()));
        assert_eq!(
            style.layout.size.width,
            crate::layout::Dimension::Percent(0.75)
        );
        assert_eq!(
            style.layout.size.height,
            crate::layout::Dimension::Length(32.0)
        );
        assert_eq!(
            style.paint.border,
            Some(Border::new(Color::rgb(0x10, 0x20, 0x30), 2.0))
        );
    }

    #[test]
    fn layout_properties_compile_without_a_layout_adapter() {
        let style = Style::new().properties([
            StyleProp::parse("display", "flex").unwrap(),
            StyleProp::parse("flex-direction", "column").unwrap(),
            StyleProp::parse("gap", "12px").unwrap(),
            StyleProp::parse("padding-left", "10px").unwrap(),
            StyleProp::parse("margin", "auto").unwrap(),
            StyleProp::parse("align-items", "center").unwrap(),
            StyleProp::parse("justify-content", "space-between").unwrap(),
            StyleProp::parse("position", "absolute").unwrap(),
            StyleProp::parse("top", "25%").unwrap(),
        ]);

        assert_eq!(style.layout.display, crate::layout::Display::Flex);
        assert_eq!(
            style.layout.flex_direction,
            crate::layout::FlexDirection::Column
        );
        assert_eq!(
            style.layout.gap,
            crate::layout::Size {
                width: crate::layout::LengthPercentage::Length(12.0),
                height: crate::layout::LengthPercentage::Length(12.0),
            }
        );
        assert_eq!(
            style.layout.padding.left,
            crate::layout::LengthPercentage::Length(10.0)
        );
        assert_eq!(
            style.layout.margin.left,
            crate::layout::LengthPercentageAuto::Auto
        );
        assert_eq!(
            style.layout.align_items,
            Some(crate::layout::AlignItems::Center)
        );
        assert_eq!(
            style.layout.justify_content,
            Some(crate::layout::JustifyContent::SpaceBetween)
        );
        assert_eq!(style.layout.position, crate::layout::Position::Absolute);
        assert_eq!(
            style.layout.inset.top,
            crate::layout::LengthPercentageAuto::Percent(0.25)
        );
    }

    #[test]
    fn parser_rejects_auto_for_padding_and_gap() {
        assert!(StyleProp::parse("padding", "auto").is_err());
        assert!(StyleProp::parse("gap", "auto").is_err());
    }

    #[test]
    fn semantic_color_tokens_resolve_against_each_scheme() {
        let value = ColorValue::Token(ColorToken::Accent);
        assert_eq!(
            value.resolve(&ColorScheme::dark()),
            ColorScheme::dark().accent
        );
        assert_eq!(
            value.resolve(&ColorScheme::light()),
            ColorScheme::light().accent
        );
    }
}
