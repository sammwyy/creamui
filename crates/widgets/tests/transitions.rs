use creamui_core::{keyed, Easing, Painter, Point, Rect, Renderer, Size, TextAlign, Transition};
use creamui_theme::{Color, Theme, ThemeProvider};
use creamui_widgets::{Checkbox, RawCheckbox, RawSwitch, RawView, Switch};
use std::time::Duration;

const VIEWPORT: Size = Size {
    width: 80.0,
    height: 40.0,
};

#[derive(Default)]
struct FramePainter {
    time: f32,
    animated: bool,
    hovered: bool,
    pressed: bool,
    fills: Vec<(Rect, Color)>,
    lines: Vec<(Point, Point, Color)>,
}

impl FramePainter {
    fn begin(&mut self, time: f32) {
        self.time = time;
        self.animated = false;
        self.fills.clear();
        self.lines.clear();
    }
}

impl Painter for FramePainter {
    fn frame_time(&self) -> Option<f32> {
        Some(self.time)
    }
    fn animation_time(&mut self) -> f32 {
        self.animated = true;
        self.time
    }
    fn hovered(&self, _: Rect) -> bool {
        self.hovered
    }
    fn pressed(&self, _: Rect) -> bool {
        self.pressed
    }
    fn fill_rect(&mut self, rect: Rect, color: Color, _: f32) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn stroke_line(&mut self, from: Point, to: Point, color: Color, _: f32) {
        self.lines.push((from, to, color));
    }
    fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
}

fn linear_transition() -> Transition {
    Transition::new(Duration::from_secs(1)).easing(Easing::Linear)
}

fn raw_switch(checked: bool) -> RawSwitch {
    RawSwitch::new(
        checked,
        Color::rgb(200, 100, 50),
        Color::rgb(0, 0, 0),
        Color::rgb(255, 255, 255),
        || {},
    )
    .transition(linear_transition())
}

#[test]
fn switch_interpolates_the_track_and_thumb_and_reverses_without_jumping() {
    let mut renderer = Renderer::new();
    let mut painter = FramePainter::default();
    renderer.render(Box::new(raw_switch(false)), VIEWPORT, &mut painter);
    assert!(!painter.animated);
    painter.begin(0.0);
    renderer.render(Box::new(raw_switch(true)), VIEWPORT, &mut painter);
    assert_eq!(painter.fills[1].0.x, 2.0);
    assert!(painter.animated);
    painter.begin(0.5);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[0].1, Color::rgb(100, 50, 25));
    assert_eq!(painter.fills[1].0.x, 11.0);
    painter.begin(0.5);
    renderer.render(Box::new(raw_switch(false)), VIEWPORT, &mut painter);
    assert_eq!(painter.fills[1].0.x, 11.0);
    painter.begin(1.0);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[1].0.x, 6.5);
    painter.begin(1.5);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[1].0.x, 2.0);
    assert!(!painter.animated);
}

#[test]
fn switch_preserves_hover_pressed_and_disabled_colors_during_transitions() {
    let mut renderer = Renderer::new();
    let mut painter = FramePainter {
        hovered: true,
        pressed: true,
        ..Default::default()
    };
    let build = |checked, disabled| {
        raw_switch(checked)
            .hover_colors(Color::rgb(240, 120, 60), Color::rgb(20, 10, 0))
            .pressed_colors(Color::rgb(160, 80, 40), Color::rgb(60, 30, 20))
            .disabled(disabled)
    };
    renderer.render(Box::new(build(false, false)), VIEWPORT, &mut painter);
    painter.begin(0.0);
    renderer.render(Box::new(build(true, false)), VIEWPORT, &mut painter);
    painter.begin(0.5);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[0].1, Color::rgb(110, 55, 30));
    painter.begin(0.5);
    renderer.render(Box::new(build(true, true)), VIEWPORT, &mut painter);
    assert_eq!(painter.fills[0].1, Color::rgb(100, 50, 25));
    painter.pressed = false;
    painter.begin(0.5);
    renderer.render(Box::new(build(true, false)), VIEWPORT, &mut painter);
    assert_eq!(painter.fills[0].1, Color::rgb(130, 65, 30));
}

