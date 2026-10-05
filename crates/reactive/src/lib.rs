//! Minimal single-threaded reactive primitives: [`Signal`] and [`create_effect`].
//!
//! This is the reactivity layer CreamUI components are built on. It is
//! Signal writes flush effects synchronously on the main thread; nested writes
//! wait for the current effect to finish and batches coalesce notifications.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::{Rc, Weak};

type ContextScope = Rc<RefCell<HashMap<TypeId, Rc<dyn Any>>>>;

thread_local! {
    static EFFECT_STACK: RefCell<Vec<Rc<EffectState>>> = const { RefCell::new(Vec::new()) };
    static EFFECT_DEPTH: Cell<usize> = const { Cell::new(0) };
    static NEXT_EFFECT_ID: Cell<u64> = const { Cell::new(1) };
    static FLUSHING: Cell<bool> = const { Cell::new(false) };
    static BATCH_DEPTH: Cell<usize> = const { Cell::new(0) };
    static PENDING_EFFECTS: RefCell<VecDeque<Rc<EffectState>>> = const { RefCell::new(VecDeque::new()) };
    static CONTEXT_STACK: RefCell<Vec<ContextScope>> =
        const { RefCell::new(Vec::new()) };
}

/// Opens a context scope for the duration of `f`. [`provide_context`] and
/// [`use_context`] only work while a scope is active; nested scopes see
/// their own values first, then fall back to the enclosing scope's. Pops
/// via a drop guard, so a panic inside `f` still leaves the stack balanced.
pub fn with_context_scope<R>(f: impl FnOnce() -> R) -> R {
    CONTEXT_STACK.with(|stack| {
        stack
            .borrow_mut()
            .push(Rc::new(RefCell::new(HashMap::new())))
    });
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
    id: u64,
    queued: Cell<bool>,
    active: Cell<bool>,
    context: Vec<ContextScope>,
    run: RefCell<Box<dyn FnMut()>>,
    /// Unsubscribes collected while the previous execution read signals.
    /// They are run before the next execution so a conditional view only
    /// remains subscribed to the branch it currently renders.
    dependencies: RefCell<Vec<Box<dyn Fn(&Rc<EffectState>)>>>,
}

/// Handle to a running [`create_effect`] closure. Drop it to stop the effect
/// from reacting to further signal changes.
pub struct Effect {
    state: Rc<EffectState>,
}

impl Drop for Effect {
    fn drop(&mut self) {
        self.state.active.set(false);
        for unsubscribe in self.state.dependencies.borrow_mut().drain(..) {
            unsubscribe(&self.state);
        }
    }
}

/// Runs `f` without subscribing the surrounding effect to its reads.
/// Effects created inside `f` still collect their own dependencies.
pub fn untrack<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(Vec<Rc<EffectState>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            EFFECT_STACK.with(|stack| *stack.borrow_mut() = std::mem::take(&mut self.0));
        }
    }
    let _restore = Restore(EFFECT_STACK.with(|stack| std::mem::take(&mut *stack.borrow_mut())));
    f()
}

/// Runs `f` immediately, then re-runs it whenever any [`Signal`] read during
/// its execution is later changed via [`Signal::set`] or [`Signal::update`].
///
/// Later runs retain the context scopes active when the effect was created.
/// The returned [`Effect`] must be kept alive for as long as the effect
/// should keep reacting; dropping it unsubscribes from all signals it read.
pub fn create_effect(f: impl FnMut() + 'static) -> Effect {
    let state = Rc::new(EffectState {
        id: NEXT_EFFECT_ID.with(|id| {
            let next = id.get();
            id.set(next.checked_add(1).expect("effect ID space exhausted"));
            next
        }),
        queued: Cell::new(false),
        active: Cell::new(true),
        context: CONTEXT_STACK.with(|stack| stack.borrow().clone()),
        run: RefCell::new(Box::new(f)),
        dependencies: RefCell::new(Vec::new()),
    });
    let effect = Effect { state };
    run_effect(&effect.state);
    flush_effects();
    effect
}

