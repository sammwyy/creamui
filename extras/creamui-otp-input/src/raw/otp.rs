use super::*;
use creamui_reactive::Signal;

#[derive(Clone)]
pub struct OtpController {
    value: Signal<String>,
    cursor: Signal<usize>,
    focused: Signal<bool>,
    length: usize,
}

impl OtpController {
    pub fn new(length: usize, value: impl Into<String>) -> Self {
        let length = length.clamp(1, 12);
        let value: String = value
            .into()
            .chars()
            .filter(|character| character.is_ascii_digit())
            .take(length)
            .collect();
        let cursor = value.len().min(length - 1);
        Self {
            value: Signal::new(value),
            cursor: Signal::new(cursor),
            focused: Signal::new(false),
            length,
        }
    }

    pub fn value(&self) -> String {
        self.value.get()
    }

    pub fn peek(&self) -> String {
        self.value.peek()
    }

    pub fn set_value(&self, value: impl Into<String>) {
        let value: String = value
            .into()
            .chars()
            .filter(|character| character.is_ascii_digit())
            .take(self.length)
            .collect();
        self.cursor.set_if_changed(value.len().min(self.length - 1));
        self.value.set_if_changed(value);
    }

    pub fn cursor(&self) -> usize {
        self.cursor.get()
    }

    pub fn set_cursor(&self, index: usize) {
        self.cursor.set_if_changed(index.min(self.length - 1));
    }

    pub fn length(&self) -> usize {
        self.length
    }

    fn insert(&self, input: &str) -> bool {
        let mut value: Vec<char> = self.value.peek().chars().collect();
        let mut index = self.cursor.peek().min(value.len());
        let mut changed = false;
        for digit in input.chars().filter(|character| character.is_ascii_digit()) {
            if index >= self.length {
                break;
            }
            if index == value.len() {
                value.push(digit);
            } else {
                value[index] = digit;
            }
            changed = true;
            index += 1;
        }
        if changed {
            self.value.set(value.into_iter().collect());
            self.cursor.set_if_changed(index.min(self.length - 1));
        }
        changed
    }

    fn backspace(&self) -> bool {
        let mut value: Vec<char> = self.value.peek().chars().collect();
        let cursor = self.cursor.peek().min(value.len());
        if cursor == 0 {
            return false;
        }
        value.remove(cursor - 1);
        self.value.set(value.into_iter().collect());
        self.cursor.set_if_changed(cursor - 1);
        true
    }
}

#[derive(Clone, Copy)]
pub struct OtpColors {
    pub text: Color,
    pub surface: Color,
    pub border: Color,
    pub focus: Color,
    pub error: Color,
}

pub struct RawOtpInput {
    pub style: creamui_core::Style,
    controller: OtpController,
    on_change: Rc<dyn Fn(String)>,
    colors: OtpColors,
    error: bool,
    masked: bool,
    disabled: bool,
    bounds: Rc<Cell<Rect>>,
    lifetime: Rc<()>,
}

impl RawOtpInput {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &OtpController,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        let _ = controller.value();
        let _ = controller.cursor();
        let _ = controller.focused.get();
        Self {
            style: style.into(),
            controller: controller.clone(),
            on_change: Rc::new(on_change),
            colors: OtpColors {
                text: Color::rgb(0, 0, 0),
                surface: Color::rgb(255, 255, 255),
                border: Color::rgb(160, 160, 160),
                focus: Color::rgb(30, 90, 210),
                error: Color::rgb(180, 30, 30),
            },
            error: false,
            masked: false,
            disabled: false,
            bounds: Rc::new(Cell::new(Rect::default())),
            lifetime: Rc::new(()),
        }
    }

    pub fn colors(mut self, colors: OtpColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn cell_width(&self, rect: Rect) -> f32 {
        rect.width / self.controller.length as f32
    }
}

impl Styled for RawOtpInput {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.style = style;
    }
}

