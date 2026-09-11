use super::*;
use std::time::Duration;

const SPEED: f32 = 26.0;
const PAUSE: Duration = Duration::from_millis(1200);

/// Scrolling ticker text: paints statically when it fits its own width, and
/// eases back and forth (not a hard reset) when it doesn't.
pub struct RawMarquee {
    pub text: String,
    pub style: creamui_core::Style,
    pub font_size: f32,
    pub color: Color,
    pub font_family: Option<String>,
    pub bold: bool,
}

impl RawMarquee {
    pub fn new(text: impl Into<String>, color: Color, font_size: f32, width: f32) -> Self {
        Self {
            text: text.into(),
            style: creamui_core::Style::new().layout(Style {
                size: crate::layout::fixed(width, 24.0),
                ..Default::default()
            }),
            font_size,
            color,
            font_family: None,
            bold: false,
        }
    }

    pub fn expanding(text: impl Into<String>, color: Color, font_size: f32) -> Self {
        Self {
            text: text.into(),
            style: creamui_core::Style::new().layout(Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Auto,
                    height: creamui_core::layout::Dimension::Length(24.0),
                },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }),
            font_size,
            color,
            font_family: None,
            bold: false,
        }
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn bold(mut self, active: bool) -> Self {
        self.bold = active;
        self
    }

    fn estimated_width(&self) -> f32 {
        self.text.chars().count() as f32 * self.font_size * 0.56
    }
}

/// Offset (in the range `0..=travel`) at `phase` seconds into the
/// pause-forward-pause-backward cycle: eases the same way it came instead of
/// resetting, so the loop point is never visually discontinuous.
fn ping_pong_offset(phase: f32, travel: f32) -> f32 {
    let move_time = travel / SPEED;
    let pause = PAUSE.as_secs_f32();
    let t1 = pause;
    let t2 = t1 + move_time;
    let t3 = t2 + pause;
    if phase < t1 {
        0.0
    } else if phase < t2 {
        (phase - t1) * SPEED
    } else if phase < t3 {
        travel
    } else {
        travel - (phase - t3) * SPEED
    }
}

impl Widget for RawMarquee {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let text_width = self.estimated_width();
        let overflow = text_width - rect.width;
        if overflow <= 0.0 {
            painter.push_clip(rect);
            painter.fill_text_font(
                Rect {
                    width: rect.width + text_width + 1024.0,
                    ..rect
                },
                &self.text,
                self.color,
                self.font_size,
                TextAlign::Start,
                self.font_family.as_deref(),
                self.bold,
                false,
            );
            painter.pop_clip();
            return;
        }
        let travel = overflow;
        let move_time = Duration::from_secs_f32(travel / SPEED);
        let cycle = PAUSE + move_time + PAUSE + move_time;
        let phase = painter.animation_time() % cycle.as_secs_f32();
        let offset = ping_pong_offset(phase, travel);
        painter.push_clip(rect);
        painter.fill_text_font(
            Rect {
                x: rect.x - offset,
                y: rect.y,
                width: text_width + rect.width + 1024.0,
                height: rect.height,
            },
            &self.text,
            self.color,
            self.font_size,
            TextAlign::Start,
            self.font_family.as_deref(),
            self.bold,
            false,
        );
        painter.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_pong_offset_eases_back_without_a_reset() {
        let travel = 100.0;
        let move_time = travel / SPEED;
        let pause = PAUSE.as_secs_f32();

        assert_eq!(ping_pong_offset(0.0, travel), 0.0);
        assert_eq!(ping_pong_offset(pause, travel), 0.0);
        assert!((ping_pong_offset(pause + move_time / 2.0, travel) - travel / 2.0).abs() < 0.01);
        assert_eq!(ping_pong_offset(pause + move_time, travel), travel);
        assert_eq!(ping_pong_offset(pause + move_time + pause, travel), travel);

        let return_start = pause + move_time + pause;
        assert!(
            (ping_pong_offset(return_start + move_time / 2.0, travel) - travel / 2.0).abs() < 0.01
        );
        let cycle_end = return_start + move_time;
        assert!(ping_pong_offset(cycle_end, travel).abs() < 0.01);
    }
}