fn run_effect(state: &Rc<EffectState>) {
    if !state.active.get() {
        return;
    }
    struct RestoreContext(Vec<ContextScope>);
    impl Drop for RestoreContext {
        fn drop(&mut self) {
            CONTEXT_STACK.with(|stack| *stack.borrow_mut() = std::mem::take(&mut self.0));
        }
    }
    struct PopEffect;
    impl Drop for PopEffect {
        fn drop(&mut self) {
            EFFECT_STACK.with(|stack| {
                stack.borrow_mut().pop();
            });
            EFFECT_DEPTH.with(|depth| depth.set(depth.get() - 1));
        }
    }
    let dependencies = std::mem::take(&mut *state.dependencies.borrow_mut());
    for unsubscribe in dependencies {
        unsubscribe(state);
    }
    let _context = RestoreContext(
        CONTEXT_STACK
            .with(|stack| std::mem::replace(&mut *stack.borrow_mut(), state.context.clone())),
    );
    EFFECT_DEPTH.with(|depth| depth.set(depth.get() + 1));
    EFFECT_STACK.with(|stack| stack.borrow_mut().push(state.clone()));
    let _pop = PopEffect;
    (state.run.borrow_mut())();
}

/// Groups synchronous signal writes into one effect run per subscriber.
/// This is particularly important for compound input updates such as a text
/// editor moving both its caret and selection during one mouse event.
pub fn batch(f: impl FnOnce()) {
    struct EndBatch;
    impl Drop for EndBatch {
        fn drop(&mut self) {
            BATCH_DEPTH.with(|depth| depth.set(depth.get() - 1));
        }
    }
    BATCH_DEPTH.with(|depth| depth.set(depth.get() + 1));
    let guard = EndBatch;
    f();
    drop(guard);
    flush_effects();
}

fn queue_effect(state: Rc<EffectState>) {
    if state.active.get() && !state.queued.replace(true) {
        PENDING_EFFECTS.with(|pending| pending.borrow_mut().push_back(state));
    }
}

fn flush_effects() {
    if BATCH_DEPTH.with(|depth| depth.get() != 0)
        || EFFECT_DEPTH.with(|depth| depth.get() != 0)
        || FLUSHING.with(Cell::get)
    {
        return;
    }
    struct EndFlush;
    impl Drop for EndFlush {
        fn drop(&mut self) {
            FLUSHING.with(|flushing| flushing.set(false));
        }
    }
    FLUSHING.with(|flushing| flushing.set(true));
    let _guard = EndFlush;
    while let Some(state) = PENDING_EFFECTS.with(|pending| pending.borrow_mut().pop_front()) {
        state.queued.set(false);
        run_effect(&state);
    }
}

#[derive(Default)]
enum Subscribers {
    #[default]
    Empty,
    One(u64, Weak<EffectState>),
    Many(BTreeMap<u64, Weak<EffectState>>),
}

impl Subscribers {
    fn insert(&mut self, state: &Rc<EffectState>) -> bool {
        match self {
            Self::Empty => {
                *self = Self::One(state.id, Rc::downgrade(state));
                true
            }
            Self::One(id, _) if *id == state.id => false,
            Self::One(_, weak) if weak.strong_count() == 0 => {
                *self = Self::One(state.id, Rc::downgrade(state));
                true
            }
            Self::One(id, weak) => {
                *self = Self::Many(BTreeMap::from([
                    (*id, weak.clone()),
                    (state.id, Rc::downgrade(state)),
                ]));
                true
            }
            Self::Many(subscribers) => {
                if subscribers.contains_key(&state.id) {
                    false
                } else {
                    subscribers.insert(state.id, Rc::downgrade(state));
                    true
                }
            }
        }
    }

    fn remove(&mut self, id: u64) {
        match self {
            Self::One(current, _) if *current == id => *self = Self::Empty,
            Self::Many(subscribers) => {
                subscribers.remove(&id);
                if subscribers.len() == 1 {
                    let (id, weak) = subscribers.pop_first().expect("single subscriber exists");
                    *self = Self::One(id, weak);
                } else if subscribers.is_empty() {
                    *self = Self::Empty;
                }
            }
            _ => {}
        }
    }