impl Widget for RawOtpInput {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        creamui_core::StyleState::NORMAL.with_disabled(self.disabled)
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn accepts_text_input(&self) -> bool {
        true
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Text
        })
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.bounds.set(rect);
        false
    }

    fn on_focus_change(&self) -> Option<Rc<dyn Fn(bool)>> {
        let controller = self.controller.clone();
        Some(Rc::new(move |focused| {
            controller.focused.set_if_changed(focused)
        }))
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        if self.disabled {
            return None;
        }
        let bounds = self.bounds.clone();
        let controller = self.controller.clone();
        Some(Rc::new(move |point| {
            let rect = bounds.get();
            if rect.width > 0.0 {
                controller.set_cursor(
                    ((point.x - rect.x) / rect.width * controller.length as f32)
                        .floor()
                        .max(0.0) as usize,
                );
            }
        }))
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.disabled {
            return None;
        }
        let controller = self.controller.clone();
        let on_change = self.on_change.clone();
        let lifetime = Rc::downgrade(&self.lifetime);
        Some(Rc::new(move |input| {
            if input.modifiers.ctrl && matches!(input.key, Key::Char('v') | Key::Char('V')) {
                let controller = controller.clone();
                let on_change = on_change.clone();
                let lifetime = lifetime.clone();
                creamui_widgets::read_clipboard(move |paste| {
                    if lifetime.upgrade().is_some() && controller.insert(&paste) {
                        on_change(controller.peek());
                    }
                });
                return;
            }
            if input.modifiers.ctrl || input.modifiers.alt {
                return;
            }
            let changed = match input.key {
                Key::Char(digit) if digit.is_ascii_digit() => controller.insert(&digit.to_string()),
                Key::Backspace => controller.backspace(),
                Key::Delete => {
                    let cursor = controller.cursor.peek();
                    let value = controller.peek();
                    if cursor < value.len() {
                        let mut value = value;
                        value.remove(cursor);
                        controller.value.set(value);
                        true
                    } else {
                        false
                    }
                }
                Key::Left => {
                    controller.set_cursor(controller.cursor.peek().saturating_sub(1));
                    false
                }
                Key::Right => {
                    controller.set_cursor(controller.cursor.peek() + 1);
                    false
                }
                Key::Home => {
                    controller.set_cursor(0);
                    false
                }
                Key::End => {
                    controller.set_cursor(controller.length - 1);
                    false
                }
                _ => false,
            };
            if changed {
                on_change(controller.peek());
            }
        }))
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let value = self.controller.value();
        let cursor = self.controller.cursor();
        let focused = self.controller.focused.get();
        let width = self.cell_width(rect);
        for index in 0..self.controller.length {
            let cell = Rect {
                x: rect.x + width * index as f32 + 2.0,
                y: rect.y + 2.0,
                width: (width - 4.0).max(0.0),
                height: (rect.height - 4.0).max(0.0),
            };
            painter.fill_rect(cell, self.colors.surface, 5.0);
            painter.stroke_rect_inside(
                cell,
                if self.error {
                    self.colors.error
                } else if focused && cursor == index {
                    self.colors.focus
                } else {
                    self.colors.border
                },
                if focused && cursor == index { 2.0 } else { 1.0 },
                5.0,
            );
            if let Some(digit) = value.chars().nth(index) {
                let visible = if self.masked {
                    "*".to_owned()
                } else {
                    digit.to_string()
                };
                painter.fill_text(
                    cell,
                    &visible,
                    self.colors.text,
                    (rect.height * 0.5).max(12.0),
                    TextAlign::Center,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_controller_filters_and_distributes_digits() {
        let controller = OtpController::new(4, "1x2");
        assert_eq!(controller.peek(), "12");
        controller.set_cursor(1);
        assert!(controller.insert("9-87"));
        assert_eq!(controller.peek(), "1987");
        assert_eq!(controller.cursor(), 3);
        assert!(controller.backspace());
        assert_eq!(controller.peek(), "197");
    }

    #[test]
    fn otp_widget_moves_cursor_and_notifies_on_key() {
        let controller = OtpController::new(4, "");
        let edits = Rc::new(Cell::new(0usize));
        let sink = edits.clone();
        let input = RawOtpInput::new(Style::default(), &controller, move |_| {
            sink.set(sink.get() + 1)
        });
        for key in [Key::Char('4'), Key::Char('5'), Key::Backspace, Key::Left] {
            input.on_key().unwrap()(KeyInput {
                key,
                modifiers: Default::default(),
            });
        }
        assert_eq!(controller.peek(), "4");
        assert_eq!(edits.get(), 3);
        assert!(RawOtpInput::new(Style::default(), &controller, |_| {})
            .disabled(true)
            .on_key()
            .is_none());
    }
}
