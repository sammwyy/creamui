//! General-purpose visual primitives and selectable controls.
use crate::RawText;
use creamui_core::layout::{AlignItems, Style};
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Painter, Point, Rect, RgbaImage, Styled, TextAlign,
    Widget,
};
use creamui_theme::{use_theme, Color, Theme};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symbol {
    Appearance,
    Display,
    Controls,
    Keyboard,
    Check,
    ChevronRight,
    ChevronLeft,
    ChevronDown,
    Search,
    Sun,
    Moon,
    Folder,
    Grid,
    Sliders,
    /// A paper-plane / "send message" arrow.
    Send,
    /// A paperclip, used for attaching a file.
    Attachment,
    /// A generic picture placeholder (frame + horizon + sun).
    Image,
    /// An "×" dismiss/remove glyph.
    Close,
    /// A serif-less "T" for typography and text.
    Text,
    /// A rounded push-button outline.
    Button,
    /// A pill with a knob, for boolean toggles.
    Toggle,
    /// A bulleted list.
    List,
    /// A table with a header row.
    Table,
    /// A notification bell.
    Bell,
    /// A calendar page.
    Calendar,
    /// A five-pointed star.
    Star,
    /// Stacked layers.
    Layers,
    /// A bar chart.
    Chart,
    /// A price-tag label.
    Tag,
    /// A branching hierarchy.
    Tree,
    /// A lightning bolt for events and actions.
    Bolt,
    /// Angle brackets for code and diffs.
    Code,
    /// A padlock for verification and security.
    Lock,
    /// A speech bubble for hints and tooltips.
    Message,
    /// A window with a side pane.
    Sidebar,
}

/// Already-decoded RGBA8 pixels for a custom (non-vector) icon. Kept
/// dependency-free of any specific image-decoding crate — apps that bundle
/// SVG/PNG icons hand in bytes they rasterized themselves.
#[derive(Clone)]
pub struct IconImage {
    pub image: RgbaImage,
    /// When true, every visible pixel is recolored to the `color` an
    /// [`Icon`] is drawn with (its alpha is all that survives) — for a
    /// single-color icon that must track the active/hover state and theme
    /// the same way a [`Symbol`] does. When false (a photo, an app's own
    /// branded icon), pixels are drawn as decoded.
    pub monochrome: bool,
}

/// What an [`Icon`]/[`NavigationItem`]/[`crate::SidebarNode`] draws: one of
/// the built-in vector [`Symbol`]s, a pre-rasterized bitmap, or a filled
/// circle with a single letter — the avatar fallback for an account with no
/// picture.
#[derive(Clone)]
pub enum IconSource {
    Symbol(Symbol),
    Image(IconImage),
    Initial {
        letter: char,
        background: Color,
        text_color: Color,
    },
}
impl From<Symbol> for IconSource {
    fn from(symbol: Symbol) -> Self {
        IconSource::Symbol(symbol)
    }
}

