//! General-purpose visual primitives and selectable controls.
use crate::layout::padding;
use crate::RawText;
use creamui_core::layout::{AlignItems, Style};
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Painter, Point, Rect, Styled, TextAlign, Widget,
};
use creamui_theme::{use_theme, Color, Theme};
use std::rc::Rc;

#[derive(Clone, Copy, Debug)]
pub enum Symbol {
    Appearance,
    Display,
    Controls,
    Keyboard,
    Check,
    ChevronRight,
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
}

/// A small, consistent line icon. Paths use a 24-unit optical grid.
pub struct Icon {
    pub symbol: Symbol,
    pub color: Color,
    pub size: f32,
}
impl Icon {
    pub fn new(symbol: Symbol, color: Color) -> Self {
        Self {
            symbol,
            color,
            size: 18.,
        }
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    pub fn draw(symbol: Symbol, painter: &mut dyn Painter, rect: Rect, color: Color) {
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
        Self::draw(self.symbol, painter, rect, self.color);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SurfaceRole {
    Panel,
    Inset,
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
impl Widget for Surface {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let radius = self.theme.card_radius;
        if matches!(self.role, SurfaceRole::Floating) {
            for spread in (1..=5).rev() {
                let s = spread as f32;
                painter.fill_rect(
                    Rect {
                        x: rect.x - s,
                        y: rect.y - s + 3.,
                        width: rect.width + s * 2.,
                        height: rect.height + s * 2.,
                    },
                    Color::rgba(0, 0, 0, 4),
                    radius + s,
                );
            }
        }
        painter.fill_rect(
            rect,
            if matches!(self.role, SurfaceRole::Inset) {
                self.theme.surface
            } else {
                self.theme.surface_elevated
            },
            radius,
        );
        painter.stroke_rect(rect, self.theme.border, 1., radius);
    }
}

/// Navigation with real symbols and a quiet, accent-tinted selection.
pub struct NavigationItem {
    theme: Theme,
    symbol: Symbol,
    label: String,
    active: bool,
    click: Rc<dyn Fn()>,
    style: creamui_core::Style,
}
impl_styled_field!(NavigationItem);
impl NavigationItem {
    pub fn new(
        symbol: Symbol,
        label: impl Into<String>,
        active: bool,
        click: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            symbol,
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
                    if painter.hovered(rect) {
                        t.accent_hover
                    } else {
                        t.accent
                    }
                } else {
                    t.surface_hover
                },
                t.sidebar_item_radius,
            );
        }
        let color = if self.active {
            Color::rgb(0x33, 0x2e, 0x34)
        } else {
            t.text_secondary
        };
        Icon::draw(
            self.symbol,
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
            if self.active {
                Color::rgb(0x33, 0x2e, 0x34)
            } else {
                t.text_secondary
            },
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
        let style = padding(
            Style {
                min_size: crate::layout::fixed(72., 30.),
                align_items: Some(AlignItems::Center),
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                ..Default::default()
            },
            7.,
        );
        let background = if selected {
            theme.surface_elevated
        } else {
            theme.surface_hover
        };
        let mut inner = crate::RawButton::new(style, click)
            .background(background)
            .corner_radius(theme.tab_radius)
            .border(
                if selected {
                    theme.border_strong
                } else {
                    background
                },
                1.,
            )
            .child(Box::new(RawText::new(
                label,
                if selected {
                    theme.text_primary
                } else {
                    theme.text_secondary
                },
                12.,
            )));
        inner = inner
            .hover_style(
                creamui_core::StateStyle::new()
                    .background(background.mix(theme.text_primary, 0.04)),
            )
            .pressed_style(
                creamui_core::StateStyle::new()
                    .background(background.mix(theme.text_primary, 0.09)),
            )
            .focus_style(creamui_core::StateStyle::new().outline(theme.accent, 2.0));
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
