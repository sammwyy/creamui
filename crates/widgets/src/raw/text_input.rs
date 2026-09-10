use super::*;
/// An unstyled single-line text input. The caller owns the current text
/// (typically a `String` `Signal`) and updates it from `on_change`, fired
/// on every keystroke — same "no internal state" pattern as every other
/// widget. Supports appending characters and backspace; cursor
/// positioning/selection is not implemented yet (see ROADMAP.md).
pub struct RawTextInput {
    pub style: Style,
    pub value: String,
    pub placeholder: String,
    pub text_color: Color,
    pub placeholder_color: Color,
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub corner_radius: f32,
    pub font_size: f32,
    pub family: Option<String>,
    pub cursor: usize,
    pub selection: TextSelection,
    pub selection_background: Option<Color>,
    pub selection_text_color: Option<Color>,
    pub on_change: Rc<dyn Fn(String)>,
    pub on_cursor_change: Rc<dyn Fn(usize)>,
    pub on_selection_change: Rc<dyn Fn(TextSelection)>,
    pub on_submit: Rc<dyn Fn()>,
    pub clipboard_enabled: bool,
    keyboard_selection: Rc<Cell<TextSelection>>,
    drag_anchor: Rc<Cell<usize>>,
}

/// An unstyled multi-line text editor. Like [`RawTextInput`], its value is
/// owned by the caller; this deliberately keeps editing state compatible
/// with CreamUI's reactive, rebuild-on-change model.
pub struct RawTextArea {
    pub style: Style,
    pub value: String,
    pub placeholder: String,
    pub text_color: Color,
    pub placeholder_color: Color,
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub corner_radius: f32,
    pub font_size: f32,
    pub family: Option<String>,
    pub cursor: usize,
    pub alternating_line_background: Option<Color>,
    pub active_line_background: Option<Color>,
    pub selection: TextSelection,
    pub selection_background: Option<Color>,
    pub selection_text_color: Option<Color>,
    pub on_change: Rc<dyn Fn(String)>,
    pub on_cursor_change: Rc<dyn Fn(usize)>,
    pub on_selection_change: Rc<dyn Fn(TextSelection)>,
    pub on_ctrl_o: Rc<dyn Fn()>,
    pub clipboard_enabled: bool,
    pub wrap: bool,
    // Pointer interaction can outlive a reactive frame when renders are
    // coalesced. This tiny ephemeral cell keeps drag selection anchored
    // without requiring every mouse move to rebuild the widget tree.
    drag_anchor: Rc<Cell<usize>>,
    drag_focus: Rc<Cell<usize>>,
    keyboard_selection: Rc<Cell<TextSelection>>,
}