fn circle(cx: f32, cy: f32, r: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// A closed rounded-rectangle outline on the icon grid.
fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<(f32, f32)> {
    let r = r.min(w / 2.).min(h / 2.);
    let corners = [
        (x + w - r, y + r, -std::f32::consts::FRAC_PI_2),
        (x + w - r, y + h - r, 0.),
        (x + r, y + h - r, std::f32::consts::FRAC_PI_2),
        (x + r, y + r, std::f32::consts::PI),
    ];
    let mut points: Vec<(f32, f32)> = corners
        .iter()
        .flat_map(|&(cx, cy, start)| {
            (0..=4).map(move |i| {
                let a = start + i as f32 / 4. * std::f32::consts::FRAC_PI_2;
                (cx + r * a.cos(), cy + r * a.sin())
            })
        })
        .collect();
    points.push(points[0]);
    points
}

/// A small, consistent line icon. Paths use a 24-unit optical grid.
pub struct Icon {
    pub source: IconSource,
    pub color: Color,
    pub size: f32,
}
impl Icon {
    pub fn new(source: impl Into<IconSource>, color: Color) -> Self {
        Self {
            source: source.into(),
            color,
            size: 18.,
        }
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    pub fn draw(
        source: impl Into<IconSource>,
        painter: &mut dyn Painter,
        rect: Rect,
        color: Color,
    ) {
        let symbol = match source.into() {
            IconSource::Image(image) => {
                painter.draw_image(rect, &image.image, image.monochrome.then_some(color));
                return;
            }
            IconSource::Initial {
                letter,
                background,
                text_color,
            } => {
                let side = rect.width.min(rect.height);
                let circle = Rect {
                    x: rect.x + (rect.width - side) / 2.,
                    y: rect.y + (rect.height - side) / 2.,
                    width: side,
                    height: side,
                };
                painter.fill_rect(circle, background, side / 2.);
                painter.fill_text(
                    circle,
                    &letter.to_string(),
                    text_color,
                    side * 0.55,
                    TextAlign::Center,
                );
                return;
            }
            IconSource::Symbol(symbol) => symbol,
        };
        let unit = rect.width.min(rect.height) / 24.;
        let p = |x: f32, y: f32| Point {
            x: rect.x + x * unit,
            y: rect.y + y * unit,
        };
        let mut line = |points: &[(f32, f32)]| {
            for pair in points.windows(2) {
                painter.stroke_line(
                    p(pair[0].0, pair[0].1),
                    p(pair[1].0, pair[1].1),
                    color,
                    1.65 * unit,
                );
            }
        };
        match symbol {
            Symbol::Check => line(&[(5., 12.), (10., 17.), (19., 7.)]),
            Symbol::ChevronRight => line(&[(9., 6.), (15., 12.), (9., 18.)]),
            Symbol::ChevronLeft => line(&[(15., 6.), (9., 12.), (15., 18.)]),
            Symbol::ChevronDown => line(&[(6., 9.), (12., 15.), (18., 9.)]),
            Symbol::Display => {
                line(&[(3., 4.), (21., 4.), (21., 17.), (3., 17.), (3., 4.)]);
                line(&[(12., 17.), (12., 21.)]);
                line(&[(8., 21.), (16., 21.)]);
            }
            Symbol::Folder => line(&[
                (3., 7.),
                (3., 4.),
                (10., 4.),
                (13., 7.),
                (21., 7.),
                (21., 20.),
                (3., 20.),
                (3., 7.),
                (21., 7.),
            ]),
            Symbol::Keyboard => {
                line(&[(2., 6.), (22., 6.), (22., 19.), (2., 19.), (2., 6.)]);
                for y in [10., 13.] {
                    for x in [6., 10., 14., 18.] {
                        line(&[(x, y), (x + 0.5, y)]);
                    }
                }
                line(&[(7., 16.), (17., 16.)]);
            }
            Symbol::Controls | Symbol::Sliders => {
                for (y, knob) in [(6., 8.), (12., 16.), (18., 10.)] {
                    line(&[(3., y), (knob - 2., y)]);
                    line(&[(knob + 2., y), (21., y)]);
                    line(&[(knob, y - 2.), (knob, y + 2.)]);
                }
            }
            Symbol::Grid => {
                for (x, y) in [(4., 4.), (14., 4.), (4., 14.), (14., 14.)] {
                    line(&[(x, y), (x + 6., y), (x + 6., y + 6.), (x, y + 6.), (x, y)]);
                }
            }
            Symbol::Moon => line(&[
                (16., 3.),
                (10., 4.),
                (6., 8.),
                (5., 13.),
                (7., 18.),
                (12., 21.),
                (17., 20.),
                (21., 16.),
                (16., 17.),
                (12., 14.),
                (11., 9.),
                (13., 5.),
                (16., 3.),
            ]),
            Symbol::Search => {
                line(&[(15., 15.), (21., 21.)]);
                let points: Vec<_> = (0..=24)
                    .map(|i| {
                        let a = i as f32 / 24. * std::f32::consts::TAU;
                        (10. + 6. * a.cos(), 10. + 6. * a.sin())
                    })
                    .collect();
                line(&points);
            }
            Symbol::Send => line(&[(21., 12.), (3., 4.), (11., 12.), (3., 20.), (21., 12.)]),
            Symbol::Attachment => {
                let points: Vec<_> = (0..=16)
                    .map(|i| {
                        let a = std::f32::consts::PI * 0.9
                            + std::f32::consts::PI * 1.2 * i as f32 / 16.;
                        (14. + 4.5 * a.cos(), 9. + 4.5 * a.sin())
                    })
                    .collect();
                line(&points);
                line(&[
                    (points.last().unwrap().0, points.last().unwrap().1),
                    (9., 19.),
                ]);
                line(&[(points[0].0, points[0].1), (13., 19.)]);
            }
            Symbol::Close => {
                line(&[(6., 6.), (18., 18.)]);
                line(&[(18., 6.), (6., 18.)]);
            }
            Symbol::Text => {
                line(&[(5., 7.), (5., 5.), (19., 5.), (19., 7.)]);
                line(&[(12., 5.), (12., 19.)]);
                line(&[(9., 19.), (15., 19.)]);
            }
            Symbol::Button => {
                line(&rounded(3., 7., 18., 10., 3.));
                line(&[(8., 12.), (16., 12.)]);
            }
            Symbol::Toggle => {
                line(&rounded(2., 7., 20., 10., 5.));
                line(&circle(16., 12., 2.6, 16));
            }
            Symbol::List => {
                for y in [6., 12., 18.] {
                    line(&[(4., y), (4.5, y)]);
                    line(&[(8., y), (20., y)]);
                }
            }
            Symbol::Table => {
                line(&rounded(3., 4., 18., 16., 2.));
                line(&[(3., 9.), (21., 9.)]);
                line(&[(3., 14.5), (21., 14.5)]);
                line(&[(9., 9.), (9., 20.)]);
            }
            Symbol::Bell => {
                line(&[
                    (6., 17.),
                    (6., 11.),
                    (7., 7.5),
                    (9.5, 5.),
                    (12., 4.3),
                    (14.5, 5.),
                    (17., 7.5),
                    (18., 11.),
                    (18., 17.),
                ]);
                line(&[(4., 17.), (20., 17.)]);
                line(&[(10., 20.), (14., 20.)]);
            }
            Symbol::Calendar => {
                line(&rounded(3., 5., 18., 16., 2.));
                line(&[(3., 10.), (21., 10.)]);
                line(&[(8., 3.), (8., 7.)]);
                line(&[(16., 3.), (16., 7.)]);
            }
            Symbol::Star => {
                let points: Vec<_> = (0..=10)
                    .map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.;
                        let r = if i % 2 == 0 { 9. } else { 4. };
                        (12. + r * a.cos(), 12.5 + r * a.sin())
                    })
                    .collect();
                line(&points);
            }
            Symbol::Layers => {
                line(&[(12., 3.), (21., 8.), (12., 13.), (3., 8.), (12., 3.)]);
                line(&[(3., 12.5), (12., 17.5), (21., 12.5)]);
                line(&[(3., 16.5), (12., 21.5), (21., 16.5)]);
            }
            Symbol::Chart => {
                line(&[(3., 20.), (21., 20.)]);
                line(&[(6., 20.), (6., 13.)]);
                line(&[(11., 20.), (11., 6.)]);
                line(&[(16., 20.), (16., 10.)]);
            }
            Symbol::Tag => {
                line(&[
                    (3., 3.),
                    (11., 3.),
                    (21., 13.),
                    (13., 21.),
                    (3., 11.),
                    (3., 3.),
                ]);
                line(&circle(7.5, 7.5, 1.3, 10));
            }
            Symbol::Tree => {
                line(&[(5., 4.), (5., 18.)]);
                line(&[(5., 8.), (11., 8.)]);
                line(&[(5., 18.), (11., 18.)]);
                line(&rounded(12., 5.5, 8., 5., 1.5));
                line(&rounded(12., 15.5, 8., 5., 1.5));
            }
            Symbol::Bolt => line(&[
                (13., 2.),
                (5., 13.),
                (11., 13.),
                (10., 22.),
                (19., 10.),
                (13., 10.),
                (13., 2.),
            ]),
            Symbol::Code => {
                line(&[(8., 7.), (3., 12.), (8., 17.)]);
                line(&[(16., 7.), (21., 12.), (16., 17.)]);
                line(&[(14., 4.), (10., 20.)]);
            }
            Symbol::Lock => {
                line(&rounded(4., 10., 16., 11., 2.));
                line(&[
                    (8., 10.),
                    (8., 7.),
                    (9.2, 4.2),
                    (12., 3.),
                    (14.8, 4.2),
                    (16., 7.),
                    (16., 10.),
                ]);
                line(&[(12., 14.), (12., 17.)]);
            }
            Symbol::Message => line(&[
                (4., 5.),
                (20., 5.),
                (20., 16.),
                (11., 16.),
                (6., 20.),
                (7., 16.),
                (4., 16.),
                (4., 5.),
            ]),
            Symbol::Sidebar => {
                line(&rounded(3., 4., 18., 16., 2.));
                line(&[(9., 4.), (9., 20.)]);
                line(&[(5., 8.), (7., 8.)]);
                line(&[(5., 11.), (7., 11.)]);
            }
            Symbol::Image => {
                line(&[(3., 4.), (21., 4.), (21., 20.), (3., 20.), (3., 4.)]);
                let sun: Vec<_> = (0..=12)
                    .map(|i| {
                        let a = i as f32 / 12. * std::f32::consts::TAU;
                        (8. + 2. * a.cos(), 9. + 2. * a.sin())
                    })
                    .collect();
                line(&sun);
                line(&[(3., 16.), (9., 11.), (14., 15.), (17., 12.), (21., 17.)]);
            }
            Symbol::Sun | Symbol::Appearance => {
                let points: Vec<_> = (0..=32)
                    .map(|i| {
                        let a = i as f32 / 32. * std::f32::consts::TAU;
                        (12. + 5. * a.cos(), 12. + 5. * a.sin())
                    })
                    .collect();
                line(&points);
                for i in 0..8 {
                    let a = i as f32 / 8. * std::f32::consts::TAU;
                    line(&[
                        (12. + 8. * a.cos(), 12. + 8. * a.sin()),
                        (12. + 10. * a.cos(), 12. + 10. * a.sin()),
                    ]);
                }
            }
        }
    }
}
impl Widget for Icon {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: crate::layout::fixed(self.size, self.size),
            flex_shrink: 0.,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        Self::draw(self.source.clone(), painter, rect, self.color);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SurfaceRole {
    /// The primary material for a page or major pane.
    Panel,
    /// A grouped region inside a panel, such as a settings section.
    Inset,
    /// A transient surface that sits above the current page.
    Floating,
}

/// A semantic material with a shared edge and elevation treatment.
pub struct Surface {
    theme: Theme,
    role: SurfaceRole,
    style: Style,
    children: Vec<BoxedWidget>,
}
impl Surface {
    pub fn new(role: SurfaceRole, style: Style) -> Self {
        let theme = use_theme();
        Self {
            theme,
            role,
            style,
            children: vec![],
        }
    }
    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.children.push(child);
        self
    }
    pub fn with_children(mut self, children: Vec<BoxedWidget>) -> Self {
        self.children = children;
        self
    }
}
impl Surface {
    fn shadow(&self) -> Option<creamui_core::BoxShadow> {
        use crate::elevation::{self, Elevation};
        if self.theme.high_contrast {
            return None;
        }
        match self.role {
            // Panels are translucent so window blur can show through, and a
            // shadow underneath would tint the whole pane.
            SurfaceRole::Panel => None,
            SurfaceRole::Inset => {
                elevation::is_light(&self.theme).then(|| Elevation::Control.shadow(&self.theme))
            }
            SurfaceRole::Floating => Some(Elevation::Overlay.shadow(&self.theme)),
        }
    }
}
impl Widget for Surface {
    fn style(&self) -> creamui_core::Style {
        let style: creamui_core::Style = self.style.clone().into();
        match self.shadow() {
            Some(shadow) => style.box_shadow(shadow),
            None => style,
        }
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        use crate::elevation;
        let theme = &self.theme;
        let (background, border, radius) = match self.role {
            // A page is the broadest visible layer: its larger radius makes
            // the pane read as one material without making every nested
            // group look like a separate floating card.
            SurfaceRole::Panel => (
                theme.colors.surface_elevated,
                theme.colors.border,
                theme.radius_large,
            ),
            // Groups sit one step forward on the panel, like grouped lists:
            // a lighter fill in dark themes and a hairline lift in light ones.
            SurfaceRole::Inset => (
                elevation::group_fill(theme),
                theme.colors.border,
                theme.radius_medium,
            ),
            SurfaceRole::Floating => (
                elevation::overlay_fill(theme),
                elevation::overlay_border(theme),
                theme.radius_medium,
            ),
        };
        painter.fill_rect(rect, background, radius);
        painter.stroke_rect_inside(rect, border, 1., radius);
    }
}