#[test]
fn checkbox_fades_its_fill_and_reveals_the_check_in_both_directions() {
    let mut renderer = Renderer::new();
    let mut painter = FramePainter::default();
    let build = |checked| {
        RawCheckbox::new(
            18.0,
            checked,
            Color::rgba(20, 80, 160, 200),
            Color::rgb(40, 40, 40),
            || {},
        )
        .transition(linear_transition())
    };
    renderer.render(Box::new(build(false)), VIEWPORT, &mut painter);
    painter.begin(0.0);
    renderer.render(Box::new(build(true)), VIEWPORT, &mut painter);
    assert!(painter.fills.is_empty());
    assert!(painter.lines.is_empty());
    painter.begin(0.5);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[0].1, Color::rgba(20, 80, 160, 100));
    assert_eq!(painter.lines.len(), 2);
    let (_, end, color) = painter.lines[1];
    assert!(end.x > 18.0 * 0.43 && end.x < 18.0 * 0.76);
    assert_eq!(color.a, 128);
    painter.begin(1.0);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[0].1.a, 200);
    assert_eq!(painter.lines[1].1.x, 18.0 * 0.76);
    assert!(!painter.animated);
    painter.begin(1.0);
    renderer.render(Box::new(build(false)), VIEWPORT, &mut painter);
    assert_eq!(painter.fills[0].1.a, 200);
    painter.begin(1.5);
    renderer.paint(&mut painter, None, true);
    assert_eq!(painter.fills[0].1.a, 100);
    painter.begin(2.0);
    renderer.paint(&mut painter, None, true);
    assert!(painter.fills.is_empty());
    assert!(painter.lines.is_empty());
    assert!(!painter.animated);
}

#[test]
fn disabling_transitions_snaps_an_active_switch_to_its_target() {
    let mut renderer = Renderer::new();
    let mut painter = FramePainter::default();
    renderer.render(Box::new(raw_switch(false)), VIEWPORT, &mut painter);
    painter.begin(0.0);
    renderer.render(Box::new(raw_switch(true)), VIEWPORT, &mut painter);
    painter.begin(0.25);
    renderer.render(
        Box::new(raw_switch(true).transition(Transition::NONE)),
        VIEWPORT,
        &mut painter,
    );
    assert_eq!(painter.fills[1].0.x, 20.0);
    assert_eq!(painter.fills[0].1, Color::rgb(200, 100, 50));
    assert!(!painter.animated);
}

#[test]
fn keyed_switches_keep_their_visual_state_when_reordered() {
    let mut renderer = Renderer::new();
    let mut painter = FramePainter::default();
    let viewport = Size {
        width: 84.0,
        height: 24.0,
    };
    let build = |checked, reversed| {
        let a = keyed(raw_switch(checked), "a".to_owned());
        let b = keyed(raw_switch(true), "b".to_owned());
        let row = RawView::new(creamui_core::Style::new().width(84.0).height(24.0));
        if reversed {
            row.child(b).child(a)
        } else {
            row.child(a).child(b)
        }
    };
    renderer.render(Box::new(build(false, false)), viewport, &mut painter);
    painter.begin(0.0);
    renderer.render(Box::new(build(true, false)), viewport, &mut painter);
    painter.begin(0.5);
    renderer.render(Box::new(build(true, true)), viewport, &mut painter);
    assert_eq!(painter.fills[1].0.x, 20.0);
    assert_eq!(painter.fills[3].0.x, 53.0);
}

#[test]
fn themed_controls_animate_with_defaults_and_allow_disabling_transitions() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let mut renderer = Renderer::new();
        let mut painter = FramePainter::default();
        renderer.render(Box::new(Switch::new(false, || {})), VIEWPORT, &mut painter);
        painter.begin(0.0);
        renderer.render(Box::new(Switch::new(true, || {})), VIEWPORT, &mut painter);
        assert!(painter.animated);
        painter.begin(0.08);
        renderer.paint(&mut painter, None, true);
        assert_eq!(painter.fills[1].0.x, 17.75);
        painter.begin(0.16);
        renderer.paint(&mut painter, None, true);
        assert_eq!(painter.fills[1].0.x, 20.0);
        assert!(!painter.animated);

        let mut renderer = Renderer::new();
        painter.begin(0.0);
        renderer.render(
            Box::new(Checkbox::new(false, || {})),
            VIEWPORT,
            &mut painter,
        );
        painter.begin(0.0);
        renderer.render(Box::new(Checkbox::new(true, || {})), VIEWPORT, &mut painter);
        assert!(painter.animated);
        painter.begin(0.08);
        renderer.paint(&mut painter, None, true);
        assert!(!painter.lines.is_empty());
        painter.begin(0.08);
        renderer.render(
            Box::new(Checkbox::new(false, || {}).transition(Transition::NONE)),
            VIEWPORT,
            &mut painter,
        );
        assert!(painter.lines.is_empty());
        assert!(!painter.animated);
    });
}