impl RawTextArea {
    pub fn new(
        style: Style,
        value: impl Into<String>,
        font_size: f32,
        text_color: Color,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            style,
            value,
            placeholder: String::new(),
            text_color,
            placeholder_color: text_color,
            background: None,
            border_color: None,
            border_width: 1.0,
            corner_radius: 0.0,
            font_size,
            family: None,
            cursor,
            alternating_line_background: None,
            active_line_background: None,
            selection: TextSelection {
                anchor: cursor,
                focus: cursor,
            },
            selection_background: None,
            selection_text_color: None,
            on_change: Rc::new(on_change),
            on_cursor_change: Rc::new(|_| {}),
            on_selection_change: Rc::new(|_| {}),
            on_ctrl_o: Rc::new(|| {}),
            clipboard_enabled: true,
            wrap: false,
            drag_anchor: Rc::new(Cell::new(cursor)),
            drag_focus: Rc::new(Cell::new(cursor)),
            keyboard_selection: Rc::new(Cell::new(TextSelection {
                anchor: cursor,
                focus: cursor,
            })),
        }
    }

    /// Supplies a controlled byte-index cursor and receives updates from
    /// keyboard navigation or pointer placement.
    pub fn cursor(mut self, cursor: usize, on_change: impl Fn(usize) + 'static) -> Self {
        self.cursor = cursor.min(self.value.len());
        self.drag_anchor.set(self.cursor);
        self.drag_focus.set(self.cursor);
        self.keyboard_selection.set(TextSelection {
            anchor: self.cursor,
            focus: self.cursor,
        });
        self.on_cursor_change = Rc::new(on_change);
        self
    }

    /// Supplies a controlled selection. The application owns it just like it
    /// owns `value` and `cursor`, allowing selection appearance/state to be
    /// coordinated across native, JSX and ABI-built UIs.
    pub fn selection(
        mut self,
        selection: TextSelection,
        on_change: impl Fn(TextSelection) + 'static,
    ) -> Self {
        self.selection = TextSelection {
            anchor: selection.anchor.min(self.value.len()),
            focus: selection.focus.min(self.value.len()),
        };
        self.drag_anchor.set(self.selection.anchor);
        self.drag_focus.set(self.selection.focus);
        self.keyboard_selection.set(self.selection);
        self.on_selection_change = Rc::new(on_change);
        self
    }

    /// Visual token for selected text. The text itself remains in the
    /// editor's normal color until rich text spans land in the renderer.
    pub fn selection_background(mut self, color: Color) -> Self {
        self.selection_background = Some(color);
        self
    }

    /// Foreground token used for the selected text. Pair it with
    /// [`Self::selection_background`] to make an editor's selection fully
    /// match its design system.
    pub fn selection_text_color(mut self, color: Color) -> Self {
        self.selection_text_color = Some(color);
        self
    }

    /// Invoked by the conventional Ctrl/Cmd+O command while this editor has
    /// focus. The app decides what opening a document means.
    pub fn on_ctrl_o(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_ctrl_o = Rc::new(callback);
        self
    }

    /// Enables the platform clipboard shortcuts (Ctrl/Cmd+A, C and V).
    /// Enabled by default; disable it for sensitive or deliberately isolated
    /// editors without changing their keyboard-editing behavior.
    pub fn clipboard_enabled(mut self, enabled: bool) -> Self {
        self.clipboard_enabled = enabled;
        self
    }

    /// When `true`, long lines break onto a new visual row at the editor's
    /// width instead of overflowing it — the caret and click-to-position
    /// follow the wrapped rows too. Off by default (a line just scrolls
    /// horizontally, as `RawTextInput` does).
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
    pub fn placeholder(mut self, text: impl Into<String>, color: Color) -> Self {
        self.placeholder = text.into();
        self.placeholder_color = color;
        self
    }

    /// Paints every second source line with a subtle reading-guide color.
    pub fn alternating_line_background(mut self, color: Color) -> Self {
        self.alternating_line_background = Some(color);
        self
    }

    /// Highlights the source line containing the caret.
    pub fn active_line_background(mut self, color: Color) -> Self {
        self.active_line_background = Some(color);
        self
    }

    /// How far to shift every line left so the caret stays inside
    /// `visible_width` instead of running off the unwrapped line's edge.
    fn horizontal_scroll(&self, visible_width: f32) -> f32 {
        let cursor = self.cursor.min(self.value.len());
        let line_start = self.value[..cursor].rfind('\n').map_or(0, |i| i + 1);
        let (cursor_x, _) = crate::text_metrics::measure_family(
            &self.value[line_start..cursor],
            self.font_size,
            crate::text_metrics::unbounded_width(),
            self.family.as_deref(),
            false,
        );
        (cursor_x - visible_width + 4.0).max(0.0)
    }

    /// One row per source line, unbounded width, scrolled horizontally so
    /// the caret stays visible — no wrapping.
    fn paint_unwrapped(
        &self,
        painter: &mut dyn Painter,
        text_rect: Rect,
        text: &str,
        color: Color,
    ) {
        let line_height = self.font_size * 1.4;
        let active_line = self.value[..self.cursor.min(self.value.len())]
            .matches('\n')
            .count();
        let scroll_x = self.horizontal_scroll(text_rect.width);
        let selected = self.selection.range();
        let mut source_offset = 0;
        for (index, line) in text.split('\n').enumerate() {
            let line_rect = Rect {
                y: text_rect.y + index as f32 * line_height,
                height: line_height,
                ..text_rect
            };
            let unbounded_line_rect = Rect {
                x: line_rect.x - scroll_x,
                width: crate::text_metrics::unbounded_width(),
                ..line_rect
            };
            if index == active_line {
                if let Some(background) = self.active_line_background {
                    painter.fill_rect(line_rect, background, 0.0);
                }
            } else if index % 2 == 1 {
                if let Some(background) = self.alternating_line_background {
                    painter.fill_rect(line_rect, background, 0.0);
                }
            }
            if !self.value.is_empty() {
                let line_end = source_offset + line.len();
                let start = selected.start.max(source_offset).min(line_end);
                let end = selected.end.max(source_offset).min(line_end);
                if start < end {
                    if let Some(background) = self.selection_background {
                        let prefix = &line[..start - source_offset];
                        let selected_text = &line[start - source_offset..end - source_offset];
                        let (x, _) = crate::text_metrics::measure_family(
                            prefix,
                            self.font_size,
                            crate::text_metrics::unbounded_width(),
                            self.family.as_deref(),
                            false,
                        );
                        let (width, _) = crate::text_metrics::measure_family(
                            selected_text,
                            self.font_size,
                            crate::text_metrics::unbounded_width(),
                            self.family.as_deref(),
                            false,
                        );
                        painter.fill_rect(
                            Rect {
                                x: line_rect.x + x - scroll_x,
                                width,
                                ..line_rect
                            },
                            background,
                            2.0,
                        );
                    }
                    painter.fill_text_selected_font(
                        unbounded_line_rect,
                        line,
                        color,
                        self.selection_text_color.unwrap_or(color),
                        start - source_offset..end - source_offset,
                        self.font_size,
                        TextAlign::Start,
                        self.family.as_deref(),
                    );
                    source_offset = line_end + 1;
                    continue;
                }
                source_offset = line_end + 1;
            }
            painter.fill_text_font(
                unbounded_line_rect,
                line,
                color,
                self.font_size,
                TextAlign::Start,
                self.family.as_deref(),
                false,
                false,
            );
        }
    }

    /// Bounded width, letting `fontdue` wrap long lines onto new visual
    /// rows; selection is highlighted per glyph since rows no longer line
    /// up with source lines.
    fn paint_wrapped(&self, painter: &mut dyn Painter, text_rect: Rect, text: &str, color: Color) {
        // `Painter::fill_text` centers a block vertically within the rect
        // it's given. A rect as tall as the whole editor would center a
        // short wrapped block partway down it, desyncing every y this
        // module computes (which all assume the first row starts at the
        // rect's very top). Sizing the rect to the block's own height
        // makes that centering a no-op.
        let block_rect = Rect {
            height: crate::text_metrics::content_height_family(
                text,
                self.font_size,
                text_rect.width,
                self.family.as_deref(),
            )
            .max(crate::text_metrics::row_height_family(
                self.font_size,
                self.family.as_deref(),
            )),
            ..text_rect
        };
        let selected = self.selection.range();
        if !self.value.is_empty() && !selected.is_empty() {
            if let Some(background) = self.selection_background {
                for glyph in crate::text_metrics::layout_family(
                    text,
                    self.font_size,
                    text_rect.width,
                    self.family.as_deref(),
                ) {
                    if selected.contains(&glyph.byte_offset) {
                        painter.fill_rect(
                            Rect {
                                x: text_rect.x + glyph.x,
                                y: text_rect.y + glyph.y,
                                width: glyph.advance,
                                height: glyph.row_height,
                            },
                            background,
                            0.0,
                        );
                    }
                }
            }
            painter.fill_text_selected_font(
                block_rect,
                text,
                color,
                self.selection_text_color.unwrap_or(color),
                selected,
                self.font_size,
                TextAlign::Start,
                self.family.as_deref(),
            );
        } else {
            painter.fill_text_font(
                block_rect,
                text,
                color,
                self.font_size,
                TextAlign::Start,
                self.family.as_deref(),
                false,
                false,
            );
        }
    }
}

