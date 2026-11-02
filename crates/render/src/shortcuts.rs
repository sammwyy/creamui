use creamui_core::{Key, KeyInput, Modifiers, Point, Rect};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Shortcut {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn primary(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers {
                ctrl: !cfg!(target_os = "macos"),
                logo: cfg!(target_os = "macos"),
                ..Default::default()
            },
        }
    }

    fn matches(&self, input: KeyInput) -> bool {
        let keys_match = match (self.key, input.key) {
            (Key::Char(a), Key::Char(b)) => a.eq_ignore_ascii_case(&b),
            (a, b) => a == b,
        };
        keys_match && self.modifiers == input.modifiers
    }
}

#[derive(Clone)]
pub enum ShortcutScope {
    Window,
    WithoutTextInput,
    Region(Rc<Cell<Rect>>),
}

impl ShortcutScope {
    fn priority(&self) -> u8 {
        match self {
            Self::Window => 0,
            Self::WithoutTextInput => 1,
            Self::Region(_) => 2,
        }
    }

    fn active(&self, text_input: bool, target: Point) -> bool {
        match self {
            Self::Window => true,
            Self::WithoutTextInput => !text_input,
            Self::Region(bounds) => !text_input && bounds.get().contains(target),
        }
    }
}

pub(crate) struct ShortcutBinding {
    pub id: u64,
    pub shortcut: Shortcut,
    pub scope: ShortcutScope,
    pub handler: Rc<dyn Fn()>,
}

pub(crate) struct Shortcuts {
    next_id: u64,
    bindings: Vec<ShortcutBinding>,
}

impl Shortcuts {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            bindings: Vec::new(),
        }
    }

    pub fn insert(
        &mut self,
        shortcut: Shortcut,
        scope: ShortcutScope,
        handler: Rc<dyn Fn()>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.bindings.push(ShortcutBinding {
            id,
            shortcut,
            scope,
            handler,
        });
        id
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|binding| binding.id != id);
        self.bindings.len() != before
    }

    pub fn resolve(
        &self,
        input: KeyInput,
        text_input: bool,
        target: Point,
    ) -> Option<Rc<dyn Fn()>> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.shortcut.matches(input) && binding.scope.active(text_input, target)
            })
            .max_by_key(|binding| (binding.scope.priority(), binding.id))
            .map(|binding| binding.handler.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn region_shortcut_wins_and_text_input_remains_undisturbed() {
        let mut shortcuts = Shortcuts::new();
        let called = Rc::new(RefCell::new(Vec::new()));
        let chord = Shortcut::new(
            Key::Char('s'),
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        let window = {
            let called = called.clone();
            shortcuts.insert(
                chord,
                ShortcutScope::Window,
                Rc::new(move || called.borrow_mut().push("window")),
            )
        };
        let local = {
            let called = called.clone();
            shortcuts.insert(
                chord,
                ShortcutScope::Region(Rc::new(Cell::new(Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 100.0,
                    height: 100.0,
                }))),
                Rc::new(move || called.borrow_mut().push("region")),
            )
        };
        let input = KeyInput {
            key: Key::Char('S'),
            modifiers: chord.modifiers,
        };
        shortcuts
            .resolve(input, false, Point { x: 15.0, y: 20.0 })
            .unwrap()();
        shortcuts
            .resolve(input, true, Point { x: 15.0, y: 20.0 })
            .unwrap()();
        shortcuts
            .resolve(input, false, Point { x: 0.0, y: 0.0 })
            .unwrap()();
        assert_eq!(*called.borrow(), ["region", "window", "window"]);
        assert!(shortcuts.remove(local));
        assert!(!shortcuts.remove(local));
        assert!(shortcuts.remove(window));
        assert!(shortcuts.resolve(input, false, Point::default()).is_none());
    }

    #[test]
    fn text_input_blocks_unmodified_shortcuts_in_non_text_scope() {
        let mut shortcuts = Shortcuts::new();
        shortcuts.insert(
            Shortcut::new(Key::Char(' '), Modifiers::default()),
            ShortcutScope::WithoutTextInput,
            Rc::new(|| {}),
        );
        let input = KeyInput {
            key: Key::Char(' '),
            modifiers: Modifiers::default(),
        };
        assert!(shortcuts.resolve(input, true, Point::default()).is_none());
        assert!(shortcuts.resolve(input, false, Point::default()).is_some());
    }
}
