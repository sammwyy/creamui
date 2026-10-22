use super::*;
use creamui_core::layout::{Dimension, LengthPercentageAuto, Position, Size as LayoutSize};
use creamui_core::{
    Style as CoreStyle, TitlebarMode, TitlebarState, WindowButton, WindowControls, WindowDragHandle,
};

/// Space between the platform's overlay controls and titlebar content.
const OVERLAY_GAP: f32 = 8.0;
/// Logical size of a Windows caption button. The Win32 backend hit-tests the
/// maximize button at this size in the top-right corner for snap layouts.
const WINDOWS_BUTTON_WIDTH: f32 = 46.0;
const WINDOWS_BUTTON_HEIGHT: f32 = 32.0;
const ROUND_BUTTON_SIZE: f32 = 24.0;
const ROUND_BUTTON_GAP: f32 = 10.0;

/// Visual family of client-drawn window controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionStyle {
    /// Flat rectangular buttons flush with the top-right corner.
    Windows,
    /// Round buttons inset from the edge, as on Linux desktops.
    Round,
}

impl CaptionStyle {
    pub fn native() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Round
        }
    }
}

/// A window titlebar drawn by the application.
///
/// On construction it asks the window for the best integration the platform
/// offers. With compositor overlay controls (`Hybrid`) the content keeps
/// clear of the rectangle they occupy, on whichever side the compositor puts
/// them; otherwise the window switches to client-side decorations and the
/// titlebar draws its own minimize, maximize and close buttons at the end.
///
/// Empty background, and the intrinsic content of the center slot, move the
/// window. Buttons and other interactive children keep their own input.
/// When the platform controls sit at the start, the `start` and `end` slots
/// trade places so app actions stay away from them.
pub struct AppTitlebar {
    style: CoreStyle,
    origin_x: f32,
    width: f32,
    height: f32,
    padding: f32,
    gap: f32,
    center_margin: f32,
    mirror: bool,
    caption_style: CaptionStyle,
    foreground: Color,
    start: Vec<BoxedWidget>,
    center: Option<BoxedWidget>,
    end: Vec<BoxedWidget>,
    state: TitlebarState,
    controls: Option<WindowControls>,
    drag: Option<WindowDragHandle>,
}
impl_styled_field!(AppTitlebar);

