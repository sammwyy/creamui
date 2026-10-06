use super::*;
/// An unstyled single-line input with application-owned text and selection.
pub struct RawTextInput {
    pub style: creamui_core::Style,
    pub value: String,
    pub placeholder: String,
    pub placeholder_color: Color,
    pub cursor: usize,
    pub selection: TextSelection,
    pub selection_background: Option<Color>,
    pub selection_text_color: Option<Color>,
    pub on_change: Rc<dyn Fn(String)>,
    pub on_cursor_change: Rc<dyn Fn(usize)>,
    pub on_selection_change: Rc<dyn Fn(TextSelection)>,
    pub on_submit: Rc<dyn Fn()>,
    pub on_key_press: Rc<dyn Fn(KeyInput)>,
    pub clipboard_enabled: bool,
    /// Paint bullets and prevent copying/cutting secrets to the clipboard.
    pub password: bool,
    paste_handler: Option<Rc<dyn Fn()>>,
    lifetime: Rc<()>,
    keyboard_revision: Rc<Cell<u64>>,
    keyboard_value: Rc<RefCell<String>>,
    keyboard_cursor: Rc<Cell<usize>>,
    keyboard_selection: Rc<Cell<TextSelection>>,
    drag_anchor: Rc<Cell<usize>>,
}

/// An unstyled multi-line text editor. Like [`RawTextInput`], its value is
/// owned by the caller; this deliberately keeps editing state compatible
/// with CreamUI's reactive, rebuild-on-change model.
pub struct RawTextArea {
    pub style: creamui_core::Style,
    pub value: String,
    pub placeholder: String,
    pub placeholder_color: Color,
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
    paste_handler: Option<Rc<dyn Fn()>>,
    lifetime: Rc<()>,
    keyboard_revision: Rc<Cell<u64>>,
    pub wrap: bool,
    // Pointer interaction can outlive a reactive frame when renders are
    // coalesced. This tiny ephemeral cell keeps drag selection anchored
    // without requiring every mouse move to rebuild the widget tree.
    drag_anchor: Rc<Cell<usize>>,
    drag_focus: Rc<Cell<usize>>,
    keyboard_selection: Rc<Cell<TextSelection>>,
    keyboard_value: Rc<RefCell<String>>,
    keyboard_cursor: Rc<Cell<usize>>,
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
        let keyboard_value = Rc::new(RefCell::new(value.clone()));
        Self {
            style: creamui_core::Style::from(style)
                .color(text_color)
                .font_size(font_size),
            value,
            placeholder: String::new(),
            placeholder_color: text_color,
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
            paste_handler: None,
            lifetime: Rc::new(()),
            keyboard_revision: Rc::new(Cell::new(0)),
            wrap: false,
            keyboard_value,
            keyboard_cursor: Rc::new(Cell::new(cursor)),
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
        self.keyboard_cursor.set(self.cursor);
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

    pub(crate) fn paste_handler(mut self, handler: impl Fn() + 'static) -> Self {
        self.paste_handler = Some(Rc::new(handler));
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

    fn font_size(&self) -> f32 {
        self.style.typography.font_size.unwrap_or(14.0)
    }

    fn family(&self) -> Option<&str> {
        self.style.typography.font_family.as_deref()
    }

    fn text_color(&self, painter: &dyn Painter) -> Color {
        self.style
            .typography
            .color
            .map(|color| color.resolve(&painter.color_scheme()))
            .unwrap_or(Color::rgb(0, 0, 0))
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
        let cursor_x = crate::text_metrics::advance_width_family(
            &self.value[line_start..cursor],
            self.font_size(),
            self.family(),
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
        let line_height = self.font_size() * 1.4;
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
                        let x = crate::text_metrics::advance_width_family(
                            prefix,
                            self.font_size(),
                            self.family(),
                        );
                        let (width, _) = crate::text_metrics::measure_family(
                            selected_text,
                            self.font_size(),
                            crate::text_metrics::unbounded_width(),
                            self.family(),
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
                        self.font_size(),
                        TextAlign::Start,
                        self.family(),
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
                self.font_size(),
                TextAlign::Start,
                self.family(),
                false,
                false,
            );
        }
    }

    /// Bounded width, letting the text layout wrap long lines onto new visual
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
                self.font_size(),
                text_rect.width,
                self.family(),
            )
            .max(crate::text_metrics::row_height_family(
                self.font_size(),
                self.family(),
            )),
            ..text_rect
        };
        let selected = self.selection.range();
        if !self.value.is_empty() && !selected.is_empty() {
            if let Some(background) = self.selection_background {
                for glyph in crate::text_metrics::layout_family(
                    text,
                    self.font_size(),
                    text_rect.width,
                    self.family(),
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
                self.font_size(),
                TextAlign::Start,
                self.family(),
            );
        } else {
            painter.fill_text_font(
                block_rect,
                text,
                color,
                self.font_size(),
                TextAlign::Start,
                self.family(),
                false,
                false,
            );
        }
    }