impl Widget for RawTextArea {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(color) = self.background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        if let Some(color) = self.border_color.filter(|_| self.border_width > 0.0) {
            painter.stroke_rect(rect, color, self.border_width, self.corner_radius);
        }
        let padding = 12.0;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: (rect.width - padding * 2.0).max(0.0),
            height: (rect.height - padding * 2.0).max(0.0),
        };
        let (text, color) = if self.value.is_empty() && !self.placeholder.is_empty() {
            (&self.placeholder, self.placeholder_color)
        } else {
            (&self.value, self.text_color)
        };
        painter.push_clip(text_rect);
        if self.wrap {
            self.paint_wrapped(painter, text_rect, text, color);
        } else {
            self.paint_unwrapped(painter, text_rect, text, color);
        }
        painter.pop_clip();
    }

    fn focusable(&self) -> bool {
        true
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        if !caret_visible {
            return;
        }
        let padding = 12.0;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: (rect.width - padding * 2.0).max(0.0),
            height: (rect.height - padding * 2.0).max(0.0),
        };
        let cursor = self.cursor.min(self.value.len());
        // Caret proportions match `RawTextInput`'s: a slim bar sized and
        // vertically centered to the glyphs, not a full-height block.
        let (caret_x, caret_y, row_height) = if self.wrap {
            let glyphs = crate::text_metrics::layout_family(
                &self.value,
                self.font_size,
                text_rect.width,
                self.family.as_deref(),
            );
            let fallback =
                crate::text_metrics::row_height_family(self.font_size, self.family.as_deref());
            let (x, y, row_height) = crate::text_metrics::caret_xy(&glyphs, cursor, fallback);
            (text_rect.x + x, text_rect.y + y, row_height)
        } else {
            let before_cursor = &self.value[..cursor];
            let line = before_cursor.rsplit('\n').next().unwrap_or("");
            let (width, _) = crate::text_metrics::measure_family(
                line,
                self.font_size,
                crate::text_metrics::unbounded_width(),
                self.family.as_deref(),
                false,
            );
            let lines = (before_cursor.matches('\n').count()) as f32;
            let line_height = self.font_size * 1.4;
            let scroll_x = self.horizontal_scroll(text_rect.width);
            (
                text_rect.x + width - scroll_x,
                text_rect.y + lines * line_height,
                line_height,
            )
        };
        let caret_height = (self.font_size * 1.2).min(row_height);
        painter.push_clip(text_rect);
        painter.fill_rect(
            Rect {
                x: caret_x,
                y: caret_y + (row_height - caret_height) / 2.0,
                width: 1.5,
                height: caret_height,
            },
            self.text_color,
            0.0,
        );
        painter.pop_clip();
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let value = self.value.clone();
        let on_change = self.on_change.clone();
        let cursor = self.cursor;
        let on_cursor_change = self.on_cursor_change.clone();
        let keyboard_selection = self.keyboard_selection.clone();
        let on_selection_change = self.on_selection_change.clone();
        let on_ctrl_o = self.on_ctrl_o.clone();
        let clipboard_enabled = self.clipboard_enabled;
        Some(Rc::new(move |input| {
            let selection = keyboard_selection.get();
            if clipboard_enabled && input.modifiers.ctrl {
                match input.key {
                    Key::Char('a') | Key::Char('A') => {
                        let all = TextSelection {
                            anchor: 0,
                            focus: value.len(),
                        };
                        keyboard_selection.set(all);
                        creamui_reactive::batch(|| {
                            on_cursor_change(value.len());
                            on_selection_change(all);
                        });
                        return;
                    }
                    Key::Char('c') | Key::Char('C') if !selection.is_empty() => {
                        clipboard_write(value[selection.range()].to_owned());
                        return;
                    }
                    Key::Char('x') | Key::Char('X') if !selection.is_empty() => {
                        let range = selection.range();
                        clipboard_write(value[range.clone()].to_owned());
                        let mut next = value.clone();
                        next.replace_range(range.clone(), "");
                        let next_cursor = range.start;
                        creamui_reactive::batch(|| {
                            on_change(next);
                            on_cursor_change(next_cursor);
                            on_selection_change(TextSelection {
                                anchor: next_cursor,
                                focus: next_cursor,
                            });
                        });
                        return;
                    }
                    Key::Char('v') | Key::Char('V') => {
                        if let Some(pasted) = clipboard_read() {
                            let mut next = value.clone();
                            let range = selection.range();
                            let next_cursor = if range.is_empty() {
                                next.insert_str(cursor.min(next.len()), &pasted);
                                cursor.min(value.len()) + pasted.len()
                            } else {
                                next.replace_range(range.clone(), &pasted);
                                range.start + pasted.len()
                            };
                            keyboard_selection.set(TextSelection {
                                anchor: next_cursor,
                                focus: next_cursor,
                            });
                            creamui_reactive::batch(|| {
                                on_change(next);
                                on_cursor_change(next_cursor);
                                on_selection_change(TextSelection {
                                    anchor: next_cursor,
                                    focus: next_cursor,
                                });
                            });
                        }
                        return;
                    }
                    _ => {}
                }
            }
            if input.modifiers.ctrl && matches!(input.key, Key::Char('o') | Key::Char('O')) {
                on_ctrl_o();
                return;
            }
            let mut next = value.clone();
            let mut next_cursor = cursor.min(next.len());
            let selected = selection.range();
            let mut edited = false;
            let mut replace_selection = |replacement: &str| {
                if !selected.is_empty() {
                    next.replace_range(selected.clone(), replacement);
                    next_cursor = selected.start + replacement.len();
                    edited = true;
                } else {
                    next.insert_str(next_cursor, replacement);
                    next_cursor += replacement.len();
                    edited = true;
                }
            };
            match input.key {
                Key::Char(c) => {
                    replace_selection(&c.to_string());
                }
                Key::Enter => {
                    replace_selection("\n");
                }
                Key::Backspace => {
                    if !selected.is_empty() {
                        next.replace_range(selected.clone(), "");
                        next_cursor = selected.start;
                        edited = true;
                    } else if let Some(previous) = next[..next_cursor]
                        .char_indices()
                        .last()
                        .map(|(index, _)| index)
                    {
                        next.drain(previous..next_cursor);
                        next_cursor = previous;
                        edited = true;
                    }
                }
                Key::Left => {
                    if let Some(previous) = next[..next_cursor]
                        .char_indices()
                        .last()
                        .map(|(index, _)| index)
                    {
                        next_cursor = previous;
                    }
                }
                Key::Right => {
                    if let Some(character) = next[next_cursor..].chars().next() {
                        next_cursor += character.len_utf8();
                    }
                }
                Key::Home => {
                    next_cursor = next[..next_cursor].rfind('\n').map_or(0, |index| index + 1);
                }
                Key::End => {
                    next_cursor = next[next_cursor..]
                        .find('\n')
                        .map_or(next.len(), |index| next_cursor + index);
                }
                Key::Up | Key::Down => {
                    let line_start = next[..next_cursor].rfind('\n').map_or(0, |index| index + 1);
                    let column = next[line_start..next_cursor].chars().count();
                    let lines: Vec<&str> = next.split('\n').collect();
                    let line = next[..next_cursor].matches('\n').count();
                    let target = if input.key == Key::Up {
                        line.checked_sub(1)
                    } else {
                        (line + 1 < lines.len()).then_some(line + 1)
                    };
                    if let Some(target) = target {
                        let start = lines
                            .iter()
                            .take(target)
                            .map(|line| line.len() + 1)
                            .sum::<usize>();
                        next_cursor = start
                            + lines[target]
                                .char_indices()
                                .nth(column)
                                .map_or(lines[target].len(), |(index, _)| index);
                    }
                }
                _ => return,
            }
            let extend = input.modifiers.shift
                && matches!(
                    input.key,
                    Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
                );
            let next_selection = if extend {
                TextSelection {
                    anchor: if selection.is_empty() {
                        cursor
                    } else {
                        selection.anchor
                    },
                    focus: next_cursor,
                }
            } else {
                TextSelection {
                    anchor: next_cursor,
                    focus: next_cursor,
                }
            };
            keyboard_selection.set(next_selection);
            creamui_reactive::batch(|| {
                if edited {
                    on_change(next);
                }
                on_cursor_change(next_cursor);
                on_selection_change(next_selection);
            });
        }))
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let value = self.value.clone();
        let font_size = self.font_size;
        let family = self.family.clone();
        let wrap = self.wrap;
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let drag_anchor = self.drag_anchor.clone();
        let drag_focus = self.drag_focus.clone();
        let keyboard_selection = self.keyboard_selection.clone();
        Some(Rc::new(move |point, rect: Rect| {
            let cursor = cursor_at_point(
                &value,
                font_size,
                family.as_deref(),
                point,
                wrap,
                rect.width - 24.0,
            );
            drag_anchor.set(cursor);
            drag_focus.set(cursor);
            keyboard_selection.set(TextSelection {
                anchor: cursor,
                focus: cursor,
            });
            creamui_reactive::batch(|| {
                on_cursor_change(cursor);
                on_selection_change(TextSelection {
                    anchor: cursor,
                    focus: cursor,
                });
            });
        }))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let value = self.value.clone();
        let font_size = self.font_size;
        let family = self.family.clone();
        let wrap = self.wrap;
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let drag_anchor = self.drag_anchor.clone();
        let drag_focus = self.drag_focus.clone();
        let keyboard_selection = self.keyboard_selection.clone();
        Some(Rc::new(move |point, rect: Rect| {
            let cursor = cursor_at_point(
                &value,
                font_size,
                family.as_deref(),
                point,
                wrap,
                rect.width - 24.0,
            );
            if cursor != drag_focus.get() {
                drag_focus.set(cursor);
                keyboard_selection.set(TextSelection {
                    anchor: drag_anchor.get(),
                    focus: cursor,
                });
                creamui_reactive::batch(|| {
                    on_cursor_change(cursor);
                    on_selection_change(TextSelection {
                        anchor: drag_anchor.get(),
                        focus: cursor,
                    });
                });
            }
        }))
    }
}

