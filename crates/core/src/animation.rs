use crate::Painter;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Easing {
    Linear,
    #[default]
    EaseOut,
}

impl Easing {
    fn sample(self, progress: f32) -> f32 {
        match self {
            Self::Linear => progress,
            Self::EaseOut => 1.0 - (1.0 - progress).powi(3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub duration: Duration,
    pub easing: Easing,
}

impl Transition {
    pub const NONE: Self = Self::new(Duration::ZERO);

    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::EaseOut,
        }
    }

    pub const fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::new(Duration::from_millis(160))
    }
}

#[derive(Debug, Default)]
pub struct TransitionState {
    segment: Option<Box<Segment>>,
}

#[derive(Debug)]
struct Segment {
    from: f32,
    target: f32,
    started: f32,
    transition: Transition,
}

impl Segment {
    fn value(&self, now: f32) -> f32 {
        let duration = self.transition.duration.as_secs_f32();
        if duration == 0.0 || now - self.started >= duration {
            return self.target;
        }
        let progress = ((now - self.started) / duration).clamp(0.0, 1.0);
        self.from + (self.target - self.from) * self.transition.easing.sample(progress)
    }
}

impl TransitionState {
    pub fn value(&mut self, target: f32, transition: Transition, painter: &mut dyn Painter) -> f32 {
        let time = painter.frame_time();
        let now = time.unwrap_or(0.0);
        let Some(segment) = &mut self.segment else {
            self.segment = Some(Box::new(Segment {
                from: target,
                target,
                started: now,
                transition,
            }));
            return target;
        };

        if time.is_none() || transition.duration.is_zero() {
            segment.from = target;
            segment.target = target;
            segment.started = now;
            segment.transition = transition;
            return target;
        }

        let current = segment.value(now);
        if segment.target != target || segment.transition != transition {
            segment.from = current;
            segment.target = target;
            segment.started = now;
            segment.transition = transition;
        }
        let value = segment.value(now);
        if value != target {
            painter.animation_time();
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rect, TextAlign};
    use creamui_theme::Color;

    #[derive(Default)]
    struct ClockPainter {
        time: Option<f32>,
        animated: bool,
        widths: Vec<f32>,
    }

    impl Painter for ClockPainter {
        fn frame_time(&self) -> Option<f32> {
            self.time
        }
        fn animation_time(&mut self) -> f32 {
            self.animated = true;
            self.time.unwrap_or(0.0)
        }
        fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
            self.widths.push(rect.width);
        }
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }

    #[test]
    fn transitions_start_at_the_initial_value_and_stop_requesting_frames() {
        let mut state = TransitionState::default();
        let mut painter = ClockPainter {
            time: Some(0.0),
            ..Default::default()
        };
        let transition = Transition::new(Duration::from_secs(1)).easing(Easing::Linear);
        assert_eq!(state.value(0.0, transition, &mut painter), 0.0);
        assert!(!painter.animated);
        assert_eq!(state.value(1.0, transition, &mut painter), 0.0);
        assert!(painter.animated);
        painter.time = Some(0.5);
        assert_eq!(state.value(1.0, transition, &mut painter), 0.5);
        painter.time = Some(1.0);
        painter.animated = false;
        assert_eq!(state.value(1.0, transition, &mut painter), 1.0);
        assert!(!painter.animated);
    }

    #[test]
    fn reversing_a_transition_preserves_the_current_value() {
        let mut state = TransitionState::default();
        let mut painter = ClockPainter {
            time: Some(0.0),
            ..Default::default()
        };
        let transition = Transition::new(Duration::from_secs(1)).easing(Easing::Linear);
        state.value(0.0, transition, &mut painter);
        state.value(1.0, transition, &mut painter);
        painter.time = Some(0.4);
        assert_eq!(state.value(0.0, transition, &mut painter), 0.4);
        painter.time = Some(0.9);
        assert!((state.value(0.0, transition, &mut painter) - 0.2).abs() < 0.0001);
    }

