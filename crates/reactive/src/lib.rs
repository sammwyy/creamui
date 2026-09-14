//! Minimal single-threaded reactive primitives: [`Signal`] and [`create_effect`].
//!
//! This is the reactivity layer CreamUI components are built on. It is
//! intentionally small (no schedulers, no async) since it only needs to
//! drive synchronous UI re-renders on the main thread.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

thread_local! {
    static EFFECT_STACK: RefCell<Vec<Rc<EffectState>>> = const { RefCell::new(Vec::new()) };
    static BATCH_DEPTH: Cell<usize> = const { Cell::new(0) };
    static PENDING_EFFECTS: RefCell<Vec<Rc<EffectState>>> = const { RefCell::new(Vec::new()) };
    static CONTEXT_STACK: RefCell<Vec<RefCell<HashMap<TypeId, Rc<dyn Any>>>>> =
        const { RefCell::new(Vec::new()) };
}

/// Opens a context scope for the duration of `f`. [`provide_context`] and
/// [`use_context`] only work while a scope is active; nested scopes see
/// their own values first, then fall back to the enclosing scope's. Pops
/// via a drop guard, so a panic inside `f` still leaves the stack balanced.
pub fn with_context_scope<R>(f: impl FnOnce() -> R) -> R {
    CONTEXT_STACK.with(|stack| stack.borrow_mut().push(RefCell::new(HashMap::new())));
    struct PopGuard;
    impl Drop for PopGuard {
        fn drop(&mut self) {
            CONTEXT_STACK.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    let _guard = PopGuard;
    f()
}

/// Whether a [`with_context_scope`] is currently active on this thread.
pub fn in_context_scope() -> bool {
    CONTEXT_STACK.with(|stack| !stack.borrow().is_empty())
}

/// Panics if no [`with_context_scope`] is active. For hooks that don't fetch
/// a typed context value but still require one to be open.
pub fn require_context_scope(hook_name: &str) {
    if !in_context_scope() {
        panic!(
            "{hook_name}() called outside a context scope — hooks only work while a window is \
             building its widget tree (inside `run`/`AppBuilder`'s `build_ui`)."
        );
    }
}

/// Makes `value` available to [`use_context`]/[`try_use_context`] for the
/// rest of the current [`with_context_scope`]. Panics outside a scope.
pub fn provide_context<T: 'static>(value: T) {
    CONTEXT_STACK.with(|stack| {
        let stack = stack.borrow();
        let frame = stack
            .last()
            .expect("provide_context() called outside a context scope — see with_context_scope");
        frame.borrow_mut().insert(TypeId::of::<T>(), Rc::new(value));
    });
}

/// Reads the nearest [`provide_context`]d value of type `T`, searching from
/// the innermost active scope outward. `None` if nothing of that type was
/// provided (including when called outside any scope).
pub fn try_use_context<T: Clone + 'static>() -> Option<T> {
    CONTEXT_STACK.with(|stack| {
        stack.borrow().iter().rev().find_map(|frame| {
            frame.borrow().get(&TypeId::of::<T>()).map(|value| {
                value
                    .downcast_ref::<T>()
                    .expect("creamui-reactive: context TypeId collision")
                    .clone()
            })
        })
    })
}

/// Like [`try_use_context`], but panics instead of returning `None`.
pub fn use_context<T: Clone + 'static>() -> T {
    try_use_context().unwrap_or_else(|| {
        panic!(
            "use_context::<{}>() found nothing provided — either called outside a context scope, \
             or no matching provide_context::<{}>() ran first.",
            std::any::type_name::<T>(),
            std::any::type_name::<T>()
        )
    })
}

struct EffectState {
    run: RefCell<Box<dyn FnMut()>>,
    /// Unsubscribes collected while the previous execution read signals.
    /// They are run before the next execution so a conditional view only
    /// remains subscribed to the branch it currently renders.
    dependencies: RefCell<Vec<Box<dyn Fn(&Rc<EffectState>)>>>,
}

/// Handle to a running [`create_effect`] closure. Drop it to stop the effect
/// from reacting to further signal changes.
pub struct Effect {
    _state: Rc<EffectState>,
}