fn cursor_at_point(
    value: &str,
    font_size: f32,
    family: Option<&str>,
    point: Point,
    wrap: bool,
    visible_width: f32,
) -> usize {
    if wrap {
        return crate::text_metrics::byte_offset_at_point_family(
            value,
            font_size,
            visible_width,
            point.x - 12.0,
            point.y - 12.0,
            family,
        );
    }
    let line = ((point.y - 12.0) / (font_size * 1.4)).floor().max(0.0) as usize;
    let lines: Vec<&str> = value.split('\n').collect();
    let line = line.min(lines.len().saturating_sub(1));
    let start = lines
        .iter()
        .take(line)
        .map(|line| line.len() + 1)
        .sum::<usize>();
    start
        + crate::text_metrics::byte_offset_at_x_family(
            lines[line],
            font_size,
            (point.x - 12.0).max(0.0),
            family,
        )
}

impl RawTextInput {
    pub fn new(
        style: Style,
        value: impl Into<String>,
        font_size: f32,
        text_color: Color,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        let value = value.into();
        let cursor = value.len();
        RawTextInput {
            style,
            value,
            placeholder: String::new(),
            text_color,
            placeholder_color: text_color,
            background: None,
            border_color: None,
            border_width: 1.0,
            corner_radius: 0.0,
            font_size,
            family: None,
            cursor,
            selection: TextSelection {
                anchor: cursor,
                focus: cursor,
            },
            selection_background: None,
            selection_text_color: None,
            on_change: Rc::new(on_change),
            on_cursor_change: Rc::new(|_| {}),
            on_selection_change: Rc::new(|_| {}),
            on_submit: Rc::new(|| {}),
            clipboard_enabled: true,
            keyboard_selection: Rc::new(Cell::new(TextSelection {
                anchor: cursor,
                focus: cursor,
            })),
            drag_anchor: Rc::new(Cell::new(cursor)),
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>, color: Color) -> Self {
        self.placeholder = text.into();
        self.placeholder_color = color;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }

    /// Enables Ctrl/Cmd+V for this field. Text inputs expose the same opt-out
    /// surface as text areas; it is enabled by default.
    pub fn clipboard_enabled(mut self, enabled: bool) -> Self {
        self.clipboard_enabled = enabled;
        self
    }

    pub fn cursor(mut self, cursor: usize, on_change: impl Fn(usize) + 'static) -> Self {
        self.cursor = cursor.min(self.value.len());
        self.selection = TextSelection {
            anchor: self.cursor,
            focus: self.cursor,
        };
        self.keyboard_selection.set(self.selection);
        self.on_cursor_change = Rc::new(on_change);
        self
    }

    pub fn selection(
        mut self,
        selection: TextSelection,
        on_change: impl Fn(TextSelection) + 'static,
    ) -> Self {
        self.selection = TextSelection {
            anchor: selection.anchor.min(self.value.len()),
            focus: selection.focus.min(self.value.len()),
        };
        self.keyboard_selection.set(self.selection);
        self.on_selection_change = Rc::new(on_change);
        self
    }

    pub fn selection_background(mut self, color: Color) -> Self {
        self.selection_background = Some(color);
        self
    }
    pub fn selection_text_color(mut self, color: Color) -> Self {
        self.selection_text_color = Some(color);
        self
    }

    /// Called on Enter. Single-line input has no use for a literal newline,
    /// so this is the hook for "submit on Enter" instead.
    pub fn on_submit(mut self, on_submit: impl Fn() + 'static) -> Self {
        self.on_submit = Rc::new(on_submit);
        self
    }

    /// How far to shift the value left so its end (editing is append-only)
    /// stays inside `visible_width` instead of running off the edge.
    fn horizontal_scroll(&self, visible_width: f32) -> f32 {
        let (text_width, _) = crate::text_metrics::measure_family(
            &self.value,
            self.font_size,
            crate::text_metrics::unbounded_width(),
            self.family.as_deref(),
            false,
        );
        (text_width - visible_width + 4.0).max(0.0)
    }
}

impl Widget for RawTextInput {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(color) = self.background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        if let Some(color) = self.border_color {
            painter.stroke_rect(rect, color, self.border_width, self.corner_radius);
        }
        let padding = 8.0;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y,
            width: (rect.width - padding * 2.0).max(0.0),
            height: rect.height,
        };
        // Unwrapped: a bounded width here would let fontdue word-wrap onto a second row.
        let unbounded = Rect {
            x: text_rect.x - self.horizontal_scroll(text_rect.width),
            width: crate::text_metrics::unbounded_width(),
            ..text_rect
        };
        painter.push_clip(text_rect);
        if self.value.is_empty() {
            if !self.placeholder.is_empty() {
                painter.fill_text_font(
                    unbounded,
                    &self.placeholder,
                    self.placeholder_color,
                    self.font_size,
                    TextAlign::Start,
                    self.family.as_deref(),
                    false,
                    false,
                );
            }
        } else {
            let selected = self.selection.range();
            if !selected.is_empty() {
                if let Some(background) = self.selection_background {
                    let (before, _) = crate::text_metrics::measure_family(
                        &self.value[..selected.start],
                        self.font_size,
                        crate::text_metrics::unbounded_width(),
                        self.family.as_deref(),
                        false,
                    );
                    let (width, _) = crate::text_metrics::measure_family(
                        &self.value[selected.clone()],
                        self.font_size,
                        crate::text_metrics::unbounded_width(),
                        self.family.as_deref(),
                        false,
                    );
                    painter.fill_rect(
                        Rect {
                            x: unbounded.x + before,
                            y: rect.y + (rect.height - self.font_size * 1.4) / 2.0,
                            width,
                            height: self.font_size * 1.4,
                        },
                        background,
                        2.0,
                    );
                }
            }
            painter.fill_text_selected_font(
                unbounded,
                &self.value,
                self.text_color,
                self.selection_text_color.unwrap_or(self.text_color),
                selected,
                self.font_size,
                TextAlign::Start,
                self.family.as_deref(),
            );
        }
        painter.pop_clip();
    }

    fn focusable(&self) -> bool {
        true
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        if !caret_visible {
            return;
        }
        let padding = 8.0;
        let visible_width = (rect.width - padding * 2.0).max(0.0);
        let cursor = self.cursor.min(self.value.len());
        let (text_width, _) = crate::text_metrics::measure_family(
            &self.value[..cursor],
            self.font_size,
            crate::text_metrics::unbounded_width(),
            self.family.as_deref(),
            false,
        );
        let text_width = if self.value.is_empty() {
            0.0
        } else {
            text_width
        };
        let caret_x = rect.x + padding + text_width - self.horizontal_scroll(visible_width);
        let caret_height = (self.font_size * 1.2).min(rect.height);
        let caret_rect = Rect {
            x: caret_x,
            y: rect.y + (rect.height - caret_height) / 2.0,
            width: 1.5,
            height: caret_height,
        };
        painter.fill_rect(caret_rect, self.text_color, 0.0);
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let value = self.value.clone();
        let on_change = self.on_change.clone();
        let cursor = self.cursor;
        let on_cursor_change = self.on_cursor_change.clone();
        let selection = self.keyboard_selection.clone();
        let on_selection_change = self.on_selection_change.clone();
        let clipboard_enabled = self.clipboard_enabled;
        let on_submit = self.on_submit.clone();
        Some(Rc::new(move |input: KeyInput| {
            if matches!(input.key, Key::Enter) {
                on_submit();
                return;
            }
            let selected = selection.get();
            if clipboard_enabled && input.modifiers.ctrl {
                match input.key {
                    Key::Char('a') | Key::Char('A') => {
                        let all = TextSelection {
                            anchor: 0,
                            focus: value.len(),
                        };
                        selection.set(all);
                        creamui_reactive::batch(|| {
                            on_cursor_change(value.len());
                            on_selection_change(all);
                        });
                        return;
                    }
                    Key::Char('c') | Key::Char('C') if !selected.is_empty() => {
                        clipboard_write(value[selected.range()].to_owned());
                        return;
                    }
                    Key::Char('x') | Key::Char('X') if !selected.is_empty() => {
                        let range = selected.range();
                        clipboard_write(value[range.clone()].to_owned());
                        let mut next = value.clone();
                        next.replace_range(range.clone(), "");
                        let at = range.start;
                        let collapsed = TextSelection {
                            anchor: at,
                            focus: at,
                        };
                        selection.set(collapsed);
                        creamui_reactive::batch(|| {
                            on_change(next);
                            on_cursor_change(at);
                            on_selection_change(collapsed);
                        });
                        return;
                    }
                    Key::Char('v') | Key::Char('V') => {
                        if let Some(paste) = clipboard_read() {
                            let range = selected.range();
                            let mut next = value.clone();
                            let at = if range.is_empty() {
                                cursor.min(next.len())
                            } else {
                                range.start
                            };
                            next.replace_range(
                                if range.is_empty() { at..at } else { range },
                                &paste,
                            );
                            let at = at + paste.len();
                            let collapsed = TextSelection {
                                anchor: at,
                                focus: at,
                            };
                            selection.set(collapsed);
                            creamui_reactive::batch(|| {
                                on_change(next);
                                on_cursor_change(at);
                                on_selection_change(collapsed);
                            });
                        }
                        return;
                    }
                    _ => {}
                }
            }
            let mut next = value.clone();
            let mut at = cursor.min(next.len());
            let range = selected.range();
            let mut changed = false;
            match input.key {
                Key::Char(c) => {
                    next.replace_range(
                        if range.is_empty() {
                            at..at
                        } else {
                            range.clone()
                        },
                        &c.to_string(),
                    );
                    at = if range.is_empty() {
                        at + c.len_utf8()
                    } else {
                        range.start + c.len_utf8()
                    };
                    changed = true;
                }
                Key::Backspace => {
                    if !range.is_empty() {
                        next.replace_range(range.clone(), "");
                        at = range.start;
                        changed = true;
                    } else if let Some(previous) = next[..at].char_indices().last().map(|(i, _)| i)
                    {
                        next.replace_range(previous..at, "");
                        at = previous;
                        changed = true;
                    }
                }
                Key::Delete => {
                    if !range.is_empty() {
                        next.replace_range(range.clone(), "");
                        at = range.start;
                        changed = true;
                    } else if let Some(ch) = next[at..].chars().next() {
                        next.replace_range(at..at + ch.len_utf8(), "");
                        changed = true;
                    }
                }
                Key::Left => {
                    if let Some(previous) = next[..at].char_indices().last().map(|(i, _)| i) {
                        at = previous;
                    }
                }
                Key::Right => {
                    if let Some(ch) = next[at..].chars().next() {
                        at += ch.len_utf8();
                    }
                }
                Key::Home => at = 0,
                Key::End => at = next.len(),
                _ => return,
            }
            let next_selection = if input.modifiers.shift
                && matches!(input.key, Key::Left | Key::Right | Key::Home | Key::End)
            {
                TextSelection {
                    anchor: if selected.is_empty() {
                        cursor
                    } else {
                        selected.anchor
                    },
                    focus: at,
                }
            } else {
                TextSelection {
                    anchor: at,
                    focus: at,
                }
            };
            selection.set(next_selection);
            creamui_reactive::batch(|| {
                if changed {
                    on_change(next);
                }
                on_cursor_change(at);
                on_selection_change(next_selection);
            });
        }))
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let value = self.value.clone();
        let font_size = self.font_size;
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let selection = self.keyboard_selection.clone();
        let anchor = self.drag_anchor.clone();
        Some(Rc::new(move |point, rect| {
            let visible = (rect.width - 16.0).max(0.0);
            let (width, _) = crate::text_metrics::measure(
                &value,
                font_size,
                crate::text_metrics::unbounded_width(),
            );
            let scroll = (width - visible + 4.0).max(0.0);
            let cursor = crate::text_metrics::byte_offset_at_x(
                &value,
                font_size,
                (point.x - 8.0 + scroll).max(0.0),
            );
            anchor.set(cursor);
            let next = TextSelection {
                anchor: cursor,
                focus: cursor,
            };
            selection.set(next);
            creamui_reactive::batch(|| {
                on_cursor_change(cursor);
                on_selection_change(next);
            });
        }))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let value = self.value.clone();
        let font_size = self.font_size;
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let selection = self.keyboard_selection.clone();
        let anchor = self.drag_anchor.clone();
        Some(Rc::new(move |point, rect| {
            let visible = (rect.width - 16.0).max(0.0);
            let (width, _) = crate::text_metrics::measure(
                &value,
                font_size,
                crate::text_metrics::unbounded_width(),
            );
            let scroll = (width - visible + 4.0).max(0.0);
            let cursor = crate::text_metrics::byte_offset_at_x(
                &value,
                font_size,
                (point.x - 8.0 + scroll).max(0.0),
            );
            let next = TextSelection {
                anchor: anchor.get(),
                focus: cursor,
            };
            selection.set(next);
            creamui_reactive::batch(|| {
                on_cursor_change(cursor);
                on_selection_change(next);
            });
        }))
    }
}