    #[test]
    fn disabling_a_transition_or_using_a_painter_without_a_clock_snaps() {
        let mut state = TransitionState::default();
        let mut painter = ClockPainter {
            time: Some(0.0),
            ..Default::default()
        };
        state.value(0.0, Transition::default(), &mut painter);
        assert_eq!(state.value(1.0, Transition::NONE, &mut painter), 1.0);
        assert!(!painter.animated);
        painter.time = None;
        assert_eq!(state.value(0.0, Transition::default(), &mut painter), 0.0);
        assert!(!painter.animated);
    }

    #[test]
    fn ease_out_approaches_the_target_faster_than_linear() {
        assert_eq!(Easing::EaseOut.sample(0.5), 0.875);
        assert_eq!(Easing::EaseOut.sample(0.0), 0.0);
        assert_eq!(Easing::EaseOut.sample(1.0), 1.0);
    }

    struct AnimatedWidget(std::rc::Rc<std::cell::Cell<f32>>);

    impl crate::Widget for AnimatedWidget {
        fn style(&self) -> crate::Style {
            crate::Style::new().width(20.0).height(20.0)
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.fill_rect(rect, Color::rgb(0, 0, 0), 0.0);
        }
        fn has_transition(&self) -> bool {
            true
        }
        fn paint_transition(
            &self,
            painter: &mut dyn Painter,
            rect: Rect,
            _: Rect,
            state: &mut TransitionState,
        ) {
            let transition = Transition::new(Duration::from_secs(1)).easing(Easing::Linear);
            let width = state.value(self.0.get(), transition, painter);
            painter.fill_rect(Rect { width, ..rect }, Color::rgb(0, 0, 0), 0.0);
        }
    }

    #[test]
    fn keyed_widgets_retain_transitions_across_rebuilds_and_paints() {
        let target = std::rc::Rc::new(std::cell::Cell::new(0.0));
        let mut renderer = crate::Renderer::new();
        let mut painter = ClockPainter {
            time: Some(0.0),
            ..Default::default()
        };
        let viewport = crate::Size {
            width: 20.0,
            height: 20.0,
        };
        renderer.render(
            crate::keyed(AnimatedWidget(target.clone()), 1_u64),
            viewport,
            &mut painter,
        );
        target.set(10.0);
        renderer.render(
            crate::keyed(AnimatedWidget(target), 1_u64),
            viewport,
            &mut painter,
        );
        assert_eq!(painter.widths.last(), Some(&0.0));
        painter.time = Some(0.5);
        renderer.paint(&mut painter, None, true);
        assert_eq!(painter.widths.last(), Some(&5.0));
        painter.time = Some(1.0);
        painter.animated = false;
        renderer.paint(&mut painter, None, true);
        assert_eq!(painter.widths.last(), Some(&10.0));
        assert!(!painter.animated);
    }

    #[test]
    fn runtime_transitions_repaint_and_skip_transparent_nodes() {
        use crate::runtime::{mount_legacy_widget, Mutation, Runtime, Transform2D};
        let target = std::rc::Rc::new(std::cell::Cell::new(0.0));
        let mut runtime = Runtime::new();
        let node = mount_legacy_widget(
            &mut runtime.transaction(),
            Box::new(AnimatedWidget(target.clone())),
            None,
        );
        runtime.set_root(Some(node));
        runtime.transaction().apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 5.0, y: 0.0 },
        });
        runtime.compute_layout(crate::Size {
            width: 30.0,
            height: 20.0,
        });
        runtime.rebuild_composite();
        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());
        let mut painter = ClockPainter {
            time: Some(0.0),
            ..Default::default()
        };
        runtime.paint_to(&mut painter);
        target.set(10.0);
        runtime.paint_to(&mut painter);
        assert!(painter.animated);
        painter.time = Some(0.5);
        runtime.paint_to(&mut painter);
        assert_eq!(painter.widths.last(), Some(&5.0));
        runtime
            .transaction()
            .apply(Mutation::SetOpacity { node, opacity: 0.0 });
        runtime.rebuild_composite();
        painter.animated = false;
        painter.widths.clear();
        runtime.paint_to(&mut painter);
        assert!(!painter.animated);
        assert!(painter.widths.is_empty());
    }
}