/// Runs `f` immediately, then re-runs it whenever any [`Signal`] read during
/// its execution is later changed via [`Signal::set`] or [`Signal::update`].
///
/// The returned [`Effect`] must be kept alive for as long as the effect
/// should keep reacting; dropping it unsubscribes from all signals it read.
pub fn create_effect(f: impl FnMut() + 'static) -> Effect {
    let state = Rc::new(EffectState {
        run: RefCell::new(Box::new(f)),
        dependencies: RefCell::new(Vec::new()),
    });
    run_effect(&state);
    Effect { _state: state }
}

fn run_effect(state: &Rc<EffectState>) {
    let dependencies = std::mem::take(&mut *state.dependencies.borrow_mut());
    for unsubscribe in dependencies {
        unsubscribe(state);
    }
    EFFECT_STACK.with(|stack| stack.borrow_mut().push(state.clone()));
    (state.run.borrow_mut())();
    EFFECT_STACK.with(|stack| {
        stack.borrow_mut().pop();
    });
}

/// Groups synchronous signal writes into one effect run per subscriber.
/// This is particularly important for compound input updates such as a text
/// editor moving both its caret and selection during one mouse event.
pub fn batch(f: impl FnOnce()) {
    BATCH_DEPTH.with(|depth| depth.set(depth.get() + 1));
    f();
    let flush = BATCH_DEPTH.with(|depth| {
        depth.set(depth.get() - 1);
        depth.get() == 0
    });
    if flush {
        let pending = PENDING_EFFECTS.with(|effects| std::mem::take(&mut *effects.borrow_mut()));
        for state in pending {
            run_effect(&state);
        }
    }
}

struct SignalInner<T> {
    value: RefCell<T>,
    subscribers: RefCell<Vec<Weak<EffectState>>>,
}

