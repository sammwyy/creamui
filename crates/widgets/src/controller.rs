//! [`TextController`]: a shareable, persistent bundle of a text editing
//! widget's value, cursor, and selection — the one object an app creates
//! once (the same discipline a `Signal` already requires: create it in
//! `main`, then clone the handle into whatever needs it) and hands to
//! [`crate::themed::TextInput`]/[`crate::themed::TextArea`], instead of
//! wiring up `value`/`on_change`/`cursor`/`on_cursor_change`/`selection`/
//! `on_selection_change` by hand every time.
//!
//! This can't be a zero-setup "hook" the way React's `useState` is: nothing
//! in this crate rebuilds widgets in place across renders (see
//! `creamui_render`'s doc comment on `build_ui`), so there's no per-call-site
//! slot to remember state in without the app holding a handle itself. A
//! `TextController` is that handle — cheap to create, `Clone` (an `Rc`
//! underneath, exactly like `Signal`), and if a widget isn't given one it
//! creates its own default, unrestricted one internally, which is as close
//! to "free" as an immediate-mode rebuild model can get.

use crate::raw::{DateTime, TextSelection};
use creamui_reactive::Signal;
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

type ChangeGuard = dyn Fn(&str, &str) -> Option<String>;

/// Shared date/time value for a [`crate::DateTimePicker`]. Like the other
/// controllers it is application-owned, so picker state survives reactive
/// widget-tree rebuilds without hidden global state.
#[derive(Clone)]
pub struct DateTimeController {
    value: Signal<DateTime>,
    open: Signal<bool>,
}

impl DateTimeController {
    pub fn new(initial: DateTime) -> Self {
        Self {
            value: Signal::new(initial.normalized()),
            open: Signal::new(false),
        }
    }

    pub fn value(&self) -> DateTime {
        self.value.get()
    }
    pub fn peek(&self) -> DateTime {
        self.value.peek()
    }
    pub fn set(&self, value: DateTime) {
        self.value.set(value.normalized())
    }
    pub fn is_open(&self) -> bool {
        self.open.get()
    }
    pub fn set_open(&self, open: bool) {
        self.open.set(open)
    }
    pub fn toggle(&self) {
        self.open.update(|open| *open = !*open)
    }
}

impl Default for DateTimeController {
    fn default() -> Self {
        Self::new(DateTime::default())
    }
}

/// Shared visibility state for a [`crate::ColorPicker`]'s portal popup.
/// The selected [`creamui_theme::Color`] deliberately remains caller-owned,
/// matching the rest of CreamUI's controlled-value APIs.
#[derive(Clone)]
pub struct ColorPickerController {
    open: Signal<bool>,
}

impl ColorPickerController {
    pub fn new() -> Self {
        Self {
            open: Signal::new(false),
        }
    }
    pub fn is_open(&self) -> bool {
        self.open.get()
    }
    pub fn set_open(&self, open: bool) {
        self.open.set(open)
    }
    pub fn toggle(&self) {
        self.open.update(|open| *open = !*open)
    }
}

impl Default for ColorPickerController {
    fn default() -> Self {
        Self::new()
    }
}

/// A controller for a text editing widget's value, cursor, and selection.
///
/// Reading [`TextController::value`] (or `.cursor()`/`.selection()`) inside
/// a reactive effect — which is what [`crate::themed::TextInput`] and
/// [`crate::themed::TextArea`] do internally when bound to one — is how you
/// "hook into changes": the same subscribe-on-read mechanism as
/// [`creamui_reactive::Signal`], since a controller is just three `Signal`s
/// under one shared handle. To additionally *veto* or rewrite a proposed
/// change (e.g. enforce a max length, strip disallowed characters), install
/// a guard with [`TextController::on_change`].
#[derive(Clone)]
pub struct TextController {
    value: Signal<String>,
    cursor: Signal<usize>,
    selection: Signal<TextSelection>,
    guard: Rc<std::cell::RefCell<Option<Box<ChangeGuard>>>>,
}

impl TextController {
    /// A new, unrestricted controller seeded with `initial`, cursor placed
    /// at its end.
    pub fn new(initial: impl Into<String>) -> Self {
        let value = initial.into();
        let end = value.len();
        TextController {
            value: Signal::new(value),
            cursor: Signal::new(end),
            selection: Signal::new(TextSelection {
                anchor: end,
                focus: end,
            }),
            guard: Rc::new(std::cell::RefCell::new(None)),
        }
    }

    /// The current value, subscribing the running reactive effect (if any)
    /// to future changes — same semantics as [`Signal::get`].
    pub fn value(&self) -> String {
        self.value.get()
    }