    fn live(&mut self) -> Vec<Rc<EffectState>> {
        match self {
            Self::Empty => Vec::new(),
            Self::One(_, weak) => {
                let state = weak.upgrade();
                if state.is_none() {
                    *self = Self::Empty;
                }
                state.into_iter().collect()
            }
            Self::Many(subscribers) => {
                subscribers.retain(|_, weak| weak.strong_count() != 0);
                subscribers.values().filter_map(Weak::upgrade).collect()
            }
        }
    }
}

struct SignalInner<T> {
    value: RefCell<T>,
    subscribers: RefCell<Subscribers>,
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
                subscribers: RefCell::new(Subscribers::Empty),
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
            if let Some(current) = stack.borrow().last().filter(|state| state.active.get()) {
                if self.inner.subscribers.borrow_mut().insert(current) {
                    let signal = self.inner.clone();
                    current
                        .dependencies
                        .borrow_mut()
                        .push(Box::new(move |effect| {
                            signal.subscribers.borrow_mut().remove(effect.id);
                        }));
                }
            }
        });
    }

    fn notify(&self) {
        let subscribers = self.inner.subscribers.borrow_mut().live();
        for state in subscribers {
            queue_effect(state);
        }
        flush_effects();
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
    disposed: Cell<bool>,
    parent: RefCell<Option<Weak<OwnerInner>>>,
    children: RefCell<Vec<Owner>>,
    effects: RefCell<Vec<Effect>>,
    cleanups: RefCell<Vec<Box<dyn FnOnce()>>>,
}

/// A clonable scope for effects, child scopes, and explicit cleanup.
/// Child scopes hold weak parent references and detach on disposal.
#[derive(Clone)]
pub struct Owner(Rc<OwnerInner>);

/// A non-owning reference that upgrades only while its scope is active.
#[derive(Clone)]
pub struct WeakOwner(Weak<OwnerInner>);

impl WeakOwner {
    pub fn upgrade(&self) -> Option<Owner> {
        self.0
            .upgrade()
            .filter(|owner| !owner.disposed.get())
            .map(Owner)
    }
}

impl Owner {
    /// Whether this scope has stopped its effects and begun disposal.
    pub fn is_disposed(&self) -> bool {
        self.0.disposed.get()
    }

    /// Returns a non-owning reference suitable for callbacks owned by this scope.
    pub fn downgrade(&self) -> WeakOwner {
        WeakOwner(Rc::downgrade(&self.0))
    }

    pub fn new() -> Self {
        Owner(Rc::new(OwnerInner {
            disposed: Cell::new(false),
            parent: RefCell::new(None),
            children: RefCell::new(Vec::new()),
            effects: RefCell::new(Vec::new()),
            cleanups: RefCell::new(Vec::new()),
        }))
    }

    /// Creates a child scope disposed whenever `self` is.
    pub fn child(&self) -> Owner {
        let child = Owner::new();
        if self.0.disposed.get() {
            child.dispose();
            return child;
        }
        *child.0.parent.borrow_mut() = Some(Rc::downgrade(&self.0));
        self.0.children.borrow_mut().push(child.clone());
        child
    }

    /// Runs `f` as a [`create_effect`], owned by `self`: dropped (and so
    /// unsubscribed) on [`Owner::dispose`] instead of needing the caller to
    /// hold the returned [`Effect`] handle itself.
    pub fn effect(&self, f: impl FnMut() + 'static) {
        if self.0.disposed.get() {
            return;
        }
        let scope = self.downgrade();
        let mut f = f;
        let effect = create_effect(move || {
            if scope.upgrade().is_some() {
                f();
            }
        });
        if !self.0.disposed.get() {
            self.0.effects.borrow_mut().push(effect);
        }
    }