/// A reactive value cell.
///
/// Reading [`Signal::get`] inside a [`create_effect`] body subscribes that
/// effect to future writes; [`Signal::peek`] reads without subscribing.
pub struct Signal<T> {
    inner: Rc<SignalInner<T>>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Signal {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone + 'static> Signal<T> {
    pub fn new(value: T) -> Self {
        Signal {
            inner: Rc::new(SignalInner {
                value: RefCell::new(value),
                subscribers: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Reads the current value, subscribing the currently running effect (if any).
    pub fn get(&self) -> T {
        self.track();
        self.inner.value.borrow().clone()
    }

    /// Reads the current value without subscribing the running effect.
    pub fn peek(&self) -> T {
        self.inner.value.borrow().clone()
    }

    /// Replaces the value and notifies subscribers.
    pub fn set(&self, value: T) {
        *self.inner.value.borrow_mut() = value;
        self.notify();
    }

    /// Mutates the value in place and notifies subscribers.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut self.inner.value.borrow_mut());
        self.notify();
    }

    fn track(&self) {
        EFFECT_STACK.with(|stack| {
            if let Some(current) = stack.borrow().last() {
                let mut subs = self.inner.subscribers.borrow_mut();
                let already = subs
                    .iter()
                    .any(|w| w.upgrade().is_some_and(|s| Rc::ptr_eq(&s, current)));
                if !already {
                    subs.push(Rc::downgrade(current));
                    let signal = self.inner.clone();
                    current
                        .dependencies
                        .borrow_mut()
                        .push(Box::new(move |effect| {
                            signal.subscribers.borrow_mut().retain(|weak| {
                                weak.upgrade()
                                    .is_some_and(|subscriber| !Rc::ptr_eq(&subscriber, effect))
                            });
                        }));
                }
            }
        });
    }

    fn notify(&self) {
        let subs: Vec<Rc<EffectState>> = {
            let mut subs = self.inner.subscribers.borrow_mut();
            subs.retain(|w| w.strong_count() > 0);
            subs.iter().filter_map(|w| w.upgrade()).collect()
        };
        let batching = BATCH_DEPTH.with(|depth| depth.get() > 0);
        if batching {
            PENDING_EFFECTS.with(|pending| {
                let mut pending = pending.borrow_mut();
                for state in subs {
                    if !pending.iter().any(|queued| Rc::ptr_eq(queued, &state)) {
                        pending.push(state);
                    }
                }
            });
        } else {
            for state in subs {
                run_effect(&state);
            }
        }
    }
}

impl<T: Clone + PartialEq + 'static> Signal<T> {
    /// Like [`Signal::set`], but reads the current value first and skips
    /// the write (and the notification it would trigger) when `value`
    /// equals it. Prefer this over `set` for internal control state whose
    /// callers don't depend on an explicit notification even when nothing
    /// changed (e.g. a pointer-move handler that recomputes a hovered
    /// index every event).
    pub fn set_if_changed(&self, value: T) {
        if *self.inner.value.borrow() != value {
            self.set(value);
        }
    }
}

struct OwnerInner {
    children: RefCell<Vec<Owner>>,
    effects: RefCell<Vec<Effect>>,
    cleanups: RefCell<Vec<Box<dyn FnOnce()>>>,
}

/// A disposable scope for effects and child scopes, so removing a UI
/// subtree can unsubscribe everything it owns in one call instead of
/// leaking effects that keep reacting after their node is gone.
///
/// Cheap to clone (an `Rc` underneath). Holds no reference back to its
/// parent — only parent-to-child, so a disposed owner's `Rc` can actually
/// reach zero instead of being kept alive by a cycle.
#[derive(Clone)]
pub struct Owner(Rc<OwnerInner>);

impl Owner {
    pub fn new() -> Self {
        Owner(Rc::new(OwnerInner {
            children: RefCell::new(Vec::new()),
            effects: RefCell::new(Vec::new()),
            cleanups: RefCell::new(Vec::new()),
        }))
    }

    /// Creates a child scope disposed whenever `self` is.
    pub fn child(&self) -> Owner {
        let child = Owner::new();
        self.0.children.borrow_mut().push(child.clone());
        child
    }

    /// Runs `f` as a [`create_effect`], owned by `self`: dropped (and so
    /// unsubscribed) on [`Owner::dispose`] instead of needing the caller to
    /// hold the returned [`Effect`] handle itself.
    pub fn effect(&self, f: impl FnMut() + 'static) {
        self.0.effects.borrow_mut().push(create_effect(f));
    }

    /// Registers `f` to run once, on [`Owner::dispose`], after this
    /// owner's effects stop and before its children are disposed.
    pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        self.0.cleanups.borrow_mut().push(Box::new(f));
    }

    /// Disposes every child scope, drops (unsubscribing) every owned
    /// effect, then runs every registered cleanup. Idempotent: disposing
    /// an already-disposed owner is a no-op.
    pub fn dispose(&self) {
        for child in self.0.children.borrow_mut().drain(..) {
            child.dispose();
        }
        self.0.effects.borrow_mut().clear();
        for cleanup in self.0.cleanups.borrow_mut().drain(..) {
            cleanup();
        }
    }
}

impl Default for Owner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn get_set_roundtrip() {
        let s = Signal::new(1);
        assert_eq!(s.get(), 1);
        s.set(42);
        assert_eq!(s.get(), 42);
    }

    #[test]
    fn effect_runs_immediately_and_on_change() {
        let s = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let s_clone = s.clone();
        let _effect = create_effect(move || {
            let _ = s_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });
        assert_eq!(runs.get(), 1);
        s.set(1);
        assert_eq!(runs.get(), 2);
        s.set(2);
        assert_eq!(runs.get(), 3);
    }

    #[test]
    fn set_if_changed_skips_notification_when_value_is_equal() {
        let s = Signal::new(1);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let s_clone = s.clone();
        let _effect = create_effect(move || {
            let _ = s_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });
        assert_eq!(runs.get(), 1);

        s.set_if_changed(1);
        assert_eq!(runs.get(), 1, "an equal value must not trigger a re-run");

        s.set_if_changed(2);
        assert_eq!(runs.get(), 2, "a changed value must still trigger a re-run");
    }

    #[test]
    fn effect_does_not_run_for_unrelated_signal() {
        let a = Signal::new(0);
        let b = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let a_clone = a.clone();
        let _effect = create_effect(move || {
            let _ = a_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });
        assert_eq!(runs.get(), 1);
        b.set(99);
        assert_eq!(runs.get(), 1, "unrelated signal must not trigger a re-run");
    }

    #[test]
    fn conditional_effect_unsubscribes_from_the_inactive_branch() {
        let show_first = Signal::new(true);
        let first = Signal::new(0);
        let second = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let branch = show_first.clone();
        let a = first.clone();
        let b = second.clone();
        let observed_runs = runs.clone();
        let _effect = create_effect(move || {
            if branch.get() {
                let _ = a.get();
            } else {
                let _ = b.get();
            }
            observed_runs.set(observed_runs.get() + 1);
        });

        show_first.set(false);
        assert_eq!(runs.get(), 2);
        first.set(1);
        assert_eq!(
            runs.get(),
            2,
            "the hidden branch must no longer invalidate the view"
        );
        second.set(1);
        assert_eq!(runs.get(), 3);
    }

    #[test]
    fn batch_coalesces_multiple_signal_writes_into_one_effect_run() {
        let first = Signal::new(0);
        let second = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let observed_first = first.clone();
        let observed_second = second.clone();
        let observed_runs = runs.clone();
        let _effect = create_effect(move || {
            let _ = (observed_first.get(), observed_second.get());
            observed_runs.set(observed_runs.get() + 1);
        });
        batch(|| {
            first.set(1);
            second.set(1);
        });
        assert_eq!(runs.get(), 2, "initial render plus one batched update");
    }

    #[test]
    fn peek_does_not_subscribe() {
        let s = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let s_clone = s.clone();
        let _effect = create_effect(move || {
            let _ = s_clone.peek();
            runs_clone.set(runs_clone.get() + 1);
        });
        assert_eq!(runs.get(), 1);
        s.set(1);
        assert_eq!(runs.get(), 1, "peek must not subscribe the effect");
    }

    #[test]
    fn use_context_reads_the_matching_provided_value() {
        with_context_scope(|| {
            provide_context(42_i32);
            provide_context("hello".to_string());
            assert_eq!(use_context::<i32>(), 42);
            assert_eq!(use_context::<String>(), "hello");
        });
    }

    #[test]
    fn try_use_context_is_none_for_an_unprovided_type() {
        with_context_scope(|| {
            assert_eq!(try_use_context::<i32>(), None);
        });
    }

    #[test]
    fn try_use_context_is_none_outside_any_scope() {
        assert_eq!(try_use_context::<i32>(), None);
        assert!(!in_context_scope());
    }

    #[test]
    #[should_panic(expected = "outside a context scope")]
    fn provide_context_outside_a_scope_panics() {
        provide_context(1_i32);
    }

    #[test]
    #[should_panic(expected = "found nothing provided")]
    fn use_context_without_a_provider_panics() {
        with_context_scope(|| {
            use_context::<i32>();
        });
    }

    #[test]
    fn nested_scope_shadows_then_restores_the_outer_value() {
        with_context_scope(|| {
            provide_context(1_i32);
            with_context_scope(|| {
                provide_context(2_i32);
                assert_eq!(use_context::<i32>(), 2);
            });
            assert_eq!(use_context::<i32>(), 1);
        });
    }

    #[test]
    fn scope_pops_even_if_f_panics() {
        let result = std::panic::catch_unwind(|| {
            with_context_scope(|| {
                provide_context(1_i32);
                panic!("boom");
            });
        });
        assert!(result.is_err());
        assert!(!in_context_scope(), "scope must be popped after unwinding");
    }

    #[test]
    fn dropping_effect_stops_updates() {
        let s = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let s_clone = s.clone();
        let effect = create_effect(move || {
            let _ = s_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });
        drop(effect);
        s.set(1);
        assert_eq!(runs.get(), 1, "dropped effect must not react anymore");
    }

    #[test]
    fn disposing_an_owner_stops_its_effects() {
        let s = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let owner = Owner::new();
        owner.effect({
            let s = s.clone();
            let runs = runs.clone();
            move || {
                let _ = s.get();
                runs.set(runs.get() + 1);
            }
        });
        assert_eq!(runs.get(), 1);

        owner.dispose();
        s.set(1);
        assert_eq!(
            runs.get(),
            1,
            "a disposed owner's effect must not react anymore"
        );
    }

    #[test]
    fn disposing_a_parent_disposes_children_transitively() {
        let s = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let parent = Owner::new();
        let child = parent.child();
        child.effect({
            let s = s.clone();
            let runs = runs.clone();
            move || {
                let _ = s.get();
                runs.set(runs.get() + 1);
            }
        });
        assert_eq!(runs.get(), 1);

        parent.dispose();
        s.set(1);
        assert_eq!(
            runs.get(),
            1,
            "disposing the parent must dispose the child's effects too"
        );
    }

    #[test]
    fn dispose_runs_cleanups_exactly_once() {
        let owner = Owner::new();
        let calls = Rc::new(Cell::new(0));
        owner.on_cleanup({
            let calls = calls.clone();
            move || calls.set(calls.get() + 1)
        });

        owner.dispose();
        assert_eq!(calls.get(), 1);
        owner.dispose();
        assert_eq!(calls.get(), 1, "disposing twice must not rerun cleanups");
    }
}