/// Navigation with real symbols and a quiet, accent-tinted selection.
pub struct NavigationItem {
    theme: Theme,
    icon: IconSource,
    label: String,
    active: bool,
    click: Rc<dyn Fn()>,
    style: creamui_core::Style,
}
impl_styled_field!(NavigationItem);
impl NavigationItem {
    pub fn new(
        icon: impl Into<IconSource>,
        label: impl Into<String>,
        active: bool,
        click: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            icon: icon.into(),
            label: label.into(),
            active,
            click: Rc::new(click),
            style: Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Percent(1.),
                    height: creamui_core::layout::Dimension::Length(36.),
                },
                flex_shrink: 0.,
                ..Default::default()
            }
            .into(),
        }
    }
}
fn activation(click: Rc<dyn Fn()>) -> Rc<dyn Fn(KeyInput)> {
    Rc::new(move |input| {
        if !input.modifiers.ctrl && matches!(input.key, Key::Enter | Key::Char(' ')) {
            click();
        }
    })
}
impl Widget for NavigationItem {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let t = self.theme;
        if self.active || painter.hovered(rect) {
            painter.fill_rect(
                rect,
                if self.active {
                    crate::elevation::with_alpha(t.accent, 40)
                } else {
                    crate::elevation::subtle(&t, 1)
                },
                t.sidebar_item_radius,
            );
        }
        let color = if self.active {
            crate::elevation::tinted_text(&t, t.accent)
        } else {
            t.text_secondary
        };
        Icon::draw(
            self.icon.clone(),
            painter,
            Rect {
                x: rect.x + 11.,
                y: rect.y + 9.,
                width: 18.,
                height: 18.,
            },
            color,
        );
        painter.fill_text_weight(
            Rect {
                x: rect.x + 40.,
                y: rect.y,
                width: rect.width - 48.,
                height: rect.height,
            },
            &self.label,
            t.text_primary,
            t.typography.body,
            TextAlign::Start,
            self.active,
            false,
        );
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.click.clone())
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        Some(activation(self.click.clone()))
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, _: bool) {
        // Selected items already provide their own focus treatment.
        if !self.active {
            p.stroke_rect(r, self.theme.accent, 2., self.theme.sidebar_item_radius);
        }
    }
}

