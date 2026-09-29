use creamui_reactive::Owner;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::transaction::RuntimeTransaction;
use super::Runtime;

/// A [`Runtime`] shared with bindings that mutate it later, off the call
/// stack that built it — from inside a `Signal` write, potentially nested
/// arbitrarily deep in application code.
#[derive(Clone)]
pub struct SharedRuntime {
    runtime: Rc<RefCell<Runtime>>,
    listeners: Rc<RefCell<Vec<Weak<dyn Fn()>>>>,
}

impl SharedRuntime {
    pub fn new(runtime: Runtime) -> Self {
        SharedRuntime {
            runtime: Rc::new(RefCell::new(runtime)),
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&Runtime) -> R) -> R {
        f(&self.runtime.borrow())
    }

    pub fn with_mut<R>(&self, f: impl FnOnce(&mut Runtime) -> R) -> R {
        let result = f(&mut self.runtime.borrow_mut());
        self.notify();
        result
    }

    pub(crate) fn with_mut_quiet<R>(&self, f: impl FnOnce(&mut Runtime) -> R) -> R {
        f(&mut self.runtime.borrow_mut())
    }

    pub fn transaction<R>(&self, f: impl FnOnce(&mut RuntimeTransaction) -> R) -> R {
        let (result, changed) = {
            let mut runtime = self.runtime.borrow_mut();
            let mut tx = runtime.transaction();
            let result = f(&mut tx);
            (result, !tx.touched().is_empty())
        };
        if changed {
            self.notify();
        }
        result
    }

    pub fn subscribe(&self, listener: &Rc<dyn Fn()>) {
        self.listeners.borrow_mut().push(Rc::downgrade(listener));
    }

    fn notify(&self) {
        let listeners = {
            let mut weak = self.listeners.borrow_mut();
            let listeners: Vec<_> = weak.iter().filter_map(Weak::upgrade).collect();
            weak.retain(|listener| listener.strong_count() > 0);
            listeners
        };
        for listener in listeners {
            listener();
        }
    }
}

/// Ties a reactive read directly to a runtime mutation. `f` runs inside a
/// [`creamui_reactive::create_effect`]-equivalent scope — any `Signal::get()`
/// inside it subscribes — and its only job is to mutate `runtime` through
/// the given [`RuntimeTransaction`]; it must never rebuild a widget tree.
/// Owned by `owner`: disposing it stops the binding from reacting.
pub fn create_binding(
    owner: &Owner,
    runtime: SharedRuntime,
    mut f: impl FnMut(&mut RuntimeTransaction) + 'static,
) {
    owner.effect(move || {
        runtime.transaction(|tx| f(tx));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Mutation, NodeKind};
    use crate::PaintStyle;
    use creamui_reactive::{batch, Signal};
    use creamui_theme::Color;

    fn paint_style(seed: u8) -> PaintStyle {
        PaintStyle {
            background: Some(Color::rgb(seed, seed, seed).into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_leaf_signal_updates_one_runtime_node_directly() {
        let mut runtime = Runtime::new();
        let node = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let color = Signal::new(1u8);
        let owner = Owner::new();
        create_binding(&owner, runtime.clone(), {
            let color = color.clone();
            move |tx| {
                tx.apply(Mutation::SetPaintStyle {
                    node,
                    style: paint_style(color.get()),
                });
            }
        });

        assert_eq!(
            runtime.with(|r| r.get(node).unwrap().paint_style.clone()),
            paint_style(1)
        );

        color.set(2);
        assert_eq!(
            runtime.with(|r| r.get(node).unwrap().paint_style.clone()),
            paint_style(2)
        );
    }

    #[test]
    fn disposing_the_owner_stops_the_binding() {
        let mut runtime = Runtime::new();
        let node = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let color = Signal::new(1u8);
        let owner = Owner::new();
        create_binding(&owner, runtime.clone(), {
            let color = color.clone();
            move |tx| {
                tx.apply(Mutation::SetPaintStyle {
                    node,
                    style: paint_style(color.get()),
                });
            }
        });

        owner.dispose();
        color.set(2);
        assert_eq!(
            runtime.with(|r| r.get(node).unwrap().paint_style.clone()),
            paint_style(1),
            "a disposed binding must not react to further signal writes"
        );
    }

    #[test]
    fn batched_writes_to_two_signals_run_one_binding_once() {
        let mut runtime = Runtime::new();
        let node = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let a = Signal::new(1u8);
        let b = Signal::new(1u8);
        let runs = Rc::new(RefCell::new(0u32));
        let owner = Owner::new();
        create_binding(&owner, runtime.clone(), {
            let (a, b, runs) = (a.clone(), b.clone(), runs.clone());
            move |tx| {
                *runs.borrow_mut() += 1;
                tx.apply(Mutation::SetPaintStyle {
                    node,
                    style: paint_style(a.get().wrapping_add(b.get())),
                });
            }
        });
        assert_eq!(*runs.borrow(), 1, "the initial run");

        batch(|| {
            a.set(2);
            b.set(3);
        });
        assert_eq!(
            *runs.borrow(),
            2,
            "two signal writes in one batch must run the binding once, not twice"
        );
    }
}