impl AppTitlebar {
    /// `width` is the window's logical width: overlay controls are reported
    /// in window coordinates and the titlebar spans the window.
    pub fn new(width: f32) -> Self {
        let controls: Option<WindowControls> = creamui_reactive::try_use_context();
        let state = controls.as_ref().map_or(
            TitlebarState {
                mode: TitlebarMode::None,
                ..Default::default()
            },
            |controls| {
                controls.request_titlebar();
                controls.state()
            },
        );
        Self {
            style: CoreStyle::default(),
            origin_x: 0.0,
            width,
            height: 40.0,
            padding: 8.0,
            gap: 4.0,
            center_margin: 96.0,
            mirror: true,
            caption_style: CaptionStyle::native(),
            foreground: use_theme().colors.text_primary,
            start: Vec::new(),
            center: None,
            end: Vec::new(),
            state,
            controls,
            drag: creamui_reactive::try_use_context(),
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Horizontal window coordinate where this titlebar begins. This lets a
    /// titlebar embedded beside a sidebar reserve compositor controls whose
    /// rectangles are reported in window coordinates.
    pub fn origin_x(mut self, origin_x: f32) -> Self {
        self.origin_x = origin_x;
        self
    }

    /// Inset of the start and end slots from the window edges.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Room kept on both sides of the centered slot, beyond any controls.
    pub fn center_margin(mut self, margin: f32) -> Self {
        self.center_margin = margin;
        self
    }

    /// Whether `start` and `end` swap when the platform controls lead.
    pub fn mirror(mut self, mirror: bool) -> Self {
        self.mirror = mirror;
        self
    }

    pub fn caption_style(mut self, style: CaptionStyle) -> Self {
        self.caption_style = style;
        self
    }

    /// Glyph color of client-drawn controls, for titlebars over media or
    /// other backgrounds that differ from the theme surface.
    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = color;
        self
    }

    pub fn start(mut self, child: impl Widget + 'static) -> Self {
        self.start.push(Box::new(child));
        self
    }

    pub fn center(mut self, child: impl Widget + 'static) -> Self {
        self.center = Some(Box::new(child));
        self
    }

    pub fn end(mut self, child: impl Widget + 'static) -> Self {
        self.end.push(Box::new(child));
        self
    }

    pub fn start_boxed(mut self, child: BoxedWidget) -> Self {
        self.start.push(child);
        self
    }

    pub fn center_boxed(mut self, child: BoxedWidget) -> Self {
        self.center = Some(child);
        self
    }

    pub fn end_boxed(mut self, child: BoxedWidget) -> Self {
        self.end.push(child);
        self
    }

    /// The decoration result this titlebar was built for.
    pub fn state(&self) -> TitlebarState {
        self.state
    }

    /// Whether the platform's controls sit at the start of the titlebar.
    pub fn controls_lead(&self) -> bool {
        self.state.mode == TitlebarMode::Overlay
            && self.state.overlay.x + self.state.overlay.width / 2.0
                < self.origin_x + self.width / 2.0
    }

    fn effective_height(&self) -> f32 {
        match self.state.mode {
            TitlebarMode::Overlay => self
                .height
                .max(self.state.overlay.y * 2.0 + self.state.overlay.height),
            TitlebarMode::Client if self.caption_style == CaptionStyle::Windows => {
                self.height.max(WINDOWS_BUTTON_HEIGHT)
            }
            _ => self.height,
        }
    }

    /// Space reserved at the start and the end for window controls.
    pub fn reserved(&self) -> (f32, f32) {
        match self.state.mode {
            TitlebarMode::Overlay => {
                let overlay = self.state.overlay;
                if self.controls_lead() {
                    (
                        (overlay.x + overlay.width + OVERLAY_GAP - self.origin_x).max(0.0),
                        0.0,
                    )
                } else {
                    (
                        0.0,
                        (self.origin_x + self.width - overlay.x + OVERLAY_GAP).max(0.0),
                    )
                }
            }
            TitlebarMode::Client => (0.0, client_controls_width(self.caption_style)),
            _ => (0.0, 0.0),
        }
    }
}

fn client_controls_width(style: CaptionStyle) -> f32 {
    match style {
        CaptionStyle::Windows => WINDOWS_BUTTON_WIDTH * 3.0,
        CaptionStyle::Round => ROUND_BUTTON_SIZE * 3.0 + ROUND_BUTTON_GAP * 3.0,
    }
}

fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        gap: LayoutSize {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

impl Widget for AppTitlebar {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        let layout = &mut style.layout;
        if layout.size.width == Dimension::Auto {
            layout.size.width = Dimension::Percent(1.0);
        }
        layout.size.height = Dimension::Length(self.effective_height());
        layout.flex_shrink = 0.0;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn on_window_drag(&self) -> Option<Rc<dyn Fn()>> {
        let drag = self.drag.clone()?;
        Some(Rc::new(move || drag.start_drag()))
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let (lead, trail) = self.reserved();
        let mut start = std::mem::take(&mut self.start);
        let mut end = std::mem::take(&mut self.end);
        if self.mirror && self.controls_lead() {
            std::mem::swap(&mut start, &mut end);
        }
        let client = self.state.mode == TitlebarMode::Client;
        let mut slots: Vec<BoxedWidget> = vec![
            Box::new(RawView::new(row(self.gap)).with_children(start)),
            Box::new(RawView::new(Style {
                flex_grow: 1.0,
                ..Default::default()
            })),
            Box::new(RawView::new(row(self.gap)).with_children(end)),
        ];
        if client {
            if let Some(controls) = &self.controls {
                slots.push(caption_buttons(
                    controls,
                    self.state,
                    self.caption_style,
                    self.foreground,
                ));
            }
        }
        let mut layout = Style {
            size: LayoutSize {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            align_items: Some(AlignItems::Center),
            gap: LayoutSize {
                width: LengthPercentage::Length(self.gap),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        };
        layout.padding.left = LengthPercentage::Length(lead.max(self.padding));
        layout.padding.right =
            LengthPercentage::Length(if client { 0.0 } else { trail.max(self.padding) });
        let mut children: Vec<BoxedWidget> =
            vec![Box::new(RawView::new(layout).with_children(slots))];
        if let Some(center) = self.center.take() {
            let side = lead.max(trail) + self.center_margin;
            children.push(Box::new(DragThrough(
                RawView::new(Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Length(side),
                        right: LengthPercentageAuto::Length(side),
                        top: LengthPercentageAuto::Length(0.0),
                        bottom: LengthPercentageAuto::Length(0.0),
                    },
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    ..Default::default()
                })
                .child(center),
            )));
        }
        children
    }
}

/// Lets the titlebar's drag region show through its intrinsic content.
struct DragThrough(RawView);

impl Widget for DragThrough {
    fn style(&self) -> creamui_core::Style {
        self.0.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.0.paint(painter, rect);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.0.children()
    }
    fn passes_window_drag(&self) -> bool {
        true
    }
}

fn caption_buttons(
    controls: &WindowControls,
    state: TitlebarState,
    style: CaptionStyle,
    foreground: Color,
) -> BoxedWidget {
    let buttons = [
        WindowButton::Minimize,
        WindowButton::Maximize,
        WindowButton::Close,
    ]
    .into_iter()
    .map(|button| -> BoxedWidget {
        Box::new(CaptionButtonView::new(
            controls.clone(),
            button,
            style,
            state.maximized,
            state.hovered == Some(button),
            foreground,
        ))
    })
    .collect();
    let mut layout = row(if style == CaptionStyle::Round {
        ROUND_BUTTON_GAP
    } else {
        0.0
    });
    match style {
        CaptionStyle::Round => {
            layout.padding.right = LengthPercentage::Length(ROUND_BUTTON_GAP);
        }
        CaptionStyle::Windows => {
            layout.align_self = Some(creamui_core::layout::AlignSelf::FlexStart);
        }
    }
    Box::new(RawView::new(layout).with_children(buttons))
}

/// A client-drawn minimize, maximize/restore or close button.
pub struct CaptionButtonView {
    style: CoreStyle,
    controls: WindowControls,
    button: WindowButton,
    caption_style: CaptionStyle,
    maximized: bool,
    glyph: Color,
    hover_glyph: Color,
}

impl CaptionButtonView {
    /// `hovered` forces the hover appearance for a button whose pointer
    /// input the platform intercepts.
    pub fn new(
        controls: WindowControls,
        button: WindowButton,
        caption_style: CaptionStyle,
        maximized: bool,
        hovered: bool,
        foreground: Color,
    ) -> Self {
        let text = foreground;
        let tint = |alpha: u8| Color { a: alpha, ..text };
        let close = button == WindowButton::Close;
        let (size, radius, rest, hover, pressed) = match caption_style {
            CaptionStyle::Windows => (
                (WINDOWS_BUTTON_WIDTH, WINDOWS_BUTTON_HEIGHT),
                0.0,
                Color::rgba(0, 0, 0, 0),
                if close {
                    Color::rgb(196, 43, 28)
                } else {
                    tint(24)
                },
                if close {
                    Color::rgb(199, 64, 49)
                } else {
                    tint(16)
                },
            ),
            CaptionStyle::Round => (
                (ROUND_BUTTON_SIZE, ROUND_BUTTON_SIZE),
                ROUND_BUTTON_SIZE / 2.0,
                tint(26),
                tint(44),
                tint(60),
            ),
        };
        let mut style: CoreStyle = Style {
            size: LayoutSize {
                width: Dimension::Length(size.0),
                height: Dimension::Length(size.1),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into();
        style.paint.background = Some(if hovered { hover } else { rest }.into());
        style.paint.corner_radius = Some(radius);
        style.states.hover.paint.background = Some(hover.into());
        style.states.pressed.paint.background = Some(pressed.into());
        Self {
            style,
            controls,
            button,
            caption_style,
            maximized,
            glyph: text,
            hover_glyph: if close && caption_style == CaptionStyle::Windows {
                Color::rgb(255, 255, 255)
            } else {
                text
            },
        }
    }
}
impl_styled_field!(CaptionButtonView);

impl Widget for CaptionButtonView {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let controls = self.controls.clone();
        let button = self.button;
        Some(Rc::new(move || controls.press(button)))
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let color = if painter.hovered(rect) {
            self.hover_glyph
        } else {
            self.glyph
        };
        let (half, width) = match self.caption_style {
            CaptionStyle::Windows => (5.0, 1.0),
            CaptionStyle::Round => (4.0, 1.5),
        };
        let cx = (rect.x + rect.width / 2.0).round();
        let cy = (rect.y + rect.height / 2.0).round();
        let point = |x: f32, y: f32| Point {
            x: cx + x,
            y: cy + y,
        };
        match self.button {
            WindowButton::Minimize => {
                painter.stroke_line(point(-half, 0.5), point(half, 0.5), color, width)
            }
            WindowButton::Maximize if self.maximized => {
                let inner = half - 2.0;
                painter.stroke_rect(
                    Rect {
                        x: cx - half,
                        y: cy - inner,
                        width: half + inner,
                        height: half + inner,
                    },
                    color,
                    width,
                    1.0,
                );
                painter.stroke_line(point(-inner, -half), point(half, -half), color, width);
                painter.stroke_line(point(half, -half), point(half, inner), color, width);
            }
            WindowButton::Maximize => painter.stroke_rect(
                Rect {
                    x: cx - half,
                    y: cy - half,
                    width: half * 2.0,
                    height: half * 2.0,
                },
                color,
                width,
                if self.caption_style == CaptionStyle::Round {
                    1.5
                } else {
                    1.0
                },
            ),
            WindowButton::Close => {
                painter.stroke_line(point(-half, -half), point(half, half), color, width);
                painter.stroke_line(point(-half, half), point(half, -half), color, width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Renderer, Size, WindowControlsBackend};
    use creamui_theme::ThemeProvider;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct FakeWindow {
        state: Cell<TitlebarState>,
        requests: Cell<u32>,
        pressed: RefCell<Vec<WindowButton>>,
    }

    struct FakeBackend(Rc<FakeWindow>);

    impl WindowControlsBackend for FakeBackend {
        fn state(&self) -> TitlebarState {
            self.0.state.get()
        }
        fn request_titlebar(&self) {
            self.0.requests.set(self.0.requests.get() + 1);
        }
        fn minimize(&self) {
            self.0.pressed.borrow_mut().push(WindowButton::Minimize);
        }
        fn toggle_maximize(&self) {
            self.0.pressed.borrow_mut().push(WindowButton::Maximize);
        }
        fn close(&self) {
            self.0.pressed.borrow_mut().push(WindowButton::Close);
        }
    }

    struct Probe(Rc<Cell<bool>>);

    impl Widget for Probe {
        fn style(&self) -> creamui_core::Style {
            Style {
                size: LayoutSize {
                    width: Dimension::Length(32.0),
                    height: Dimension::Length(32.0),
                },
                ..Default::default()
            }
            .into()
        }
        fn paint(&self, _: &mut dyn Painter, _: Rect) {}
        fn on_click(&self) -> Option<Rc<dyn Fn()>> {
            let clicked = self.0.clone();
            Some(Rc::new(move || clicked.set(true)))
        }
    }

    struct NullPainter;

    impl Painter for NullPainter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }

    const WIDTH: f32 = 600.0;

    fn render(
        window: &Rc<FakeWindow>,
        style: CaptionStyle,
    ) -> (
        creamui_core::Scene,
        Rc<Cell<bool>>,
        Rc<Cell<bool>>,
        Rc<Cell<u32>>,
    ) {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            creamui_reactive::provide_context(WindowControls::new(FakeBackend(window.clone())));
            let drags = Rc::new(Cell::new(0));
            creamui_reactive::provide_context(WindowDragHandle::new({
                let drags = drags.clone();
                move || drags.set(drags.get() + 1)
            }));
            let (start, end) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
            let titlebar = AppTitlebar::new(WIDTH)
                .caption_style(style)
                .start(Probe(start.clone()))
                .center(crate::Text::new("Title"))
                .end(Probe(end.clone()));
            let scene = Renderer::new().render(
                Box::new(titlebar),
                Size {
                    width: WIDTH,
                    height: 400.0,
                },
                &mut NullPainter,
            );
            (scene, start, end, drags)
        })
    }

    fn click(scene: &creamui_core::Scene, x: f32, y: f32) -> bool {
        scene
            .hit_test(Point { x, y })
            .map(|handler| handler())
            .is_some()
    }

    #[test]
    fn overlay_controls_on_the_right_keep_content_clear() {
        let window = Rc::new(FakeWindow::default());
        window.state.set(TitlebarState {
            mode: TitlebarMode::Overlay,
            overlay: Rect {
                x: 480.0,
                y: 4.0,
                width: 112.0,
                height: 32.0,
            },
            ..Default::default()
        });
        let (scene, start, end, drags) = render(&window, CaptionStyle::Round);
        assert_eq!(window.requests.get(), 1);
        assert!(click(&scene, 20.0, 20.0));
        assert!(start.get());
        assert!(click(&scene, 480.0 - OVERLAY_GAP - 16.0, 20.0));
        assert!(end.get());
        for x in (480..592).step_by(8) {
            assert!(scene
                .hit_test(Point {
                    x: x as f32,
                    y: 20.0
                })
                .is_none());
        }
        scene
            .window_drag_at(Point {
                x: WIDTH / 2.0,
                y: 20.0,
            })
            .expect("the centered title moves the window")();
        assert_eq!(drags.get(), 1);
    }

    #[test]
    fn leading_overlay_controls_swap_the_slots() {
        let window = Rc::new(FakeWindow::default());
        window.state.set(TitlebarState {
            mode: TitlebarMode::Overlay,
            overlay: Rect {
                x: 8.0,
                y: 4.0,
                width: 80.0,
                height: 32.0,
            },
            ..Default::default()
        });
        let (scene, start, end, _) = render(&window, CaptionStyle::Round);
        assert!(scene.hit_test(Point { x: 40.0, y: 20.0 }).is_none());
        assert!(click(&scene, 8.0 + 80.0 + OVERLAY_GAP + 16.0, 20.0));
        assert!(end.get(), "the end slot moved to the start");
        assert!(click(&scene, WIDTH - 24.0, 20.0));
        assert!(start.get(), "the start slot moved to the end");
    }

    #[test]
    fn client_mode_draws_working_controls_at_the_end() {
        for style in [CaptionStyle::Windows, CaptionStyle::Round] {
            let window = Rc::new(FakeWindow::default());
            window.state.set(TitlebarState {
                mode: TitlebarMode::Client,
                ..Default::default()
            });
            let (scene, _, end, _) = render(&window, style);
            let (width, gap) = match style {
                CaptionStyle::Windows => (WINDOWS_BUTTON_WIDTH, 0.0),
                CaptionStyle::Round => (ROUND_BUTTON_SIZE, ROUND_BUTTON_GAP),
            };
            let close_x = WIDTH - gap - width / 2.0;
            let y = match style {
                CaptionStyle::Windows => WINDOWS_BUTTON_HEIGHT / 2.0,
                CaptionStyle::Round => 20.0,
            };
            assert!(click(&scene, close_x, y));
            assert!(click(&scene, close_x - width - gap, y));
            assert!(click(&scene, close_x - 2.0 * (width + gap), y));
            assert_eq!(
                *window.pressed.borrow(),
                [
                    WindowButton::Close,
                    WindowButton::Maximize,
                    WindowButton::Minimize
                ]
            );
            assert!(click(
                &scene,
                WIDTH - client_controls_width(style) - 4.0 - 16.0,
                20.0
            ));
            assert!(end.get(), "the end slot sits before the controls");
        }
    }

    #[test]
    fn server_frames_get_no_controls() {
        let window = Rc::new(FakeWindow::default());
        window.state.set(TitlebarState {
            mode: TitlebarMode::Server,
            ..Default::default()
        });
        let (scene, _, end, _) = render(&window, CaptionStyle::Round);
        assert!(click(&scene, WIDTH - 8.0 - 16.0, 20.0));
        assert!(end.get());
        assert!(window.pressed.borrow().is_empty());
    }
}