    fn drag_handler(&self, content_box: Option<Rect>, start: bool) -> Rc<dyn Fn(Point, Rect)> {
        let value = self.value.clone();
        let font_size = self.font_size();
        let family = self.style.typography.font_family.clone();
        let wrap = self.wrap;
        let cursor = self.cursor.min(self.value.len());
        let line = self.value[..cursor].rsplit('\n').next().unwrap_or("");
        let cursor_x =
            crate::text_metrics::advance_width_family(line, font_size, family.as_deref());
        let style = self.style.clone();
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let drag_anchor = self.drag_anchor.clone();
        let drag_focus = self.drag_focus.clone();
        let keyboard_selection = self.keyboard_selection.clone();
        Rc::new(move |point, rect: Rect| {
            let content = content_box.unwrap_or_else(|| style.content_rect(rect));
            let scroll_x = if wrap {
                0.0
            } else {
                (cursor_x - content.width + 4.0).max(0.0)
            };
            let cursor = cursor_at_point(
                &value,
                font_size,
                family.as_deref(),
                Point {
                    x: point.x - (content.x - rect.x),
                    y: point.y - (content.y - rect.y),
                },
                wrap,
                content.width,
                scroll_x,
            );
            if start {
                drag_anchor.set(cursor);
            } else if cursor == drag_focus.get() {
                return;
            }
            drag_focus.set(cursor);
            let next = TextSelection {
                anchor: drag_anchor.get(),
                focus: cursor,
            };
            keyboard_selection.set(next);
            creamui_reactive::batch(|| {
                on_cursor_change(cursor);
                on_selection_change(next);
            });
        })
    }
}