    /// Reads the current value without subscribing.
    pub fn peek(&self) -> String {
        self.value.peek()
    }

    pub fn cursor(&self) -> usize {
        self.cursor.get()
    }

    pub fn selection(&self) -> TextSelection {
        self.selection.get()
    }

    /// Called on every pointer-move while dragging a text selection, so a
    /// clamped position repeated across consecutive moves (e.g. dragging
    /// past either end of the text) must not re-notify subscribers each
    /// time — see [`Signal::set_if_changed`].
    pub fn set_cursor(&self, cursor: usize) {
        self.cursor
            .set_if_changed(cursor.min(self.value.peek().len()));
    }

    /// Like [`TextController::set_cursor`], called just as often while
    /// dragging a selection.
    pub fn set_selection(&self, selection: TextSelection) {
        let len = self.value.peek().len();
        self.selection.set_if_changed(TextSelection {
            anchor: selection.anchor.min(len),
            focus: selection.focus.min(len),
        });
    }

    /// Proposes `next` as the new value. If a guard is installed (see
    /// [`TextController::on_change`]), it decides what actually happens:
    /// returning `None` rejects the change outright (`value()` keeps its
    /// current contents, as if the keystroke never happened); returning
    /// `Some(text)` accepts `text` — usually `next` unchanged, but the guard
    /// may rewrite it (e.g. truncate, strip characters). With no guard
    /// installed, `next` is always accepted as-is.
    pub fn set_value(&self, next: impl Into<String>) {
        let next = next.into();
        let current = self.value.peek();
        let accepted = match self.guard.borrow().as_ref() {
            Some(guard) => guard(&current, &next),
            None => Some(next),
        };
        let Some(text) = accepted else { return };
        let len = text.len();
        self.value.set(text);
        if self.cursor.peek() > len {
            self.cursor.set(len);
        }
        let selection = self.selection.peek();
        if selection.anchor > len || selection.focus > len {
            self.selection.set(TextSelection {
                anchor: selection.anchor.min(len),
                focus: selection.focus.min(len),
            });
        }
    }

    /// Installs a hook run before every [`TextController::set_value`] call
    /// (including edits typed into a bound `TextInput`/`TextArea`): given
    /// `(current, proposed)`, return `Some(text)` to accept the change
    /// (optionally rewriting it), or `None` to reject it and keep `current`.
    /// Replaces any previously installed guard.
    pub fn on_change(&self, guard: impl Fn(&str, &str) -> Option<String> + 'static) {
        *self.guard.borrow_mut() = Some(Box::new(guard));
    }

    /// Removes any guard installed via [`TextController::on_change`],
    /// returning to unrestricted edits.
    pub fn clear_guard(&self) {
        *self.guard.borrow_mut() = None;
    }
}

impl Default for TextController {
    /// An empty, unrestricted controller — what a `TextInput`/`TextArea`
    /// creates internally when constructed without one.
    fn default() -> Self {
        TextController::new(String::new())
    }
}

/// Shared selected-index state for one tab bar and the content it controls.
#[derive(Clone)]
pub struct TabController {
    selected: Signal<usize>,
}

/// Shared state for a [`crate::Select`] or [`crate::ComboBox`].
///
/// Like the other controllers, this is deliberately owned by the application:
/// it keeps a select's chosen item and open/closed state stable while the
/// immediate-mode widget tree is rebuilt.
#[derive(Clone)]
pub struct SelectController {
    selected: Signal<usize>,
    open: Signal<bool>,
}