    /// Registers `f` to run once, on [`Owner::dispose`], after this
    /// owner's effects and child scopes stop.
    pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        if self.0.disposed.get() {
            f();
        } else {
            self.0.cleanups.borrow_mut().push(Box::new(f));
        }
    }

    /// Stops owned effects, disposes child scopes, and runs cleanups once.
    /// Disposed scopes cannot start new effects or retain child scopes.
    pub fn dispose(&self) {
        Self::dispose_many([self.clone()]);
    }

    /// Stops every listed scope and descendant before running any cleanup.
    pub fn dispose_many(owners: impl IntoIterator<Item = Owner>) {
        let mut stack: Vec<_> = owners.into_iter().map(|owner| (owner, false)).collect();
        let mut stopped = Vec::new();
        let mut cleanup_order = Vec::new();
        while let Some((owner, complete)) = stack.pop() {
            if complete {
                cleanup_order.push(owner);
                continue;
            }
            if owner.0.disposed.replace(true) {
                continue;
            }
            stopped.push(owner.clone());
            stack.push((owner.clone(), true));
            let children: Vec<_> = owner.0.children.borrow_mut().drain(..).collect();
            stack.extend(children.into_iter().map(|child| (child, false)));
        }
        let mut parents = HashMap::new();
        for owner in &stopped {
            if let Some(parent) = owner
                .0
                .parent
                .borrow_mut()
                .take()
                .and_then(|parent| parent.upgrade())
            {
                if !parent.disposed.get() {
                    parents.insert(Rc::as_ptr(&parent), parent);
                }
            }
        }
        for owner in &stopped {
            owner.0.effects.borrow_mut().clear();
        }
        for parent in parents.into_values() {
            parent
                .children
                .borrow_mut()
                .retain(|child| !child.0.disposed.get());
        }
        for owner in cleanup_order {
            let cleanups: Vec<_> = owner.0.cleanups.borrow_mut().drain(..).collect();
            for cleanup in cleanups {
                cleanup();
            }
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
    fn nested_signal_writes_finish_after_the_current_effect_without_reentry() {
        let signal = Signal::new(0u32);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let _effect = create_effect({
            let signal = signal.clone();
            let observed = observed.clone();
            move || {
                let value = signal.get();
                observed.borrow_mut().push(value);
                if value < 3 {
                    signal.set(value + 1);
                }
                assert_eq!(observed.borrow().last(), Some(&value));
            }
        });
        assert_eq!(&*observed.borrow(), &[0, 1, 2, 3]);
        signal.set(2);
        assert_eq!(&*observed.borrow(), &[0, 1, 2, 3, 2, 3]);
    }

    #[test]
    fn shared_dependencies_coalesce_before_a_downstream_effect_runs() {
        let source = Signal::new(0u32);
        let left = Signal::new(0u32);
        let right = Signal::new(0u32);
        let _left = create_effect({
            let source = source.clone();
            let left = left.clone();
            move || left.set(source.get() * 2)
        });
        let _right = create_effect({
            let source = source.clone();
            let right = right.clone();
            move || right.set(source.get() * 3)
        });
        let observed = Rc::new(RefCell::new(Vec::new()));
        let _result = create_effect({
            let observed = observed.clone();
            move || observed.borrow_mut().push((left.get(), right.get()))
        });
        source.set(4);
        assert_eq!(&*observed.borrow(), &[(0, 0), (8, 12)]);
    }

    #[test]
    fn panicking_effect_preserves_remaining_notifications_and_can_run_again() {
        let source = Signal::new(0u32);
        let first = Rc::new(Cell::new(0));
        let second = Rc::new(Cell::new(0));
        let _first = create_effect({
            let source = source.clone();
            let first = first.clone();
            move || {
                first.set(first.get() + 1);
                assert_ne!(source.get(), 1, "invalid value");
            }
        });
        let _second = create_effect({
            let source = source.clone();
            let second = second.clone();
            move || {
                source.get();
                second.set(second.get() + 1);
            }
        });
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.set(1))).is_err());
        source.set(2);
        assert_eq!(first.get(), 3);
        assert_eq!(second.get(), 2);
        assert!(PENDING_EFFECTS.with(|pending| pending.borrow().is_empty()));
        assert_eq!(EFFECT_DEPTH.with(Cell::get), 0);
        assert!(!FLUSHING.with(Cell::get));
    }

    #[test]
    fn an_initial_effect_panic_cancels_its_queued_rerun() {
        let source = Signal::new(0u32);
        let runs = Rc::new(Cell::new(0));
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            create_effect({
                let source = source.clone();
                let runs = runs.clone();
                move || {
                    let value = source.get();
                    runs.set(runs.get() + 1);
                    if value == 0 {
                        source.set(1);
                        panic!("initial effect failed");
                    }
                }
            });
        }));
        assert!(failed.is_err());
        source.set(2);
        assert_eq!(runs.get(), 1);
        assert!(matches!(
            *source.inner.subscribers.borrow(),
            Subscribers::Empty
        ));
        assert!(PENDING_EFFECTS.with(|pending| pending.borrow().is_empty()));
    }

    #[test]
    fn panicking_batch_restores_depth_and_preserves_queued_updates() {
        let source = Signal::new(0u32);
        let observed = Rc::new(Cell::new(0));
        let _effect = create_effect({
            let source = source.clone();
            let observed = observed.clone();
            move || observed.set(source.get())
        });
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            batch(|| {
                source.set(1);
                panic!("batch failed");
            });
        }));
        assert!(failed.is_err());
        assert_eq!(BATCH_DEPTH.with(Cell::get), 0);
        source.set(2);
        assert_eq!(observed.get(), 2);
        assert!(PENDING_EFFECTS.with(|pending| pending.borrow().is_empty()));
    }

    #[test]
    fn disposal_stops_all_sibling_scopes_before_any_cleanup_notifies() {
        let owner = Owner::new();
        let source = Signal::new(0u32);
        let runs = Rc::new(Cell::new(0));
        let scopes: Vec<_> = (0..4096).map(|_| owner.child()).collect();
        for scope in &scopes {
            scope.effect({
                let source = source.clone();
                let runs = runs.clone();
                move || {
                    source.get();
                    runs.set(runs.get() + 1);
                }
            });
            scope.on_cleanup({
                let source = source.clone();
                move || source.set(1)
            });
        }
        Owner::dispose_many(scopes);
        assert_eq!(runs.get(), 4096);
        assert!(owner.0.children.borrow().is_empty());
        assert!(matches!(
            *source.inner.subscribers.borrow(),
            Subscribers::Empty
        ));
    }

    #[test]
    fn deep_scope_disposal_uses_child_before_parent_cleanup_order() {
        let root = Owner::new();
        let mut current = root.clone();
        let cleaned = Rc::new(RefCell::new(Vec::new()));
        for depth in 0..10_000 {
            current.on_cleanup({
                let cleaned = cleaned.clone();
                move || cleaned.borrow_mut().push(depth)
            });
            current = current.child();
        }
        root.dispose();
        assert_eq!(*cleaned.borrow(), (0..10_000).rev().collect::<Vec<_>>());
        assert!(root.0.children.borrow().is_empty());
        assert!(current.downgrade().upgrade().is_none());
    }

    #[test]
    fn untracked_reads_leave_nested_effects_tracked() {
        let a = Signal::new(0);
        let b = Signal::new(0);
        let c = Signal::new(0);
        let outer = Rc::new(Cell::new(0));
        let inner = Rc::new(Cell::new(0));
        let nested = Rc::new(RefCell::new(None));
        let _effect = create_effect({
            let (a, b, c, outer, inner, nested) = (
                a.clone(),
                b.clone(),
                c.clone(),
                outer.clone(),
                inner.clone(),
                nested.clone(),
            );
            move || {
                a.get();
                outer.set(outer.get() + 1);
                untrack(|| {
                    b.get();
                    *nested.borrow_mut() = Some(create_effect({
                        let c = c.clone();
                        let inner = inner.clone();
                        move || {
                            c.get();
                            inner.set(inner.get() + 1);
                        }
                    }));
                });
            }
        });
        b.set(1);
        assert_eq!(outer.get(), 1);
        c.set(1);
        assert_eq!(inner.get(), 2);
        assert_eq!(outer.get(), 1);
        a.set(1);
        assert_eq!(outer.get(), 2);
    }

    #[test]
    fn effects_keep_their_context_after_scope_exit_and_under_other_providers() {
        let signal = Signal::new(0);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let effect = with_context_scope(|| {
            provide_context(42u32);
            create_effect({
                let signal = signal.clone();
                let observed = observed.clone();
                move || {
                    signal.get();
                    observed.borrow_mut().push(use_context::<u32>());
                }
            })
        });
        signal.set(1);
        with_context_scope(|| {
            provide_context(9u32);
            signal.set(2);
            assert_eq!(use_context::<u32>(), 9);
        });
        assert_eq!(&*observed.borrow(), &[42, 42, 42]);
        assert!(!in_context_scope());
        drop(effect);
    }

    #[test]
    fn disposed_effects_do_not_run_from_a_pending_batch() {
        let owner = Owner::new();
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        owner.effect({
            let signal = signal.clone();
            let runs = runs.clone();
            move || {
                signal.get();
                runs.set(runs.get() + 1);
            }
        });
        batch(|| {
            signal.set(1);
            owner.dispose();
        });
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn cleanup_stops_parent_effects_and_cannot_reopen_a_disposed_scope() {
        let parent = Owner::new();
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        parent.effect({
            let signal = signal.clone();
            let runs = runs.clone();
            move || {
                signal.get();
                runs.set(runs.get() + 1);
            }
        });
        parent.child().on_cleanup(move || signal.set(1));
        parent.dispose();
        assert_eq!(runs.get(), 1);
        parent.effect(|| panic!("disposed owner started an effect"));
        parent
            .child()
            .effect(|| panic!("disposed child started an effect"));
        let cleaned = Rc::new(Cell::new(false));
        parent.on_cleanup({
            let cleaned = cleaned.clone();
            move || cleaned.set(true)
        });
        assert!(cleaned.get());
        assert!(parent.0.children.borrow().is_empty());
    }

    #[test]
    fn disposing_an_owner_during_its_initial_effect_does_not_keep_the_effect() {
        let owner = Owner::new();
        owner.effect({
            let owner = owner.clone();
            move || owner.dispose()
        });
        assert!(owner.0.effects.borrow().is_empty());
        assert!(owner.downgrade().upgrade().is_none());
    }

    #[test]
    fn panic_restores_effect_tracking_and_context_stacks() {
        let signal = Signal::new(0);
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_context_scope(|| {
                provide_context(42u32);
                create_effect(|| untrack(|| panic!("mount failed")));
            });
        }));
        assert!(failed.is_err());
        assert!(EFFECT_STACK.with(|stack| stack.borrow().is_empty()));
        assert!(!in_context_scope());
        let runs = Rc::new(Cell::new(0));
        let _effect = create_effect({
            let signal = signal.clone();
            let runs = runs.clone();
            move || {
                signal.get();
                runs.set(runs.get() + 1);
            }
        });
        signal.set(1);
        assert_eq!(runs.get(), 2);
    }

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

    #[test]
    fn disposing_a_child_directly_removes_it_from_the_parents_child_list() {
        let parent = Owner::new();
        let child = parent.child();
        assert_eq!(
            Rc::strong_count(&child.0),
            2,
            "one strong ref held by the caller, one by the parent's child list"
        );

        child.dispose();
        assert_eq!(
            Rc::strong_count(&child.0),
            1,
            "the parent must drop its reference instead of leaking a disposed entry"
        );
    }

    #[test]
    fn disposing_the_parent_after_a_child_already_disposed_itself_does_not_panic() {
        let parent = Owner::new();
        let a = parent.child();
        let _b = parent.child();
        let _c = parent.child();
        a.dispose();

        parent.dispose();
    }

    #[test]
    fn disposing_a_parent_with_several_children_does_not_panic() {
        let parent = Owner::new();
        for _ in 0..5 {
            parent.child();
        }
        parent.dispose();
    }
}