impl Widget for RawTextArea {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.paint_content(painter, rect, self.style.content_rect(rect));
    }

    fn paint_content(&self, painter: &mut dyn Painter, _rect: Rect, text_rect: Rect) {
        let (text, color) = if self.value.is_empty() && !self.placeholder.is_empty() {
            (&self.placeholder, self.placeholder_color)
        } else {
            (&self.value, self.text_color(painter))
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

    fn accepts_text_input(&self) -> bool {
        true
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        self.paint_focused_overlay_with_content(
            painter,
            rect,
            self.style.content_rect(rect),
            caret_visible,
        );
    }

    fn paint_focused_overlay_with_content(
        &self,
        painter: &mut dyn Painter,
        _rect: Rect,
        text_rect: Rect,
        caret_visible: bool,
    ) {
        if !caret_visible {
            return;
        }
        let cursor = self.cursor.min(self.value.len());
        // Caret proportions match `RawTextInput`'s: a slim bar sized and
        // vertically centered to the glyphs, not a full-height block.
        let (caret_x, caret_y, row_height) = if self.wrap {
            let glyphs = crate::text_metrics::layout_family(
                &self.value,
                self.font_size(),
                text_rect.width,
                self.family(),
            );
            let fallback = crate::text_metrics::row_height_family(self.font_size(), self.family());
            let (x, y, row_height) = crate::text_metrics::caret_xy(&glyphs, cursor, fallback);
            (text_rect.x + x, text_rect.y + y, row_height)
        } else {
            let before_cursor = &self.value[..cursor];
            let line = before_cursor.rsplit('\n').next().unwrap_or("");
            let width =
                crate::text_metrics::advance_width_family(line, self.font_size(), self.family());
            let lines = (before_cursor.matches('\n').count()) as f32;
            let line_height = self.font_size() * 1.4;
            let scroll_x = self.horizontal_scroll(text_rect.width);
            (
                text_rect.x + width - scroll_x,
                text_rect.y + lines * line_height,
                line_height,
            )
        };
        let caret_height = (self.font_size() * 1.2).min(row_height);
        painter.push_clip(text_rect);
        painter.fill_rect(
            Rect {
                x: caret_x,
                y: caret_y + (row_height - caret_height) / 2.0,
                width: 1.5,
                height: caret_height,
            },
            self.text_color(painter),
            0.0,
        );
        painter.pop_clip();
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let keyboard_value = self.keyboard_value.clone();
        let on_change = self.on_change.clone();
        let keyboard_cursor = self.keyboard_cursor.clone();
        let on_cursor_change = self.on_cursor_change.clone();
        let keyboard_selection = self.keyboard_selection.clone();
        let on_selection_change = self.on_selection_change.clone();
        let on_ctrl_o = self.on_ctrl_o.clone();
        let clipboard_enabled = self.clipboard_enabled;
        let paste_handler = self.paste_handler.clone();
        let lifetime = Rc::downgrade(&self.lifetime);
        let revision = self.keyboard_revision.clone();
        Some(Rc::new(move |input| {
            revision.set(revision.get().wrapping_add(1));
            let value = keyboard_value.borrow().clone();
            let cursor = keyboard_cursor.get();
            let selection = keyboard_selection.get();
            if clipboard_enabled && input.modifiers.ctrl {
                match input.key {
                    Key::Char('a') | Key::Char('A') => {
                        let all = TextSelection {
                            anchor: 0,
                            focus: value.len(),
                        };
                        keyboard_selection.set(all);
                        keyboard_cursor.set(value.len());
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
                        *keyboard_value.borrow_mut() = next.clone();
                        keyboard_cursor.set(next_cursor);
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
                        return;
                    }
                    Key::Char('v') | Key::Char('V') => {
                        if let Some(handler) = &paste_handler {
                            handler();
                        } else {
                            PasteRequest {
                                value: keyboard_value.clone(),
                                cursor: keyboard_cursor.clone(),
                                selection: keyboard_selection.clone(),
                                revision: revision.clone(),
                                lifetime: lifetime.clone(),
                                on_change: on_change.clone(),
                                on_cursor_change: on_cursor_change.clone(),
                                on_selection_change: on_selection_change.clone(),
                            }
                            .read();
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
            if input.modifiers.ctrl && matches!(input.key, Key::Char(_)) {
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
            keyboard_cursor.set(next_cursor);
            if edited {
                *keyboard_value.borrow_mut() = next.clone();
            }
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
        Some(self.drag_handler(None, true))
    }

    fn on_drag_start_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(Some(content), true))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(None, false))
    }

    fn on_drag_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(Some(content), false))
    }
}

fn cursor_at_point(
    value: &str,
    font_size: f32,
    family: Option<&str>,
    point: Point,
    wrap: bool,
    visible_width: f32,
    scroll_x: f32,
) -> usize {
    if wrap {
        return crate::text_metrics::byte_offset_at_point_family(
            value,
            font_size,
            visible_width,
            point.x,
            point.y,
            family,
        );
    }
    let line = (point.y / (font_size * 1.4)).floor().max(0.0) as usize;
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
            (point.x + scroll_x).max(0.0),
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
        let keyboard_value = Rc::new(RefCell::new(value.clone()));
        RawTextInput {
            style: creamui_core::Style::from(style)
                .color(text_color)
                .font_size(font_size),
            value,
            placeholder: String::new(),
            placeholder_color: text_color,
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
            on_key_press: Rc::new(|_| {}),
            clipboard_enabled: true,
            password: false,
            paste_handler: None,
            lifetime: Rc::new(()),
            keyboard_revision: Rc::new(Cell::new(0)),
            keyboard_value,
            keyboard_cursor: Rc::new(Cell::new(cursor)),
            keyboard_selection: Rc::new(Cell::new(TextSelection {
                anchor: cursor,
                focus: cursor,
            })),
            drag_anchor: Rc::new(Cell::new(cursor)),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>, color: Color) -> Self {
        self.placeholder = text.into();
        self.placeholder_color = color;
        self
    }

    fn font_size(&self) -> f32 {
        self.style.typography.font_size.unwrap_or(14.0)
    }

    fn family(&self) -> Option<&str> {
        self.style.typography.font_family.as_deref()
    }

    fn text_color(&self, painter: &dyn Painter) -> Color {
        self.style
            .typography
            .color
            .map(|color| color.resolve(&painter.color_scheme()))
            .unwrap_or(Color::rgb(0, 0, 0))
    }

    pub(crate) fn paste_handler(mut self, handler: impl Fn() + 'static) -> Self {
        self.paste_handler = Some(Rc::new(handler));
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
        self.keyboard_cursor.set(self.cursor);
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

    /// Observes key presses while this input is focused without replacing
    /// its built-in text editing, selection, or clipboard behavior.
    pub fn on_key_press(mut self, on_key_press: impl Fn(KeyInput) + 'static) -> Self {
        self.on_key_press = Rc::new(on_key_press);
        self
    }

    fn horizontal_scroll(&self, visible_width: f32) -> f32 {
        let (text_width, _) = crate::text_metrics::measure_family(
            &self.display_value(),
            self.font_size(),
            crate::text_metrics::unbounded_width(),
            self.family(),
            false,
        );
        (text_width - visible_width + 4.0).max(0.0)
    }

    fn drag_handler(&self, content_box: Option<Rect>, start: bool) -> Rc<dyn Fn(Point, Rect)> {
        let value = self.value.clone();
        let displayed = self.display_value();
        let password = self.password;
        let font_size = self.font_size();
        let family = self.style.typography.font_family.clone();
        let style = self.style.clone();
        let on_cursor_change = self.on_cursor_change.clone();
        let on_selection_change = self.on_selection_change.clone();
        let selection = self.keyboard_selection.clone();
        let anchor = self.drag_anchor.clone();
        Rc::new(move |point, rect| {
            let content = content_box.unwrap_or_else(|| style.content_rect(rect));
            let width =
                crate::text_metrics::advance_width_family(&displayed, font_size, family.as_deref());
            let scroll = (width - content.width + 4.0).max(0.0);
            let cursor = crate::text_metrics::byte_offset_at_x_family(
                &displayed,
                font_size,
                (point.x - (content.x - rect.x) + scroll).max(0.0),
                family.as_deref(),
            );
            let cursor = if password {
                value
                    .char_indices()
                    .nth(displayed[..cursor].chars().count())
                    .map(|(offset, _)| offset)
                    .unwrap_or(value.len())
            } else {
                cursor
            };
            if start {
                anchor.set(cursor);
            }
            let next = TextSelection {
                anchor: anchor.get(),
                focus: cursor,
            };
            selection.set(next);
            creamui_reactive::batch(|| {
                on_cursor_change(cursor);
                on_selection_change(next);
            });
        })
    }

    fn display_value(&self) -> String {
        if self.password {
            "•".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
    }
    fn display_offset(&self, byte: usize) -> usize {
        if self.password {
            self.value[..byte.min(self.value.len())].chars().count() * '•'.len_utf8()
        } else {
            byte.min(self.value.len())
        }
    }
}

impl Widget for RawTextInput {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.paint_content(painter, rect, self.style.content_rect(rect));
    }

    fn paint_content(&self, painter: &mut dyn Painter, _rect: Rect, text_rect: Rect) {
        // Unwrapped: a bounded width here would word-wrap onto a second row.
        let displayed = self.display_value();
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
                    self.font_size(),
                    TextAlign::Start,
                    self.family(),
                    false,
                    false,
                );
            }
        } else {
            let selected = self.selection.range();
            let selected = self.display_offset(selected.start)..self.display_offset(selected.end);
            if !selected.is_empty() {
                if let Some(background) = self.selection_background {
                    let before = crate::text_metrics::advance_width_family(
                        &displayed[..selected.start],
                        self.font_size(),
                        self.family(),
                    );
                    let (width, _) = crate::text_metrics::measure_family(
                        &displayed[selected.clone()],
                        self.font_size(),
                        crate::text_metrics::unbounded_width(),
                        self.family(),
                        false,
                    );
                    painter.fill_rect(
                        Rect {
                            x: unbounded.x + before,
                            y: text_rect.y + (text_rect.height - self.font_size() * 1.4) / 2.0,
                            width,
                            height: self.font_size() * 1.4,
                        },
                        background,
                        2.0,
                    );
                }
            }
            painter.fill_text_selected_font(
                unbounded,
                &displayed,
                self.text_color(painter),
                self.selection_text_color
                    .unwrap_or(self.text_color(painter)),
                selected,
                self.font_size(),
                TextAlign::Start,
                self.family(),
            );
        }
        painter.pop_clip();
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        self.paint_focused_overlay_with_content(
            painter,
            rect,
            self.style.content_rect(rect),
            caret_visible,
        );
    }

    fn paint_focused_overlay_with_content(
        &self,
        painter: &mut dyn Painter,
        _rect: Rect,
        text_rect: Rect,
        caret_visible: bool,
    ) {
        if !caret_visible {
            return;
        }
        let visible_width = text_rect.width;
        let cursor = self.cursor.min(self.value.len());
        let displayed = self.display_value();
        let text_width = crate::text_metrics::advance_width_family(
            &displayed[..self.display_offset(cursor)],
            self.font_size(),
            self.family(),
        );
        let caret_x = text_rect.x + text_width - self.horizontal_scroll(visible_width);
        let caret_height = (self.font_size() * 1.2).min(text_rect.height);
        let caret_rect = Rect {
            x: caret_x,
            y: text_rect.y + (text_rect.height - caret_height) / 2.0,
            width: 1.5,
            height: caret_height,
        };
        painter.push_clip(text_rect);
        painter.fill_rect(caret_rect, self.text_color(painter), 0.0);
        painter.pop_clip();
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let keyboard_value = self.keyboard_value.clone();
        let on_change = self.on_change.clone();
        let keyboard_cursor = self.keyboard_cursor.clone();
        let on_cursor_change = self.on_cursor_change.clone();
        let selection = self.keyboard_selection.clone();
        let on_selection_change = self.on_selection_change.clone();
        let clipboard_enabled = self.clipboard_enabled;
        let on_submit = self.on_submit.clone();
        let password = self.password;
        let on_key_press = self.on_key_press.clone();
        let paste_handler = self.paste_handler.clone();
        let lifetime = Rc::downgrade(&self.lifetime);
        let revision = self.keyboard_revision.clone();
        Some(Rc::new(move |input: KeyInput| {
            revision.set(revision.get().wrapping_add(1));
            on_key_press(input);
            if matches!(input.key, Key::Enter) {
                on_submit();
                return;
            }
            let value = keyboard_value.borrow().clone();
            let cursor = keyboard_cursor.get();
            let selected = selection.get();
            if clipboard_enabled && input.modifiers.ctrl {
                match input.key {
                    Key::Char('a') | Key::Char('A') => {
                        let all = TextSelection {
                            anchor: 0,
                            focus: value.len(),
                        };
                        selection.set(all);
                        keyboard_cursor.set(value.len());
                        creamui_reactive::batch(|| {
                            on_cursor_change(value.len());
                            on_selection_change(all);
                        });
                        return;
                    }
                    Key::Char('c') | Key::Char('C') if !selected.is_empty() && !password => {
                        clipboard_write(value[selected.range()].to_owned());
                        return;
                    }
                    Key::Char('x') | Key::Char('X') if !selected.is_empty() && !password => {
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
                        *keyboard_value.borrow_mut() = next.clone();
                        keyboard_cursor.set(at);
                        creamui_reactive::batch(|| {
                            on_change(next);
                            on_cursor_change(at);
                            on_selection_change(collapsed);
                        });
                        return;
                    }
                    Key::Char('v') | Key::Char('V') => {
                        if let Some(handler) = &paste_handler {
                            handler();
                        } else {
                            PasteRequest {
                                value: keyboard_value.clone(),
                                cursor: keyboard_cursor.clone(),
                                selection: selection.clone(),
                                revision: revision.clone(),
                                lifetime: lifetime.clone(),
                                on_change: on_change.clone(),
                                on_cursor_change: on_cursor_change.clone(),
                                on_selection_change: on_selection_change.clone(),
                            }
                            .read();
                        }
                        return;
                    }
                    _ => {}
                }
            }
            if input.modifiers.ctrl && matches!(input.key, Key::Char(_)) {
                return;
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
            keyboard_cursor.set(at);
            if changed {
                *keyboard_value.borrow_mut() = next.clone();
            }
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
        Some(self.drag_handler(None, true))
    }

    fn on_drag_start_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(Some(content), true))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(None, false))
    }

    fn on_drag_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(self.drag_handler(Some(content), false))
    }
}

struct PasteRequest {
    value: Rc<RefCell<String>>,
    cursor: Rc<Cell<usize>>,
    selection: Rc<Cell<TextSelection>>,
    revision: Rc<Cell<u64>>,
    lifetime: std::rc::Weak<()>,
    on_change: Rc<dyn Fn(String)>,
    on_cursor_change: Rc<dyn Fn(usize)>,
    on_selection_change: Rc<dyn Fn(TextSelection)>,
}

impl PasteRequest {
    fn read(self) {
        crate::clipboard::read(self.completion());
    }

    fn completion(self) -> impl FnOnce(String) {
        let stamp = self.revision.get();
        let selected = self.selection.get();
        let position = self.cursor.get();
        move |paste| {
            if self.lifetime.upgrade().is_none()
                || self.revision.get() != stamp
                || self.selection.get() != selected
                || self.cursor.get() != position
            {
                return;
            }
            let mut next = self.value.borrow().clone();
            let range = selected.range();
            let range = if range.is_empty() {
                position..position
            } else {
                range
            };
            if next.get(range.clone()).is_none() {
                return;
            }
            let position = range.start + paste.len();
            next.replace_range(range, &paste);
            let collapsed = TextSelection {
                anchor: position,
                focus: position,
            };
            *self.value.borrow_mut() = next.clone();
            self.cursor.set(position);
            self.selection.set(collapsed);
            creamui_reactive::batch(|| {
                (self.on_change)(next);
                (self.on_cursor_change)(position);
                (self.on_selection_change)(collapsed);
            });
        }
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    fn request(input: &RawTextInput) -> PasteRequest {
        PasteRequest {
            value: input.keyboard_value.clone(),
            cursor: input.keyboard_cursor.clone(),
            selection: input.keyboard_selection.clone(),
            revision: input.keyboard_revision.clone(),
            lifetime: Rc::downgrade(&input.lifetime),
            on_change: input.on_change.clone(),
            on_cursor_change: input.on_cursor_change.clone(),
            on_selection_change: input.on_selection_change.clone(),
        }
    }

    #[test]
    fn raw_paste_discards_results_after_edits_pointer_selection_and_disposal() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let recorded = changes.clone();
        let input = RawTextInput::new(
            Style::default(),
            "abc",
            14.0,
            Color::rgb(0, 0, 0),
            move |text| recorded.borrow_mut().push(text),
        );
        let stale = request(&input).completion();
        let key = input.on_key().unwrap();
        key(KeyInput {
            key: Key::Char('x'),
            modifiers: Default::default(),
        });
        key(KeyInput {
            key: Key::Backspace,
            modifiers: Default::default(),
        });
        stale("stale".into());
        assert_eq!(&*changes.borrow(), &["abcx", "abc"]);
        let stale = request(&input).completion();
        input.keyboard_selection.set(TextSelection {
            anchor: 0,
            focus: 1,
        });
        stale("stale".into());
        assert_eq!(changes.borrow().len(), 2);
        let stale = request(&input).completion();
        drop(input);
        stale("stale".into());
        assert_eq!(changes.borrow().len(), 2);
    }

    #[test]
    fn disabled_clipboard_shortcuts_do_not_insert_command_characters() {
        let input = RawTextInput::new(Style::default(), "abc", 14.0, Color::rgb(0, 0, 0), |_| {})
            .clipboard_enabled(false);
        let area = RawTextArea::new(Style::default(), "abc", 14.0, Color::rgb(0, 0, 0), |_| {})
            .clipboard_enabled(false);
        for key in [input.on_key().unwrap(), area.on_key().unwrap()] {
            for ch in ['a', 'c', 'x', 'v'] {
                key(KeyInput {
                    key: Key::Char(ch),
                    modifiers: creamui_core::Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                });
            }
            key(KeyInput {
                key: Key::Char('x'),
                modifiers: Default::default(),
            });
        }
        assert_eq!(&*input.keyboard_value.borrow(), "abcx");
        assert_eq!(&*area.keyboard_value.borrow(), "abcx");
    }

    #[test]
    fn text_area_keeps_queued_keyboard_edits_before_a_rebuild() {
        let value = Rc::new(RefCell::new(String::new()));
        let recorded = value.clone();
        let area = RawTextArea::new(
            Style::default(),
            "abc",
            14.0,
            Color::rgb(0, 0, 0),
            move |text| *recorded.borrow_mut() = text,
        );
        let key = area.on_key().unwrap();
        for ch in ['x', 'y'] {
            key(KeyInput {
                key: Key::Char(ch),
                modifiers: Default::default(),
            });
        }
        assert_eq!(&*value.borrow(), "abcxy");
    }
}