impl SelectController {
    pub fn new(selected: usize) -> Self {
        Self {
            selected: Signal::new(selected),
            open: Signal::new(false),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected.get()
    }

    pub fn peek_selected(&self) -> usize {
        self.selected.peek()
    }

    pub fn select(&self, index: usize) {
        creamui_reactive::batch(|| {
            self.selected.set(index);
            self.open.set(false);
        });
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn peek_open(&self) -> bool {
        self.open.peek()
    }

    pub fn set_open(&self, open: bool) {
        self.open.set(open);
    }

    pub fn toggle(&self) {
        self.open.update(|open| *open = !*open);
    }
}

impl Default for SelectController {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TabController {
    pub fn new(selected: usize) -> Self {
        Self {
            selected: Signal::new(selected),
        }
    }

    /// Reads the selected index and subscribes the current reactive render.
    pub fn selected(&self) -> usize {
        self.selected.get()
    }

    /// Reads the selected index without subscribing.
    pub fn peek(&self) -> usize {
        self.selected.peek()
    }

    pub fn select(&self, index: usize) {
        self.selected.set(index);
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected() == index
    }
}

impl Default for TabController {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Shared, clamped offset state for a controlled scroll view.
#[derive(Clone)]
pub struct ScrollController {
    offset: Rc<Cell<f32>>,
    /// The content's resolved overflow (content height minus viewport
    /// height), refreshed every paint via [`creamui_core::Widget::on_content_overflow`].
    /// Lets a [`crate::raw::RawScrollbar`] sharing this controller size and
    /// position its thumb without its own access to the scroll view's
    /// content layout.
    max_offset: Rc<Cell<f32>>,
}

impl ScrollController {
    pub fn new(offset: f32) -> Self {
        Self {
            offset: Rc::new(Cell::new(offset.max(0.0))),
            max_offset: Rc::new(Cell::new(f32::INFINITY)),
        }
    }

    pub fn offset(&self) -> f32 {
        self.offset.get()
    }

    pub fn peek(&self) -> f32 {
        self.offset.get()
    }

    /// The content's current scrollable overflow, i.e. the maximum value
    /// [`ScrollController::offset`] can take. `f32::INFINITY` until the
    /// scroll view this controller is attached to has painted at least
    /// once (so an unbounded [`ScrollController::set`] before then doesn't
    /// clamp away a caller's intended initial offset).
    pub fn max_offset(&self) -> f32 {
        self.max_offset.get()
    }

    /// Called by the scroll view's [`creamui_core::Widget::on_content_overflow`]
    /// hook every paint. Not meant to be called directly by applications.
    pub fn report_max_offset(&self, max_offset: f32) {
        self.max_offset.set(max_offset.max(0.0));
    }

    pub fn set(&self, offset: f32) {
        self.offset.set(offset.clamp(0.0, self.max_offset.get()));
    }

    pub fn scroll_by(&self, delta: f32, max_offset: f32) {
        let current = self.offset.get();
        let next = (current + delta).clamp(0.0, max_offset.max(0.0));
        if (next - current).abs() > f32::EPSILON {
            self.offset.set(next);
        }
    }
}

impl Default for ScrollController {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// A [`ScrollController`] that eases toward the bottom while pinned, and
/// unpins on manual scroll away from it. Call `tick()` once per rebuild.
#[derive(Clone)]
pub struct AutoScrollController {
    scroll: ScrollController,
    pinned: Rc<Cell<bool>>,
    last_written: Rc<Cell<f32>>,
    last_tick: Rc<Cell<Option<Instant>>>,
    pin_threshold: f32,
    ease_rate: f32,
}

impl AutoScrollController {
    pub fn new() -> Self {
        Self {
            scroll: ScrollController::default(),
            pinned: Rc::new(Cell::new(true)),
            last_written: Rc::new(Cell::new(0.0)),
            last_tick: Rc::new(Cell::new(None)),
            pin_threshold: 24.0,
            ease_rate: 12.0,
        }
    }

    /// How close to the true bottom (in pixels) counts as "at the bottom"
    /// for re-pinning after a manual scroll. Default `24.0`.
    pub fn pin_threshold(mut self, px: f32) -> Self {
        self.pin_threshold = px.max(0.0);
        self
    }

    /// Higher tracks the target offset faster; lower feels slower/softer.
    /// Default `12.0`.
    pub fn ease_rate(mut self, rate: f32) -> Self {
        self.ease_rate = rate.max(0.0);
        self
    }

    /// The underlying controller to hand to `ScrollView::controlled`.
    pub fn scroll(&self) -> ScrollController {
        self.scroll.clone()
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned.get()
    }

    /// Advances the follow animation. Call once per rebuild.
    pub fn tick(&self) {
        let current = self.scroll.offset();
        let max = self.scroll.max_offset();
        if !max.is_finite() {
            return;
        }
        if (current - self.last_written.get()).abs() > 0.5 {
            self.pinned.set(current >= max - self.pin_threshold);
        }

        let now = Instant::now();
        let dt = match self.last_tick.replace(Some(now)) {
            Some(previous) => (now - previous).as_secs_f32(),
            None => 0.0,
        };

        if self.pinned.get() {
            let next = if (max - current).abs() < 0.5 {
                max
            } else {
                current + (max - current) * (1.0 - (-dt * self.ease_rate).exp())
            };
            self.scroll.set(next);
            self.last_written.set(next);
        } else {
            self.last_written.set(current);
        }
    }

    /// Pins and jumps to the bottom immediately, no ease.
    pub fn snap_to_bottom(&self) {
        self.pinned.set(true);
        let max = self.scroll.max_offset();
        if max.is_finite() {
            self.scroll.set(max);
            self.last_written.set(max);
        }
        self.last_tick.set(None);
    }

    /// Jumps to 0 and re-pins, for content whose `max_offset` isn't measured yet.
    pub fn reset(&self) {
        self.pinned.set(true);
        self.scroll.set(0.0);
        self.last_written.set(0.0);
        self.last_tick.set(None);
    }
}

impl Default for AutoScrollController {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared expanded/selected state for a [`crate::themed::TreeView`], keyed
/// by each node's own `id`. Backed by `Signal`s (unlike [`ScrollController`])
/// because a toggled node changes which rows exist at all — `TreeView` reads
/// this controller once, in its own constructor rather than lazily in
/// `Widget::children`, so that read happens on the same call stack as the
/// application's `build_ui` and actually subscribes to future changes.
#[derive(Clone)]
pub struct TreeController {
    expanded: Signal<HashSet<u64>>,
    selected: Signal<Option<u64>>,
}

impl TreeController {
    pub fn new() -> Self {
        Self {
            expanded: Signal::new(HashSet::new()),
            selected: Signal::new(None),
        }
    }

    pub fn is_expanded(&self, id: u64) -> bool {
        self.expanded.get().contains(&id)
    }

    pub fn set_expanded(&self, id: u64, expanded: bool) {
        self.expanded.update(|set| {
            if expanded {
                set.insert(id);
            } else {
                set.remove(&id);
            }
        });
    }

    pub fn toggle(&self, id: u64) {
        self.set_expanded(id, !self.expanded.peek().contains(&id));
    }

    pub fn selected(&self) -> Option<u64> {
        self.selected.get()
    }

    pub fn peek_selected(&self) -> Option<u64> {
        self.selected.peek()
    }

    pub fn select(&self, id: u64) {
        self.selected.set(Some(id));
    }
}

impl Default for TreeController {
    fn default() -> Self {
        Self::new()
    }
}

/// Drill-down navigation state for [`crate::themed::nested_sidebar`]: the
/// stack of category ids entered so far, root when empty.
#[derive(Clone)]
pub struct SidebarNavController<T: Clone + PartialEq + 'static> {
    path: Signal<Vec<T>>,
}

impl<T: Clone + PartialEq + 'static> SidebarNavController<T> {
    pub fn new() -> Self {
        Self {
            path: Signal::new(Vec::new()),
        }
    }

    pub fn path(&self) -> Vec<T> {
        self.path.get()
    }

    pub fn is_root(&self) -> bool {
        self.path.get().is_empty()
    }

    pub fn enter(&self, id: T) {
        self.path.update(|path| path.push(id));
    }

    pub fn back(&self) {
        self.path.update(|path| {
            path.pop();
        });
    }

    pub fn reset(&self) {
        self.path.set(Vec::new());
    }
}

impl<T: Clone + PartialEq + 'static> Default for SidebarNavController<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_value_updates_cursor_and_selection_when_they_fall_out_of_range() {
        let controller = TextController::new("hello");
        controller.set_cursor(5);
        controller.set_selection(TextSelection {
            anchor: 2,
            focus: 5,
        });
        controller.set_value("hi");
        assert_eq!(controller.value(), "hi");
        assert_eq!(controller.cursor(), 2);
        assert_eq!(
            controller.selection(),
            TextSelection {
                anchor: 2,
                focus: 2
            }
        );
    }

    #[test]
    fn on_change_guard_can_reject_a_change() {
        let controller = TextController::new("ok");
        controller.on_change(|_current, proposed| {
            if proposed.len() > 5 {
                None
            } else {
                Some(proposed.to_owned())
            }
        });
        controller.set_value("still ok");
        assert_eq!(
            controller.value(),
            "ok",
            "change longer than 5 chars should be rejected"
        );
        controller.set_value("short");
        assert_eq!(controller.value(), "short");
    }

    #[test]
    fn on_change_guard_can_rewrite_a_change() {
        let controller = TextController::new("");
        controller.on_change(|_current, proposed| Some(proposed.to_uppercase()));
        controller.set_value("shout");
        assert_eq!(controller.value(), "SHOUT");
    }

    #[test]
    fn default_controller_is_empty_and_unrestricted() {
        let controller = TextController::default();
        assert_eq!(controller.value(), "");
        controller.set_value("anything");
        assert_eq!(controller.value(), "anything");
    }

    #[test]
    fn tab_controller_tracks_one_shared_selection() {
        let tabs = TabController::new(1);
        assert!(tabs.is_selected(1));
        tabs.select(2);
        assert_eq!(tabs.selected(), 2);
    }
}