/// One option in a segmented choice. Keep selection in application state.
pub struct Choice {
    inner: crate::RawButton,
}
impl_styled_inner!(Choice);
impl Choice {
    pub fn new(label: impl Into<String>, selected: bool, click: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let style = Style {
            min_size: crate::layout::fixed(72., 28.),
            align_items: Some(AlignItems::Center),
            justify_content: Some(creamui_core::layout::JustifyContent::Center),
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(12.),
                right: creamui_core::layout::LengthPercentage::Length(12.),
                top: creamui_core::layout::LengthPercentage::Length(0.),
                bottom: creamui_core::layout::LengthPercentage::Length(0.),
            },
            ..Default::default()
        };
        let label = RawText::new(
            label,
            if selected {
                theme.text_primary
            } else {
                theme.text_secondary
            },
            13.,
        )
        .font_family(theme.font_family);
        let mut inner = crate::RawButton::new(style, click)
            .corner_radius(theme.tab_radius)
            .child(Box::new(label));
        inner = if selected {
            let inner = inner
                .background(crate::elevation::segment_fill(&theme))
                .border(crate::elevation::clear(), 1.);
            match crate::elevation::control_shadow(&theme) {
                Some(shadow) => inner.box_shadow(shadow),
                None => inner,
            }
        } else {
            inner
                .background(crate::elevation::clear())
                .border(crate::elevation::clear(), 1.)
                .hover_style(
                    creamui_core::StateStyle::new().background(crate::elevation::subtle(&theme, 1)),
                )
                .pressed_style(
                    creamui_core::StateStyle::new().background(crate::elevation::subtle(&theme, 2)),
                )
        };
        inner = inner.focus_style(
            creamui_core::StateStyle::new().outline(crate::elevation::focus_ring(&theme), 2.0),
        );
        Self { inner }
    }
}
impl Widget for Choice {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.inner.paint(p, r);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn focusable(&self) -> bool {
        true
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, c: bool) {
        self.inner.paint_focused_overlay(p, r, c);
    }
}
